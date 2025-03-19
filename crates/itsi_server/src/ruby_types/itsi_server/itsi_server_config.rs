use crate::{
    ruby_types::ITSI_CONFIG,
    server::{
        bind::Bind,
        filter_stack::FilterStack,
        listener::Listener,
        serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode, ServeStrategy},
        signal::SIGNAL_HANDLER_CHANNEL,
    },
};
use itsi_rb_helpers::{HeapVal, HeapValue};
use magnus::{
    block::Proc,
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs, ScanArgsKw, ScanArgsOpt, ScanArgsRequired},
    value::{LazyId, ReprValue},
    ArgList, RArray, RHash, Ruby, Symbol, Value,
};
use parking_lot::{Mutex, RwLock};
use std::{cmp::max, collections::HashMap, sync::Arc};
use tracing::{info, instrument};

static DEFAULT_BIND: &str = "http://localhost:3000";
static ID_LOAD: LazyId = LazyId::new("load");
use derive_more::Debug;

use super::ItsiServer;

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

#[derive(Debug)]
pub struct ItsiServerConfig {
    #[allow(unused)]
    pub workers: u8,
    #[allow(unused)]
    pub threads: u8,
    #[allow(unused)]
    pub shutdown_timeout: f64,
    pub script_name: String,
    pub(crate) binds: Mutex<Vec<Bind>>,
    #[debug(skip)]
    pub hooks: HashMap<String, HeapValue<Proc>>,
    pub scheduler_class: Option<String>,
    pub stream_body: Option<bool>,
    pub worker_memory_limit: Option<u64>,
    #[debug(skip)]
    pub(crate) strategy: RwLock<Option<ServeStrategy>>,
    pub silence: bool,
    pub oob_gc_responses_threshold: Option<u64>,
    pub filter_stack: FilterStack,
    pub config_file_path: Option<String>,
    pub(crate) cli_args: RubyServerArgs,
}

#[derive(Debug, Clone)]
pub(crate) struct RubyServerArgs {
    pub app: HeapVal,
    pub workers: Option<u8>,
    pub threads: Option<u8>,
    pub shutdown_timeout: Option<f64>,
    pub script_name: Option<String>,
    pub binds: Option<Vec<String>>,
    pub stream_body: Option<bool>,
    pub hooks: Option<HeapValue<RHash>>,
    pub scheduler_class: Option<String>,
    pub worker_memory_limit: Option<u64>,
    pub oob_gc_responses_threshold: Option<u64>,
    pub silence: Option<bool>,
    pub routes: Option<HeapVal>,
    pub config_file_path: Option<String>,
}

impl ItsiServerConfig {
    pub fn new(args: &[Value]) -> Result<Self> {
        let server_args =
            Self::rb_args_from_hash(scan_args::<(), (), (), (), RHash, ()>(args)?.keywords)?;
        let cli_args = server_args.clone();
        let hooks = server_args
            .hooks
            .map(|rhash| -> Result<HashMap<String, HeapValue<Proc>>> {
                let mut hook_map: HashMap<String, HeapValue<Proc>> = HashMap::new();
                for pair in rhash.enumeratorize::<_, ()>("each", ()) {
                    if let Some(pair_value) = RArray::from_value(pair?) {
                        if let (Ok(key), Ok(value)) =
                            (pair_value.entry::<Value>(0), pair_value.entry::<Proc>(1))
                        {
                            hook_map.insert(key.to_string(), HeapValue::from(value));
                        }
                    }
                }
                Ok(hook_map)
            })
            .transpose()?
            .unwrap_or_default();

        Ok(ItsiServerConfig {
            workers: max(server_args.workers.unwrap_or(1), 1),
            threads: max(server_args.threads.unwrap_or(1), 1),
            shutdown_timeout: server_args.shutdown_timeout.unwrap_or(5.0),
            script_name: server_args.script_name.unwrap_or("".to_string()),
            binds: Mutex::new(
                server_args
                    .binds
                    .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
                    .into_iter()
                    .map(|s| s.parse())
                    .collect::<itsi_error::Result<Vec<Bind>>>()?,
            ),
            stream_body: server_args.stream_body,
            hooks,
            scheduler_class: server_args.scheduler_class,
            worker_memory_limit: server_args.worker_memory_limit,
            strategy: RwLock::new(None),
            oob_gc_responses_threshold: server_args.oob_gc_responses_threshold,
            silence: server_args.silence.is_some_and(|s| s),
            filter_stack: FilterStack::new(server_args.routes, server_args.app)?,
            config_file_path: server_args.config_file_path,
            cli_args,
        })
    }

