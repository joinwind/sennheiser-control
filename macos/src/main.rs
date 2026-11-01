//! Sennheiser Control for macOS — Menu Bar & Liquid Glass companion for the Sennheiser MOMENTUM 4.
//!
//! Flags: `--minimized` (start in the Menu Bar), `--sim` (simulated headset), `--debug` (verbose log),
//! `--quit` (ask the running instance to exit), `--render-icons DIR` (generate macOS iconset + previews).

mod platform;
mod settings;
mod shared;
mod tray;
mod ui;

use std::io::Write;
use std::sync::Arc;

use engine::{Engine, EngineOptions};
use parking_lot::Mutex;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub const APP_NAME: &str = "Sennheiser Control";
pub const APP_DIR: &str = "SennheiserControl";

struct FileLogger {
    file: Mutex<std::fs::File>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= self.level
            && (m.target().starts_with("engine")
                || m.target().starts_with("bluetooth")
                || m.target().starts_with("headset")
                || m.target().starts_with("sennheiser")
                || m.level() <= log::Level::Warn)
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let _ = writeln!(
                self.file.lock(),
                "{t:.3} {:<5} {}: {}",
                r.level(),
                r.target(),
                r.args()
            );
        }
    }
    fn flush(&self) {
        let _ = self.file.lock().flush();
    }
}

fn init_logging(debug: bool) {
    let dir = settings::dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("app.log");
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > 2_000_000
    {
        let _ = std::fs::rename(&path, dir.join("app.old.log"));
    }
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let level = if debug {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        };
        if log::set_boxed_logger(Box::new(FileLogger {
            file: Mutex::new(file),
            level,
        }))
        .is_ok()
        {
            log::set_max_level(level);
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let (minimized, simulate) = (has("--minimized"), has("--sim"));
    if let Some(i) = args.iter().position(|a| a == "--render-icons") {
        let dir = std::path::PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or("."));
        tray::icon::dev_render(&dir);
        return Ok(());
    }
    init_logging(has("--debug"));
    settings::set_simulated(simulate);

    let shared_slot: Arc<Mutex<Option<Arc<shared::Shared>>>> = Arc::new(Mutex::new(None));
    if matches!(
        platform::claim_single_instance(has("--quit"), simulate, shared_slot.clone()),
        platform::Instance::Secondary
    ) {
        return Ok(());
    }

    log::info!(
        "starting {APP_NAME} (macOS Liquid Glass) {} (minimized={minimized}, sim={simulate})",
        env!("CARGO_PKG_VERSION")
    );

    let settings = settings::Settings::load();
    let icon = egui::IconData {
        rgba: tray::icon::app_icon(128),
        width: 128,
        height: 128,
    };
    let height = std::env::var("HC_DEV_HEIGHT")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(740.0);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("com.sennheiser-control.app")
            .with_inner_size([416.0, height])
            .with_min_inner_size([388.0, 500.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_visible(!minimized)
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            ui::theme::install_fonts(&cc.egui_ctx);
            if let Ok(h) = cc.window_handle()
                && let RawWindowHandle::AppKit(w) = h.as_raw()
            {
                platform::style_macos_window(w.ns_view.as_ptr() as usize);
            }
            let ctx = cc.egui_ctx.clone();
            let engine = Engine::start(
                EngineOptions {
                    simulate,
                    preferred_device: settings.preferred_device.clone(),
                },
                move || {
                    ctx.request_repaint();
                },
            );
            let (ui_tx, ui_rx) = crossbeam_channel::unbounded();
            let shared = Arc::new(shared::Shared::new(
                engine,
                settings,
                ui_tx,
                cc.egui_ctx.clone(),
            ));
            *shared_slot.lock() = Some(shared.clone());

            // On macOS, NSStatusItem & NSMenu must be created and updated on the main thread
            let tray_controller = tray::TrayController::new(shared.clone());
            let mut app = ui::HeadsetApp::new(shared, ui_rx, tray_controller, !minimized);
            match std::env::var("HC_DEV_PAGE").as_deref() {
                Ok("settings") => app.page = ui::Page::Settings,
                Ok("devices") => app.show_devices = true,
                Ok("panels") => app.open_dev_panels(),
                _ => {}
            }
            app.dev_scroll = std::env::var("HC_DEV_SCROLL")
                .ok()
                .and_then(|v| v.parse().ok());
            Ok(Box::new(app))
        }),
    )
}

