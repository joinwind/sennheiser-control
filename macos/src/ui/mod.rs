//! Main window for macOS with Liquid Glass UI styling and Menu Bar popover layout.

pub mod eq;
pub mod theme;
mod views;
pub mod widgets;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use egui::{Align, Align2, CornerRadius, Layout, Sense, UiBuilder, ViewportCommand, pos2, vec2};
use egui_phosphor::regular as ph;
use engine::{Action, AppState, LinkState};

use crate::settings::{Settings, ThemePref};
use crate::shared::{Shared, UiCmd};
use crate::tray::TrayController;
use theme::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Main,
    Settings,
}

/// A value the user is editing locally; shown instead of the device value until the device catches up.
struct Pending<T> {
    value: T,
    since: Instant,
    dragging: bool,
}

/// A multipoint connect/disconnect the user started; shown as in progress until the headset confirms.
#[derive(Clone, Copy)]
struct HostPending {
    index: u8,
    connect: bool,
    since: Instant,
}

const HOST_PENDING_TIMEOUT: Duration = Duration::from_secs(12);
const SEND_INTERVAL: Duration = Duration::from_millis(80);
const PENDING_GRACE: Duration = Duration::from_millis(1500);

pub struct HeadsetApp {
    shared: Arc<Shared>,
    rx: Receiver<UiCmd>,
    tray: Option<TrayController>,
    visible: bool,
    quitting: bool,
    pub page: Page,
    state: AppState,
    state_version: u64,
    settings: Settings,
    settings_version: u64,
    palette: Palette,
    theme_applied: bool,
    autostart: bool,
    transparency: Option<Pending<f32>>,
    eq_bands: Option<Pending<[f32; protocol::eq::BANDS]>>,
    last_send: Instant,
    new_preset_name: Option<String>,
    pub show_devices: bool,
    /// Dev aid: initial scroll offset (applied once).
    pub dev_scroll: Option<f32>,
    show_about: bool,
    show_gestures: bool,
    /// Title-bar logo: (pixel size, texture).
    logo: Option<(u32, egui::TextureHandle)>,
    host_pending: Option<HostPending>,
    /// Paired-list index whose "Forget device" confirmation is showing.
    confirm_forget: Option<u8>,
    /// Hi-Res enable warning is showing.
    confirm_hi_res: bool,
    /// Factory-reset confirmation is showing.
    confirm_reset: bool,
    user_moved: bool,
    needs_position: bool,
    focus_lost_at: Option<Instant>,
    was_focused: bool,
}

impl HeadsetApp {
    pub fn new(
        shared: Arc<Shared>,
        rx: Receiver<UiCmd>,
        tray: TrayController,
        visible: bool,
    ) -> Self {
        let settings = shared.settings();
        Self {
            state: shared.engine.snapshot(),
            state_version: u64::MAX,
            settings_version: shared.settings_version(),
            palette: Palette::DARK,
            theme_applied: false,
            autostart: crate::platform::autostart_enabled(),
            shared,
            rx,
            tray: Some(tray),
            visible,
            quitting: false,
            page: Page::Main,
            settings,
            transparency: None,
            eq_bands: None,
            last_send: Instant::now(),
            new_preset_name: None,
            show_devices: false,
            dev_scroll: None,
            show_about: false,
            show_gestures: false,
            logo: None,
            host_pending: None,
            confirm_forget: None,
            confirm_hi_res: false,
            confirm_reset: false,
            user_moved: false,
            needs_position: true,
            focus_lost_at: None,
            was_focused: false,
        }
    }

    /// Dev aid: open every collapsible panel and confirmation for screenshots.
    pub fn open_dev_panels(&mut self) {
        self.show_about = true;
        self.show_gestures = true;
        self.confirm_hi_res = true;
        self.confirm_reset = true;
        self.confirm_forget = Some(2);
    }

    fn act(&self, a: Action) {
        self.shared.dispatch(a);
    }

    fn set_visible(&mut self, ctx: &egui::Context, v: bool) {
        self.visible = v;
        if v {
            if !self.user_moved {
                self.needs_position = true;
            }
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.autostart = crate::platform::autostart_enabled();
            self.act(Action::Refresh);
        } else {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.show_devices = false;
        }
    }

