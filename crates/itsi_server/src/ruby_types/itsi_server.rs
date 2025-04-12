use crate::server::{
    lifecycle_event::LifecycleEvent,
    serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode, ServeStrategy},
    signal::{clear_signal_handlers, reset_signal_handlers, send_lifecycle_event},
};
use itsi_rb_helpers::{call_without_gvl, print_rb_backtrace};
use itsi_server_config::ItsiServerConfig;
use itsi_tracing::{error, run_silently};
use magnus::{block::Proc, error::Result, RHash, Ruby};
use parking_lot::Mutex;
use std::{path::PathBuf, sync::Arc};
use tracing::{info, instrument};
mod file_watcher;
pub mod itsi_server_config;
#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Clone)]
pub struct ItsiServer {
    pub config: Arc<Mutex<Arc<ItsiServerConfig>>>,
}

impl ItsiServer {
    pub fn new(
        ruby: &Ruby,
        cli_params: RHash,
        itsifile_path: Option<PathBuf>,
        itsi_config_proc: Option<Proc>,
    ) -> Result<Self> {
        Ok(Self {
            config: Arc::new(Mutex::new(Arc::new(ItsiServerConfig::new(
                ruby,
                cli_params,
                itsifile_path,
                itsi_config_proc,
            )?))),
        })
    }

    pub fn stop(&self) -> Result<()> {
        send_lifecycle_event(LifecycleEvent::Shutdown);
        Ok(())
    }

    #[instrument(skip(self))]
    pub fn start(&self) -> Result<()> {
        let result = if self.config.lock().server_params.read().silence {
            run_silently(|| self.build_and_run_strategy())
        } else {
            info!("Itsi - Rolling into action. ⚪💨");
            self.build_and_run_strategy()
        };
        if let Err(e) = result {
            error!("Error starting server: {:?}", e);
            if let Some(err_value) = e.value() {
                print_rb_backtrace(err_value);
            }
            return Err(e);
        }
        Ok(())
    }

    pub(crate) fn build_strategy(&self) -> Result<ServeStrategy> {
        let server_config = self.config.lock();
        Ok(if server_config.server_params.read().workers > 1 {
            ServeStrategy::Cluster(Arc::new(ClusterMode::new(server_config.clone())))
        } else {
            ServeStrategy::Single(Arc::new(SingleMode::new(server_config.clone())?))
        })
    }

    fn build_and_run_strategy(&self) -> Result<()> {
        reset_signal_handlers();
        call_without_gvl(move || -> Result<()> {
            let strategy = self.build_strategy()?;
            if let Err(e) = strategy.clone().run() {
                error!("Error running server: {}", e);
                send_lifecycle_event(LifecycleEvent::Shutdown);
                strategy.stop()?;
            }
            Ok(())
        })?;
        clear_signal_handlers();
        info!("Server stopped");
        Ok(())
    }
}
