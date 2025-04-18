use atty::{Stream, is};
use std::{
    env,
    sync::{Mutex, OnceLock},
};
use tracing::Level;
pub use tracing::{debug, error, info, trace, warn};
use tracing_appender::rolling;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::{EnvFilter, fmt, prelude::*, reload};
use tracing_subscriber::{Layer, Registry, layer::Layered};

// Global reload handle for changing the level at runtime.
static RELOAD_HANDLE: OnceLock<
    Mutex<Option<reload::Handle<EnvFilter, tracing_subscriber::Registry>>>,
> = OnceLock::new();

// Global reload handle for changing the formatting layer (log target/format) at runtime.
type ReloadFmtHandle = reload::Handle<
    Box<
        dyn Layer<
                tracing_subscriber::layer::Layered<
                    reload::Layer<EnvFilter, tracing_subscriber::Registry>,
                    tracing_subscriber::Registry,
                >,
            > + Send
            + Sync,
    >,
    Layered<tracing_subscriber::reload::Layer<EnvFilter, Registry>, Registry>,
>;

static RELOAD_FMT_HANDLE: OnceLock<Mutex<Option<ReloadFmtHandle>>> = OnceLock::new();

// Global current log configuration for formatting options.
static CURRENT_CONFIG: OnceLock<Mutex<LogConfig>> = OnceLock::new();

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

fn default_log_file() -> String {
    env::var("ITSI_LOG_FILE").unwrap_or_else(|_| "itsi-app.log".into())
}

