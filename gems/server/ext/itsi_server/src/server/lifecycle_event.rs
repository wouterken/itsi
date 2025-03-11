#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    Shutdown,
    Restart,
    IncreaseWorkers,
    DecreaseWorkers,
    ForceShutdown,
}
