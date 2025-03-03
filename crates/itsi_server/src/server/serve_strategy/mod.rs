use cluster_mode::ClusterMode;
use itsi_error::Result;
use single_mode::SingleMode;
use std::sync::Arc;
pub mod cluster_mode;
pub mod single_mode;

pub(crate) enum ServeStrategy {
    Single(Arc<SingleMode>),
    Cluster(Arc<ClusterMode>),
}

impl ServeStrategy {
    pub fn run(&self) -> Result<()> {
        match self {
            ServeStrategy::Single(single_router) => single_router.clone().run(),
            ServeStrategy::Cluster(cluster_router) => cluster_router.clone().run(),
        }
    }
}
