use crate::{
    ruby_types::ITSI_SERVER_CONFIG,
    server::{bind::Bind, listener::Listener, middleware_stack::MiddlewareSet},
};
use derive_more::Debug;
use globset::{Glob, GlobSet, GlobSetBuilder};
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
use notify::{Config, RecommendedWatcher};
use notify::{Event, RecursiveMode, Watcher};
use parking_lot::{Mutex, RwLock};
use std::path::Path;
use std::{
    collections::HashMap,
    os::fd::RawFd,
    path::PathBuf,
    process::Command,
    sync::{mpsc, Arc, OnceLock},
    thread,
};
use std::{collections::HashSet, fs};
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

    pub notify_watchers: Option<Vec<(String, Vec<Vec<String>>)>>,
    /// Worker params
    pub threads: u8,
    pub script_name: String,
    pub streamable_body: bool,
    pub multithreaded_reactor: bool,
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
        call_with_gvl(|ruby| -> Result<()> {
            if self
                .scheduler_class
                .as_ref()
                .is_some_and(|t| t == "Itsi::Scheduler")
            {
                ruby.require("itsi/scheduler")?;
            }
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
        let multithreaded_reactor: bool = rb_param_hash.fetch("multithreaded_reactor")?;
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
            default_app_loader: default_app_loader.into(),
            middleware: OnceLock::new(),
        })
    }
}

impl ItsiServerConfig {
    pub fn new(ruby: &Ruby, cli_params: RHash, itsifile_path: Option<PathBuf>) -> Result<Self> {
        let server_params = Self::combine_params(ruby, cli_params, itsifile_path.as_ref())?;
        cli_params.delete::<_, Value>(Symbol::new("listeners"))?;

        if let Some(watchers) = server_params.notify_watchers.clone() {
            if server_params.workers == 1 {
                watch_groups(watchers)?;
            }
        }

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

/// Represents a set of patterns and commands.
#[derive(Debug, Clone)]
struct PatternGroup {
    base_dir: PathBuf,
    glob_set: GlobSet,
    commands: Vec<Vec<String>>,
}

/// Extracts the base directory from a wildcard pattern by taking the portion up to the first
/// component that contains a wildcard character.
fn extract_and_canonicalize_base_dir(pattern: &str) -> PathBuf {
    let path = Path::new(pattern);
    let mut base = PathBuf::new();
    for comp in path.components() {
        let comp_str = comp.as_os_str().to_string_lossy();
        if comp_str.contains('*') || comp_str.contains('?') || comp_str.contains('[') {
            break;
        } else {
            base.push(comp);
        }
    }
    // If no base was built, default to "."
    let base = if base.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        base
    };
    // Canonicalize to get the absolute path.
    fs::canonicalize(&base).unwrap_or(base)
}

pub fn watch_groups(pattern_groups: Vec<(String, Vec<Vec<String>>)>) -> Result<()> {
    let mut groups: Vec<PatternGroup> = Vec::new();
    for (pattern, commands) in pattern_groups.into_iter() {
        let base_dir = extract_and_canonicalize_base_dir(&pattern);
        let glob = Glob::new(&pattern).map_err(|e| {
            magnus::Error::new(
                magnus::exception::exception(),
                format!("Failed to create watch glob: {}", e),
            )
        })?;
        let glob_set = GlobSetBuilder::new().add(glob).build().map_err(|e| {
            magnus::Error::new(
                magnus::exception::exception(),
                format!("Failed to create watch glob set: {}", e),
            )
        })?;
        groups.push(PatternGroup {
            base_dir,
            glob_set,
            commands,
        });
    }

    thread::spawn(move || {
        // To avoid duplicate watches, track the base directories that have already been watched.
        let mut watched_dirs = HashSet::new();
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher: RecommendedWatcher =
            RecommendedWatcher::new(tx, Config::default()).unwrap();
        for group in &groups {
            if watched_dirs.insert(group.base_dir.clone()) {
                watcher
                    .watch(&group.base_dir, RecursiveMode::Recursive)
                    .unwrap();
            }
        }
        for res in rx {
            match res {
                Ok(event) => {
                    // For each pattern group, check if any path in the event matches.
                    for group in &groups {
                        // Check every path in the event.
                        for path in event.paths.iter() {
                            if let Ok(rel_path) = path.strip_prefix(&group.base_dir) {
                                if group.glob_set.is_match(rel_path) {
                                    for command in &group.commands {
                                        if command.is_empty() {
                                            continue;
                                        }
                                        let mut cmd = Command::new(&command[0]);
                                        if command.len() > 1 {
                                            cmd.args(&command[1..]);
                                        }
                                        match cmd.spawn() {
                                            Ok(mut child) => {
                                                if let Err(e) = child.wait() {
                                                    eprintln!(
                                                        "Command {:?} failed: {:?}",
                                                        command, e
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Failed to execute command {:?}: {:?}",
                                                    command, e
                                                );
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => println!("Watch error: {:?}", e),
            }
        }
    });

    Ok(())
}
