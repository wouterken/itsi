use derive_more::Debug;
use globset::{Glob, GlobSet, GlobSetBuilder};
use magnus::error::Result;
use nix::unistd::{close, fork, pipe, read};
use notify::{event::ModifyKind, EventKind, RecommendedWatcher};
use notify::{Event, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::{collections::HashSet, fs};
use std::{
    os::fd::{AsRawFd, IntoRawFd, OwnedFd},
    path::PathBuf,
    process::Command,
    sync::mpsc,
    thread::{self},
};

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

pub fn watch_groups(pattern_groups: Vec<(String, Vec<Vec<String>>)>) -> Result<Option<OwnedFd>> {
    let (r_fd, w_fd): (OwnedFd, OwnedFd) = pipe().map_err(|e| {
        magnus::Error::new(
            magnus::exception::exception(),
            format!("Failed to create watcher pipe: {}", e),
        )
    })?;

    let fork_result = unsafe {
        fork().map_err(|e| {
            magnus::Error::new(
                magnus::exception::exception(),
                format!("Failed to fork file watcher: {}", e),
            )
        })
    }?;

    if fork_result.is_child() {
        let _ = close(w_fd.into_raw_fd());
        thread::spawn(move || {
            let mut buf = [0u8; 1];
            loop {
                match read(r_fd.as_raw_fd(), &mut buf) {
                    Ok(0) => {
                        std::process::exit(0);
                    }
                    Ok(_) => {}
                    Err(_) => {
                        std::process::exit(0);
                    }
                }
            }
        });

        let mut groups = Vec::new();
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

        // Create a channel and a watcher.
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let sender = tx.clone();
        fn event_fn(sender: Sender<notify::Result<Event>>) -> impl Fn(notify::Result<Event>) {
            move |res| match res {
                Ok(event) => {
                    sender.send(Ok(event)).unwrap();
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }

        let mut watched_dirs = HashSet::new();
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(event_fn(sender)).expect("Failed to create watcher");
        for group in &groups {
            if watched_dirs.insert(group.base_dir.clone()) {
                watcher
                    .watch(&group.base_dir, RecursiveMode::Recursive)
                    .expect("Failed to add watch");
            }
        }

        // Main event loop.
        for res in rx {
            match res {
                Ok(event) => {
                    if !matches!(event.kind, EventKind::Modify(ModifyKind::Metadata(_))) {
                        continue;
                    }
                    for group in &groups {
                        for path in event.paths.iter() {
                            if let Ok(rel_path) = path.strip_prefix(&group.base_dir) {
                                if group.glob_set.is_match(rel_path) {
                                    // Execute the commands for this group.
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

        // Clean up the watches.
        for group in &groups {
            watcher
                .unwatch(&group.base_dir)
                .expect("Failed to remove watch");
        }
        drop(watcher);
        std::process::exit(0);
    } else {
        let _ = close(r_fd.into_raw_fd());
        Ok(Some(w_fd))
    }
}
