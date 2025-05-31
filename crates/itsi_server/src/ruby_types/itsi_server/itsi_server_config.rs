use super::file_watcher::{self};
use crate::{
    ruby_types::ITSI_SERVER_CONFIG,
    server::{
        binds::{bind::Bind, listener::Listener},
        middleware_stack::MiddlewareSet,
    },
};
use derive_more::Debug;
use itsi_error::ItsiError;
use itsi_rb_helpers::{call_with_gvl, print_rb_backtrace, HeapValue};
use itsi_tracing::{set_format, set_level, set_target, set_target_filters};
use magnus::{
    block::Proc,
    error::Result,
    value::{LazyId, ReprValue},
    RArray, RHash, Ruby, Symbol, TryConvert, Value,
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
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc, OnceLock,
    },
    time::Duration,
};
use tracing::{debug, error};
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

    pub request_timeout: Option<Duration>,
    pub header_read_timeout: Duration,
    pub notify_watchers: Option<Vec<(String, Vec<Vec<String>>)>>,

    /// Worker params
    pub threads: u8,
    pub scheduler_threads: Option<u8>,
    pub streamable_body: bool,
    pub multithreaded_reactor: bool,
    pub pin_worker_cores: bool,
    pub scheduler_class: Option<String>,
    pub oob_gc_responses_threshold: Option<u64>,
    pub ruby_thread_request_backlog_size: Option<usize>,
    pub middleware_loader: HeapValue<Proc>,
    pub middleware: OnceLock<MiddlewareSet>,
    pub pipeline_flush: bool,
    pub writev: Option<bool>,
    pub max_concurrent_streams: Option<u32>,
    pub max_local_error_reset_streams: Option<usize>,
    pub max_header_list_size: u32,
    pub max_send_buf_size: usize,
    pub binds: Vec<Bind>,
    #[debug(skip)]
    pub(crate) listeners: Mutex<Vec<Listener>>,
    listener_info: Mutex<HashMap<String, i32>>,
    pub itsi_server_token_preference: ItsiServerTokenPreference,
    pub preloaded: AtomicBool,
    socket_opts: SocketOpts,
    preexisting_listeners: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum ItsiServerTokenPreference {
    Version,
    Name,
    None,
}

impl FromStr for ItsiServerTokenPreference {
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "version" => ItsiServerTokenPreference::Version,
            "name" => ItsiServerTokenPreference::Name,
            "none" => ItsiServerTokenPreference::None,
            _ => ItsiServerTokenPreference::Version,
        })
    }

    type Err = ItsiError;
}

#[derive(Debug, Clone)]
pub struct SocketOpts {
    pub reuse_address: bool,
    pub reuse_port: bool,
    pub listen_backlog: usize,
    pub nodelay: bool,
    pub recv_buffer_size: usize,
    pub send_buffer_size: usize,
}

impl ServerParams {
    pub fn preload_ruby(self: &Arc<Self>) -> Result<()> {
        if self.preloaded.load(Relaxed) {
            return Ok(());
        }
        call_with_gvl(|ruby| -> Result<()> {
            if self
                .scheduler_class
                .as_ref()
                .is_some_and(|t| t == "Itsi::Scheduler")
            {
                debug!("Loading Itsi Scheduler");
                ruby.require("itsi/scheduler")?;
            }
            let result_pair = self
                .middleware_loader
                .call::<(), RArray>(())
                .inspect_err(|e| {
                    eprintln!("Error loading middleware: {:?}", e);
                    if let Some(err_value) = e.value() {
                        print_rb_backtrace(err_value);
                    }
                })?;
            let routes_raw = result_pair
                .entry::<Option<Value>>(0)
                .inspect_err(|e| {
                    eprintln!("Error loading middleware: {:?}", e);
                    if let Some(err_value) = e.value() {
                        print_rb_backtrace(err_value);
                    }
                })?
                .map(|mw| mw.into());
            let error_lines = result_pair.entry::<Option<RArray>>(1).inspect_err(|e| {
                eprintln!("Error loading middleware: {:?}", e);
                if let Some(err_value) = e.value() {
                    print_rb_backtrace(err_value);
                }
            })?;
            if error_lines.is_some_and(|r| !r.is_empty()) {
                let errors: Vec<String> =
                    Vec::<String>::try_convert(error_lines.unwrap().as_value())?;
                ItsiServerConfig::print_config_errors(errors);
                return Err(magnus::Error::new(
                    magnus::exception::runtime_error(),
                    "Failed to set middleware",
                ));
            }
            let middleware = MiddlewareSet::new(routes_raw)?;
            self.middleware.set(middleware).map_err(|_| {
                magnus::Error::new(
                    magnus::exception::runtime_error(),
                    "Failed to set middleware",
                )
            })?;
            Ok(())
        })?;
        self.preloaded.store(true, Relaxed);
        Ok(())
    }

    pub async fn initialize_middleware(self: &Arc<Self>) -> Result<()> {
        self.middleware.get().unwrap().initialize_layers().await?;
        Ok(())
    }

