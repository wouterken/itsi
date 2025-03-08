use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use itsi_rb_helpers::HeapFiber;

#[derive(Debug, Clone)]
pub struct Immediate {
    pub fiber: HeapFiber,
    pub canceled: Arc<AtomicBool>,
}

impl Immediate {
    pub fn new(fiber: HeapFiber) -> Self {
        Self {
            fiber,
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Relaxed);
    }

    pub fn canceled(&self) -> bool {
        self.canceled.load(Ordering::Relaxed)
    }
}
