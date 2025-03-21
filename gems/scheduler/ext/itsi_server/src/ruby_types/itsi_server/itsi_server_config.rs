use crate::{
    ruby_types::ITSI_SERVER_CONFIG,
    server::{bind::Bind, listener::Listener, middleware_stack::MiddlewareSet},
};
use derive_more::Debug;
use itsi_rb_helpers::{call_with_gvl, print_rb_backtrace, HeapVal, HeapValue};
use magnus::{
    block::Proc,
    error::Result,
    value::{LazyId, ReprValue},
    RArray, RHash, Ruby, Symbol, Value,
};
use nix::{
    fcntl::{fcntl, FcntlArg, FdFlag},
    unistd::dup,
};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    os::fd::RawFd,
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use tracing::info;

static DEFAULT_BIND: &str = "http://localhost:3000";
static ID_BUILD_CONFIG: LazyId = LazyId::new("build_config");
static ID_RELOAD_EXEC: LazyId = LazyId::new("reload_exec");

#[derive(Debug, Clone)]
pub struct ItsiServerConfig {
    pub cli_params: Arc<HeapValue<RHash>>,
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
    pub default_app_loader: HeapValue<Proc>,
    pub middleware: OnceLock<MiddlewareSet>,
    pub binds: Vec<Bind>,
    #[debug(skip)]
    pub(crate) listeners: Mutex<Vec<Listener>>,
    listener_info: Mutex<HashMap<String, i32>>,
}

impl ServerParams {
    pub fn preload_ruby(self: &Arc<Self>) -> Result<()> {
        call_with_gvl(|_| -> Result<()> {
            let default_app: HeapVal = self.default_app_loader.call::<_, Value>(())?.into();
            let middleware = MiddlewareSet::new(
                self.middleware_loader
                    .call::<_, Option<Value>>(())
                    .inspect_err(|e| {
                        if let Some(err_value) = e.value() {
                            print_rb_backtrace(err_value);
                        }
                    })?
                    .map(|mw| mw.into()),
                default_app,
            )?;
            self.middleware.set(middleware).map_err(|_| {
                magnus::Error::new(
                    magnus::exception::runtime_error(),
                    "Failed to set middleware",
                )
            })?;
            Ok(())
        })?;
        Ok(())
    }

    fn from_rb_hash(rb_param_hash: RHash) -> Result<ServerParams> {
        let workers = rb_param_hash
            .fetch::<_, Option<u8>>("workers")?
            .unwrap_or(num_cpus::get() as u8);
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
        let default_app_loader: Proc = rb_param_hash.fetch("default_app_loader")?;

        let binds: Option<Vec<String>> = rb_param_hash.fetch("binds")?;
        let binds = binds
            .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
            .into_iter()
            .map(|s| s.parse())
            .collect::<itsi_error::Result<Vec<Bind>>>()?;

        let listeners = if let Some(preexisting_listeners) =
            rb_param_hash.delete::<_, Option<String>>("listeners")?
        {
            let bind_to_fd_map: HashMap<String, i32> = serde_json::from_str(&preexisting_listeners)
                .map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Invalid listener info: {}", e),
                    )
                })?;
            info!("Received listener info: {:?}", bind_to_fd_map);
            binds
                .iter()
                .cloned()
                .map(|bind| {
                    if let Some(fd) = bind_to_fd_map.get(&bind.listener_address_string()) {
                        Listener::inherit_fd(bind, *fd)
                    } else {
                        Listener::try_from(bind)
                    }
                })
                .collect::<std::result::Result<Vec<Listener>, _>>()?
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            binds
                .iter()
                .cloned()
                .map(Listener::try_from)
                .collect::<std::result::Result<Vec<Listener>, _>>()?
                .into_iter()
                .collect::<Vec<_>>()
        };

        let listener_info = listeners
            .iter()
            .map(|listener| {
                listener.handover().map_err(|e| {
                    magnus::Error::new(magnus::exception::runtime_error(), e.to_string())
                })
            })
            .collect::<Result<HashMap<String, i32>>>()?;

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
            listener_info: Mutex::new(listener_info),
            listeners: Mutex::new(listeners),
            middleware_loader: middleware_loader.into(),
            default_app_loader: default_app_loader.into(),
            middleware: OnceLock::new(),
        })
    }
}