    /// Here we reload application configuration from the Itsi.rb config file
    /// and merge these with the arguments given on the command line (these always take precedence).
    /// This allows us to reload the entire Itsi configuration while the process is running
    pub fn load_itsi_file(self: &Arc<Self>) -> Result<Self> {
        let itsi_file_hash = Ruby::get()
            .unwrap()
            .get_inner(&ITSI_CONFIG)
            .funcall::<_, _, RHash>(*ID_LOAD, (self.config_file_path.clone(),))?;
        let server_args = Self::rb_args_from_hash(itsi_file_hash);
        Ok(Self::merge_build(self.cli_args.clone(), server_args))
    }

    /// This is where we trigger code loading of all Ruby dependencies.
    /// This is only meaningful if preload is disabled which is a requirement for hot-reloading.
    /// If preloading is enabled, a restart is required to apply changes.
    pub fn reload_dependencies(self: Arc<Self>) -> Result<Arc<Self>> {
        todo!();
        Ok(self)
    }

    #[instrument(name = "Bind", skip_all, fields(binds=format!("{:?}", self.binds.lock())))]
    pub(crate) fn build_listeners(&self) -> Result<Vec<Listener>> {
        let listeners = self
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

    pub(crate) fn build_strategy(self: Arc<Self>, server: &ItsiServer) -> Result<()> {
        let listeners = self.build_listeners()?;
        let server_config_clone = self.clone();

        let strategy = if server_config_clone.workers == 1 {
            ServeStrategy::Single(Arc::new(SingleMode::new(
                self,
                listeners,
                SIGNAL_HANDLER_CHANNEL.0.clone(),
            )?))
        } else {
            ServeStrategy::Cluster(Arc::new(ClusterMode::new(
                server.clone().into(),
                listeners,
                SIGNAL_HANDLER_CHANNEL.0.clone(),
            )))
        };

        *server_config_clone.strategy.write() = Some(strategy);
        Ok(())
    }

    pub(crate) fn rb_args_from_hash(keywords: RHash) -> Result<RubyServerArgs> {
        let args = Args {
            keywords,
            required: (),
            optional: (),
            splat: (),
            trailing: (),
            block: (),
        };
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
                // Hooks
                Option<RHash>,
                // Scheduler Class
                Option<String>,
                // Worker Memory Limit
                Option<u64>,
                // Out-of-band GC Responses Threshold
                Option<u64>,
                // Silence
                Option<bool>,
                // Filters
                Option<Value>,
                // Config File Path
                Option<String>,
            ),
            (),
        >;

        let args1: Args1 = extract_args(
            &args,
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
            &args,
            &[],
            &[
                "hooks",
                "scheduler_class",
                "worker_memory_limit",
                "oob_gc_responses_threshold",
                "silence",
                "routes",
                "config_file_path",
            ],
        )?;

        Ok(RubyServerArgs {
            app: args1.required.0.into(),
            workers: args1.optional.0,
            threads: args1.optional.1,
            shutdown_timeout: args1.optional.2,
            script_name: args1.optional.3,
            binds: args1.optional.4,
            stream_body: args1.optional.5,
            hooks: args2.optional.0.map(|hv| hv.into()),
            scheduler_class: args2.optional.1,
            worker_memory_limit: args2.optional.2,
            oob_gc_responses_threshold: args2.optional.3,
            silence: args2.optional.4,
            routes: args2.optional.5.map(|hv| hv.into()),
            config_file_path: args2.optional.6,
        })
    }

    fn merge_build(
        cli_args: RubyServerArgs,
        server_args: std::result::Result<RubyServerArgs, magnus::Error>,
    ) -> ItsiServerConfig {
        todo!()
    }
}
