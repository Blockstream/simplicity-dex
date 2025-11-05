use std::io;

use tracing::{level_filters::LevelFilter, trace};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug)]
pub struct LoggerGuard {
    _std_out_guard: WorkerGuard,
    _std_err_guard: WorkerGuard,
}

pub fn init_logger() -> LoggerGuard {
    let (std_out_writer, std_out_guard) = tracing_appender::non_blocking(io::stdout());
    let (std_err_writer, std_err_guard) = tracing_appender::non_blocking(io::stderr());
    let std_out_layer = fmt::layer()
        .with_writer(std_out_writer)
        .with_target(false)
        .with_level(true)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("TRACE")));

    let std_err_layer = fmt::layer()
        .with_writer(std_err_writer)
        .with_target(false)
        .with_level(true)
        .with_filter(LevelFilter::WARN);

    tracing_subscriber::registry()
        .with(std_out_layer)
        .with(std_err_layer)
        .init();

    trace!("Logger successfully initialized!");
    LoggerGuard {
        _std_out_guard: std_out_guard,
        _std_err_guard: std_err_guard,
    }
}