impl ItsiServerConfig {
    pub fn new(ruby: &Ruby, cli_params: RHash, itsifile_path: Option<PathBuf>) -> Result<Self> {
        let server_params = Self::combine_params(ruby, cli_params, itsifile_path.as_ref())?;
        cli_params.delete::<_, Value>(Symbol::new("listeners"))?;

        Ok(ItsiServerConfig {
            cli_params: Arc::new(cli_params.into()),
            server_params: Arc::new(RwLock::new(server_params.clone())),
            itsifile_path,
        })
    }

    /// Reload
    pub fn reload(self: Arc<Self>, cluster_worker: bool) -> Result<bool> {
        let ruby = Ruby::get().unwrap();
        let server_params = call_with_gvl(|_| {
            Self::combine_params(&ruby, self.cli_params.cloned(), self.itsifile_path.as_ref())
        })?;

        let is_single_mode = self.server_params.read().workers == 1;

        let requires_exec = if !is_single_mode && !server_params.preload {
            // In cluster mode children are cycled during a reload
            // and if preload is disabled, will get a clean memory slate,
            // so we don't need to exec.
            false
        } else {
            // In non-cluster mode, or when preloading is enabled, we shouldn't try to
            // reload inside the existing process (as new code may conflict with old),
            // and should re-exec instead.
            true
        };

        *self.server_params.write() = server_params.clone();
        Ok(requires_exec && (cluster_worker || is_single_mode))
    }

    fn combine_params(
        ruby: &Ruby,
        cli_params: RHash,
        itsifile_path: Option<&PathBuf>,
    ) -> Result<Arc<ServerParams>> {
        let rb_param_hash: RHash = ruby
            .get_inner_ref(&ITSI_SERVER_CONFIG)
            .funcall(*ID_BUILD_CONFIG, (cli_params, itsifile_path.cloned()))?;
        Ok(Arc::new(ServerParams::from_rb_hash(rb_param_hash)?))
    }

    fn clear_cloexec(fd: RawFd) -> nix::Result<()> {
        let current_flags = fcntl(fd, FcntlArg::F_GETFD)?;
        let mut flags = FdFlag::from_bits_truncate(current_flags);
        // Remove the FD_CLOEXEC flag
        flags.remove(FdFlag::FD_CLOEXEC);
        // Set the new flags back on the file descriptor
        fcntl(fd, FcntlArg::F_SETFD(flags))?;
        Ok(())
    }

    pub fn dup_fds(self: &Arc<Self>) -> Result<()> {
        let binding = self.server_params.read();
        let mut listener_info_guard = binding.listener_info.lock();
        let dupped_fd_map = listener_info_guard
            .iter()
            .map(|(str, fd)| {
                let dupped_fd = dup(*fd).map_err(|errno| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Errno {} while trying to dup {}", errno, fd),
                    )
                })?;
                info!("Mapped fd {} to {}", fd, dupped_fd);
                Self::clear_cloexec(dupped_fd).map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Failed to clear cloexec flag for fd {}: {}", dupped_fd, e),
                    )
                })?;
                Ok((str.clone(), dupped_fd))
            })
            .collect::<Result<HashMap<String, i32>>>()?;
        *listener_info_guard = dupped_fd_map;
        Ok(())
    }

    pub fn reload_exec(self: &Arc<Self>) -> Result<()> {
        let listener_json =
            serde_json::to_string(&self.server_params.read().listener_info.lock().clone())
                .map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Invalid listener info: {}", e),
                    )
                })?;
        call_with_gvl(|ruby| -> Result<()> {
            ruby.get_inner_ref(&ITSI_SERVER_CONFIG)
                .funcall::<_, _, Value>(*ID_RELOAD_EXEC, (listener_json,))?;
            Ok(())
        })?;
        Ok(())
    }
}
