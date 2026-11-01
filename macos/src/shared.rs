//! State shared between the UI and the Menu Bar controller.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crossbeam_channel::Sender;
use engine::{Action, Engine};
use parking_lot::Mutex;

use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCmd {
    Show,
    Toggle,
    OpenSettings,
    Quit,
}

pub struct Shared {
    pub engine: Engine,
    settings: Mutex<Settings>,
    settings_version: AtomicU64,
    pub ui_tx: Sender<UiCmd>,
    pub egui_ctx: egui::Context,
    pub quitting: AtomicBool,
}

impl Shared {
    pub fn new(
        engine: Engine,
        settings: Settings,
        ui_tx: Sender<UiCmd>,
        egui_ctx: egui::Context,
    ) -> Self {
        Self {
            engine,
            settings: Mutex::new(settings),
            settings_version: AtomicU64::new(0),
            ui_tx,
            egui_ctx,
            quitting: AtomicBool::new(false),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().clone()
    }

    pub fn settings_version(&self) -> u64 {
        self.settings_version.load(Ordering::SeqCst)
    }

    /// Mutate, persist and broadcast settings.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let new = {
            let mut s = self.settings.lock();
            let before = s.clone();
            f(&mut s);
            if *s == before {
                return;
            }
            s.clone()
        };
        new.save();
        self.settings_version.fetch_add(1, Ordering::SeqCst);
        self.wake_all();
    }

    pub fn send_ui(&self, cmd: UiCmd) {
        let _ = self.ui_tx.send(cmd);
        self.egui_ctx.request_repaint();
    }

    pub fn wake_all(&self) {
        self.egui_ctx.request_repaint();
    }

    pub fn dispatch(&self, a: Action) {
        self.engine.dispatch(a);
    }
}

/// Wording and order shared by the popover window and the Menu Bar menu.
pub const WIND_ORDER: [protocol::AntiWind; 3] = [
    protocol::AntiWind::Off,
    protocol::AntiWind::Auto,
    protocol::AntiWind::Max,
];
pub const SIDETONE_LABELS: [&str; 5] = ["Off", "Low", "Medium", "High", "Max"];
pub const PROMPTS_TITLE: &str = "Tones and voice prompts";
pub const SIDETONE_TITLE: &str = "Sidetone";
pub const COMFORT_CALLS_TITLE: &str = "Comfort calls";

/// Name for a Custom-mode balance: below 50 is noise cancelling of strength (50 − level) × 2,
/// above 50 is transparency of (level − 50) × 2.
pub fn custom_level_label(level: u8) -> String {
    let level = level.min(100);
    match level {
        0 => "Max Noise Cancellation".into(),
        50 => "Neutral".into(),
        100 => "Max Transparency".into(),
        l if l < 50 => format!("Noise Cancellation {}%", (50 - l) * 2),
        l => format!("Transparency {}%", (l - 50) * 2),
    }
}

