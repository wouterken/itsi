use derive_more::Debug;
use globset::{Glob, GlobSet, GlobSetBuilder};
use magnus::error::Result;
use nix::unistd::{close, dup, fork, pipe, read, write};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Debug, Clone)]
struct PatternGroup {
    base_dir: PathBuf,
    glob_set: GlobSet,
    commands: Vec<Vec<String>>,
    pattern: String,
    last_triggered: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatcherCommand {
    Stop,
    ConfigError,
    Continue,
}

#[derive(Debug)]
pub struct WatcherPipes {
    pub read_fd: OwnedFd,
    pub write_fd: OwnedFd,
}

impl AsRawFd for WatcherPipes {
    fn as_raw_fd(&self) -> RawFd {
        self.read_fd.as_raw_fd()
    }
}

impl Drop for WatcherPipes {
    fn drop(&mut self) {
        let _ = send_watcher_command(&self.write_fd, WatcherCommand::Stop);
        let _ = close(self.read_fd.as_raw_fd());
        let _ = close(self.write_fd.as_raw_fd());
    }
}

fn extract_and_canonicalize_base_dir(pattern: &str) -> (PathBuf, String) {
    let path = Path::new(pattern);
    let mut base = PathBuf::new();
    let mut remaining_components = Vec::new();
    let mut found_glob = false;

    for comp in path.components() {
        let comp_str = comp.as_os_str().to_string_lossy();
        if !found_glob
            && (comp_str.contains('*') || comp_str.contains('?') || comp_str.contains('['))
        {
            found_glob = true;
            remaining_components.push(comp_str.to_string());
        } else if found_glob {
            remaining_components.push(comp_str.to_string());
        } else {
            base.push(comp);
        }
    }

    let base = if base.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        base
    };
    let base = fs::canonicalize(&base).unwrap_or(base);
    let remaining_pattern = remaining_components.join("/");

    (base, remaining_pattern)
}

const DEBOUNCE_DURATION: Duration = Duration::from_millis(300);
const EVENT_DEDUP_DURATION: Duration = Duration::from_millis(50);
const AUTO_RECOVERY_TIMEOUT: Duration = Duration::from_secs(5);

fn serialize_command(cmd: WatcherCommand) -> u8 {
    match cmd {
        WatcherCommand::Stop => 0,
        WatcherCommand::ConfigError => 1,
        WatcherCommand::Continue => 2,
    }
}

fn deserialize_command(byte: u8) -> Option<WatcherCommand> {
    match byte {
        0 => Some(WatcherCommand::Stop),
        1 => Some(WatcherCommand::ConfigError),
        2 => Some(WatcherCommand::Continue),
        _ => None,
    }
}

pub fn send_watcher_command(fd: &OwnedFd, cmd: WatcherCommand) -> Result<()> {
    let buf = [serialize_command(cmd)];
    match write(fd, &buf) {
        Ok(_) => Ok(()),
        Err(e) => Err(magnus::Error::new(
            magnus::Ruby::get().unwrap().exception_standard_error(),
            format!("Failed to send command to watcher: {}", e),
        )),
    }
}

