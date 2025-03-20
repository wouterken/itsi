use crate::{
    ruby_types::ITSI_SERVER_CONFIG,
    server::{
        bind::Bind,
        filter_stack::FilterStack,
        listener::Listener,
        serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode, ServeStrategy},
    },
};
use derive_more::Debug;
use itsi_rb_helpers::HeapValue;
use magnus::{
    block::Proc,
    error::Result,
    value::{LazyId, ReprValue},
    RArray, RHash, Ruby, Value,
};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

static DEFAULT_BIND: &str = "http://localhost:3000";
static ID_BUILD_CONFIG: LazyId = LazyId::new("build_config");
static ID_RELOAD_EXEC: LazyId = LazyId::new("reload_exec");

#[derive(Debug, Clone)]
pub struct ItsiServerConfig {
    pub cli_params: HeapValue<RHash>,
    pub itsifile_path: Option<PathBuf>,
    #[debug(skip)]
    pub server_params: Arc<RwLock<Arc<ServerParams>>>,
}

#[derive(Debug)]
pub struct ServerParams {
    /// Cluster params
    pub workers: u8,
    pub worker_memory_limit: Option<u64>,
    pub silence: bool,
    pub shutdown_timeout: f64,
    pub hooks: HashMap<String, HeapValue<Proc>>,
    pub preload: bool,

    /// Worker params
    pub threads: u8,
    pub script_name: String,
    pub streamable_body: bool,
    pub scheduler_class: Option<String>,
    pub oob_gc_responses_threshold: Option<u64>,
    pub middleware_loader: HeapValue<Proc>,
    pub middleware: OnceLock<FilterStack>,
    pub binds: Vec<Bind>,
    #[debug(skip)]
    pub(crate) listeners: Mutex<Vec<Listener>>,
    listener_info: HashMap<i32, String>,
}

impl ServerParams {
    pub fn preload_ruby(self: &Arc<Self>) -> Result<()> {
        Ok(())
    }

    fn from_rb_hash(
        rb_param_hash: RHash,
        initial_listener_info: Option<HashMap<u32, String>>,
    ) -> Result<ServerParams> {
        let workers: u8 = rb_param_hash.fetch("workers")?;
        let worker_memory_limit: Option<u64> = rb_param_hash.fetch("worker_memory_limit")?;
        let silence: bool = rb_param_hash.fetch("silence")?;
        let shutdown_timeout: f64 = rb_param_hash.fetch("shutdown_timeout")?;

        let hooks: Option<RHash> = rb_param_hash.fetch("hooks")?;
        let hooks = hooks
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
        let preload: bool = rb_param_hash.fetch("preload")?;
        let threads: u8 = rb_param_hash.fetch("threads")?;
        let script_name: String = rb_param_hash.fetch("script_name")?;
        let streamable_body: bool = rb_param_hash.fetch("streamable_body")?;
        let scheduler_class: Option<String> = rb_param_hash.fetch("scheduler_class")?;
        let oob_gc_responses_threshold: Option<u64> =
            rb_param_hash.fetch("oob_gc_responses_threshold")?;
        let middleware_loader: Proc = rb_param_hash.fetch("middleware_loader")?;

        let binds: Option<Vec<String>> = rb_param_hash.fetch("binds")?;
        let binds = binds
            .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
            .into_iter()
            .map(|s| s.parse())
            .collect::<itsi_error::Result<Vec<Bind>>>()?;

        let listeners = binds
            .iter()
            .cloned()
            .map(Listener::try_from)
            .collect::<std::result::Result<Vec<Listener>, _>>()?
            .into_iter()
            .collect::<Vec<_>>();

        let listener_info = listeners
            .iter()
            .map(|listener| {
                listener.handover().map_err(|e| {
                    magnus::Error::new(magnus::exception::runtime_error(), e.to_string())
                })
            })
            .collect::<Result<HashMap<i32, String>>>()?;

        Ok(ServerParams {
            workers,
            worker_memory_limit,
            silence,
            shutdown_timeout,
            hooks,
            preload,
            threads,
            script_name,
            streamable_body,
            scheduler_class,
            oob_gc_responses_threshold,
            binds,
            listener_info,
            listeners: Mutex::new(listeners),
            middleware_loader: middleware_loader.into(),
            middleware: OnceLock::new(),
        })
    }
}

