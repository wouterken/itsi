#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleEvent {
    Start,
    Shutdown,
    Restart,
    Reload,
    IncreaseWorkers,
    DecreaseWorkers,
    ForceShutdown,
    PrintInfo,
    ChildTerminated,
}
