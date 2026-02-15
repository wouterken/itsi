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
#[derive(Clone, Default)]
pub struct ItsiServer {
    pub config: Arc<Mutex<Option<Arc<ItsiServerConfig>>>>,
}

impl ItsiServer {
    pub fn initialize(
        &self,
        cli_params: RHash,
        itsifile_path: Option<PathBuf>,
        itsi_config_proc: Option<Proc>,
    ) -> Result<()> {
        let ruby = Ruby::get().map_err(|_| {
            magnus::Error::new(
                magnus::exception::runtime_error(),
                "Failed to acquire Ruby VM handle",
            )
        })?;
        let config = Arc::new(ItsiServerConfig::new(
            &ruby,
            cli_params,
            itsifile_path,
            itsi_config_proc,
        )?);
        *self.config.lock() = Some(config);
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        send_lifecycle_event(LifecycleEvent::Shutdown);
        Ok(())
    }

    fn config(&self) -> Result<Arc<ItsiServerConfig>> {
        self.config.lock().as_ref().cloned().ok_or_else(|| {
            magnus::Error::new(
                magnus::exception::runtime_error(),
                "Itsi::Server not initialized",
            )
        })
    }

    #[instrument(skip(self))]
    pub fn start(&self) -> Result<()> {
        let server_config = self.config()?;
        server_config.server_params.read().setup_listeners()?;
        let result = if server_config.server_params.read().silence {
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
        let server_config = self.config()?;
        Ok(if server_config.server_params.read().workers > 1 {
            ServeStrategy::Cluster(Arc::new(ClusterMode::new(server_config)))
        } else {
            ServeStrategy::Single(Arc::new(SingleMode::new(server_config, 0)?))
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