    /// Position the popover window directly beneath the macOS Menu Bar on the right side.
    fn position_below_menubar(&mut self, ctx: &egui::Context) {
        let Some((left, top, right, _)) = crate::platform::screen_work_area() else {
            return;
        };
        let Some(outer) = ctx.input(|i| i.viewport().outer_rect) else {
            return;
        };
        let margin_x = 16.0;
        let margin_y = 10.0;
        let x = (right - outer.width() - margin_x).max(left);
        let y = top + margin_y;
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos2(x, y)));
        self.needs_position = false;
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        self.shared.quitting.store(true, Ordering::SeqCst);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn sync(&mut self, ctx: &egui::Context) {
        let v = self.shared.engine.version();
        if v != self.state_version {
            self.state_version = v;
            self.state = self.shared.engine.snapshot();
            if self.state.link == LinkState::Connected
                && let Some(id) = &self.state.active
                && self.settings.preferred_device.as_ref() != Some(id)
            {
                let id = id.clone();
                self.shared
                    .update_settings(|s| s.preferred_device = Some(id));
            }
        }
        let sv = self.shared.settings_version();
        if sv != self.settings_version {
            self.settings_version = sv;
            self.settings = self.shared.settings();
        }
        let dark = match self.settings.theme {
            ThemePref::Dark => true,
            ThemePref::Light => false,
            ThemePref::System => !crate::platform::apps_are_light(),
        };
        let p = if dark { Palette::DARK } else { Palette::LIGHT };
        if p != self.palette || !self.theme_applied {
            self.palette = p;
            self.theme_applied = true;
            theme::apply(ctx, &p);
        }
        if let Some(t) = &self.transparency
            && !t.dragging
            && (t.since.elapsed() > PENDING_GRACE
                || self.state.live().and_then(|h| h.transparency) == Some(t.value.round() as u8))
        {
            self.transparency = None;
        }
        if let Some(e) = &self.eq_bands
            && !e.dragging
            && (e.since.elapsed() > PENDING_GRACE
                || self.state.live().is_some_and(|h| {
                    h.eq.bands
                        .iter()
                        .zip(e.value)
                        .all(|(a, b)| (a - b).abs() < 0.051)
                }))
        {
            self.eq_bands = None;
        }
    }

    fn logo_texture(&mut self, ctx: &egui::Context, points: f32) -> egui::TextureId {
        let px = (points * ctx.pixels_per_point()).round().max(1.0) as u32;
        if self.logo.as_ref().is_none_or(|(size, _)| *size != px) {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [px as usize, px as usize],
                &crate::tray::icon::app_icon(px),
            );
            let tex = ctx.load_texture("app-logo", image, egui::TextureOptions::LINEAR);
            self.logo = Some((px, tex));
        }
        self.logo
            .as_ref()
            .map(|(_, t)| t.id())
            .expect("logo texture")
    }

    fn throttled(&mut self, force: bool) -> bool {
        if force || self.last_send.elapsed() >= SEND_INTERVAL {
            self.last_send = Instant::now();
            true
        } else {
            false
        }
    }

    /// Title bar styled with native macOS traffic lights and Liquid Glass toolbar items.
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let (bar, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click_and_drag());
        if resp.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            self.user_moved = true;
        }

        // Left: macOS Traffic Lights (Close, Minimize, Zoom)
        let tl_rect = egui::Rect::from_min_size(pos2(bar.left() + 14.0, bar.center().y - 8.0), vec2(60.0, 16.0));
        let mut tl_ui = ui.new_child(UiBuilder::new().max_rect(tl_rect));
        let (tl_close, tl_min, _) = widgets::traffic_lights(&mut tl_ui, &p);

        if tl_close {
            if self.settings.close_to_tray {
                self.set_visible(&ctx, false);
            } else {
                self.quit(&ctx);
            }
        }
        if tl_min {
            self.set_visible(&ctx, false);
        }

        // Center: App title and subtle icon
        let center = bar.center();
        let logo = self.logo_texture(&ctx, 20.0);
        let icon_rect = egui::Rect::from_center_size(pos2(center.x - 68.0, center.y), vec2(20.0, 20.0));
        ui.painter().image(
            logo,
            icon_rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().text(
            pos2(center.x + 8.0, center.y),
            Align2::CENTER_CENTER,
            crate::APP_NAME,
            theme::semibold(13.5),
            p.text,
        );

        // Right: Settings / Back navigation button
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(bar.shrink2(vec2(12.0, 0.0)))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                let (glyph, tip) = if self.page == Page::Settings {
                    (ph::ARROW_LEFT, "Back")
                } else {
                    (ph::GEAR_SIX, "Settings")
                };
                if widgets::ghost_button(ui, &p, Some(glyph), tip).clicked() {
                    self.page = if self.page == Page::Settings {
                        Page::Main
                    } else {
                        Page::Settings
                    };
                }
            },
        );
    }

    fn toast(&self, ctx: &egui::Context) {
        let p = self.palette;
        let Some(msg) = &self.state.last_error else {
            return;
        };
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -18.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(if p.dark {
                        egui::Color32::from_rgb(0x2A, 0x1C, 0x20)
                    } else {
                        egui::Color32::from_rgb(0xFF, 0xEE, 0xEF)
                    })
                    .stroke(egui::Stroke::new(1.0, p.bad.gamma_multiply(0.5)))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(340.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(ph::WARNING_CIRCLE)
                                    .font(theme::icon(16.0))
                                    .color(p.bad),
                            );
                            ui.label(
                                egui::RichText::new(msg)
                                    .font(theme::regular(13.0))
                                    .color(p.text),
                            );
                        });
                    });
            });
    }
}

