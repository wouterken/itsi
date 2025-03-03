#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    Start = 1,
    Shutdown = 2,
    Restart = 3,
}
