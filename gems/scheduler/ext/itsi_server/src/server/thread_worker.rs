use super::itsi_server::RequestJob;
use crate::ITSI_SERVER;
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, kill_threads, HeapValue,
};
use itsi_tracing::{debug, error, info, warn};
use magnus::{
    value::{InnerValue, Lazy, LazyId, Opaque, ReprValue},
    Fiber, Module, RClass, Ruby, Thread, Value,
};
use nix::unistd::Pid;
use parking_lot::{Mutex, RwLock};
use std::{
    num::NonZeroU8,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{
        mpsc::{Receiver, Sender},
        watch,
    },
};
use tracing::instrument;
pub struct ThreadWorker {
    pub id: String,
    pub app: Opaque<Value>,
    pub receiver: Arc<Mutex<Receiver<RequestJob>>>,
    pub sender: Sender<RequestJob>,
    pub thread: RwLock<Option<HeapValue<Thread>>>,
    pub terminated: Arc<AtomicBool>,
    pub scheduler_class: Option<String>,
}

static ID_CALL: LazyId = LazyId::new("call");
static ID_ALIVE: LazyId = LazyId::new("alive?");
static ID_SCHEDULER: LazyId = LazyId::new("scheduler");
static ID_SCHEDULE: LazyId = LazyId::new("schedule");
static ID_BLOCK: LazyId = LazyId::new("block");
static ID_YIELD: LazyId = LazyId::new("yield");
static CLASS_FIBER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.module_kernel()
        .const_get::<_, RClass>("Fiber")
        .unwrap()
});

pub struct TerminateWakerSignal(bool);

#[instrument(skip(threads, app))]
pub fn build_thread_workers(
    pid: Pid,
    threads: NonZeroU8,
    app: Opaque<Value>,
    scheduler_class: Option<String>,
) -> (Arc<Vec<ThreadWorker>>, Sender<RequestJob>) {
    let (sender, receiver) = tokio::sync::mpsc::channel(20);
    let receiver_ref = Arc::new(Mutex::new(receiver));
    let sender_ref = sender;
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
        receiver: Arc<Mutex<Receiver<RequestJob>>>,
        sender: Sender<RequestJob>,
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

    pub async fn request_shutdown(&self) {
        match self.sender.send(RequestJob::Shutdown).await {
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
                    kill_threads(vec![thread.as_value()]);
                }
                if thread.funcall::<_, _, bool>(*ID_ALIVE, ()).unwrap_or(false) {
                    return true;
                }
                info!("Thread {} has shut down gracefully", self.id);
            }
            self.thread.write().take();

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

    #[instrument(skip_all, fields(thread_worker=id))]
    pub fn fiber_accept_loop(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<Mutex<Receiver<RequestJob>>>,
        scheduler_class: Option<String>,
        terminated: Arc<AtomicBool>,
    ) {
        let ruby = Ruby::get().unwrap();
        let (waker_sender, waker_receiver) = watch::channel(TerminateWakerSignal(false));
        let leader: Arc<Mutex<Option<RequestJob>>> = Arc::new(Mutex::new(None));
        let server = ruby.get_inner(&ITSI_SERVER);
        let scheduler_task = ServerSchedulerTask::new(
            app,
            leader.clone(),
            receiver.clone(),
            terminated.clone(),
            waker_sender.clone(),
        );
        let (scheduler, scheduler_fiber) = server
            .funcall::<_, _, (Value, Fiber)>(
                "start_scheduler_loop",
                (scheduler_class, scheduler_task),
            )
            .unwrap();
        Self::start_waker_thread(
            scheduler.into(),
            scheduler_fiber.into(),
            leader,
            receiver,
            waker_receiver,
        );
    }

    #[allow(clippy::await_holding_lock)]
    pub fn start_waker_thread(
        scheduler: Opaque<Value>,
        scheduler_fiber: Opaque<Fiber>,
        leader: Arc<Mutex<Option<RequestJob>>>,
        receiver: Arc<Mutex<Receiver<RequestJob>>>,
        mut waker_receiver: watch::Receiver<TerminateWakerSignal>,
    ) {
        create_ruby_thread(move || {
            let scheduler = scheduler.get_inner_with(&Ruby::get().unwrap());
            let leader = leader.clone();
            call_without_gvl(|| {
                RuntimeBuilder::new_current_thread()
                    .build()
                    .expect("Failed to build Tokio runtime")
                    .block_on(async {
                        loop {
                            waker_receiver.changed().await.ok();
                            if waker_receiver.borrow().0 {
                                break;
                            }
                            let mut receiver_guard = receiver.lock();
                            tokio::select! {
                                _ = waker_receiver.changed() => {
                                  if waker_receiver.borrow().0 {
                                      break;
                                  }
                                },
                                next_msg = receiver_guard.recv() => {
                                  *leader.lock() = next_msg;
                                  call_with_gvl(|_| {
                                      scheduler
                                          .funcall::<_, _, Value>(
                                              "unblock",
                                              (None::<u8>, scheduler_fiber),
                                          )
                                          .unwrap();
                                  });
                                }
                            }
                        }
                    })
            });
        });
    }

    #[instrument(skip_all, fields(thread_worker=id))]
    pub fn accept_loop(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<Mutex<Receiver<RequestJob>>>,
        terminated: Arc<AtomicBool>,
    ) {
        let ruby = Ruby::get().unwrap();
        let server = ruby.get_inner(&ITSI_SERVER);
        call_without_gvl(|| loop {
            match receiver.lock().blocking_recv() {
                Some(RequestJob::ProcessRequest(request)) => {
                    if terminated.load(Ordering::Relaxed) {
                        break;
                    }
                    call_with_gvl(|_ruby| request.process(&ruby, server, app))
                }
                Some(RequestJob::Shutdown) => {
                    debug!("Shutting down thread worker");
                    break;
                }
                None => {
                    thread::sleep(Duration::from_micros(1));
                }
            }
        });
    }
}

