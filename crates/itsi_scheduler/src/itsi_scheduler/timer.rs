use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};

use mio::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    pub wake_time: Instant,
    pub token: Token,
}
impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse the order: a timer with an earlier wake_time should be considered greater.
        other
            .wake_time
            .cmp(&self.wake_time)
            .then_with(|| other.token.cmp(&self.token))
    }
}

impl Timer {
    pub fn new(wake_in: Duration, token: Token) -> Self {
        Self {
            wake_time: Instant::now() + wake_in,
            token,
        }
    }

    pub fn is_due(&self) -> bool {
        self.wake_time <= Instant::now()
    }

    pub(crate) fn duration(&self) -> Option<Duration> {
        self.wake_time.checked_duration_since(Instant::now())
    }
}
