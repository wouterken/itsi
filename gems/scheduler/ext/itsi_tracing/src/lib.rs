use atty::{Stream, is};
use std::{
    env,
    sync::{Mutex, OnceLock},
};
pub use tracing::{debug, error, info, trace, warn};
use tracing_appender::rolling;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::{EnvFilter, fmt, prelude::*, reload};

// Global reload handle for changing the level at runtime.
static RELOAD_HANDLE: OnceLock<
    Mutex<Option<reload::Handle<EnvFilter, tracing_subscriber::Registry>>>,
> = OnceLock::new();

/// Log format: Plain or JSON.
#[derive(Debug, Clone)]
pub enum LogFormat {
    Plain,
    Json,
}

/// Log target: STDOUT, File, or Both.
#[derive(Debug, Clone)]
pub enum LogTarget {
    Stdout,
    File(String), // file name (rotated daily)
    Both(String), // file name (rotated daily) plus STDOUT
}

/// Logger configuration.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log level as a string (e.g. "info", "debug").
    pub level: String,
    /// Format: Plain (with optional ANSI) or JSON.
    pub format: LogFormat,
    /// Target: STDOUT, File, or Both.
    pub target: LogTarget,
    /// Whether to enable ANSI coloring (for plain text).
    pub use_ansi: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        let level = env::var("ITSI_LOG").unwrap_or_else(|_| "info".into());
        let format = match env::var("ITSI_LOG_FORMAT").as_deref() {
            Ok("json") => LogFormat::Json,
            _ => LogFormat::Plain,
        };
        let target = match env::var("ITSI_LOG_TARGET").as_deref() {
            Ok("file") => {
                let file = env::var("ITSI_LOG_FILE").unwrap_or_else(|_| "app.log".into());
                LogTarget::File(file)
            }
            Ok("both") => {
                let file = env::var("ITSI_LOG_FILE").unwrap_or_else(|_| "app.log".into());
                LogTarget::Both(file)
            }
            _ => LogTarget::Stdout,
        };
        // If ITSI_LOG_ANSI is set, use that; otherwise, use ANSI if stdout is a TTY.
        let use_ansi = env::var("ITSI_LOG_ANSI")
            .map(|s| s == "true")
            .unwrap_or_else(|_| is(Stream::Stdout));
        Self {
            level,
            format,
            target,
            use_ansi,
        }
    }
}

/// Initialize the global tracing subscriber with the default configuration.
pub fn init() {
    init_with_config(LogConfig::default());
}

/// Initialize the global tracing subscriber with a given configuration.
pub fn init_with_config(config: LogConfig) {
    // Build an EnvFilter from the configured level.
    let env_filter = EnvFilter::new(config.level);

    // Build the formatting layer based on target and format.
    let fmt_layer = match config.target {
        LogTarget::Stdout => match config.format {
            LogFormat::Plain => fmt::layer()
                .compact()
                .with_file(false)
                .with_line_number(false)
                .with_target(false)
                .with_thread_ids(false)
                .with_writer(BoxMakeWriter::new(std::io::stdout))
                .with_ansi(config.use_ansi)
                .boxed(),
            LogFormat::Json => fmt::layer()
                .compact()
                .with_file(false)
                .with_line_number(false)
                .with_target(false)
                .with_thread_ids(false)
                .with_writer(BoxMakeWriter::new(std::io::stdout))
                .with_ansi(config.use_ansi)
                .json()
                .boxed(),
        },
        LogTarget::File(file) => match config.format {
            LogFormat::Plain => fmt::layer()
                .compact()
                .with_file(false)
                .with_line_number(false)
                .with_target(false)
                .with_thread_ids(false)
                .with_writer(BoxMakeWriter::new({
                    let file = file.clone();
                    move || rolling::daily(".", file.clone())
                }))
                .with_ansi(false)
                .boxed(),
            LogFormat::Json => fmt::layer()
                .compact()
                .with_file(false)
                .with_line_number(false)
                .with_target(false)
                .with_thread_ids(false)
                .with_writer(BoxMakeWriter::new({
                    let file = file.clone();
                    move || rolling::daily(".", file.clone())
                }))
                .with_ansi(false)
                .json()
                .boxed(),
        },
        LogTarget::Both(file) => {
            // For "Both" target, handle each format separately to avoid type mismatches
            match config.format {
                LogFormat::Plain => {
                    let stdout_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(false)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(std::io::stdout))
                        .with_ansi(config.use_ansi);

                    let file_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(false)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new({
                            let file = file.clone();
                            move || rolling::daily(".", file.clone())
                        }))
                        .with_ansi(false);

                    stdout_layer.and_then(file_layer).boxed()
                }
                LogFormat::Json => {
                    let stdout_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(false)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(std::io::stdout))
                        .with_ansi(config.use_ansi)
                        .json();

                    let file_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(false)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new({
                            let file = file.clone();
                            move || rolling::daily(".", file.clone())
                        }))
                        .with_ansi(false)
                        .json();

                    stdout_layer.and_then(file_layer).boxed()
                }
            }
        }
    };

    // Create a reloadable filter layer so we can update the level at runtime.
    let (filter_layer, handle) = reload::Layer::new(env_filter);

    // Build the subscriber registry
    let subscriber = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global tracing subscriber");

    RELOAD_HANDLE.set(Mutex::new(Some(handle))).unwrap();
}

/// Change the log level at runtime.
pub fn set_level(new_level: &str) {
    if let Some(handle) = RELOAD_HANDLE.get().unwrap().lock().unwrap().as_ref() {
        handle
            .modify(|filter| *filter = EnvFilter::new(new_level))
            .expect("Failed to update log level");
    } else {
        eprintln!("Reload handle not initialized; call init() first.");
    }
}

/// Run a function silently by temporarily setting a no-op subscriber.
pub fn run_silently<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let no_op_subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::sink)
        .with_max_level(tracing_subscriber::filter::LevelFilter::OFF)
        .finish();
    let dispatch = tracing::Dispatch::new(no_op_subscriber);
    tracing::dispatcher::with_default(&dispatch, f)
}
