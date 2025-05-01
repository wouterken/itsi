use std::sync::Arc;

use cluster_mode::ClusterMode;
use itsi_error::Result;
use single_mode::SingleMode;

pub mod acceptor;
pub mod cluster_mode;
pub mod single_mode;

#[derive(Clone)]
pub(crate) enum ServeStrategy {
    Single(Arc<SingleMode>),
    Cluster(Arc<ClusterMode>),
}

impl ServeStrategy {
    pub fn run(self) -> Result<()> {
        match self {
            ServeStrategy::Single(single_router) => single_router.run(),
            ServeStrategy::Cluster(cluster_router) => cluster_router.run(),
        }
    }

    pub(crate) fn stop(&self) -> Result<()> {
        match self {
            ServeStrategy::Single(single_router) => single_router.stop(),
            ServeStrategy::Cluster(cluster_router) => cluster_router.stop(),
        }
    }
}
