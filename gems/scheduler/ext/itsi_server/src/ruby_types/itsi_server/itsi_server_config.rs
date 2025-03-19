use crate::server::{
    bind::Bind,
    filter_stack::FilterStack,
    listener::Listener,
    serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode, ServeStrategy},
    signal::SIGNAL_HANDLER_CHANNEL,
};
use itsi_rb_helpers::{HeapVal, HeapValue};
use magnus::{
    block::Proc,
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs, ScanArgsKw, ScanArgsOpt, ScanArgsRequired},
    value::ReprValue,
    ArgList, RArray, RHash, Ruby, Symbol, Value,
};
use parking_lot::{Mutex, RwLock};
use std::{cmp::max, collections::HashMap, sync::Arc};
use tracing::{info, instrument};

static DEFAULT_BIND: &str = "http://localhost:3000";

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
}

impl ItsiServerConfig {
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
                "hooks",
                "scheduler_class",
                "worker_memory_limit",
                "oob_gc_responses_threshold",
                "silence",
                "routes",
            ],
        )?;

        let hooks = args2
            .optional
            .0
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
            hooks,
            scheduler_class: args2.optional.1.clone(),
            worker_memory_limit: args2.optional.2,
            strategy: RwLock::new(None),
            oob_gc_responses_threshold: args2.optional.3,
            silence: args2.optional.4.is_some_and(|s| s),
            filter_stack: FilterStack::new(args2.optional.5, args1.required.0)?,
        })
    }

    pub fn load(self: Arc<Self>) -> Result<Arc<Self>> {
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
}
