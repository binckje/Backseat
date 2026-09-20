use log::LevelFilter;
use simplelog::*;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use log::{info};

/// Path to the log file (platform-specific)
pub fn log_path() -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("backseat").join("backseat.log")
}

/// Initialize logging. Called once at startup.
///
/// Verbose mode is activated by any of:
///   - `--verbose` or `--debug` CLI argument
///   - `BACKSEAT_LOG=debug` environment variable
///   - `debug_logging: true` in the config file
pub fn init(verbose: bool) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let level = if verbose { LevelFilter::Debug } else { LevelFilter::Info };

    // Rotate: keep a .old backup of the previous log
    if path.exists() {
        let old = path.with_extension("old.log");
        std::fs::rename(&path, old).ok();
    }

    let file = File::create(&path)
        .or_else(|_| OpenOptions::new().append(true).create(true).open(&path));

    let config = ConfigBuilder::new()
        .set_time_format_rfc3339()
        .set_target_level(LevelFilter::Debug)
        .build();

    match file {
        Ok(f) => {
            WriteLogger::init(level, config, f).ok();
        }
        Err(_) => {
            // Fallback: no file logging if we can't create the file
            TermLogger::init(level, config, TerminalMode::Mixed, ColorChoice::Auto).ok();
        }
    }

    info!("╔══════════════════════════════════════╗");
    info!("║  Backseat v{}  |  verbose={}  ║",
        env!("CARGO_PKG_VERSION"), verbose);
    info!("╚══════════════════════════════════════╝");
    info!("Log file: {}", log_path().display());
    info!("OS: {} {}", std::env::consts::OS, std::env::consts::ARCH);
}

/// Returns true if verbose/debug logging is requested via CLI or env var.
pub fn is_verbose_requested() -> bool {
    let cli = std::env::args().any(|a| a == "--verbose" || a == "--debug" || a == "-v");
    let env = std::env::var("BACKSEAT_LOG")
        .map(|v| matches!(v.to_lowercase().as_str(), "debug" | "verbose" | "1" | "true"))
        .unwrap_or(false);
    cli || env
}
