use super::serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode};
use itsi_rb_helpers::{call_with_gvl, fork};
use itsi_tracing::error;
use nix::{
    errno::Errno,
    sys::{
        signal::{
            kill,
            Signal::{SIGKILL, SIGTERM},
        },
        wait::{waitpid, WaitPidFlag, WaitStatus},
    },
    unistd::{setpgid, Pid},
};
use parking_lot::Mutex;
use signal_hook::low_level::exit;
use std::{process, sync::Arc};
use tracing::instrument;

#[derive(Default, Clone, Debug)]
pub struct ProcessWorker {
    pub worker_id: u8,
    pub child_pid: Arc<Mutex<Option<Pid>>>,
}

impl ProcessWorker {
    #[instrument(skip(self, cluster_template), fields(self.worker_id = %self.worker_id))]
    pub(crate) fn boot(&self, cluster_template: Arc<ClusterMode>) {
        let child_pid = *self.child_pid.lock();
        if let Some(pid) = child_pid {
            if let Err(e) = kill(pid, SIGTERM) {
                error!("Failed to send SIGTERM to process {}: {}", pid, e);
            }
            *self.child_pid.lock() = None;
        }
        match call_with_gvl(|_ruby| fork(cluster_template.lifecycle.after_fork.clone())) {
            Some(pid) => {
                *self.child_pid.lock() = Some(Pid::from_raw(pid));
            }
            None => {
                if let Err(e) = setpgid(
                    Pid::from_raw(process::id() as i32),
                    Pid::from_raw(process::id() as i32),
                ) {
                    error!("Failed to set process group ID: {}", e);
                }
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
                exit(0)
            }
        }
    }

    pub(crate) fn request_shutdown(&self) {
        let child_pid = *self.child_pid.lock();
        if let Some(pid) = child_pid {
            if let Err(e) = kill(pid, SIGTERM) {
                error!("Failed to send SIGTERM to process {}: {}", pid, e);
            }
        }
    }

    pub(crate) fn force_kill(&self) {
        let child_pid = *self.child_pid.lock();
        if let Some(pid) = child_pid {
            if let Err(e) = kill(pid, SIGKILL) {
                error!("Failed to force kill process {}: {}", pid, e);
            }
        }
    }

    pub(crate) fn is_alive(&self) -> bool {
        let child_pid = *self.child_pid.lock();
        if let Some(pid) = child_pid {
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, _)) | Ok(WaitStatus::Signaled(_, _, _)) => {
                    return false;
                }
                Ok(WaitStatus::StillAlive) | Ok(_) => {}
                Err(_) => return false,
            }
            match kill(pid, None) {
                Ok(_) => true,
                Err(errno) => !matches!(errno, Errno::ESRCH),
            }
        } else {
            false
        }
    }
}
