use super::{
    bind::Bind,
    listener::Listener,
    serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode},
    signal::{
        clear_signal_handlers, reset_signal_handlers, send_shutdown_event, SIGNAL_HANDLER_CHANNEL,
    },
};
use crate::{request::itsi_request::ItsiRequest, server::serve_strategy::ServeStrategy};
use derive_more::Debug;
use itsi_rb_helpers::{call_without_gvl, HeapVal};
use itsi_tracing::{error, run_silently};
use magnus::{
    block::Proc,
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs, ScanArgsKw, ScanArgsOpt, ScanArgsRequired},
    value::{InnerValue, Opaque, ReprValue},
    ArgList, RHash, Ruby, Symbol, Value,
};
use parking_lot::{Mutex, RwLock};
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
    pub app: HeapVal,
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
    #[debug(skip)]
    pub(crate) strategy: RwLock<Option<ServeStrategy>>,
    pub silence: bool,
}

#[derive(Debug)]
pub enum RequestJob {
    ProcessRequest(ItsiRequest),
    Shutdown,
}

fn extract_args<Req, Opt, Splat>(
    scan_args: &Args<(), (), (), (), RHash, ()>,
    primaries: &[&str],
    rest: &[&str],
) -> Result<KwArgs<Req, Opt, Splat>>
where
    Req: ScanArgsRequired,
    Opt: ScanArgsOpt,
    Splat: ScanArgsKw,
{
    let symbols: Vec<Symbol> = primaries
        .iter()
        .chain(rest.iter())
        .map(|&name| Symbol::new(name))
        .collect();

    let hash = scan_args
        .keywords
        .funcall::<_, _, RHash>("slice", symbols.into_arg_list_with(&Ruby::get().unwrap()))
        .unwrap();

    get_kwargs(hash, primaries, rest)
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

        type Args1 = KwArgs<
            (Value,),
            (
                // Workers
                Option<u8>,
                // Threads
                Option<u8>,
                // Shutdown Timeout
                Option<f64>,
                // Script Name
                Option<String>,
                // Binds
                Option<Vec<String>>,
                // Stream Body
                Option<bool>,
            ),
            (),
        >;

        type Args2 = KwArgs<
            (),
            (
                // Before Fork
                Option<Proc>,
                // After Fork
                Option<Proc>,
                // Scheduler Class
                Option<String>,
                // Worker Memory Limit
                Option<u64>,
                // Silence
                Option<bool>,
            ),
            (),
        >;

        let args1: Args1 = extract_args(
            &scan_args,
            &["app"],
            &[
                "workers",
                "threads",
                "shutdown_timeout",
                "script_name",
                "binds",
                "stream_body",
            ],
        )?;

        let args2: Args2 = extract_args(
            &scan_args,
            &[],
            &[
                "before_fork",
                "after_fork",
                "scheduler_class",
                "worker_memory_limit",
                "silence",
            ],
        )?;

        let config = ServerConfig {
            app: HeapVal::from(args1.required.0),
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
            stream_body: args1.optional.5,
            before_fork: Mutex::new(args2.optional.0.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn FnOnce() + Send + Sync>
            })),
            after_fork: Mutex::new(Arc::new(args2.optional.1.map(|p| {
                let opaque_proc = Opaque::from(p);
                Box::new(move || {
                    opaque_proc
                        .get_inner_with(&Ruby::get().unwrap())
                        .call::<_, Value>(())
                        .unwrap();
                }) as Box<dyn Fn() + Send + Sync>
            }))),
            scheduler_class: args2.optional.2.clone(),
            worker_memory_limit: args2.optional.3,
            silence: args2.optional.4.is_some_and(|s| s),
            strategy: RwLock::new(None),
        };

        if !config.silence {
            if let Some(scheduler_class) = args2.optional.2 {
                info!(scheduler_class, fiber_scheduler = true);
            } else {
                info!(fiber_scheduler = false);
            }
        }

        Ok(Server {
            config: Arc::new(config),
        })
    }

    #[instrument(name = "Bind", skip_all, fields(binds=format!("{:?}", self.config.binds.lock())))]
    pub(crate) fn build_listeners(&self) -> Result<Vec<Listener>> {
        let listeners = self
            .config
            .binds
            .lock()
            .iter()
            .cloned()
            .map(Listener::try_from)
            .collect::<std::result::Result<Vec<Listener>, _>>()?
            .into_iter()
            .collect::<Vec<_>>();
        info!("Bound {:?} listeners", listeners.len());
        Ok(listeners)
    }

    pub(crate) fn build_strategy(self) -> Result<()> {
        let listeners = self.build_listeners()?;
        let server = Arc::new(self);
        let server_clone = server.clone();

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

        *server_clone.strategy.write() = Some(strategy);
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        send_shutdown_event();
        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        if self.silence {
            run_silently(|| self.build_and_run_strategy())
        } else {
            self.build_and_run_strategy()
        }
    }

    fn build_and_run_strategy(&self) -> Result<()> {
        reset_signal_handlers();
        let rself = self.clone();
        call_without_gvl(move || -> Result<()> {
            rself.clone().build_strategy()?;
            if let Err(e) = rself.strategy.read().as_ref().unwrap().run() {
                error!("Error running server: {}", e);
                rself.strategy.read().as_ref().unwrap().stop()?;
            }
            Ok(())
        })?;
        clear_signal_handlers();
        self.strategy.write().take();
        info!("Server stopped");
        Ok(())
    }
}
