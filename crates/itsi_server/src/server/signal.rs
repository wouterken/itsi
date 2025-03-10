use std::sync::LazyLock;

use nix::libc::{self, sighandler_t};
use tokio::sync::{self, broadcast};

use super::lifecycle_event::LifecycleEvent;

pub static SIGNAL_HANDLER_CHANNEL: LazyLock<(
    broadcast::Sender<LifecycleEvent>,
    broadcast::Receiver<LifecycleEvent>,
)> = LazyLock::new(|| sync::broadcast::channel(5));

fn receive_signal(signum: i32, _: sighandler_t) {
    match signum {
        libc::SIGTERM => {
            SIGNAL_HANDLER_CHANNEL.0.send(LifecycleEvent::Shutdown).ok();
        }
        libc::SIGINT => {
            SIGNAL_HANDLER_CHANNEL.0.send(LifecycleEvent::Shutdown).ok();
        }
        libc::SIGUSR1 => {
            SIGNAL_HANDLER_CHANNEL
                .0
                .send(LifecycleEvent::RestartWorkers)
                .ok();
        }
        libc::SIGUSR2 => {
            SIGNAL_HANDLER_CHANNEL
                .0
                .send(LifecycleEvent::RestartWorkersFreshConfig)
                .ok();
        }
        libc::SIGTTIN => {
            SIGNAL_HANDLER_CHANNEL
                .0
                .send(LifecycleEvent::IncreaseWorkers)
                .ok();
        }
        libc::SIGTTOU => {
            SIGNAL_HANDLER_CHANNEL
                .0
                .send(LifecycleEvent::DecreaseWorkers)
                .ok();
        }
        _ => {}
    }
}

pub fn reset_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, receive_signal as usize);
        libc::signal(libc::SIGINT, receive_signal as usize);
        libc::signal(libc::SIGUSR1, receive_signal as usize);
        libc::signal(libc::SIGUSR2, receive_signal as usize);
        libc::signal(libc::SIGTTIN, receive_signal as usize);
        libc::signal(libc::SIGTTOU, receive_signal as usize);
    }
}
