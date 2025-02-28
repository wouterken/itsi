pub use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::fmt;

pub fn init() {
    let format = fmt::format()
        .with_level(false)
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .compact();
    tracing_subscriber::fmt().event_format(format).init();
}
