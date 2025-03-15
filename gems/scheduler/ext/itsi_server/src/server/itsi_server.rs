use super::{
    bind::Bind,
    listener::Listener,
    serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode},
    signal::{clear_signal_handlers, reset_signal_handlers, SIGNAL_HANDLER_CHANNEL},
};
use crate::{request::itsi_request::ItsiRequest, server::serve_strategy::ServeStrategy};
use derive_more::Debug;
use itsi_rb_helpers::call_without_gvl;
use itsi_tracing::error;
use magnus::{
    block::Proc,
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs},
    value::{InnerValue, Opaque, ReprValue},
    RHash, Ruby, Symbol, Value,
};
use parking_lot::Mutex;
use std::{cmp::max, ops::Deref, sync::Arc};
use tracing::{info, instrument};

static DEFAULT_BIND: &str = "http://localhost:3000";

#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Clone)]
pub struct Server {
    pub config: Arc<ServerConfig>,
}

impl Deref for Server {
    type Target = ServerConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}
type AfterFork = Mutex<Arc<Option<Box<dyn Fn() + Send + Sync>>>>;

#[derive(Debug)]
pub struct ServerConfig {
    #[debug(skip)]
    pub app: Opaque<Value>,
    #[allow(unused)]
    pub workers: u8,
    #[allow(unused)]
    pub threads: u8,
    #[allow(unused)]
    pub shutdown_timeout: f64,
    pub script_name: String,
    pub(crate) binds: Mutex<Vec<Bind>>,
    #[debug(skip)]
    pub before_fork: Mutex<Option<Box<dyn FnOnce() + Send + Sync>>>,
    #[debug(skip)]
    pub after_fork: AfterFork,
    pub scheduler_class: Option<String>,
    pub stream_body: Option<bool>,
    pub worker_memory_limit: Option<u64>,
}

#[derive(Debug)]
pub enum RequestJob {
    ProcessRequest(ItsiRequest),
    Shutdown,
}

impl Server {
    #[instrument(
        name = "Itsi",
        parent=None,
        skip(args),
        fields(workers = 1, threads = 1, shutdown_timeout = 5)
    )]
    pub fn new(args: &[Value]) -> Result<Self> {
        let scan_args: Args<(), (), (), (), RHash, ()> = scan_args(args)?;

        type ArgSet1 = (
            Option<u8>,
            Option<u8>,
            Option<f64>,
            Option<String>,
            Option<Vec<String>>,
            Option<Proc>,
            Option<Proc>,
            Option<String>,
            Option<bool>,
        );

        type ArgSet2 = (Option<u64>,);

        let args1: KwArgs<(Value,), ArgSet1, ()> = get_kwargs(
            scan_args
                .keywords
                .funcall::<_, _, RHash>(
                    "slice",
                    (
                        Symbol::new("app"),
                        Symbol::new("workers"),
                        Symbol::new("threads"),
                        Symbol::new("shutdown_timeout"),
                        Symbol::new("script_name"),
                        Symbol::new("binds"),
                        Symbol::new("before_fork"),
                        Symbol::new("after_fork"),
                        Symbol::new("scheduler_class"),
                        Symbol::new("stream_body"),
                    ),
                )
                .unwrap(),
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
                "stream_body",
            ],
        )?;

        let args2: KwArgs<(), ArgSet2, ()> = get_kwargs(
            scan_args
                .keywords
                .funcall::<_, _, RHash>("slice", (Symbol::new("worker_memory_limit"),))
                .unwrap(),
            &[],
            &["worker_memory_limit"],
        )?;

        let config = ServerConfig {
            app: Opaque::from(args1.required.0),
            workers: max(args1.optional.0.unwrap_or(1), 1),
            threads: max(args1.optional.1.unwrap_or(1), 1),
            shutdown_timeout: args1.optional.2.unwrap_or(5.0),
            script_name: args1.optional.3.unwrap_or("".to_string()),
            binds: Mutex::new(
                args1
                    .optional
                    .4
                    .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
                    .into_iter()
                    .map(|s| s.parse())
                    .collect::<itsi_error::Result<Vec<Bind>>>()?,
            ),
            before_fork: Mutex::new(args1.optional.5.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn FnOnce() + Send + Sync>
            })),
            after_fork: Mutex::new(Arc::new(args1.optional.6.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn Fn() + Send + Sync>
            }))),
            scheduler_class: args1.optional.7.clone(),
            stream_body: args1.optional.8,
            worker_memory_limit: args2.optional.0,
        };

        if let Some(scheduler_class) = args1.optional.7 {
            info!(scheduler_class, fiber_scheduler = true);
        } else {
            info!(fiber_scheduler = false);
        }

        Ok(Server {
            config: Arc::new(config),
        })
    }

    #[instrument(name = "Bind", skip_all, fields(binds=format!("{:?}", self.config.binds.lock())))]
    pub(crate) fn listeners(&self) -> Result<Arc<Vec<Arc<Listener>>>> {
        let listeners = self
            .config
            .binds
            .lock()
            .iter()
            .cloned()
            .map(Listener::try_from)
            .collect::<std::result::Result<Vec<Listener>, _>>()?
            .into_iter()
            .map(Arc::new)
            .collect::<Vec<_>>();
        info!("Bound {:?} listeners", listeners.len());
        Ok(Arc::new(listeners))
    }

    pub(crate) fn build_strategy(
        self,
        listeners: Arc<Vec<Arc<Listener>>>,
    ) -> Result<ServeStrategy> {
        let server = Arc::new(self);

        let strategy = if server.config.workers == 1 {
            ServeStrategy::Single(Arc::new(SingleMode::new(
                server,
                listeners,
                SIGNAL_HANDLER_CHANNEL.0.clone(),
            )?))
        } else {
            ServeStrategy::Cluster(Arc::new(ClusterMode::new(
                server,
                listeners,
                SIGNAL_HANDLER_CHANNEL.0.clone(),
            )))
        };
        Ok(strategy)
    }

    pub fn start(&self) -> Result<()> {
        reset_signal_handlers();
        let rself = self.clone();
        let listeners = self.listeners()?;
        let listeners_clone = listeners.clone();
        call_without_gvl(move || -> Result<()> {
            let strategy = rself.build_strategy(listeners_clone)?;
            if let Err(e) = strategy.run() {
                error!("Error running server: {}", e);
                strategy.stop()?;
            }
            drop(strategy);
            Ok(())
        })?;
        clear_signal_handlers();
        Ok(())
    }
}
