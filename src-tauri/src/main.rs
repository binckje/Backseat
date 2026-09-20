#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use log::info;

fn main() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
            if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
                std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            }
        }
    }

    let verbose = backseat_lib::logger::is_verbose_requested()
        || backseat_lib::settings::load().debug_logging.unwrap_or(false);

    backseat_lib::logger::init(verbose);
    info!("Starting Backseat...");
    backseat_lib::run()
}