pub fn watch_groups(
    pattern_groups: Vec<(String, Vec<Vec<String>>)>,
) -> Result<Option<WatcherPipes>> {
    // Create bidirectional pipes for communication
    let (parent_read_fd, child_write_fd): (OwnedFd, OwnedFd) = pipe().map_err(|e| {
        magnus::Error::new(
            magnus::Ruby::get().unwrap().exception_standard_error(),
            format!("Failed to create parent read pipe: {}", e),
        )
    })?;

    let (child_read_fd, parent_write_fd): (OwnedFd, OwnedFd) = pipe().map_err(|e| {
        magnus::Error::new(
            magnus::Ruby::get().unwrap().exception_standard_error(),
            format!("Failed to create child read pipe: {}", e),
        )
    })?;

    let fork_result = unsafe {
        fork().map_err(|e| {
            magnus::Error::new(
                magnus::Ruby::get().unwrap().exception_standard_error(),
                format!("Failed to fork file watcher: {}", e),
            )
        })
    }?;

    if fork_result.is_child() {
        // Child process - close the parent ends of the pipes
        let _ = close(parent_read_fd.into_raw_fd());
        let _ = close(parent_write_fd.into_raw_fd());

        let _child_read_fd_clone =
            unsafe { OwnedFd::from_raw_fd(dup(child_read_fd.as_raw_fd()).unwrap()) };
        let child_write_fd_clone =
            unsafe { OwnedFd::from_raw_fd(dup(child_write_fd.as_raw_fd()).unwrap()) };

        let command_channel = Arc::new(Mutex::new(None));
        let command_channel_clone = command_channel.clone();

        // Thread to read commands from parent
        thread::spawn(move || {
            let mut buf = [0u8; 1];
            loop {
                match read(child_read_fd.as_raw_fd(), &mut buf) {
                    Ok(0) => {
                        info!("Parent closed command pipe, exiting watcher");
                        std::process::exit(0);
                    }
                    Ok(_) => {
                        if let Some(cmd) = deserialize_command(buf[0]) {
                            info!("Received command from parent: {:?}", cmd);
                            *command_channel_clone.lock() = Some(cmd);

                            if matches!(cmd, WatcherCommand::Stop) {
                                info!("Received stop command, exiting watcher");
                                std::process::exit(0);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading from command pipe: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        });

        let mut groups = Vec::new();
        for (pattern, commands) in pattern_groups.into_iter() {
            let (base_dir, remaining_pattern) = extract_and_canonicalize_base_dir(&pattern);
            info!(
                "Watching base directory {:?} with pattern {:?} (original: {:?})",
                base_dir, remaining_pattern, pattern
            );

            let glob = Glob::new(&remaining_pattern).map_err(|e| {
                magnus::Error::new(
                    magnus::Ruby::get().unwrap().exception_standard_error(),
                    format!(
                        "Failed to create watch glob for pattern '{}': {}",
                        remaining_pattern, e
                    ),
                )
            })?;
            let glob_set = GlobSetBuilder::new().add(glob).build().map_err(|e| {
                magnus::Error::new(
                    magnus::Ruby::get().unwrap().exception_standard_error(),
                    format!("Failed to create watch glob set: {}", e),
                )
            })?;

            groups.push(PatternGroup {
                base_dir,
                glob_set,
                commands,
                pattern: remaining_pattern,
                last_triggered: None,
            });
        }

        // Create a channel and a watcher
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let startup_time = Instant::now();
        let sender = tx.clone();

        let event_fn = move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                sender.send(Ok(event)).unwrap_or_else(|e| {
                    error!("Failed to send event: {}", e);
                });
            } else if let Err(e) = res {
                error!("Watch error: {:?}", e);
            }
        };

        let mut watched_paths = HashSet::new();
        let mut watcher = notify::recommended_watcher(event_fn).expect("Failed to create watcher");

        for group in &groups {
            if watched_paths.insert(group.base_dir.clone()) {
                let recursive = if group.pattern.is_empty() {
                    RecursiveMode::NonRecursive
                } else {
                    RecursiveMode::Recursive
                };

                watcher
                    .watch(&group.base_dir, recursive)
                    .expect("Failed to add watch");
            }
        }

        // Wait briefly to avoid initial event storm
        thread::sleep(Duration::from_millis(100));

        // State management
        let mut recent_events: HashMap<(PathBuf, EventKind), Instant> = HashMap::new();
        let restart_state = Arc::new(Mutex::new(None::<Instant>));

        // Main event loop
        for res in rx {
            match res {
                Ok(event) => {
                    if !matches!(event.kind, EventKind::Modify(ModifyKind::Data(_))) {
                        continue;
                    }

                    let now = Instant::now();

                    // Skip startup events
                    if now.duration_since(startup_time) < Duration::from_millis(500) {
                        continue;
                    }

                    // Deduplicate events
                    let mut should_process = true;
                    for path in &event.paths {
                        let event_key = (path.clone(), event.kind);
                        if let Some(&last_seen) = recent_events.get(&event_key) {
                            if now.duration_since(last_seen) < EVENT_DEDUP_DURATION {
                                should_process = false;
                                break;
                            }
                        }
                        recent_events.insert(event_key, now);
                    }

                    if !should_process {
                        continue;
                    }

                    // Clean up old entries
                    recent_events
                        .retain(|_, &mut time| now.duration_since(time) < Duration::from_secs(1));

                    // Check restart state
                    let should_skip = {
                        let state = restart_state.lock();
                        if let Some(restart_time) = *state {
                            now.duration_since(restart_time) < Duration::from_millis(500)
                        } else {
                            false
                        }
                    };

                    if should_skip {
                        continue;
                    }

                    // Process commands from parent
                    let command_to_process = {
                        let mut command_guard = command_channel.lock();
                        let cmd = *command_guard;
                        *command_guard = None;
                        cmd
                    };

                    if let Some(cmd) = command_to_process {
                        match cmd {
                            WatcherCommand::ConfigError => {
                                info!("Received config error notification, resuming file watching");
                                *restart_state.lock() = None;
                                for group in &mut groups {
                                    group.last_triggered = None;
                                }
                                recent_events.clear();
                            }
                            WatcherCommand::Continue => {
                                info!("Received continue notification, resuming file watching");
                                *restart_state.lock() = None;
                            }
                            WatcherCommand::Stop => { /* Handled in command thread */ }
                        }
                    }

                    // Process file events
                    for group in &mut groups {
                        // Apply debounce
                        if let Some(last_triggered) = group.last_triggered {
                            if now.duration_since(last_triggered) < DEBOUNCE_DURATION {
                                continue;
                            }
                        }

                        for path in event.paths.iter() {
                            let matches = if group.pattern.is_empty() {
                                path == &group.base_dir
                            } else if let Ok(rel_path) = path.strip_prefix(&group.base_dir) {
                                group.glob_set.is_match(rel_path)
                            } else {
                                false
                            };

                            if matches {
                                group.last_triggered = Some(now);

                                // Execute commands
                                for command in &group.commands {
                                    if command.is_empty() {
                                        continue;
                                    }

                                    // Check for shell command or restart/reload
                                    let is_shell_command = command.len() == 1
                                        && (command[0].contains("&&")
                                            || command[0].contains("||")
                                            || command[0].contains("|")
                                            || command[0].contains(";"));

                                    let is_restart = command
                                        .windows(2)
                                        .any(|w| w[0] == "itsi" && w[1] == "restart")
                                        || (is_shell_command
                                            && command[0].contains("itsi restart"));

                                    let is_reload = command
                                        .windows(2)
                                        .any(|w| w[0] == "itsi" && w[1] == "reload")
                                        || (is_shell_command && command[0].contains("itsi reload"));

                                    // Handle restart/reload
                                    if is_restart || is_reload {
                                        let cmd_type =
                                            if is_restart { "restart" } else { "reload" };
                                        let mut should_run = false;

                                        {
                                            let mut state = restart_state.lock();
                                            if let Some(last_time) = *state {
                                                if now.duration_since(last_time)
                                                    < Duration::from_secs(3)
                                                {
                                                    info!(
                                                        "Ignoring {} command - too soon",
                                                        cmd_type
                                                    );
                                                } else {
                                                    *state = Some(now);
                                                    should_run = true;
                                                }
                                            } else {
                                                *state = Some(now);
                                                should_run = true;
                                            }
                                        }

                                        if !should_run {
                                            continue;
                                        }

                                        // Notify parent (optional)
                                        let _ = write(&child_write_fd_clone, &[3]);
                                    }

                                    // Build and execute command
                                    let mut cmd = if is_shell_command {
                                        let mut shell_cmd = Command::new("sh");
                                        shell_cmd.arg("-c").arg(command.join(" "));
                                        shell_cmd
                                    } else {
                                        let mut cmd = Command::new(&command[0]);
                                        if command.len() > 1 {
                                            cmd.args(&command[1..]);
                                        }
                                        cmd
                                    };

                                    match cmd.spawn() {
                                        Ok(mut child) => {
                                            if let Err(e) = child.wait() {
                                                error!("Command {:?} failed: {:?}", command, e);
                                            }

                                            if is_restart || is_reload {
                                                info!("Itsi command submitted, waiting for parent response");

                                                // Set auto-recovery timer
                                                let restart_state_clone =
                                                    Arc::clone(&restart_state);
                                                let now_clone = now;
                                                thread::spawn(move || {
                                                    thread::sleep(AUTO_RECOVERY_TIMEOUT);
                                                    let mut state = restart_state_clone.lock();
                                                    if let Some(restart_time) = *state {
                                                        if now_clone.duration_since(restart_time)
                                                            < Duration::from_secs(1)
                                                        {
                                                            info!("Auto-recovering from potential restart failure");
                                                            *state = None;
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            error!(
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
                Err(e) => error!("Watch error: {:?}", e),
            }
        }

        // Clean up
        drop(watcher);
        std::process::exit(0);
    } else {
        // Parent process - close the child ends of the pipes
        let _ = close(child_read_fd.into_raw_fd());
        let _ = close(child_write_fd.into_raw_fd());

        // Create a paired structure to return
        let watcher_pipes = WatcherPipes {
            read_fd: parent_read_fd,
            write_fd: parent_write_fd,
        };

        Ok(Some(watcher_pipes))
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_extract_patterns() {
        // Save current dir to restore later
        let original_dir = env::current_dir().unwrap();

        // Create a temp dir and work from there for consistent results
        let temp_dir = env::temp_dir().join("itsi_test_patterns");
        let _ = fs::create_dir_all(&temp_dir);
        env::set_current_dir(&temp_dir).unwrap();

        // Test glob patterns
        let (base, pattern) = extract_and_canonicalize_base_dir("assets/*/**.tsx");
        assert!(base.ends_with("assets"));
        assert_eq!(pattern, "*/**.tsx");

        let (base, pattern) = extract_and_canonicalize_base_dir("./assets/*/**.tsx");
        assert!(base.ends_with("assets"));
        assert_eq!(pattern, "*/**.tsx");

        // Test non-glob patterns - exact files should have empty pattern
        let (base, pattern) = extract_and_canonicalize_base_dir("foo/bar.txt");
        assert!(base.ends_with("bar.txt"));
        assert_eq!(pattern, "");

        // Test current directory patterns
        let (base, pattern) = extract_and_canonicalize_base_dir("*.txt");
        assert_eq!(base, temp_dir.canonicalize().unwrap());
        assert_eq!(pattern, "*.txt");

        // Test file in current directory
        let (base, pattern) = extract_and_canonicalize_base_dir("test.txt");
        assert!(base.ends_with("test.txt"));
        assert_eq!(pattern, "");

        // Restore original directory and clean up
        env::set_current_dir(original_dir).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_watcher_commands() {
        assert_eq!(serialize_command(WatcherCommand::Stop), 0);
        assert_eq!(serialize_command(WatcherCommand::ConfigError), 1);
        assert_eq!(serialize_command(WatcherCommand::Continue), 2);

        assert_eq!(deserialize_command(0), Some(WatcherCommand::Stop));
        assert_eq!(deserialize_command(1), Some(WatcherCommand::ConfigError));
        assert_eq!(deserialize_command(2), Some(WatcherCommand::Continue));
        assert_eq!(deserialize_command(99), None);
    }
}
