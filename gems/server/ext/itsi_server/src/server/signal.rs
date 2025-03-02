use std::sync::Arc;

use itsi_tracing::info;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;

use futures::stream::StreamExt;
use tokio::sync::broadcast::{self, error::SendError};

use super::lifecycle_event::LifecycleEvent;

pub async fn handle_signals(
    lifecycle_tx: Arc<broadcast::Sender<LifecycleEvent>>,
) -> Result<(), SendError<LifecycleEvent>> {
    let mut signals = Signals::new([
        SIGHUP, SIGTERM, SIGINT, SIGQUIT, SIGTTIN, SIGTTOU, SIGWINCH, SIGUSR2,
    ])
    .expect("Failed to create signal handler");
    while let Some(signal) = signals.next().await {
        info!("Got signal: {:?}", signal);
        match signal {
            SIGHUP => {
                // Reload configuration
                // Reopen the log file
            }
            SIGTERM | SIGINT | SIGQUIT => {
                lifecycle_tx.send(LifecycleEvent::Shutdown)?;
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
