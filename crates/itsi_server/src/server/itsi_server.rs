use super::{
    bind::Bind,
    listener::Listener,
    serve_strategy::{
        cluster_mode::{ClusterLifecycle, ClusterMode},
        single_mode::SingleMode,
    },
};
use crate::{request::itsi_request::ItsiRequest, server::serve_strategy::ServeStrategy};
use derive_more::Debug;
use hyper_util::{rt::TokioExecutor, server::conn::auto::Builder};
use itsi_rb_helpers::call_without_gvl;
use itsi_tracing::{error, info};
use magnus::{
    block::Proc,
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs},
    value::{InnerValue, Opaque},
    RHash, Ruby, Value,
};
use parking_lot::Mutex;
use std::{cmp::max, num::NonZero, sync::Arc};
use tracing::instrument;

static DEFAULT_BIND: &str = "localhost:3000";

#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Debug)]
pub struct Server {
    #[debug(skip)]
    app: Opaque<Value>,
    #[allow(unused)]
    workers: u8,
    #[allow(unused)]
    threads: u8,
    #[allow(unused)]
    shutdown_timeout: f64,
    script_name: String,
    pub(crate) binds: Mutex<Vec<Bind>>,
    #[debug(skip)]
    before_fork: Mutex<Option<Box<dyn FnOnce() + Send + Sync>>>,
    #[debug(skip)]
    after_fork: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

pub enum RequestJob {
    ProcessRequest(ItsiRequest),
    Shutdown,
}

impl Server {
    pub fn new(args: &[Value]) -> Result<Self> {
        type OptionalArgs = (
            Option<u8>,
            Option<u8>,
            Option<f64>,
            Option<String>,
            Option<Vec<String>>,
            Option<Proc>,
            Option<Proc>,
            Option<Value>,
            Option<bool>,
        );

        let scan_args: Args<(), (), (), (), RHash, ()> = scan_args(args)?;
        let args: KwArgs<(Value,), OptionalArgs, ()> = get_kwargs(
            scan_args.keywords,
            &["app"],
            &[
                "workers",
                "threads",
                "shutdown_timeout",
                "script_name",
                "binds",
                "before_fork",
                "after_fork",
                "scheduler_class",
                "use_scheduler",
            ],
        )?;
        let server = Server {
            app: Opaque::from(args.required.0),
            workers: max(args.optional.0.unwrap_or(1), 1),
            threads: max(args.optional.1.unwrap_or(1), 1),
            shutdown_timeout: args.optional.2.unwrap_or(5.0),
            script_name: args.optional.3.unwrap_or("".to_string()),
            binds: Mutex::new(
                args.optional
                    .4
                    .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
                    .into_iter()
                    .map(|s| s.parse().unwrap_or_else(|_| Bind::default()))
                    .collect(),
            ),
            before_fork: Mutex::new(args.optional.5.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn FnOnce() + Send + Sync>
            })),
            after_fork: Mutex::new(args.optional.6.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn Fn() + Send + Sync>
            })),
        };
        Ok(server)
    }

    pub(crate) fn listeners(&self) -> Arc<Vec<Arc<Listener>>> {
        Arc::new(
            self.binds
                .lock()
                .iter()
                .cloned()
                .map(Listener::from)
                .map(Arc::new)
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn build_strategy(&self) -> ServeStrategy {
        if self.workers == 1 {
            ServeStrategy::Single(Arc::new(SingleMode::new(
                self.app,
                self.listeners(),
                Builder::new(TokioExecutor::new()),
                NonZero::new(self.threads).unwrap(),
                self.script_name.clone(),
                self.shutdown_timeout,
            )))
        } else {
            let before_fork = self.before_fork.lock().take();
            let after_fork = self.after_fork.lock().take();
            let lifecycle_hooks = ClusterLifecycle {
                before_fork,
                after_fork: Arc::new(after_fork),
                shutdown_timeout: self.shutdown_timeout,
            };
            ServeStrategy::Cluster(Arc::new(ClusterMode::new(
                self.app,
                self.listeners(),
                Builder::new(TokioExecutor::new()),
                self.script_name.clone(),
                NonZero::new(self.threads).unwrap(),
                NonZero::new(self.workers).unwrap(),
                lifecycle_hooks,
            )))
        }
    }

    pub fn start(&self) {
        info!(
            "Starting Itsi Server on {:?}. Threads: {}",
            self.binds.lock(),
            self.threads
        );

        call_without_gvl(|| {
            if let Err(e) = self.build_strategy().run() {
                error!("Error running server: {}", e);
            }
        });
    }
}
