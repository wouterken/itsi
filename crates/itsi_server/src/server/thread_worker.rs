use async_channel::Sender;
use itsi_error::ItsiError;
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, kill_threads, HeapValue,
};
use itsi_tracing::{debug, error};
use magnus::{
    error::Result,
    value::{InnerValue, Lazy, LazyId, Opaque, ReprValue},
    Module, RClass, Ruby, Thread, Value,
};
use parking_lot::{Mutex, RwLock};
use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{runtime::Builder as RuntimeBuilder, sync::watch};
use tracing::instrument;

use crate::{
    ruby_types::{
        itsi_grpc_call::ItsiGrpcCall, itsi_http_request::ItsiHttpRequest,
        itsi_server::itsi_server_config::ServerParams, ITSI_SERVER,
    },
    server::process_worker::CORE_IDS,
};

use super::request_job::RequestJob;
pub struct ThreadWorker {
    pub params: Arc<ServerParams>,
    pub id: u8,
    pub worker_id: usize,
    pub request_id: AtomicU64,
    pub current_request_start: AtomicU64,
    pub receiver: Arc<async_channel::Receiver<RequestJob>>,
    pub sender: Sender<RequestJob>,
    pub thread: RwLock<Option<HeapValue<Thread>>>,
    pub terminated: Arc<AtomicBool>,
    pub scheduler_class: Option<Opaque<Value>>,
}

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
type ThreadWorkerBuildResult = Result<(
    Arc<Vec<Arc<ThreadWorker>>>,
    Sender<RequestJob>,
    Sender<RequestJob>,
)>;

