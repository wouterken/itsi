use crate::server::{listener::Listener, process_worker::ProcessWorker};
use hyper_util::{rt::TokioExecutor, server::conn::auto::Builder};
use itsi_error::Result;
use magnus::{value::Opaque, Value};
use std::{num::NonZeroU8, sync::Arc};

pub(crate) struct ClusterMode {
    pub app: Opaque<Value>,
    pub server: Builder<TokioExecutor>,
    pub listeners: Arc<Vec<Arc<Listener>>>,
    pub script_name: String,
    pub thread_count: NonZeroU8,
    pub process_workers: Vec<ProcessWorker>,
    pub lifecycle: ClusterLifecycle,
}

pub(crate) struct ClusterLifecycle {
    pub before_fork: Option<Box<dyn FnOnce() + Send + Sync>>,
    pub after_fork: Arc<Option<Box<dyn Fn() + Send + Sync>>>,
    pub shutdown_timeout: f64,
}

impl ClusterMode {
    pub fn new(
        app: Opaque<Value>,
        listeners: Arc<Vec<Arc<Listener>>>,
        server: Builder<TokioExecutor>,
        script_name: String,
        thread_count: NonZeroU8,
        worker_count: NonZeroU8,
        mut lifecycle: ClusterLifecycle,
    ) -> Self {
        if let Some(f) = lifecycle.before_fork.take() {
            f();
        }
        let process_workers = (0..worker_count.get())
            .map(|worker_id| ProcessWorker {
                worker_id,
                ..Default::default()
            })
            .collect();

        Self {
            app,
            server,
            listeners,
            script_name,
            thread_count,
            process_workers,
            lifecycle,
        }
    }

    pub fn run(self: Arc<Self>) -> Result<()> {
        self.process_workers
            .iter()
            .for_each(|worker| worker.boot(Arc::clone(&self)));
        Ok(())
    }
}
