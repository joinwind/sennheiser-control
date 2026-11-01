//! Main-thread Menu Bar controller (`NSStatusItem` + `NSMenu`).
//!
//! Unlike Windows, macOS `AppKit` requires `NSStatusItem` and `NSMenu` to live on the main thread.
//! `TrayController::poll()` is called from `HeadsetApp::logic()` whenever state or settings change.

pub mod icon;
pub mod menu;

use std::sync::Arc;

use engine::{AppState, LinkState};
use tray_icon::menu::MenuEvent;
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::shared::{Shared, UiCmd};
use icon::{IconRenderer, TrayVisual};

/// Retina 2x Menu Bar icon size (22 pt × 2 = 44 px).
const MENUBAR_ICON_PX: u32 = 44;

fn visual(state: &AppState) -> TrayVisual {
    if let Some(h) = state.live() {
        return TrayVisual::Battery {
            percent: h.battery,
            charging: h.charging_now(),
        };
    }
    let connected = state.active_device().is_some_and(|d| d.connected);
    match &state.link {
        LinkState::Connecting | LinkState::Retrying(_) if connected => TrayVisual::Connecting,
        _ => TrayVisual::Disconnected,
    }
}

pub struct TrayController {
    shared: Arc<Shared>,
    renderer: IconRenderer,
    tray: Option<TrayIcon>,
    rendered_visual: TrayVisual,
    rendered_light: bool,
    seen: (u64, u64, bool),
    low_warned: bool,
}

impl TrayController {
    pub fn new(shared: Arc<Shared>) -> Self {
        let renderer = IconRenderer::new();
        let state = shared.engine.snapshot();
        let settings = shared.settings();
        let autostart = crate::platform::autostart_enabled();
        let vis = visual(&state);
        let light = crate::platform::menubar_is_light();

        let icon = Icon::from_rgba(
            renderer.tray(vis, MENUBAR_ICON_PX, light),
            MENUBAR_ICON_PX,
            MENUBAR_ICON_PX,
        )
        .ok();

        let tray = match icon {
            Some(ic) => TrayIconBuilder::new()
                .with_tooltip(menu::tooltip(&state))
                .with_icon(ic)
                .with_menu(Box::new(menu::build(&state, &settings, autostart)))
                .with_menu_on_left_click(false)
                .build()
                .map_err(|e| log::error!("menu bar icon failed: {e}"))
                .ok(),
            None => None,
        };

        {
            let shared = shared.clone();
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = e
                {
                    shared.send_ui(UiCmd::Toggle);
                }
            }));
        }
        {
            let shared = shared.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| menu::handle(&e.id.0, &shared)));
        }

        Self {
            seen: (shared.engine.version(), shared.settings_version(), autostart),
            shared,
            renderer,
            tray,
            rendered_visual: vis,
            rendered_light: light,
            low_warned: false,
        }
    }

    pub fn poll(&mut self) {
        let Some(tray) = &self.tray else { return };
        let autostart = crate::platform::autostart_enabled();
        let now = (
            self.shared.engine.version(),
            self.shared.settings_version(),
            autostart,
        );
        let light = crate::platform::menubar_is_light();
        if now == self.seen && light == self.rendered_light {
            return;
        }
        let state = self.shared.engine.snapshot();
        let next_vis = visual(&state);
        if next_vis != self.rendered_visual || light != self.rendered_light {
            self.rendered_visual = next_vis;
            self.rendered_light = light;
            let rgba = self.renderer.tray(next_vis, MENUBAR_ICON_PX, light);
            let _ = tray.set_icon(Icon::from_rgba(rgba, MENUBAR_ICON_PX, MENUBAR_ICON_PX).ok());
        }
        if now != self.seen {
            self.seen = now;
            let settings = self.shared.settings();
            let _ = tray.set_tooltip(Some(menu::tooltip(&state)));
            tray.set_menu(Some(Box::new(menu::build(&state, &settings, autostart))));
            self.check_low_battery(&state, settings.low_battery_threshold);
        }
    }

    fn check_low_battery(&mut self, state: &AppState, threshold: u8) {
        let Some(h) = state.live() else { return };
        let Some(p) = h.battery else { return };
        if threshold == 0 || h.charging_now() || p > threshold {
            self.low_warned = false;
            return;
        }
        if self.low_warned {
            return;
        }
        self.low_warned = true;
        let name = state
            .active_device()
            .map(|d| d.name.as_str())
            .unwrap_or("Headset");
        crate::platform::notify_low_battery(
            &format!("{name} Battery Low"),
            &format!("{p}% remaining — connect to power soon."),
        );
    }
}

