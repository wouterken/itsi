use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use itsi_rb_helpers::HeapFiber;

use super::FdReadinessPair;

#[derive(Debug, Clone)]
pub enum TimerKind {
    IoWait(FdReadinessPair),
    Block(usize),
    Sleep,
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub wake_time: Instant,
    pub fiber: HeapFiber,
    pub kind: TimerKind,
    pub canceled: Arc<AtomicBool>,
}

impl Timer {
    pub fn new(wake_in: Duration, fiber: HeapFiber, kind: TimerKind) -> Self {
        Self {
            fiber,
            kind,
            wake_time: Instant::now() + wake_in,
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Relaxed);
    }

    pub fn canceled(&self) -> bool {
        self.canceled.load(Ordering::Relaxed)
    }

    pub fn due(&self) -> bool {
        self.wake_time <= Instant::now() && !self.canceled()
    }
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.wake_time.eq(&other.wake_time)
    }
}
impl Eq for Timer {}
impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.wake_time.cmp(&self.wake_time)
    }
}
