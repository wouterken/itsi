#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    Start,
    Shutdown,
    Restart,
    IncreaseWorkers,
    DecreaseWorkers,
    ForceShutdown,
}
