use crate::server::signal::{clear_signal_handlers, reset_signal_handlers, send_shutdown_event};
use itsi_rb_helpers::call_without_gvl;
use itsi_server_config::ItsiServerConfig;
use itsi_tracing::{error, run_silently};
use magnus::{error::Result, RHash, Ruby};
use parking_lot::Mutex;
use std::{path::PathBuf, sync::Arc};
use tracing::info;
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
        reexec_params: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            config: Arc::new(Mutex::new(Arc::new(ItsiServerConfig::new(
                ruby,
                cli_params,
                itsifile_path,
                reexec_params,
            )?))),
        })
    }

    pub fn stop(&self) -> Result<()> {
        send_shutdown_event();
        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        if self.config.lock().server_params.read().silence {
            run_silently(|| self.build_and_run_strategy())
        } else {
            info!("Itsi - Rolling into action. 💨 ⚪ ");
            self.build_and_run_strategy()
        }
    }

    fn build_and_run_strategy(&self) -> Result<()> {
        reset_signal_handlers();
        let server_clone = self.clone();
        call_without_gvl(move || -> Result<()> {
            let strategy = server_clone.config.lock().clone().build_strategy()?;
            if let Err(e) = strategy.clone().run() {
                error!("Error running server: {}", e);
                strategy.stop()?;
            }
            Ok(())
        })?;
        clear_signal_handlers();
        info!("Server stopped");
        Ok(())
    }
}
