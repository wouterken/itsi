use std::sync::{atomic::AtomicI8, LazyLock};

use nix::libc::{self, sighandler_t};
use tokio::sync::{self, broadcast};

use super::lifecycle_event::LifecycleEvent;

pub static SIGNAL_HANDLER_CHANNEL: LazyLock<(
    broadcast::Sender<LifecycleEvent>,
    broadcast::Receiver<LifecycleEvent>,
)> = LazyLock::new(|| sync::broadcast::channel(5));

pub fn send_shutdown_event() {
    SIGNAL_HANDLER_CHANNEL
        .0
        .send(LifecycleEvent::Shutdown)
        .expect("Failed to send shutdown event");
}

pub static SIGINT_COUNT: AtomicI8 = AtomicI8::new(0);
fn receive_signal(signum: i32, _: sighandler_t) {
    SIGINT_COUNT.fetch_add(-1, std::sync::atomic::Ordering::SeqCst);
    match signum {
        libc::SIGTERM | libc::SIGINT => {
            SIGINT_COUNT.fetch_add(2, std::sync::atomic::Ordering::SeqCst);
            if SIGINT_COUNT.load(std::sync::atomic::Ordering::SeqCst) < 2 {
                SIGNAL_HANDLER_CHANNEL.0.send(LifecycleEvent::Shutdown).ok();
            } else {
                // Not messing about. Force shutdown.
                SIGNAL_HANDLER_CHANNEL
                    .0
                    .send(LifecycleEvent::ForceShutdown)
                    .ok();
            }
        }
        libc::SIGUSR1 => {
            SIGNAL_HANDLER_CHANNEL.0.send(LifecycleEvent::Restart).ok();
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

pub fn reset_signal_handlers() -> bool {
    SIGINT_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
    unsafe {
        libc::signal(libc::SIGTERM, receive_signal as usize);
        libc::signal(libc::SIGINT, receive_signal as usize);
        libc::signal(libc::SIGUSR1, receive_signal as usize);
        libc::signal(libc::SIGUSR2, receive_signal as usize);
        libc::signal(libc::SIGTTIN, receive_signal as usize);
        libc::signal(libc::SIGTTOU, receive_signal as usize);
    }
    true
}

pub fn clear_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGUSR1, libc::SIG_DFL);
        libc::signal(libc::SIGUSR2, libc::SIG_DFL);
        libc::signal(libc::SIGTTIN, libc::SIG_DFL);
        libc::signal(libc::SIGTTOU, libc::SIG_DFL);
    }
}