    fn from_rb_hash(rb_param_hash: RHash) -> Result<ServerParams> {
        let num_cpus = num_cpus::get_physical() as u8;
        let workers = rb_param_hash
            .fetch::<_, Option<u8>>("workers")?
            .unwrap_or(num_cpus);
        let worker_memory_limit: Option<u64> = rb_param_hash.fetch("worker_memory_limit")?;
        let silence: bool = rb_param_hash.fetch("silence")?;
        let multithreaded_reactor: bool = rb_param_hash
            .fetch::<_, Option<bool>>("multithreaded_reactor")?
            .unwrap_or(workers <= (num_cpus / 3));
        let pin_worker_cores: bool = rb_param_hash
            .fetch::<_, Option<bool>>("pin_worker_cores")?
            .unwrap_or(false);
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
        let request_timeout: Option<f64> = rb_param_hash.fetch("request_timeout")?;
        let request_timeout = request_timeout.map(Duration::from_secs_f64);
        let header_read_timeout: Duration = rb_param_hash
            .fetch::<_, Option<f64>>("header_read_timeout")?
            .map(Duration::from_secs_f64)
            .unwrap_or(Duration::from_secs(1));

        let notify_watchers: Option<Vec<(String, Vec<Vec<String>>)>> =
            rb_param_hash.fetch("notify_watchers")?;
        let threads: u8 = rb_param_hash.fetch("threads")?;
        let scheduler_threads: Option<u8> = rb_param_hash.fetch("scheduler_threads")?;
        let streamable_body: bool = rb_param_hash.fetch("streamable_body")?;
        let scheduler_class: Option<String> = rb_param_hash.fetch("scheduler_class")?;
        let oob_gc_responses_threshold: Option<u64> =
            rb_param_hash.fetch("oob_gc_responses_threshold")?;

        let ruby_thread_request_backlog_size: Option<usize> =
            rb_param_hash.fetch("ruby_thread_request_backlog_size")?;

        let middleware_loader: Proc = rb_param_hash.fetch("middleware_loader")?;
        let log_level: Option<String> = rb_param_hash.fetch("log_level")?;
        let log_target: Option<String> = rb_param_hash.fetch("log_target")?;
        let log_format: Option<String> = rb_param_hash.fetch("log_format")?;
        let log_target_filters: Option<Vec<String>> = rb_param_hash.fetch("log_target_filters")?;

        let reuse_address: bool = rb_param_hash
            .fetch::<_, Option<bool>>("reuse_address")?
            .unwrap_or(true);
        let reuse_port: bool = rb_param_hash
            .fetch::<_, Option<bool>>("reuse_port")?
            .unwrap_or(true);
        let listen_backlog: usize = rb_param_hash
            .fetch::<_, Option<usize>>("listen_backlog")?
            .unwrap_or(1024);
        let nodelay: bool = rb_param_hash
            .fetch::<_, Option<bool>>("nodelay")?
            .unwrap_or(true);
        let recv_buffer_size: usize = rb_param_hash
            .fetch::<_, Option<usize>>("recv_buffer_size")?
            .unwrap_or(262_144);
        let send_buffer_size: usize = rb_param_hash
            .fetch::<_, Option<usize>>("send_buffer_size")?
            .unwrap_or(262_144);

        if let Some(level) = log_level {
            set_level(&level);
        }

        if let Some(target) = log_target {
            set_target(&target);
        }

        if let Some(format) = log_format {
            set_format(&format);
        }

        if let Some(target_filters) = log_target_filters {
            let target_filters = target_filters
                .iter()
                .filter_map(|filter| {
                    let mut parts = filter.splitn(2, '=');
                    if let (Some(target), Some(level_str)) = (parts.next(), parts.next()) {
                        if let Ok(level) = level_str.parse::<tracing::Level>() {
                            return Some((target, level));
                        }
                    }
                    None
                })
                .collect::<Vec<(&str, tracing::Level)>>();
            set_target_filters(target_filters);
        }

        let pipeline_flush: bool = rb_param_hash.fetch("pipeline_flush")?;
        let writev: Option<bool> = rb_param_hash.fetch("writev")?;
        let max_concurrent_streams: Option<u32> = rb_param_hash.fetch("max_concurrent_streams")?;
        let max_local_error_reset_streams: Option<usize> =
            rb_param_hash.fetch("max_local_error_reset_streams")?;
        let max_header_list_size: u32 = rb_param_hash.fetch("max_header_list_size")?;
        let max_send_buf_size: usize = rb_param_hash.fetch("max_send_buf_size")?;

        let binds: Option<Vec<String>> = rb_param_hash.fetch("binds")?;
        let binds = binds
            .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
            .into_iter()
            .map(|s| s.parse())
            .collect::<itsi_error::Result<Vec<Bind>>>()?;

        let itsi_server_token_preference: String = rb_param_hash
            .fetch("itsi_server_token_preference")
            .unwrap_or_default();
        let itsi_server_token_preference: ItsiServerTokenPreference =
            itsi_server_token_preference.parse()?;

        let socket_opts = SocketOpts {
            reuse_address,
            reuse_port,
            listen_backlog,
            nodelay,
            recv_buffer_size,
            send_buffer_size,
        };
        let preexisting_listeners = rb_param_hash.delete::<_, Option<String>>("listeners")?;

        let params = ServerParams {
            workers,
            worker_memory_limit,
            silence,
            multithreaded_reactor,
            pin_worker_cores,
            shutdown_timeout,
            hooks,
            preload,
            request_timeout,
            header_read_timeout,
            notify_watchers,
            threads,
            scheduler_threads,
            streamable_body,
            scheduler_class,
            ruby_thread_request_backlog_size,
            oob_gc_responses_threshold,
            pipeline_flush,
            writev,
            max_concurrent_streams,
            max_local_error_reset_streams,
            max_header_list_size,
            max_send_buf_size,
            binds,
            itsi_server_token_preference,
            socket_opts,
            preexisting_listeners,
            listener_info: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
            middleware_loader: middleware_loader.into(),
            middleware: OnceLock::new(),
            preloaded: AtomicBool::new(false),
        };

        Ok(params)
    }

