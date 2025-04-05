use super::file_watcher::{self};
use crate::{
    ruby_types::ITSI_SERVER_CONFIG,
    server::{
        binds::{bind::Bind, listener::Listener},
        middleware_stack::MiddlewareSet,
    },
};
use derive_more::Debug;
use itsi_rb_helpers::{call_with_gvl, print_rb_backtrace, HeapValue};
use itsi_tracing::{set_format, set_level, set_target};
use magnus::{
    block::Proc,
    error::Result,
    value::{LazyId, ReprValue},
    RArray, RHash, Ruby, Symbol, Value,
};
use nix::{
    fcntl::{fcntl, FcntlArg, FdFlag},
    unistd::{close, dup},
};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    os::fd::{AsRawFd, OwnedFd, RawFd},
    path::PathBuf,
    sync::{Arc, OnceLock},
};
static DEFAULT_BIND: &str = "http://localhost:3000";
static ID_BUILD_CONFIG: LazyId = LazyId::new("build_config");
static ID_RELOAD_EXEC: LazyId = LazyId::new("reload_exec");

#[derive(Debug, Clone)]
pub struct ItsiServerConfig {
    pub cli_params: Arc<HeapValue<RHash>>,
    pub itsifile_path: Option<PathBuf>,
    pub itsi_config_proc: Arc<Option<HeapValue<Proc>>>,
    #[debug(skip)]
    pub server_params: Arc<RwLock<Arc<ServerParams>>>,
    pub watcher_fd: Arc<Option<OwnedFd>>,
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

    pub notify_watchers: Option<Vec<(String, Vec<Vec<String>>)>>,
    /// Worker params
    pub threads: u8,
    pub script_name: String,
    pub streamable_body: bool,
    pub multithreaded_reactor: bool,
    pub scheduler_class: Option<String>,
    pub oob_gc_responses_threshold: Option<u64>,
    pub middleware_loader: HeapValue<Proc>,
    pub middleware: OnceLock<MiddlewareSet>,
    pub binds: Vec<Bind>,
    #[debug(skip)]
    pub(crate) listeners: Mutex<Vec<Listener>>,
    listener_info: Mutex<HashMap<String, i32>>,
}

impl ServerParams {
    pub fn preload_ruby(self: &Arc<Self>) -> Result<()> {
        call_with_gvl(|ruby| -> Result<()> {
            if self
                .scheduler_class
                .as_ref()
                .is_some_and(|t| t == "Itsi::Scheduler")
            {
                ruby.require("itsi/scheduler")?;
            }
            let middleware = MiddlewareSet::new(
                self.middleware_loader
                    .call::<_, Option<Value>>(())
                    .inspect_err(|e| {
                        if let Some(err_value) = e.value() {
                            print_rb_backtrace(err_value);
                        }
                    })?
                    .map(|mw| mw.into()),
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
        let multithreaded_reactor: bool = rb_param_hash
            .fetch::<_, Option<bool>>("multithreaded_reactor")?
            .unwrap_or(workers == 1);
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
        let notify_watchers: Option<Vec<(String, Vec<Vec<String>>)>> =
            rb_param_hash.fetch("notify_watchers")?;
        let threads: u8 = rb_param_hash.fetch("threads")?;
        let script_name: String = rb_param_hash.fetch("script_name")?;
        let streamable_body: bool = rb_param_hash.fetch("streamable_body")?;
        let scheduler_class: Option<String> = rb_param_hash.fetch("scheduler_class")?;
        let oob_gc_responses_threshold: Option<u64> =
            rb_param_hash.fetch("oob_gc_responses_threshold")?;
        let middleware_loader: Proc = rb_param_hash.fetch("middleware_loader")?;
        let log_level: Option<String> = rb_param_hash.fetch("log_level")?;
        let log_target: Option<String> = rb_param_hash.fetch("log_target")?;
        let log_format: Option<String> = rb_param_hash.fetch("log_format")?;

        if let Some(level) = log_level {
            set_level(&level);
        }

        if let Some(target) = log_target {
            set_target(&target);
        }

        if let Some(format) = log_format {
            set_format(&format);
        }

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
                        magnus::exception::standard_error(),
                        format!("Invalid listener info: {}", e),
                    )
                })?;

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
            multithreaded_reactor,
            shutdown_timeout,
            hooks,
            preload,
            notify_watchers,
            threads,
            script_name,
            streamable_body,
            scheduler_class,
            oob_gc_responses_threshold,
            binds,
            listener_info: Mutex::new(listener_info),
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
        itsi_config_proc: Option<Proc>,
    ) -> Result<Self> {
        let itsi_config_proc = Arc::new(itsi_config_proc.map(HeapValue::from));
        let server_params = Self::combine_params(
            ruby,
            cli_params,
            itsifile_path.as_ref(),
            itsi_config_proc.clone(),
        )?;
        cli_params.delete::<_, Value>(Symbol::new("listeners"))?;

        let watcher_fd = if let Some(watchers) = server_params.notify_watchers.clone() {
            file_watcher::watch_groups(watchers)?
        } else {
            None
        };

        Ok(ItsiServerConfig {
            cli_params: Arc::new(cli_params.into()),
            server_params: RwLock::new(server_params.clone()).into(),
            itsi_config_proc,
            itsifile_path,
            watcher_fd: watcher_fd.into(),
        })
    }

    /// Reload
    pub fn reload(self: Arc<Self>, cluster_worker: bool) -> Result<bool> {
        let server_params = call_with_gvl(|ruby| {
            Self::combine_params(
                &ruby,
                self.cli_params.cloned(),
                self.itsifile_path.as_ref(),
                self.itsi_config_proc.clone(),
            )
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
        itsi_config_proc: Arc<Option<HeapValue<Proc>>>,
    ) -> Result<Arc<ServerParams>> {
        let inner = itsi_config_proc
            .as_ref()
            .clone()
            .map(|hv| hv.clone().inner());
        let rb_param_hash: RHash = ruby.get_inner_ref(&ITSI_SERVER_CONFIG).funcall(
            *ID_BUILD_CONFIG,
            (cli_params, itsifile_path.cloned(), inner),
        )?;
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
                        magnus::exception::standard_error(),
                        format!("Errno {} while trying to dup {}", errno, fd),
                    )
                })?;
                Self::clear_cloexec(dupped_fd).map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::standard_error(),
                        format!("Failed to clear cloexec flag for fd {}: {}", dupped_fd, e),
                    )
                })?;
                Ok((str.clone(), dupped_fd))
            })
            .collect::<Result<HashMap<String, i32>>>()?;
        *listener_info_guard = dupped_fd_map;
        Ok(())
    }

    pub fn stop_watcher(self: &Arc<Self>) -> Result<()> {
        if let Some(r_fd) = self.watcher_fd.as_ref() {
            close(r_fd.as_raw_fd()).ok();
        }
        Ok(())
    }

    pub fn reload_exec(self: &Arc<Self>) -> Result<()> {
        let listener_json =
            serde_json::to_string(&self.server_params.read().listener_info.lock().clone())
                .map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::standard_error(),
                        format!("Invalid listener info: {}", e),
                    )
                })?;

        self.stop_watcher()?;
        call_with_gvl(|ruby| -> Result<()> {
            ruby.get_inner_ref(&ITSI_SERVER_CONFIG)
                .funcall::<_, _, Value>(*ID_RELOAD_EXEC, (listener_json,))?;
            Ok(())
        })?;
        Ok(())
    }
}
