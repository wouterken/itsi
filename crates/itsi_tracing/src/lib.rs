pub use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{EnvFilter, fmt};

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let format = fmt::format().with_level(true).with_target(false).compact();
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(format)
        .init();
}
