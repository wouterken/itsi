use std::env;

use atty::{Stream, is};
use tracing::level_filters::LevelFilter;
pub use tracing::{debug, error, info, trace, warn};
pub use tracing_attributes::instrument; // Explicitly export from tracing-attributes
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, format},
    layer::SubscriberExt,
};

#[instrument]
pub fn init() {
    let env_filter = EnvFilter::builder()
        .with_env_var("ITSI_LOG")
        .try_from_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

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

    if (is_tty && env::var("ITSI_LOG_PLAIN").is_err()) || env::var("ITSI_LOG_ANSI").is_ok() {
        subscriber.with_ansi(true).init();
    } else {
        subscriber
            .fmt_fields(format::JsonFields::default())
            .event_format(fmt::format().json())
            .init();
    }
}

pub fn run_silently<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    // Build a minimal subscriber that filters *everything* out
    let no_op_subscriber =
        tracing_subscriber::registry().with(fmt::layer().with_filter(LevelFilter::OFF));

    // Turn that subscriber into a `Dispatch`
    let no_op_dispatch = tracing::dispatcher::Dispatch::new(no_op_subscriber);

    // Temporarily set `no_op_dispatch` as the *default* within this closure
    tracing::dispatcher::with_default(&no_op_dispatch, f)
}
