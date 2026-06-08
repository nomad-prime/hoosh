//! File-based logging for hoosh.
//!
//! Writes daily-rotated logs to `~/.config/hoosh/logs/hoosh.log`. Level is
//! controlled by `HOOSH_LOG` (preferred) or `RUST_LOG`, defaulting to `info`.
//!
//! The returned `LogGuard` must be kept alive for the lifetime of the program
//! — dropping it stops the background flush worker and truncates pending logs.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::ChronoLocal;

pub struct LogGuard(#[allow(dead_code)] WorkerGuard);

pub fn init_logging() -> Result<LogGuard> {
    let log_dir = log_dir().context("Failed to resolve log directory")?;
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create log dir at {}", log_dir.display()))?;

    let file_appender = rolling::daily(&log_dir, "hoosh.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_env("HOOSH_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_timer(ChronoLocal::new("%Y-%m-%dT%H:%M:%S%.3f%z".to_string()))
        .finish();

    // set_global_default fails if a subscriber is already installed (e.g. in
    // tests). That's not a fatal condition — just log to stderr and move on.
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Warning: tracing subscriber already set: {e}");
    }

    Ok(LogGuard(guard))
}

/// `~/.config/hoosh/logs` (or `$XDG_CONFIG_HOME/hoosh/logs`). Falls back to
/// the current directory if a home directory can't be resolved — better to
/// produce *some* log than to silently drop diagnostics.
fn log_dir() -> Result<PathBuf> {
    if let Some(base) = dirs::config_dir() {
        Ok(base.join("hoosh").join("logs"))
    } else {
        Ok(PathBuf::from(".hoosh-logs"))
    }
}
