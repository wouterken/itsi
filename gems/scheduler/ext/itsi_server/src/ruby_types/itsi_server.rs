use crate::server::{
    listener::Listener,
    serve_strategy::{cluster_mode::ClusterMode, single_mode::SingleMode, ServeStrategy},
    signal::{
        clear_signal_handlers, reset_signal_handlers, send_shutdown_event, SIGNAL_HANDLER_CHANNEL,
    },
};
use itsi_rb_helpers::call_without_gvl;
use itsi_server_config::ItsiServerConfig;
use itsi_tracing::{error, run_silently};
use magnus::{error::Result, Value};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, instrument};
pub mod itsi_server_config;

static DEFAULT_BIND: &str = "http://localhost:3000";

#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Clone)]
pub struct ItsiServer {
    pub config: Arc<Mutex<Arc<ItsiServerConfig>>>,
}

impl ItsiServer {
    pub fn new(args: &[Value]) -> Result<Self> {
        Ok(Self {
            config: Arc::new(Mutex::new(Arc::new(ItsiServerConfig::new(args)?))),
        })
    }

    pub fn preload(self: &Arc<Self>) -> Result<()> {
        self.config.lock().filter_stack.preload()?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        send_shutdown_event();
        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        if self.config.lock().silence {
            run_silently(|| self.build_and_run_strategy())
        } else {
            self.build_and_run_strategy()
        }
    }

    fn build_and_run_strategy(&self) -> Result<()> {
        reset_signal_handlers();
        let config = self.config.lock().clone();
        let config_clone = config.clone();
        let server_clone = self.clone();
        call_without_gvl(move || -> Result<()> {
            config.build_strategy(&server_clone)?;
            if let Err(e) = config_clone.strategy.read().as_ref().unwrap().run() {
                error!("Error running server: {}", e);
                config_clone.strategy.read().as_ref().unwrap().stop()?;
            }
            Ok(())
        })?;
        clear_signal_handlers();
        self.config.lock().strategy.write().take();
        info!("Server stopped");
        Ok(())
    }
}
