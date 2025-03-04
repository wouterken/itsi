use super::itsi_server::RequestJob;
use crate::ITSI_SERVER;
use crossbeam::channel::{Receiver, Sender};
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, soft_kill_threads, RetainedValue,
};
use itsi_tracing::{debug, error, info, warn};
use magnus::{
    value::{InnerValue, LazyId, Opaque, ReprValue},
    Ruby, Thread, Value,
};
use nix::unistd::Pid;
use std::{num::NonZeroU8, sync::Arc, time::Instant};
use tracing::instrument;
pub struct ThreadWorker {
    pub id: String,
    pub app: Opaque<Value>,
    pub receiver: Arc<Receiver<RequestJob>>,
    pub sender: Arc<Sender<RequestJob>>,
    pub thread: RetainedValue<Thread>,
}

static ID_CALL: LazyId = LazyId::new("call");

pub fn load_app(app: Opaque<Value>) -> Opaque<Value> {
    call_with_gvl(|ruby| {
        let app = app.get_inner_with(&ruby);
        Opaque::from(
            app.funcall::<_, _, Value>(*ID_CALL, ())
                .expect("Couldn't load app"),
        )
    })
}

#[instrument(skip(threads, app))]
pub fn build_thread_workers(
    pid: Pid,
    threads: NonZeroU8,
    app: Opaque<Value>,
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
                    )
                })
                .collect::<Vec<_>>(),
        ),
        sender_ref,
    )
}

impl ThreadWorker {
    pub fn new(
        id: String,
        app: Opaque<Value>,
        receiver: Arc<Receiver<RequestJob>>,
        sender: Arc<Sender<RequestJob>>,
    ) -> Self {
        let mut worker = Self {
            id,
            app,
            receiver,
            sender,
            thread: RetainedValue::empty(),
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
        info!("Polling worker thread {} for shutdown", self.id);

        call_with_gvl(|_ruby| {
            if let Some(thread) = self.thread.as_value() {
                if Instant::now() > deadline {
                    warn!("Worker thread {} timed out. Killing thread", self.id);
                    soft_kill_threads(vec![thread]);
                }
                if thread.funcall::<_, _, bool>("alive?", ()).unwrap_or(false) {
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
        call_with_gvl(|_ruby| {
            let thread = create_ruby_thread(move || {
                let ruby = Ruby::get().unwrap();
                let server = ruby.get_inner(&ITSI_SERVER);
                call_without_gvl(|| loop {
                    match receiver.recv() {
                        Ok(RequestJob::ProcessRequest(request)) => {
                            debug!("Incoming request for worker {}", id);
                            match call_with_gvl(|ruby| request.process(&ruby, server, app)) {
                                Ok(_) => {}
                                Err(err) => error!("Request processing failed: {}", err),
                            }
                        }
                        Ok(RequestJob::Shutdown) => {
                            debug!("Shutting down thread worker {}", id);
                            break;
                        }
                        Err(err) => {
                            error!("Error receiving request job: {}", err);
                        }
                    }
                });
                0
            });
            self.thread = RetainedValue::new(thread);
        });
    }
}