impl ItsiServerConfig {
    pub fn new(
        ruby: &Ruby,
        cli_params: RHash,
        itsifile_path: Option<PathBuf>,
        reexec_params: Option<String>,
    ) -> Result<Self> {
        let (cli_params, listener_info) = if let Some(reexec_params) = reexec_params {
            Self::parse_reexec_params(ruby, cli_params, reexec_params)?
        } else {
            (cli_params, None)
        };
        let server_params =
            Self::combine_params(ruby, cli_params, itsifile_path.as_ref(), listener_info)?;
        Ok(ItsiServerConfig {
            cli_params: cli_params.into(),
            server_params: Arc::new(RwLock::new(server_params.clone())),
            itsifile_path,
        })
    }

    pub(crate) fn build_strategy(self: Arc<Self>) -> Result<ServeStrategy> {
        if self.server_params.read().workers > 1 {
            Ok(ServeStrategy::Cluster(Arc::new(ClusterMode::new(
                self.clone(),
            ))))
        } else {
            Ok(ServeStrategy::Single(Arc::new(SingleMode::new(
                self.clone(),
            )?)))
        }
    }

    pub fn reload(self: Arc<Self>) -> Result<()> {
        let ruby = Ruby::get().unwrap();
        let server_params = Self::combine_params(
            &ruby,
            self.cli_params.clone().into_inner(),
            self.itsifile_path.as_ref(),
            None,
        )?;

        let requires_exec = if self.server_params.read().preload && !server_params.preload {
            // If we've disabled `preload`, we need to fully clear sever memory using an exec
            true
        } else if (self.server_params.read().workers == 1 && server_params.workers > 1)
            || (self.server_params.read().workers > 1 && server_params.workers == 1)
        {
            // If we've switched from single to cluster mode  `workers`, or vice-versa
            // we need to fully clear sever memory using an exec to cleanly switch strategies
            true
        } else {
            false
        };

        if requires_exec {
            Self::reload_exec(&ruby, self.clone())?;
        }
        *self.server_params.write() = server_params.clone();
        Ok(())
    }

    fn combine_params(
        ruby: &Ruby,
        cli_params: RHash,
        itsifile_path: Option<&PathBuf>,
        initial_listener_info: Option<HashMap<u32, String>>,
    ) -> Result<Arc<ServerParams>> {
        let rb_param_hash: RHash = ruby
            .get_inner_ref(&ITSI_SERVER_CONFIG)
            .funcall(*ID_BUILD_CONFIG, (cli_params, itsifile_path.cloned()))?;

        Ok(Arc::new(ServerParams::from_rb_hash(
            rb_param_hash,
            initial_listener_info,
        )?))
    }

    fn reload_exec(ruby: &Ruby, rb_self: Arc<Self>) -> Result<()> {
        ruby.get_inner_ref(&ITSI_SERVER_CONFIG)
            .funcall::<_, _, Value>(
                *ID_RELOAD_EXEC,
                (
                    rb_self.cli_params.clone(),
                    rb_self.server_params.read().listener_info.clone(),
                ),
            )?;
        Ok(())
    }

    fn parse_reexec_params(
        ruby: &Ruby,
        cli_params: RHash,
        reexec_params: String,
    ) -> Result<(RHash, Option<HashMap<u32, String>>)> {
        let result: (RHash, Option<HashMap<u32, String>>) = ruby
            .get_inner_ref(&ITSI_SERVER_CONFIG)
            .funcall(*ID_RELOAD_EXEC, (cli_params.clone(), reexec_params))?;
        Ok(result)
    }
}