    pub fn setup_listeners(&self) -> Result<()> {
        let listeners = if let Some(preexisting_listeners) = self.preexisting_listeners.as_ref() {
            let bind_to_fd_map: HashMap<String, i32> = serde_json::from_str(preexisting_listeners)
                .map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::standard_error(),
                        format!("Invalid listener info: {}", e),
                    )
                })?;

            self.binds
                .iter()
                .cloned()
                .map(|bind| {
                    if let Some(fd) = bind_to_fd_map.get(&bind.listener_address_string()) {
                        Listener::inherit_fd(bind, *fd, &self.socket_opts)
                    } else {
                        Listener::build(bind, &self.socket_opts)
                    }
                })
                .collect::<std::result::Result<Vec<Listener>, _>>()?
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            self.binds
                .iter()
                .cloned()
                .map(|b| Listener::build(b, &self.socket_opts))
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

        *self.listener_info.lock() = listener_info;
        *self.listeners.lock() = listeners;
        Ok(())
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
        match Self::combine_params(
            ruby,
            cli_params,
            itsifile_path.as_ref(),
            itsi_config_proc.clone(),
        ) {
            Ok(server_params) => {
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
            Err(err) => Err(magnus::Error::new(
                magnus::exception::standard_error(),
                format!("Error loading initial configuration {:?}", err),
            )),
        }
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
            // so we don't need to exec. We do need to rebind our listeners here.
            server_params.setup_listeners()?;
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
        let (rb_param_hash, errors): (RHash, Vec<String>) =
            ruby.get_inner_ref(&ITSI_SERVER_CONFIG).funcall(
                *ID_BUILD_CONFIG,
                (cli_params, itsifile_path.cloned(), inner),
            )?;
        if !errors.is_empty() {
            Self::print_config_errors(errors);
            return Err(magnus::Error::new(
                magnus::exception::standard_error(),
                "Invalid server config",
            ));
        }
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

    pub async fn get_config_errors(&self) -> Option<Vec<String>> {
        let rb_param_hash = call_with_gvl(|ruby| {
            let inner = self
                .itsi_config_proc
                .as_ref()
                .clone()
                .map(|hv| hv.clone().inner());
            let cli_params = self.cli_params.cloned();
            let itsifile_path = self.itsifile_path.clone();

            let (rb_param_hash, errors): (RHash, Vec<String>) = ruby
                .get_inner_ref(&ITSI_SERVER_CONFIG)
                .funcall(*ID_BUILD_CONFIG, (cli_params, itsifile_path, inner))
                .unwrap();
            if !errors.is_empty() {
                return Err(errors);
            }
            Ok(rb_param_hash)
        });
        match rb_param_hash {
            Ok(rb_param_hash) => match ServerParams::from_rb_hash(rb_param_hash) {
                Ok(test_params) => {
                    let params_arc = Arc::new(test_params);
                    if let Err(err) = params_arc.clone().preload_ruby() {
                        let err_val = call_with_gvl(|_| format!("{}", err));
                        return Some(vec![err_val]);
                    }

                    if let Err(err) = params_arc
                        .middleware
                        .get()
                        .unwrap()
                        .initialize_layers()
                        .await
                    {
                        let err_val = call_with_gvl(|_| format!("{}", err));
                        return Some(vec![err_val]);
                    }
                    None
                }
                Err(err) => Some(vec![format!("{:?}", err)]),
            },
            Err(err) => Some(err),
        }
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

    pub fn print_config_errors(errors: Vec<String>) {
        error!("Refusing to reload configuration due to fatal errors:");
        for error in errors {
            eprintln!("{}", error);
        }
    }

    pub async fn check_config(&self) -> bool {
        if let Some(errors) = self.get_config_errors().await {
            Self::print_config_errors(errors);
            return false;
        }
        true
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
