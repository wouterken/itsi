use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, AtomicI8},
};

use nix::libc::{self, sighandler_t};
use parking_lot::Mutex;
use tokio::sync::broadcast;

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
    let (sender, receiver) = broadcast::channel(5);
    let sender_clone = sender.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        for event in PENDING_QUEUE.lock().drain(..) {
            sender_clone.send(event).ok();
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
        sender.send(event).ok();
    } else {
        PENDING_QUEUE.lock().push_back(event);
    }
}

fn receive_signal(signum: i32, _: sighandler_t) {
    SIGINT_COUNT.fetch_add(-1, std::sync::atomic::Ordering::SeqCst);
    let event = match signum {
        libc::SIGTERM | libc::SIGINT => {
            SHUTDOWN_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
            SIGINT_COUNT.fetch_add(2, std::sync::atomic::Ordering::SeqCst);
            if SIGINT_COUNT.load(std::sync::atomic::Ordering::SeqCst) < 2 {
                Some(LifecycleEvent::Shutdown)
            } else {
                // Not messing about. Force shutdown.
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
        send_lifecycle_event(event);
    }
}

pub fn reset_signal_handlers() -> bool {
    SIGINT_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
    SHUTDOWN_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);

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
