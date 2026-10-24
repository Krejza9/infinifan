#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod lang;
mod app;
mod icons;
mod background;
mod config;
mod curve;
mod device;
mod effects;
mod fxsim;
mod icon;
mod ipc;
mod lconnect_import;
mod pawnio_setup;
mod paths;
mod preview;
mod sensors;
mod system;
mod theme;
mod widgets;
mod worker;

use std::time::{Duration, Instant};

use eframe::egui;

fn init_logging(name: &str) {
    let mut b = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if let Some(f) = paths::open_log(name) {
        b.target(env_logger::Target::Pipe(Box::new(f)));
    }
    b.init();
}

/// Run modes:
/// * `--background` (also the older `--minimized` from autostart): background process – control and
///   tray icon, no graphics;
/// * `--gui`: window that connects to the background process and exits when closed;
/// * no arguments: starts the background process (if not running) and opens the window;
/// * `--demo`: window without a controller, with made-up values (UI preview).
fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let has = |a: &str| args.iter().any(|x| x == a);

    // Texts come only from the locales folder; without the base language there is nothing to show.
    if let Err(msg) = lang::load() {
        system::error_box("InfiniFan", &msg);
        return Ok(());
    }

    if has("--background") || has("--minimized") {
        init_logging("infinifan");
        // After a restart as administrator, wait a while in case the old instance is still exiting.
        let wait = if has("--elevated-restart") { Duration::from_secs(30) } else { Duration::ZERO };
        let start = Instant::now();
        while !system::acquire_single_instance() {
            if start.elapsed() >= wait {
                log::info!("Background process already running");
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        background::run(has("--open-gui"), has("--demo"));
        return Ok(());
    }

    init_logging("infinifan-gui");
    let conn = if has("--demo") {
        None
    } else {
        if !system::background_running() {
            // A window without control makes no sense: start the background process, it opens the window.
            system::spawn_background(&["--open-gui"]);
            return Ok(());
        }
        if !system::acquire_gui_instance() {
            log::info!("Window already open");
            return Ok(());
        }
        match ipc::connect(Duration::from_secs(10)) {
            Some(c) => Some(c),
            None => {
                log::error!("Could not connect to the background process");
                return Ok(());
            }
        }
    };

    let cfg = config::AppConfig::load();
    lang::apply(cfg.language.as_deref());
    let icon = egui::IconData { rgba: icon::icon_rgba(64), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("InfiniFan – SL Infinity")
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([980.0, 620.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native("InfiniFan", options, Box::new(|cc| Ok(Box::new(app::App::new(cc, cfg, conn)))))
}
