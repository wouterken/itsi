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
use std::collections::HashMap;
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
                magnus::Ruby::get().unwrap().exception_runtime_error(),
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

    fn selected_acme_manager(
        &self,
        listener_id: Option<String>,
    ) -> Result<crate::server::binds::tls::DynamicAcmeManager> {
        let config = self.config()?;
        let server_params = config.server_params.read();
        if server_params.workers > 1 {
            return Err(magnus::Error::new(
                magnus::Ruby::get().unwrap().exception_runtime_error(),
                "Dynamic TLS domain management currently only supports single-worker mode",
            ));
        }

        let managers = server_params.acme_managers.read();
        if managers.is_empty() {
            return Err(magnus::Error::new(
                magnus::Ruby::get().unwrap().exception_runtime_error(),
                "No ACME-managed TLS bindings are configured",
            ));
        }

        if let Some(listener_id) = listener_id {
            managers
                .iter()
                .find(|(id, _)| id == &listener_id)
                .map(|(_, manager)| manager.clone())
                .ok_or_else(|| {
                    magnus::Error::new(
                        magnus::Ruby::get().unwrap().exception_runtime_error(),
                        format!("Unknown ACME TLS binding: {}", listener_id),
                    )
                })
        } else if managers.len() == 1 {
            Ok(managers[0].1.clone())
        } else {
            Err(magnus::Error::new(
                magnus::Ruby::get().unwrap().exception_runtime_error(),
                "Multiple ACME TLS bindings are configured; specify a listener_id",
            ))
        }
    }

    pub fn tls_bindings(&self) -> Result<Vec<String>> {
        let config = self.config()?;
        let server_params = config.server_params.read();
        let bindings = server_params
            .acme_managers
            .read()
            .iter()
            .map(|(listener_id, _)| listener_id.clone())
            .collect();
        Ok(bindings)
    }

    pub fn tls_domains(&self, listener_id: Option<String>) -> Result<Vec<String>> {
        let manager = self.selected_acme_manager(listener_id)?;
        Ok(manager
            .statuses()
            .into_iter()
            .map(|status| status.domain)
            .collect())
    }

    pub fn tls_domain_statuses(
        &self,
        listener_id: Option<String>,
    ) -> Result<Vec<HashMap<String, String>>> {
        let manager = self.selected_acme_manager(listener_id)?;
        Ok(manager
            .statuses()
            .into_iter()
            .map(|status| {
                let mut out = HashMap::new();
                out.insert("domain".to_string(), status.domain);
                out.insert("status".to_string(), status.status);
                if let Some(last_error) = status.last_error {
                    out.insert("last_error".to_string(), last_error);
                }
                out
            })
            .collect())
    }

    pub fn register_tls_domain(&self, domain: String, listener_id: Option<String>) -> Result<()> {
        let manager = self.selected_acme_manager(listener_id)?;
        manager.register_domain(domain);
        Ok(())
    }

    pub fn unregister_tls_domain(
        &self,
        domain: String,
        listener_id: Option<String>,
    ) -> Result<()> {
        let manager = self.selected_acme_manager(listener_id)?;
        manager.unregister_domain(&domain);
        Ok(())
    }

    fn config(&self) -> Result<Arc<ItsiServerConfig>> {
        self.config.lock().as_ref().cloned().ok_or_else(|| {
            magnus::Error::new(
                magnus::Ruby::get().unwrap().exception_runtime_error(),
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
