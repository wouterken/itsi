use super::itsi_server::RequestJob;
use crate::{request::itsi_request::ItsiRequest, ITSI_SERVER};
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, kill_threads, HeapVal, HeapValue,
};
use itsi_tracing::{debug, error, info, warn};
use magnus::{
    error::Result,
    value::{InnerValue, Lazy, LazyId, Opaque, ReprValue},
    Module, RClass, Ruby, Thread, Value,
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
use tokio::{runtime::Builder as RuntimeBuilder, sync::watch};
use tracing::instrument;
pub struct ThreadWorker {
    pub id: String,
    pub app: Opaque<Value>,
    pub receiver: Arc<async_channel::Receiver<RequestJob>>,
    pub sender: async_channel::Sender<RequestJob>,
    pub thread: RwLock<Option<HeapValue<Thread>>>,
    pub terminated: Arc<AtomicBool>,
    pub scheduler_class: Option<Opaque<Value>>,
}

static ID_CALL: LazyId = LazyId::new("call");
static ID_ALIVE: LazyId = LazyId::new("alive?");
static ID_SCHEDULER: LazyId = LazyId::new("scheduler");
static ID_SCHEDULE: LazyId = LazyId::new("schedule");
static ID_BLOCK: LazyId = LazyId::new("block");
static ID_YIELD: LazyId = LazyId::new("yield");
static ID_CONST_GET: LazyId = LazyId::new("const_get");
static CLASS_FIBER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.module_kernel()
        .const_get::<_, RClass>("Fiber")
        .unwrap()
});

pub struct TerminateWakerSignal(bool);

