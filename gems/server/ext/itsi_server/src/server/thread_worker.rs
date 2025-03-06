use super::itsi_server::RequestJob;
use crate::{request::itsi_request::ItsiRequest, ITSI_SERVER};
use crossbeam::channel::{Receiver, Sender};
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, heap_value::HeapValue, soft_kill_threads,
};
use itsi_tracing::{debug, error, info, warn};
use magnus::{
    value::{InnerValue, LazyId, Opaque, ReprValue},
    Module, RClass, Ruby, Thread, Value,
};
use nix::unistd::Pid;
use parking_lot::RwLock;
use std::{
    num::NonZeroU8,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tracing::instrument;
pub struct ThreadWorker {
    pub id: String,
    pub app: Opaque<Value>,
    pub receiver: Arc<Receiver<RequestJob>>,
    pub sender: Arc<Sender<RequestJob>>,
    pub thread: RwLock<Option<HeapValue<Thread>>>,
    pub terminated: Arc<AtomicBool>,
    pub scheduler_class: Option<String>,
}

static ID_CALL: LazyId = LazyId::new("call");
static ID_ALIVE: LazyId = LazyId::new("alive?");

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
            thread: RwLock::new(None),
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
            if let Some(thread) = self.thread.read().deref() {
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
            self.thread.write().take();
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
            *self.thread.write() = Some(
                create_ruby_thread(move || {
                    if scheduler_class.is_none() {
                        Self::accept_loop(id, app, receiver, terminated)
                    } else {
                        Self::fiber_accept_loop(
                            id,
                            app,
                            receiver,
                            scheduler_class.clone(),
                            terminated,
                        )
                    }
                })
                .into(),
            );
        });
    }

    fn gather_requests(
        receiver: &Arc<Receiver<RequestJob>>,
        timeout: Duration,
    ) -> (Vec<ItsiRequest>, bool) {
        let batch_size = 25;
        let mut batch = Vec::with_capacity(batch_size);
        let mut should_close: bool = false;

        while batch.len() < batch_size {
            match receiver.try_recv() {
                Ok(RequestJob::ProcessRequest(request)) => batch.push(request),
                Ok(RequestJob::Shutdown) => {
                    should_close = true;
                    break;
                }
                Err(_) => break,
            }
        }

        if batch.is_empty() {
            match receiver.recv_timeout(timeout) {
                Ok(RequestJob::ProcessRequest(request)) => batch.push(request),
                Ok(RequestJob::Shutdown) => {
                    should_close = true;
                }
                Err(_) => (),
            }
        }

        (batch, should_close)
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
        let scheduler_proc = ruby.proc_from_fn(move |ruby, _args, _blk| {
            let class_fiber: RClass = ruby
                .module_kernel()
                .const_get::<_, RClass>("Fiber")
                .unwrap();
            let scheduler = class_fiber.funcall::<_, _, Value>("scheduler", ()).unwrap();
            let server = ruby.get_inner(&ITSI_SERVER);
            loop {
                scheduler.funcall::<_, _, Value>("yield", ()).unwrap();
                let (reqs, should_close) = call_without_gvl(|| {
                    Self::gather_requests(&receiver, Duration::from_micros(50))
                });
                reqs.into_iter().for_each(|req| {
                    server
                        .funcall::<_, _, Value>("schedule", (app, req))
                        .unwrap();
                });
                if should_close || terminated.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
        let server = ruby.get_inner(&ITSI_SERVER);
        server
            .funcall::<_, _, Value>("start_scheduler_loop", (scheduler_class, scheduler_proc))
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
                    call_with_gvl(|_ruby| request.process(&ruby, server, app))
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
