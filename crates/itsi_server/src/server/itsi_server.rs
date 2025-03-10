use super::{
    bind::Bind,
    listener::Listener,
    serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode},
    signal::{reset_signal_handlers, SIGNAL_HANDLER_CHANNEL},
};
use crate::{request::itsi_request::ItsiRequest, server::serve_strategy::ServeStrategy};
use derive_more::Debug;
use itsi_rb_helpers::call_without_gvl;
use itsi_tracing::error;
use magnus::{
    block::Proc,
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs},
    value::{InnerValue, Opaque},
    RHash, Ruby, Value,
};
use parking_lot::Mutex;
use std::{cmp::max, ops::Deref, sync::Arc};
use tracing::instrument;

static DEFAULT_BIND: &str = "localhost:3000";

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
}

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
        type OptionalArgs = (
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
                "stream_body",
            ],
        )?;

        let config = ServerConfig {
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
                    .map(|s| s.parse())
                    .collect::<itsi_error::Result<Vec<Bind>>>()?,
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
            after_fork: Mutex::new(Arc::new(args.optional.6.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn Fn() + Send + Sync>
            }))),
            scheduler_class: args.optional.7,
            stream_body: args.optional.8,
        };

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
        Ok(Arc::new(listeners))
    }

    pub(crate) fn build_strategy(self) -> Result<ServeStrategy> {
        let server = Arc::new(self);
        let listeners = server.listeners()?;

        let strategy = if server.config.workers == 1 {
            ServeStrategy::Single(Arc::new(SingleMode::new(
                server,
                listeners,
                SIGNAL_HANDLER_CHANNEL.0.clone(),
            )))
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
        call_without_gvl(|| {
            if let Err(e) = self.clone().build_strategy()?.run() {
                error!("Error running server: {}", e);
            }
            Ok(())
        })
    }
}