#[instrument(name = "Boot", parent=None, skip(threads, app, pid, scheduler_class))]
pub fn build_thread_workers(
    pid: Pid,
    threads: NonZeroU8,
    app: HeapVal,
    scheduler_class: Option<String>,
) -> Result<(Arc<Vec<ThreadWorker>>, async_channel::Sender<RequestJob>)> {
    let (sender, receiver) = async_channel::bounded(20);
    let receiver_ref = Arc::new(receiver);
    let sender_ref = sender;
    let (app, scheduler_class) = load_app(app, scheduler_class)?;
    Ok((
        Arc::new(
            (1..=u8::from(threads))
                .map(|id| {
                    info!(pid = pid.as_raw(), id, "Thread");
                    ThreadWorker::new(
                        format!("{:?}#{:?}", pid, id),
                        app,
                        receiver_ref.clone(),
                        sender_ref.clone(),
                        scheduler_class,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        sender_ref,
    ))
}

pub fn load_app(
    app: HeapVal,
    scheduler_class: Option<String>,
) -> Result<(Opaque<Value>, Option<Opaque<Value>>)> {
    call_with_gvl(|ruby| {
        let app = Opaque::from(
            app.funcall::<_, _, Value>(*ID_CALL, ())
                .expect("Couldn't load app"),
        );
        let scheduler_class = if let Some(scheduler_class) = scheduler_class {
            Some(Opaque::from(
                ruby.module_kernel()
                    .funcall::<_, _, Value>(*ID_CONST_GET, (scheduler_class,))?,
            ))
        } else {
            None
        };
        Ok((app, scheduler_class))
    })
}
impl ThreadWorker {
    pub fn new(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<async_channel::Receiver<RequestJob>>,
        sender: async_channel::Sender<RequestJob>,
        scheduler_class: Option<Opaque<Value>>,
    ) -> Result<Self> {
        let mut worker = Self {
            id,
            app,
            receiver,
            sender,
            thread: RwLock::new(None),
            terminated: Arc::new(AtomicBool::new(false)),
            scheduler_class,
        };
        worker.run()?;
        Ok(worker)
    }

    #[instrument(skip(self), fields(id = self.id))]
    pub async fn request_shutdown(&self) {
        match self.sender.send(RequestJob::Shutdown).await {
            Ok(_) => {}
            Err(err) => error!("Failed to send shutdown request: {}", err),
        };
        info!("Requesting shutdown");
    }

    #[instrument(skip(self, deadline), fields(id = self.id))]
    pub fn poll_shutdown(&self, deadline: Instant) -> bool {
        call_with_gvl(|_ruby| {
            if let Some(thread) = self.thread.read().deref() {
                if Instant::now() > deadline {
                    warn!("Worker shutdown timed out. Killing thread");
                    self.terminated.store(true, Ordering::SeqCst);
                    kill_threads(vec![thread.as_value()]);
                }
                if thread.funcall::<_, _, bool>(*ID_ALIVE, ()).unwrap_or(false) {
                    return true;
                }
                info!("Thread has shut down");
            }
            self.thread.write().take();

            false
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let id = self.id.clone();
        let app = self.app;
        let receiver = self.receiver.clone();
        let terminated = self.terminated.clone();
        let scheduler_class = self.scheduler_class;
        call_with_gvl(|_| {
            *self.thread.write() = Some(
                create_ruby_thread(move || {
                    if let Some(scheduler_class) = scheduler_class {
                        if let Err(err) =
                            Self::fiber_accept_loop(id, app, receiver, scheduler_class, terminated)
                        {
                            error!("Error in fiber_accept_loop: {:?}", err);
                        }
                    } else {
                        Self::accept_loop(id, app, receiver, terminated);
                    }
                })
                .into(),
            );
            Ok::<(), magnus::Error>(())
        })?;
        Ok(())
    }

    pub fn build_scheduler_proc(
        app: Opaque<Value>,
        leader: &Arc<Mutex<Option<RequestJob>>>,
        receiver: &Arc<async_channel::Receiver<RequestJob>>,
        terminated: &Arc<AtomicBool>,
        waker_sender: &watch::Sender<TerminateWakerSignal>,
    ) -> magnus::block::Proc {
        let leader = leader.clone();
        let receiver = receiver.clone();
        let terminated = terminated.clone();
        let waker_sender = waker_sender.clone();
        Ruby::get().unwrap().proc_from_fn(move |ruby, _args, _blk| {
            let scheduler = ruby
                .get_inner(&CLASS_FIBER)
                .funcall::<_, _, Value>(*ID_SCHEDULER, ())
                .unwrap();
            let server = ruby.get_inner(&ITSI_SERVER);
            let thread_current = ruby.thread_current();
            let leader_clone = leader.clone();
            let receiver = receiver.clone();
            let terminated = terminated.clone();
            let waker_sender = waker_sender.clone();
            let mut batch = Vec::with_capacity(MAX_BATCH_SIZE as usize);

            static MAX_BATCH_SIZE: i32 = 25;
            call_without_gvl(move || loop {
                let mut idle_counter = 0;
                if let Some(v) = leader_clone.lock().take() {
                    match v {
                        RequestJob::ProcessRequest(itsi_request) => {
                            batch.push(RequestJob::ProcessRequest(itsi_request))
                        }
                        RequestJob::Shutdown => {
                            waker_sender.send(TerminateWakerSignal(true)).unwrap();
                            break;
                        }
                    }
                }
                for _ in 0..MAX_BATCH_SIZE {
                    if let Ok(req) = receiver.try_recv() {
                        batch.push(req);
                    } else {
                        break;
                    }
                }

                let shutdown_requested = call_with_gvl(|_| {
                    for req in batch.drain(..) {
                        match req {
                            RequestJob::ProcessRequest(request) => {
                                let response = request.response.clone();
                                if let Err(err) =
                                    server.funcall::<_, _, Value>(*ID_SCHEDULE, (app, request))
                                {
                                    ItsiRequest::internal_error(ruby, response, err)
                                }
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

                let yield_result = if receiver.is_empty() {
                    waker_sender.send(TerminateWakerSignal(false)).unwrap();
                    idle_counter = (idle_counter + 1) % 100;
                    call_with_gvl(|ruby| {
                        if idle_counter == 0 {
                            ruby.gc_start();
                        }
                        scheduler.funcall::<_, _, Value>(*ID_BLOCK, (thread_current, None::<u8>))
                    })
                } else {
                    call_with_gvl(|_| scheduler.funcall::<_, _, Value>(*ID_YIELD, ()))
                };

                if yield_result.is_err() {
                    break;
                }
            })
        })
    }

    #[instrument(skip_all, fields(thread_worker=id))]
    pub fn fiber_accept_loop(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<async_channel::Receiver<RequestJob>>,
        scheduler_class: Opaque<Value>,
        terminated: Arc<AtomicBool>,
    ) -> Result<()> {
        let ruby = Ruby::get().unwrap();
        let (waker_sender, waker_receiver) = watch::channel(TerminateWakerSignal(false));
        let leader: Arc<Mutex<Option<RequestJob>>> = Arc::new(Mutex::new(None));
        let server = ruby.get_inner(&ITSI_SERVER);
        let scheduler_proc =
            Self::build_scheduler_proc(app, &leader, &receiver, &terminated, &waker_sender);
        let (scheduler, scheduler_fiber) = server.funcall::<_, _, (Value, Value)>(
            "start_scheduler_loop",
            (scheduler_class, scheduler_proc),
        )?;
        Self::start_waker_thread(
            scheduler.into(),
            scheduler_fiber.into(),
            leader,
            receiver,
            waker_receiver,
        );
        Ok(())
    }

    #[allow(clippy::await_holding_lock)]
    pub fn start_waker_thread(
        scheduler: Opaque<Value>,
        scheduler_fiber: Opaque<Value>,
        leader: Arc<Mutex<Option<RequestJob>>>,
        receiver: Arc<async_channel::Receiver<RequestJob>>,
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
                            tokio::select! {
                                _ = waker_receiver.changed() => {
                                  if waker_receiver.borrow().0 {
                                      break;
                                  }
                                },
                                next_msg = receiver.recv() => {
                                  *leader.lock() = next_msg.ok();
                                  call_with_gvl(|_| {
                                      scheduler
                                          .funcall::<_, _, Value>(
                                              "unblock",
                                              (None::<u8>, scheduler_fiber),
                                          )
                                          .ok();
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
        receiver: Arc<async_channel::Receiver<RequestJob>>,
        terminated: Arc<AtomicBool>,
    ) {
        let ruby = Ruby::get().unwrap();
        let server = ruby.get_inner(&ITSI_SERVER);
        call_without_gvl(|| loop {
            match receiver.recv_blocking() {
                Ok(RequestJob::ProcessRequest(request)) => {
                    if terminated.load(Ordering::Relaxed) {
                        break;
                    }
                    call_with_gvl(|_ruby| {
                        request.process(&ruby, server, app).ok();
                    })
                }
                Ok(RequestJob::Shutdown) => {
                    debug!("Shutting down thread worker");
                    break;
                }
                Err(_) => {
                    thread::sleep(Duration::from_micros(1));
                }
            }
        });
    }
}