#[magnus::wrap(class = "Itsi::ServerSchedulerTask")]
pub struct ServerSchedulerTask {
    app: Opaque<Value>,
    leader: Arc<Mutex<Option<RequestJob>>>,
    receiver: Arc<Mutex<Receiver<RequestJob>>>,
    terminated: Arc<AtomicBool>,
    waker_sender: watch::Sender<TerminateWakerSignal>,
}

impl ServerSchedulerTask {
    pub fn new(
        app: Opaque<Value>,
        leader: Arc<Mutex<Option<RequestJob>>>,
        receiver: Arc<Mutex<Receiver<RequestJob>>>,
        terminated: Arc<AtomicBool>,
        waker_sender: watch::Sender<TerminateWakerSignal>,
    ) -> Self {
        ServerSchedulerTask {
            app,
            leader,
            receiver,
            terminated,
            waker_sender,
        }
    }

    pub fn run(ruby: &Ruby, rself: &Self) {
        let scheduler = ruby
            .get_inner(&CLASS_FIBER)
            .funcall::<_, _, Value>(*ID_SCHEDULER, ())
            .unwrap();
        let server = ruby.get_inner(&ITSI_SERVER);
        let thread_current = ruby.thread_current();
        let leader_clone = rself.leader.clone();
        let receiver = rself.receiver.clone();
        let terminated = rself.terminated.clone();
        let waker_sender = rself.waker_sender.clone();
        let mut batch = Vec::with_capacity(MAX_BATCH_SIZE as usize);

        static MAX_BATCH_SIZE: i32 = 25;
        call_without_gvl(move || loop {
            if let Some(v) = leader_clone.lock().take() {
                if matches!(v, RequestJob::Shutdown) {
                    waker_sender.send(TerminateWakerSignal(true)).unwrap();
                    break;
                }
                batch.push(v)
            }

            let mut recv_lock = receiver.lock();
            for _ in 0..MAX_BATCH_SIZE {
                if let Ok(req) = recv_lock.try_recv() {
                    if matches!(req, RequestJob::Shutdown) {
                        batch.push(req);
                        break;
                    }
                    batch.push(req);
                } else {
                    break;
                }
            }
            drop(recv_lock);

            let shutdown_requested = call_with_gvl(|_| {
                for req in batch.drain(..) {
                    match req {
                        RequestJob::ProcessRequest(request) => {
                            server
                                .funcall::<_, _, Value>(*ID_SCHEDULE, (rself.app, request))
                                .ok();
                        }
                        RequestJob::Shutdown => return true,
                    }
                }
                false
            });

            if shutdown_requested || terminated.load(Ordering::Relaxed) {
                waker_sender.send(TerminateWakerSignal(true)).unwrap();
                break;
            }

            // if receiver.lock().is_empty() {
            //     waker_sender.send(TerminateWakerSignal(false)).unwrap();
            //     call_with_gvl(|_| {
            //         scheduler
            //             .funcall::<_, _, Value>(*ID_BLOCK, (thread_current, None::<u8>))
            //             .unwrap();
            //     });
            // } else {
            call_with_gvl(|_| scheduler.funcall::<_, _, Value>(*ID_YIELD, ()).unwrap());
            // }
        })
    }
}
