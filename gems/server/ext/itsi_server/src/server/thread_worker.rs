use super::itsi_server::RequestJob;
use crate::ITSI_SERVER;
use crossbeam::{
    channel::{Receiver, Sender},
    epoch::Atomic,
};
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, soft_kill_threads, RetainedValue,
};
use itsi_tracing::{debug, error, info, warn};
use magnus::{
    gc,
    value::{InnerValue, Lazy, LazyId, Opaque, ReprValue},
    Class, Module, Object, RClass, RObject, Ruby, Thread, Value,
};
use nix::unistd::Pid;
use std::{
    num::NonZeroU8,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::spawn,
    time::Instant,
};
use tracing::instrument;
pub struct ThreadWorker {
    pub id: String,
    pub app: Opaque<Value>,
    pub receiver: Arc<Receiver<RequestJob>>,
    pub sender: Arc<Sender<RequestJob>>,
    pub thread: RetainedValue<Thread>,
    pub terminated: Arc<AtomicBool>,
    pub scheduler_class: Option<String>,
}

static ID_CALL: LazyId = LazyId::new("call");
static ID_ALIVE: LazyId = LazyId::new("alive?");

static CLASS_QUEUE: Lazy<RClass> =
    Lazy::new(|ruby| ruby.module_kernel().const_get("QueueWithTimeout").unwrap());

#[instrument(skip(threads, app))]
pub fn build_thread_workers(
    pid: Pid,
    threads: NonZeroU8,
    app: Opaque<Value>,
    scheduler_class: Option<String>,
) -> (
    Arc<Vec<ThreadWorker>>,
    Arc<crossbeam::channel::Sender<RequestJob>>,
) {
    let (sender, receiver) = crossbeam::channel::bounded(20);
    let receiver_ref = Arc::new(receiver);
    let sender_ref = Arc::new(sender);
    let app = load_app(app);
    (
        Arc::new(
            (1..=u8::from(threads))
                .map(|id| {
                    info!("Creating worker thread {}", id);
                    ThreadWorker::new(
                        format!("{:?}#{:?}", pid, id),
                        app,
                        receiver_ref.clone(),
                        sender_ref.clone(),
                        scheduler_class.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        ),
        sender_ref,
    )
}

pub fn load_app(app: Opaque<Value>) -> Opaque<Value> {
    call_with_gvl(|ruby| {
        let app = app.get_inner_with(&ruby);
        Opaque::from(
            app.funcall::<_, _, Value>(*ID_CALL, ())
                .expect("Couldn't load app"),
        )
    })
}
impl ThreadWorker {
    pub fn new(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<Receiver<RequestJob>>,
        sender: Arc<Sender<RequestJob>>,
        scheduler_class: Option<String>,
    ) -> Self {
        let mut worker = Self {
            id,
            app,
            receiver,
            sender,
            thread: RetainedValue::empty(),
            terminated: Arc::new(AtomicBool::new(false)),
            scheduler_class,
        };
        worker.run();
        worker
    }

    pub fn request_shutdown(&self) {
        match self.sender.send(RequestJob::Shutdown) {
            Ok(_) => {}
            Err(err) => error!("Failed to send shutdown request: {}", err),
        };
        info!("Requesting shutdown for worker thread {}", self.id);
    }

    pub fn poll_shutdown(&self, deadline: Instant) -> bool {
        call_with_gvl(|_ruby| {
            if let Some(thread) = self.thread.inner().lock().unwrap().as_mut() {
                info!("Polling worker thread {:?} for shutdown", thread);
                if Instant::now() > deadline {
                    warn!("Worker thread {} timed out. Killing thread", self.id);
                    self.terminated.store(true, Ordering::SeqCst);
                    soft_kill_threads(vec![thread.as_value()]);
                }
                if thread.funcall::<_, _, bool>(*ID_ALIVE, ()).unwrap_or(false) {
                    return true;
                }
            }
            self.thread.clear();
            info!("Thread {} has been shut down", self.id);

            false
        })
    }

    pub fn run(&mut self) {
        let id = self.id.clone();
        let app = self.app;
        let receiver = self.receiver.clone();
        let terminated = self.terminated.clone();
        let scheduler_class = self.scheduler_class.clone();
        call_with_gvl(|_| {
            self.thread = RetainedValue::new(create_ruby_thread(move || {
                if scheduler_class.is_none() {
                    Self::accept_loop(id, app, receiver, terminated)
                } else {
                    Self::fiber_accept_loop(id, app, receiver, scheduler_class.clone(), terminated)
                }
            }));
        });
    }

    #[instrument(skip_all, fields(thread_worker=id))]
    pub fn fiber_accept_loop(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<Receiver<RequestJob>>,
        scheduler_class: Option<String>,
        terminated: Arc<AtomicBool>,
    ) {
        let ruby = Ruby::get().unwrap();
        let queue: Opaque<Value> =
            Opaque::from(ruby.get_inner(&CLASS_QUEUE).new_instance(()).unwrap());
        gc::register_mark_object(queue);

        // Push to our work queue in side thread.
        create_ruby_thread(move || {
            call_without_gvl(move || loop {
                match receiver.recv() {
                    Ok(RequestJob::ProcessRequest(request)) => {
                        if terminated.load(Ordering::Relaxed) {
                            break;
                        }
                        call_with_gvl(|ruby| {
                            ruby.get_inner_ref(&queue)
                                .funcall::<_, _, Value>("push", (request,))
                                .unwrap()
                        });
                    }
                    Ok(RequestJob::Shutdown) => {
                        debug!("Shutting down thread worker");
                        call_with_gvl(|ruby| {
                            let queue = ruby.get_inner_ref(&queue);
                            queue
                                .funcall::<_, _, Value>(
                                    "instance_variable_set",
                                    ("@finished", true),
                                )
                                .unwrap();
                        });
                        break;
                    }
                    Err(err) => {
                        error!("Error Receiving RequestJob: {}", err);
                    }
                }
            })
        });

        ruby.get_inner(&ITSI_SERVER)
            .funcall::<_, _, Value>("start_scheduler_loop", (queue, app, scheduler_class))
            .unwrap();
    }

    #[instrument(skip_all, fields(thread_worker=id))]
    pub fn accept_loop(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<Receiver<RequestJob>>,
        terminated: Arc<AtomicBool>,
    ) {
        let ruby = Ruby::get().unwrap();
        let server = ruby.get_inner(&ITSI_SERVER);
        call_without_gvl(|| loop {
            match receiver.recv() {
                Ok(RequestJob::ProcessRequest(request)) => {
                    if terminated.load(Ordering::Relaxed) {
                        break;
                    }
                    request.process(&ruby, server, app);
                }
                Ok(RequestJob::Shutdown) => {
                    debug!("Shutting down thread worker");
                    break;
                }
                Err(err) => {
                    error!("Error Receiving RequestJob: {}", err);
                }
            }
        });
    }
}