impl eframe::App for HeadsetApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll macOS Menu Bar tray events on the main thread
        if let Some(tray) = &mut self.tray {
            tray.poll();
        }

        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                UiCmd::Show => self.set_visible(ctx, true),
                UiCmd::Toggle => {
                    let in_front = self.was_focused
                        || self
                            .focus_lost_at
                            .is_some_and(|t| t.elapsed() < Duration::from_millis(500));
                    let show = !(self.visible && in_front);
                    self.set_visible(ctx, show);
                }
                UiCmd::OpenSettings => {
                    self.page = Page::Settings;
                    self.set_visible(ctx, true);
                }
                UiCmd::Quit => self.quit(ctx),
            }
        }

        let (close, minimized, focused) = ctx.input(|i| {
            let v = i.viewport();
            (v.close_requested(), v.minimized, v.focused)
        });
        let focused = focused.unwrap_or(false) && self.visible;
        if self.was_focused && !focused {
            self.focus_lost_at = Some(Instant::now());
        }
        self.was_focused = focused;
        if close && !self.quitting {
            if self.settings.close_to_tray {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.set_visible(ctx, false);
            } else {
                self.quitting = true;
                self.shared.quitting.store(true, Ordering::SeqCst);
            }
        }
        if minimized == Some(true) && self.visible {
            self.set_visible(ctx, false);
        }
        self.sync(ctx);
        if self.visible && self.needs_position {
            self.position_below_menubar(ctx);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let p = self.palette;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg).corner_radius(CornerRadius::same(18)))
            .show(ui, |ui| {
                // Draw soft ambient glowing backdrop behind translucent glass cards
                let canvas_rect = ui.max_rect();
                widgets::ambient_backdrop(ui.painter(), canvas_rect, &p);

                self.title_bar(ui);
                let mut scroll = egui::ScrollArea::vertical().auto_shrink([false; 2]);
                let synced = self
                    .state
                    .live()
                    .is_some_and(|h| !h.paired_hosts.is_empty());
                if synced && let Some(y) = self.dev_scroll.take() {
                    scroll = scroll.vertical_scroll_offset(y);
                }
                scroll.show(ui, |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 14,
                            right: 14,
                            top: 4,
                            bottom: 18,
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 12.0;
                            match self.page {
                                Page::Main => self.main_page(ui),
                                Page::Settings => self.settings_page(ui),
                            }
                        });
                });
            });
        self.toast(ui.ctx());
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.quitting.store(true, Ordering::SeqCst);
        self.shared.engine.shutdown();
        self.tray = None;
    }
}