impl Default for LogConfig {
    fn default() -> Self {
        let level = env::var("ITSI_LOG").unwrap_or_else(|_| "info".into());
        let format = match env::var("ITSI_LOG_FORMAT").as_deref() {
            Ok("json") => LogFormat::Json,
            _ => LogFormat::Plain,
        };
        let target = match env::var("ITSI_LOG_TARGET").as_deref() {
            Ok("file") => LogTarget::File(default_log_file()),
            Ok("both") => LogTarget::Both(default_log_file()),
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

/// Build the formatting layer based on the provided configuration.
fn build_fmt_layer(
    config: &LogConfig,
) -> Box<
    dyn Layer<
            tracing_subscriber::layer::Layered<
                reload::Layer<EnvFilter, tracing_subscriber::Registry>,
                tracing_subscriber::Registry,
            >,
        > + Send
        + Sync,
> {
    match &config.target {
        LogTarget::Stdout => match config.format {
            LogFormat::Plain => fmt::layer()
                .compact()
                .with_file(false)
                .with_line_number(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_writer(BoxMakeWriter::new(std::io::stdout))
                .with_ansi(config.use_ansi)
                .boxed(),
            LogFormat::Json => fmt::layer()
                .compact()
                .with_file(false)
                .with_line_number(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_writer(BoxMakeWriter::new(std::io::stdout))
                .with_ansi(config.use_ansi)
                .json()
                .boxed(),
        },
        LogTarget::File(file) => {
            let file_clone = file.clone();
            match config.format {
                LogFormat::Plain => fmt::layer()
                    .compact()
                    .with_file(false)
                    .with_line_number(false)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_writer(BoxMakeWriter::new(move || {
                        rolling::daily(".", file_clone.clone())
                    }))
                    .with_ansi(false)
                    .boxed(),
                LogFormat::Json => {
                    let file_clone = file.clone();
                    fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(move || {
                            rolling::daily(".", file_clone.clone())
                        }))
                        .with_ansi(false)
                        .json()
                        .boxed()
                }
            }
        }
        LogTarget::Both(file) => {
            let file_clone = file.clone();
            match config.format {
                LogFormat::Plain => {
                    let stdout_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(std::io::stdout))
                        .with_ansi(config.use_ansi);
                    let file_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(move || {
                            rolling::daily(".", file_clone.clone())
                        }))
                        .with_ansi(false);
                    stdout_layer.and_then(file_layer).boxed()
                }
                LogFormat::Json => {
                    let stdout_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(std::io::stdout))
                        .with_ansi(config.use_ansi)
                        .json();
                    let file_layer = fmt::layer()
                        .compact()
                        .with_file(false)
                        .with_line_number(false)
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_writer(BoxMakeWriter::new(move || {
                            rolling::daily(".", file_clone.clone())
                        }))
                        .with_ansi(false)
                        .json();
                    stdout_layer.and_then(file_layer).boxed()
                }
            }
        }
    }
}

/// Update the formatting layer using the current configuration.
fn update_fmt_layer(config: &LogConfig) {
    if let Some(handle) = RELOAD_FMT_HANDLE.get().unwrap().lock().unwrap().as_ref() {
        let new_layer = build_fmt_layer(config);
        handle
            .modify(|layer| {
                *layer = new_layer;
            })
            .expect("Failed to update formatting layer");
    } else {
        eprintln!("Reload handle for formatting layer not initialized; call init() first.");
    }
}

/// Initialize the global tracing subscriber with the default configuration.
pub fn init() {
    init_with_config(LogConfig::default());
}

/// Initialize the global tracing subscriber with a given configuration.
pub fn init_with_config(config: LogConfig) {
    // Store the current config in a global for future updates.
    CURRENT_CONFIG.set(Mutex::new(config.clone())).ok();

    // Build an EnvFilter from the configured level.
    let env_filter = EnvFilter::new(config.clone().level);

    // Build the formatting layer based on the configuration.
    let fmt_layer = build_fmt_layer(&config);

    // Create a reloadable filter layer so we can update the level at runtime.
    let (filter_layer, filter_handle) = reload::Layer::new(env_filter);

    // Create a reloadable formatting layer so we can update the target/format at runtime.
    let (fmt_layer, fmt_handle) = reload::Layer::new(fmt_layer);

    // Build the subscriber registry.
    let subscriber = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global tracing subscriber");

    RELOAD_HANDLE.set(Mutex::new(Some(filter_handle))).unwrap();
    RELOAD_FMT_HANDLE.set(Mutex::new(Some(fmt_handle))).ok();
}

/// Change the log level at runtime.
pub fn set_level(new_level: &str) {
    if let Some(handle) = RELOAD_HANDLE.get().unwrap().lock().unwrap().as_ref() {
        handle
            .modify(|filter| *filter = EnvFilter::new(new_level))
            .expect("Failed to update log level");

        // Also update the stored config.
        if let Some(config_mutex) = CURRENT_CONFIG.get() {
            let mut config = config_mutex.lock().unwrap();
            config.level = new_level.to_string();
        }
    } else {
        eprintln!("Reload handle not initialized; call init() first.");
    }
}

/// Change the log target at runtime.
pub fn set_target(new_target: &str) {
    let target: LogTarget = match new_target {
        "stdout" => LogTarget::Stdout,
        "both" => LogTarget::Both(default_log_file()),
        path => LogTarget::File(path.to_string()),
    };
    if let Some(config_mutex) = CURRENT_CONFIG.get() {
        let mut config = config_mutex.lock().unwrap();
        config.target = target;
        update_fmt_layer(&config);
    } else {
        eprintln!("Current configuration not initialized; call init() first.");
    }
}

/// Change the log format at runtime.
pub fn set_format(new_format: &str) {
    let format = match new_format {
        "json" => LogFormat::Json,
        "plain" => LogFormat::Plain,
        _ => LogFormat::Json,
    };
    if let Some(config_mutex) = CURRENT_CONFIG.get() {
        let mut config = config_mutex.lock().unwrap();
        config.format = format;
        update_fmt_layer(&config);
    } else {
        eprintln!("Current configuration not initialized; call init() first.");
    }
}
pub fn set_target_filters(targets: Vec<(&str, Level)>) {
    if let Some(reload_handle_mutex) = RELOAD_HANDLE.get() {
        if let Ok(handle_guard) = reload_handle_mutex.lock() {
            if let Some(handle) = handle_guard.as_ref() {
                let mut new_filter = EnvFilter::new("");

                if let Some(config_mutex) = CURRENT_CONFIG.get() {
                    if let Ok(config) = config_mutex.lock() {
                        if let Ok(directive) = config.level.parse() {
                            new_filter = new_filter.add_directive(directive);
                        }
                    }
                }

                for (target, level) in targets {
                    let directive_str = format!("{}={}", target, level);
                    if let Ok(directive) = directive_str.parse() {
                        new_filter = new_filter.add_directive(directive);
                    }
                }

                if let Err(e) = handle.modify(|filter| *filter = new_filter) {
                    eprintln!("Failed to update filter with target directives: {}", e);
                }
            }
        }
    } else {
        eprintln!("Reload handle for filter not initialized; call init() first.");
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