#[instrument(name = "boot", parent=None, skip(params, worker_id))]
pub fn build_thread_workers(
    params: Arc<ServerParams>,
    worker_id: usize,
) -> ThreadWorkerBuildResult {
    let blocking_thread_count = params.threads;
    let nonblocking_thread_count = params.scheduler_threads;
    let ruby_thread_request_backlog_size: usize = params
        .ruby_thread_request_backlog_size
        .unwrap_or_else(|| (blocking_thread_count as u16 * 30) as usize);

    let (blocking_sender, blocking_receiver) =
        async_channel::bounded(ruby_thread_request_backlog_size);
    let blocking_receiver_ref = Arc::new(blocking_receiver);
    let blocking_sender_ref = blocking_sender;
    let scheduler_class = load_scheduler_class(params.scheduler_class.clone())?;

    let mut workers = (1..=blocking_thread_count)
        .map(|id| {
            ThreadWorker::new(
                params.clone(),
                id,
                worker_id,
                blocking_receiver_ref.clone(),
                blocking_sender_ref.clone(),
                if nonblocking_thread_count.is_some() {
                    None
                } else {
                    scheduler_class
                },
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let nonblocking_sender_ref = if let (Some(nonblocking_thread_count), Some(scheduler_class)) =
        (nonblocking_thread_count, scheduler_class)
    {
        let (nonblocking_sender, nonblocking_receiver) =
            async_channel::bounded((nonblocking_thread_count as u16 * 30) as usize);
        let nonblocking_receiver_ref = Arc::new(nonblocking_receiver);
        let nonblocking_sender_ref = nonblocking_sender.clone();
        for id in 0..nonblocking_thread_count {
            workers.push(ThreadWorker::new(
                params.clone(),
                id,
                worker_id,
                nonblocking_receiver_ref.clone(),
                nonblocking_sender_ref.clone(),
                Some(scheduler_class),
            )?)
        }
        nonblocking_sender
    } else {
        blocking_sender_ref.clone()
    };

    Ok((
        Arc::new(workers),
        blocking_sender_ref,
        nonblocking_sender_ref,
    ))
}

pub fn load_scheduler_class(scheduler_class: Option<String>) -> Result<Option<Opaque<Value>>> {
    call_with_gvl(|ruby| {
        let scheduler_class = if let Some(scheduler_class) = scheduler_class {
            Some(Opaque::from(
                ruby.module_kernel()
                    .funcall::<_, _, Value>(*ID_CONST_GET, (scheduler_class,))?,
            ))
        } else {
            None
        };
        Ok(scheduler_class)
    })
}
impl ThreadWorker {
    pub fn new(
        params: Arc<ServerParams>,
        id: u8,
        worker_id: usize,
        receiver: Arc<async_channel::Receiver<RequestJob>>,
        sender: Sender<RequestJob>,
        scheduler_class: Option<Opaque<Value>>,
    ) -> Result<Arc<Self>> {
        let worker = Arc::new(Self {
            params,
            id,
            worker_id,
            request_id: AtomicU64::new(0),
            current_request_start: AtomicU64::new(0),
            receiver,
            sender,
            thread: RwLock::new(None),
            terminated: Arc::new(AtomicBool::new(false)),
            scheduler_class,
        });
        worker.clone().run()?;
        Ok(worker)
    }

    #[instrument(skip(self, deadline), fields(id = self.id))]
    pub fn poll_shutdown(&self, deadline: Instant) -> bool {
        if let Some(thread) = self.thread.read().deref() {
            if Instant::now() > deadline {
                debug!("Worker shutdown timed out. Killing thread {:?}", thread);
                self.terminated.store(true, Ordering::SeqCst);
                kill_threads(vec![thread.as_value()]);
            }
            if thread.funcall::<_, _, bool>(*ID_ALIVE, ()).unwrap_or(false) {
                return true;
            }
            debug!("Thread has shut down");
        }
        self.thread.write().take();

        false
    }

    pub fn run(self: Arc<Self>) -> Result<()> {
        let receiver = self.receiver.clone();
        let terminated = self.terminated.clone();
        let scheduler_class = self.scheduler_class;
        let params = self.params.clone();
        let self_ref = self.clone();
        let worker_id = self.worker_id;
        call_with_gvl(|_| {
            *self.thread.write() = Some(
                create_ruby_thread(move || {
                    if params.pin_worker_cores {
                        core_affinity::set_for_current(
                            CORE_IDS[((2 * worker_id) + 1) % CORE_IDS.len()],
                        );
                    }
                    debug!("Ruby thread worker started");
                    if let Some(scheduler_class) = scheduler_class {
                        if let Err(err) = self_ref.fiber_accept_loop(
                            params,
                            receiver,
                            scheduler_class,
                            terminated,
                        ) {
                            error!("Error in fiber_accept_loop: {:?}", err);
                        }
                    } else {
                        self_ref.accept_loop(params, receiver, terminated);
                    }
                })
                .ok_or_else(|| {
                    ItsiError::InternalServerError("Failed to create Ruby thread".to_owned())
                })?
                .into(),
            );
            Ok::<(), magnus::Error>(())
        })?;
        Ok(())
    }

    pub fn build_scheduler_proc(
        self: Arc<Self>,
        leader: &Arc<Mutex<Option<RequestJob>>>,
        receiver: &Arc<async_channel::Receiver<RequestJob>>,
        terminated: &Arc<AtomicBool>,
        waker_sender: &watch::Sender<TerminateWakerSignal>,
        oob_gc_responses_threshold: Option<u64>,
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
            let self_ref = self.clone();
            let mut batch = Vec::with_capacity(MAX_BATCH_SIZE as usize);

            static MAX_BATCH_SIZE: i32 = 25;
            call_without_gvl(move || loop {
                let mut idle_counter = 0;
                if let Some(v) = leader_clone.lock().take() {
                    match v {
                        RequestJob::ProcessHttpRequest(itsi_request, app_proc) => {
                            batch.push(RequestJob::ProcessHttpRequest(itsi_request, app_proc))
                        }
                        RequestJob::ProcessGrpcRequest(itsi_request, app_proc) => {
                            batch.push(RequestJob::ProcessGrpcRequest(itsi_request, app_proc))
                        }
                        RequestJob::Shutdown => {
                            waker_sender.send(TerminateWakerSignal(true)).unwrap();
                            break;
                        }
                    }
                }

                for _ in 0..MAX_BATCH_SIZE {
                    if let Ok(req) = receiver.try_recv() {
                        let should_break = matches!(req, RequestJob::Shutdown);
                        batch.push(req);
                        if should_break {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                let shutdown_requested = call_with_gvl(|_| {
                    for req in batch.drain(..) {
                        match req {
                            RequestJob::ProcessHttpRequest(request, app_proc) => {
                                self_ref.request_id.fetch_add(1, Ordering::Relaxed);
                                self_ref.current_request_start.store(
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    Ordering::Relaxed,
                                );
                                let response = request.response.clone();
                                if let Err(err) = server.funcall::<_, _, Value>(
                                    *ID_SCHEDULE,
                                    (app_proc.as_value(), request),
                                ) {
                                    ItsiHttpRequest::internal_error(ruby, response, err)
                                }
                            }
                            RequestJob::ProcessGrpcRequest(request, app_proc) => {
                                self_ref.request_id.fetch_add(1, Ordering::Relaxed);
                                self_ref.current_request_start.store(
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    Ordering::Relaxed,
                                );
                                let response = request.stream.clone();
                                if let Err(err) = server.funcall::<_, _, Value>(
                                    *ID_SCHEDULE,
                                    (app_proc.as_value(), request),
                                ) {
                                    ItsiGrpcCall::internal_error(ruby, response, err)
                                }
                            }
                            RequestJob::Shutdown => {
                                return true;
                            }
                        }
                    }
                    false
                });

                if shutdown_requested || terminated.load(Ordering::Relaxed) {
                    waker_sender.send(TerminateWakerSignal(true)).unwrap();
                    break;
                }

                let yield_result = if receiver.is_empty() {
                    let should_gc = if let Some(oob_gc_threshold) = oob_gc_responses_threshold {
                        idle_counter = (idle_counter + 1) % oob_gc_threshold;
                        idle_counter == 0
                    } else {
                        false
                    };
                    waker_sender.send(TerminateWakerSignal(false)).unwrap();
                    call_with_gvl(|ruby| {
                        if should_gc {
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
            });
        })
    }

    #[instrument(skip_all, fields(thread_worker=format!("{}:{}", self.id, self.worker_id)))]
    pub fn fiber_accept_loop(
        self: Arc<Self>,
        params: Arc<ServerParams>,
        receiver: Arc<async_channel::Receiver<RequestJob>>,
        scheduler_class: Opaque<Value>,
        terminated: Arc<AtomicBool>,
    ) -> Result<()> {
        let ruby = Ruby::get().unwrap();
        let (waker_sender, waker_receiver) = watch::channel(TerminateWakerSignal(false));
        let leader: Arc<Mutex<Option<RequestJob>>> = Arc::new(Mutex::new(None));
        let server_class = ruby.get_inner(&ITSI_SERVER);
        let scheduler_proc = self.build_scheduler_proc(
            &leader,
            &receiver,
            &terminated,
            &waker_sender,
            params.oob_gc_responses_threshold,
        );
        let (scheduler, scheduler_fiber) = server_class.funcall::<_, _, (Value, Value)>(
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

    #[instrument(skip_all, fields(thread_worker=format!("{}:{}", self.id, self.worker_id)))]
    pub fn accept_loop(
        self: Arc<Self>,
        params: Arc<ServerParams>,
        receiver: Arc<async_channel::Receiver<RequestJob>>,
        terminated: Arc<AtomicBool>,
    ) {
        let mut idle_counter = 0;
        call_without_gvl(|| loop {
            match receiver.recv_blocking() {
                Err(_) => break,
                Ok(RequestJob::Shutdown) => break,
                Ok(request_job) => call_with_gvl(|ruby| {
                    self.process_one(&ruby, request_job, &terminated);
                    while let Ok(request_job) = receiver.try_recv() {
                        if matches!(request_job, RequestJob::Shutdown) {
                            terminated.store(true, Ordering::Relaxed);
                            break;
                        }
                        self.process_one(&ruby, request_job, &terminated);
                    }
                    if let Some(thresh) = params.oob_gc_responses_threshold {
                        idle_counter = (idle_counter + 1) % thresh;
                        if idle_counter == 0 {
                            ruby.gc_start();
                        }
                    }
                }),
            };
            if terminated.load(Ordering::Relaxed) {
                break;
            }
        });
    }

    fn process_one(self: &Arc<Self>, ruby: &Ruby, job: RequestJob, terminated: &Arc<AtomicBool>) {
        match job {
            RequestJob::ProcessHttpRequest(request, app_proc) => {
                if terminated.load(Ordering::Relaxed) {
                    request.response().unwrap().service_unavailable();
                    return;
                }
                self.request_id.fetch_add(1, Ordering::Relaxed);
                self.current_request_start.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    Ordering::Relaxed,
                );
                request.process(ruby, app_proc).ok();
            }

            RequestJob::ProcessGrpcRequest(request, app_proc) => {
                if terminated.load(Ordering::Relaxed) {
                    request.stream().unwrap().close().ok();
                    return;
                }
                self.request_id.fetch_add(1, Ordering::Relaxed);
                self.current_request_start.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    Ordering::Relaxed,
                );
                request.process(ruby, app_proc).ok();
            }

            RequestJob::Shutdown => unreachable!(),
        }
    }
}
