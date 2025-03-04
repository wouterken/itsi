use atty::{Stream, is};
pub use tracing::{debug, error, info, trace, warn};
pub use tracing_attributes::instrument; // Explicitly export from tracing-attributes
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format},
};

#[instrument]
pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let format = fmt::format()
        .compact()
        .with_file(false)
        .with_level(true)
        .with_line_number(false)
        .with_source_location(false)
        .with_target(false)
        .with_thread_ids(false);

    let is_tty = is(Stream::Stdout);

    let subscriber = tracing_subscriber::fmt()
        .event_format(format)
        .with_env_filter(env_filter);

    if is_tty {
        subscriber.with_ansi(true).init();
    } else {
        subscriber.event_format(fmt::format().json()).init();
    }
}
