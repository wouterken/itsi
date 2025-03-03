use super::itsi_server::RequestJob;
use crate::ITSI_SERVER;
use crossbeam::channel::{Receiver, Sender};
use itsi_rb_helpers::{call_with_gvl, call_without_gvl, create_ruby_thread};
use itsi_tracing::{debug, error, info};
use magnus::{
    value::{Opaque, ReprValue},
    Ruby, Thread, Value,
};
use std::{
    num::NonZeroU8,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct ThreadWorker {
    pub id: u8,
    pub app: Opaque<Value>,
    pub receiver: Arc<Receiver<RequestJob>>,
    pub sender: Arc<Sender<RequestJob>>,
    pub thread: Option<Opaque<Thread>>,
}

pub fn build_thread_workers(
    threads: NonZeroU8,
    app: Opaque<Value>,
) -> (
    Arc<Vec<ThreadWorker>>,
    Arc<crossbeam::channel::Sender<RequestJob>>,
) {
    let (sender, receiver) = crossbeam::channel::bounded(1000);
    let receiver_ref = Arc::new(receiver);
    let sender_ref = Arc::new(sender);
    (
        Arc::new(
            (1..=u8::from(threads))
                .map(|id| {
                    info!("Creating worker thread {}", id);
                    ThreadWorker::new(id, app, receiver_ref.clone(), sender_ref.clone())
                })
                .collect::<Vec<_>>(),
        ),
        sender_ref,
    )
}

impl ThreadWorker {
    pub fn new(
        id: u8,
        app: Opaque<Value>,
        receiver: Arc<Receiver<RequestJob>>,
        sender: Arc<Sender<RequestJob>>,
    ) -> Self {
        let mut worker = Self {
            id,
            app,
            receiver,
            sender,
            thread: None,
        };
        worker.run();
        worker
    }

    pub async fn shutdown(&self, timeout: f64) {
        info!("Sending shutdown to worker {}", self.id);
        match self.sender.send(RequestJob::Shutdown) {
            Ok(_) => {}
            Err(err) => error!("Failed to send shutdown request: {}", err),
        };
        call_with_gvl(|ruby| {
            let hard_kill_deadline =
                Instant::now() + Duration::from_millis((timeout * 1000.0) as u64);
            if let Some(opaque) = self.thread {
                let thread = ruby.get_inner_ref(&opaque);
                while Instant::now() < hard_kill_deadline {
                    if thread.funcall::<_, _, bool>("alive?", ()).unwrap_or(false) {
                        call_without_gvl(|| {
                            std::thread::sleep(Duration::from_millis(10));
                        });
                    } else {
                        break;
                    }
                }
                thread.kill().ok();
            };
        });
    }

    pub fn run(&mut self) {
        let id = self.id;
        let app = self.app;
        let receiver = self.receiver.clone();
        call_with_gvl(|_ruby| {
            self.thread = Some(Opaque::from(create_ruby_thread(move || {
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
            })))
        });
    }
}
