use super::serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode};
use itsi_error::Result;
use itsi_rb_helpers::{call_with_gvl, fork};
use itsi_tracing::error;
use nix::{
    libc::kill,
    sys::socket::{socketpair, AddressFamily, SockFlag, SockType},
};
use parking_lot::Mutex;
use signal_hook::consts::signal::*;
use std::{os::fd::OwnedFd, sync::Arc};

#[derive(Default)]
pub struct ProcessWorker {
    pub worker_id: u8,
    pub child_pid: Mutex<Option<i32>>,
    pub child_fd: Mutex<Option<OwnedFd>>,
}

impl ProcessWorker {
    pub(crate) fn boot(&self, cluster_template: Arc<ClusterMode>) {
        let child_pid = *self.child_pid.lock();
        if let Some(pid) = child_pid {
            unsafe {
                kill(pid, SIGTERM);
            };
            *self.child_pid.lock() = None;
            *self.child_fd.lock() = None;
        }
        let (parent_fd, child_fd) = setup_ipc_channel().expect("Failed to set up IPC channel");
        match call_with_gvl(|_ruby| fork(cluster_template.lifecycle.after_fork.clone())) {
            Some(pid) => {
                drop(child_fd);
                *self.child_pid.lock() = Some(pid);
                *self.child_fd.lock() = Some(parent_fd);
            }
            None => {
                drop(parent_fd);
                if let Err(e) = Arc::new(SingleMode::new(
                    cluster_template.app,
                    cluster_template.listeners.clone(),
                    cluster_template.server.clone(),
                    cluster_template.thread_count,
                    cluster_template.script_name.clone(),
                    cluster_template.lifecycle.shutdown_timeout,
                ))
                .run()
                {
                    error!("Failed to boot into worker mode: {}", e);
                }
            }
        }
    }
}

fn setup_ipc_channel() -> Result<(OwnedFd, OwnedFd)> {
    Ok(socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )?)
}
