use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, AtomicI8, Ordering},
};

use nix::libc::{self, sighandler_t};
use parking_lot::Mutex;
use tokio::sync::broadcast;
use tracing::{debug, warn};

use super::lifecycle_event::LifecycleEvent;

pub static SIGINT_COUNT: AtomicI8 = AtomicI8::new(0);
pub static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
pub static SIGNAL_HANDLER_CHANNEL: Mutex<Option<broadcast::Sender<LifecycleEvent>>> =
    Mutex::new(None);

pub static PENDING_QUEUE: Mutex<VecDeque<LifecycleEvent>> = Mutex::new(VecDeque::new());

pub fn subscribe_runtime_to_signals() -> broadcast::Receiver<LifecycleEvent> {
    let mut guard = SIGNAL_HANDLER_CHANNEL.lock();
    if let Some(sender) = guard.as_ref() {
        return sender.subscribe();
    }
    let (sender, receiver) = broadcast::channel(32);
    let sender_clone = sender.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        for event in PENDING_QUEUE.lock().drain(..) {
            if let Err(e) = sender_clone.send(event) {
                eprintln!("Warning: Failed to send pending lifecycle event {:?}", e);
            }
        }
    });

    guard.replace(sender);

    receiver
}

pub fn unsubscribe_runtime() {
    SIGNAL_HANDLER_CHANNEL.lock().take();
}

pub fn send_lifecycle_event(event: LifecycleEvent) {
    if let Some(sender) = SIGNAL_HANDLER_CHANNEL.lock().as_ref() {
        if let Err(e) = sender.send(event) {
            if matches!(
                e.0,
                LifecycleEvent::Shutdown | LifecycleEvent::ForceShutdown
            ) {
                SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                warn!(
                    "Dropping shutdown lifecycle event after receiver closed: {:?}",
                    e
                );
            } else {
                eprintln!("Warning: Failed to send lifecycle event {:?}", e);
            }
        }
    } else {
        PENDING_QUEUE.lock().push_back(event);
    }
}

fn receive_signal(signum: i32, _: sighandler_t) {
    debug!("Received signal: {}", signum);
    SIGINT_COUNT.fetch_add(-1, Ordering::SeqCst);
    let event = match signum {
        libc::SIGTERM | libc::SIGINT => {
            debug!("Received shutdown signal (SIGTERM/SIGINT)");
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
            SIGINT_COUNT.fetch_add(2, Ordering::SeqCst);
            if SIGINT_COUNT.load(Ordering::SeqCst) < 2 {
                debug!("First shutdown signal, requesting graceful shutdown");
                Some(LifecycleEvent::Shutdown)
            } else {
                warn!("Multiple shutdown signals received, forcing immediate shutdown");
                Some(LifecycleEvent::ForceShutdown)
            }
        }
        libc::SIGUSR2 => Some(LifecycleEvent::PrintInfo),
        libc::SIGUSR1 => Some(LifecycleEvent::Restart),
        libc::SIGHUP => Some(LifecycleEvent::Reload),
        libc::SIGTTIN => Some(LifecycleEvent::IncreaseWorkers),
        libc::SIGTTOU => Some(LifecycleEvent::DecreaseWorkers),
        libc::SIGCHLD => Some(LifecycleEvent::ChildTerminated),
        _ => None,
    };

    if let Some(event) = event {
        debug!("Signal {} mapped to lifecycle event: {:?}", signum, event);
        send_lifecycle_event(event);
    } else {
        debug!("Signal {} not mapped to any lifecycle event", signum);
    }
}

pub fn reset_signal_handlers() -> bool {
    debug!("Resetting signal handlers");
    SIGINT_COUNT.store(0, Ordering::SeqCst);
    SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);

    unsafe {
        libc::signal(libc::SIGTERM, receive_signal as usize);
        libc::signal(libc::SIGINT, receive_signal as usize);
        libc::signal(libc::SIGUSR2, receive_signal as usize);
        libc::signal(libc::SIGUSR1, receive_signal as usize);
        libc::signal(libc::SIGHUP, receive_signal as usize);
        libc::signal(libc::SIGTTIN, receive_signal as usize);
        libc::signal(libc::SIGTTOU, receive_signal as usize);
        libc::signal(libc::SIGCHLD, receive_signal as usize);
    }
    true
}

pub fn clear_signal_handlers() {
    debug!("Clearing signal handlers");
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGUSR2, libc::SIG_DFL);
        libc::signal(libc::SIGUSR1, libc::SIG_DFL);
        libc::signal(libc::SIGHUP, libc::SIG_DFL);
        libc::signal(libc::SIGTTIN, libc::SIG_DFL);
        libc::signal(libc::SIGTTOU, libc::SIG_DFL);
        libc::signal(libc::SIGCHLD, libc::SIG_DFL);
    }
}
