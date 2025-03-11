use super::serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode};
use itsi_rb_helpers::{call_with_gvl, call_without_gvl, create_ruby_thread, fork};
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
use std::{
    process::{self, exit},
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{info, instrument, warn};

#[derive(Default, Clone, Debug)]
pub struct ProcessWorker {
    pub worker_id: usize,
    pub child_pid: Arc<Mutex<Option<Pid>>>,
}

impl ProcessWorker {
    #[instrument(skip(self, cluster_template), fields(self.worker_id = %self.worker_id))]
    pub(crate) fn boot(&self, cluster_template: Arc<ClusterMode>) {
        let child_pid = *self.child_pid.lock();
        if let Some(pid) = child_pid {
            if self.is_alive() {
                if let Err(e) = kill(pid, SIGTERM) {
                    info!("Failed to send SIGTERM to process {}: {}", pid, e);
                }
            }
            *self.child_pid.lock() = None;
        }

        match call_with_gvl(|_ruby| fork(cluster_template.server.after_fork.lock().clone())) {
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
                    cluster_template.server.clone(),
                    cluster_template.listeners.clone(),
                    cluster_template.lifecycle_channel.clone(),
                ))
                .run()
                {
                    error!("Failed to boot into worker mode: {}", e);
                }
                exit(0)
            }
        }
    }

    pub(crate) async fn reboot(&self, cluster_template: Arc<ClusterMode>) {
        self.graceful_shutdown(cluster_template.clone()).await;
        let self_clone = self.clone();
        create_ruby_thread(move || call_without_gvl(move || self_clone.boot(cluster_template)));
    }

    pub(crate) async fn graceful_shutdown(&self, cluster_template: Arc<ClusterMode>) {
        let self_clone = self.clone();
        self_clone.request_shutdown();
        let force_kill_time =
            Instant::now() + Duration::from_secs_f64(cluster_template.server.shutdown_timeout);
        while self_clone.is_alive() && force_kill_time > Instant::now() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if self_clone.is_alive() {
            self_clone.force_kill();
        }
    }

    pub(crate) fn boot_if_dead(&self, cluster_template: Arc<ClusterMode>) {
        if !self.is_alive() {
            warn!("Worker no longer running. Rebooting");
            let self_clone = self.clone();
            create_ruby_thread(move || call_without_gvl(move || self_clone.boot(cluster_template)));
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
