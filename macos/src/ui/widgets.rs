//! macOS 27 "Liquid Glass" widgets:
//! - Ambient glowing backdrop (subtle radial light orbs behind translucent frosted cards)
//! - Translucent frosted glass cards with 1px specular top highlight and soft shadows
//! - macOS native traffic lights (Close, Minimize, Zoom) with hover glyphs
//! - Liquid Glass capsule segmented controls with floating specular pills
//! - macOS Control Center style capsule switches and balance sliders
//! - Battery ring, status pills, and SF-styled buttons

use std::f32::consts::{PI, TAU};

use egui::{
    Align, Align2, Color32, CornerRadius, Layout, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke,
    Ui, Vec2, pos2, vec2,
};
use egui_phosphor::regular as ph;

use super::theme::{Palette, bold, icon, regular, semibold};

const ANIM: f32 = 0.16;

/// Soft radial light orbs drawn in the window background to provide authentic refraction
/// through translucent frosted Liquid Glass cards.
pub fn ambient_backdrop(painter: &egui::Painter, rect: Rect, p: &Palette) {
    let mut mesh = Mesh::default();
    let center1 = pos2(rect.right() - 80.0, rect.top() + 100.0);
    let color1 = p.accent.gamma_multiply(if p.dark { 0.16 } else { 0.10 });
    radial_gradient(&mut mesh, center1, 220.0, color1, Color32::TRANSPARENT);

    let center2 = pos2(rect.left() + 70.0, rect.top() + 380.0);
    let color2 = p.accent_secondary.gamma_multiply(if p.dark { 0.14 } else { 0.08 });
    radial_gradient(&mut mesh, center2, 260.0, color2, Color32::TRANSPARENT);

    painter.add(Shape::mesh(mesh));
}

fn radial_gradient(mesh: &mut Mesh, center: Pos2, radius: f32, inner: Color32, outer: Color32) {
    let segments = 32;
    let center_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, inner);
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * TAU;
        let pos = pos2(center.x + angle.cos() * radius, center.y + angle.sin() * radius);
        mesh.colored_vertex(pos, outer);
    }
    for i in 0..segments {
        mesh.add_triangle(center_idx, center_idx + 1 + i, center_idx + 2 + i);
    }
}

/// Translucent frosted glass card with 1px specular top rim highlight, soft shadow,
/// and rounded corners (macOS 27 Liquid Glass aesthetic).
pub fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let card_radius = CornerRadius::same(18);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.glass_rim_side))
        .corner_radius(card_radius)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let r = ui.max_rect();
            // Draw 1px specular top-edge highlight on the glass surface
            let top_left = pos2(r.left() + 16.0, r.top());
            let top_right = pos2(r.right() - 16.0, r.top());
            ui.painter().line_segment(
                [top_left, top_right],
                Stroke::new(1.0, p.glass_rim_top),
            );
            add(ui)
        })
        .inner
}

/// macOS Traffic Lights (Close, Minimize, Zoom) on the top-left of the window.
/// Displays glyphs when hovered, replicating macOS Cocoa window buttons.
pub fn traffic_lights(
    ui: &mut Ui,
    p: &Palette,
) -> (bool, bool, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(60.0, 16.0), Sense::hover());
    let hovered = resp.hovered();
    let r = 6.0;
    let y = rect.center().y;

    let c_close = pos2(rect.left() + 8.0, y);
    let c_min = pos2(rect.left() + 26.0, y);
    let c_zoom = pos2(rect.left() + 44.0, y);

    let hit_close = Rect::from_center_size(c_close, vec2(14.0, 14.0));
    let hit_min = Rect::from_center_size(c_min, vec2(14.0, 14.0));
    let hit_zoom = Rect::from_center_size(c_zoom, vec2(14.0, 14.0));

    let resp_close = ui.interact(hit_close, ui.id().with("tl_close"), Sense::click());
    let resp_min = ui.interact(hit_min, ui.id().with("tl_min"), Sense::click());
    let resp_zoom = ui.interact(hit_zoom, ui.id().with("tl_zoom"), Sense::click());

    let painter = ui.painter();
    for (center, color) in [(c_close, p.tl_close), (c_min, p.tl_min), (c_zoom, p.tl_zoom)] {
        painter.circle_filled(center, r, color);
        painter.circle_stroke(center, r, Stroke::new(0.5, Color32::from_black_alpha(40)));
    }

    if hovered {
        let glyph_col = Color32::from_black_alpha(160);
        painter.text(c_close, Align2::CENTER_CENTER, "×", bold(11.0), glyph_col);
        painter.text(c_min, Align2::CENTER_CENTER, "−", bold(11.0), glyph_col);
        painter.text(c_zoom, Align2::CENTER_CENTER, "+", bold(9.0), glyph_col);
    }

    (resp_close.clicked(), resp_min.clicked(), resp_zoom.clicked())
}

pub fn section_title(ui: &mut Ui, p: &Palette, glyph: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(egui::RichText::new(glyph).font(icon(17.0)).color(p.accent));
        ui.label(
            egui::RichText::new(title)
                .font(semibold(15.0))
                .color(p.text),
        );
    });
    ui.add_space(4.0);
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    ui.add_space(2.0);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, p.border);
    ui.add_space(2.0);
}

/// macOS Control Center capsule switch. Returns a response that is `changed()` when flipped.
pub fn toggle(ui: &mut Ui, p: &Palette, on: &mut bool, enabled: bool) -> Response {
    let size = vec2(42.0, 24.0);
    let (rect, mut resp) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, ANIM);
    let bg = lerp_color(p.track(), p.accent, t);
    let alpha = if enabled { 1.0 } else { 0.4 };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(12), bg.gamma_multiply(alpha));
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);

    let r = 9.0;
    let cx = egui::lerp((rect.left() + 3.0 + r)..=(rect.right() - 3.0 - r), t);
    let knob_center = pos2(cx, rect.center().y);
    painter.circle_filled(
        knob_center + vec2(0.0, 1.0),
        r,
        Color32::from_black_alpha(45),
    );
    painter.circle_filled(
        knob_center,
        r,
        Color32::WHITE.gamma_multiply(alpha),
    );
    // Subtle specular ring on the knob
    painter.circle_stroke(
        knob_center,
        r,
        Stroke::new(0.5, Color32::from_white_alpha(180)),
    );

    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Liquid Glass capsule segmented control with a floating frosted glass pill.
pub fn segmented(
    ui: &mut Ui,
    p: &Palette,
    id_salt: &str,
    labels: &[&str],
    selected: Option<usize>,
    enabled: bool,
    height: f32,
) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    let id = ui.id().with(id_salt);
    let n = labels.len().max(1) as f32;
    let inner = rect.shrink(3.0);
    let seg_w = inner.width() / n;
    let painter = ui.painter();
    let radius = CornerRadius::same((height / 2.0).min(14.0) as u8);

    painter.rect_filled(rect, radius, p.surface2);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);

    let alpha = if enabled { 1.0 } else { 0.45 };
    if let Some(sel) = selected {
        let x = ui
            .ctx()
            .animate_value_with_time(id.with("pill"), sel as f32, ANIM);
        let pill = Rect::from_min_size(
            pos2(inner.left() + x * seg_w, inner.top()),
            vec2(seg_w, inner.height()),
        );
        let pill_radius = CornerRadius::same(((height - 6.0) / 2.0).min(11.0) as u8);
        painter.rect_filled(pill, pill_radius, p.pill_active.gamma_multiply(alpha));
        painter.rect_stroke(pill, pill_radius, Stroke::new(1.0, p.glass_rim_top), egui::StrokeKind::Inside);
    }
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(inner.left() + i as f32 * seg_w, inner.top()),
            vec2(seg_w, inner.height()),
        );
        let resp = ui.interact(
            r,
            id.with(i),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let is_sel = selected == Some(i);
        let color = if is_sel {
            p.text
        } else if resp.hovered() && enabled {
            p.text
        } else {
            p.text_dim
        };
        let font = if is_sel {
            semibold(13.0)
        } else {
            regular(13.0)
        };
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            font,
            color.gamma_multiply(alpha),
        );
        if enabled && resp.clicked() {
            clicked = Some(i);
        }
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    clicked
}

/// macOS Control Center style thick glass balance slider with a luminous knob and center notch.
pub fn balance_slider(ui: &mut Ui, p: &Palette, value: &mut f32, enabled: bool) -> Response {
    const SNAP: f32 = 3.0;
    let h = 28.0;
    let w = ui.available_width();
    let (rect, mut resp) = ui.allocate_exact_size(
        vec2(w, h),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 12.0, 10.0));
    if enabled
        && (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let mut v = (((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) * 100.0).round();
        if (v - 50.0).abs() <= SNAP {
            v = 50.0;
        }
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            resp.mark_changed();
        }
    }
    let t = (*value / 100.0).clamp(0.0, 1.0);
    let alpha = if enabled { 1.0 } else { 0.4 };
    let painter = ui.painter();
    painter.rect_filled(track, CornerRadius::same(5), p.track());
    painter.rect_stroke(track, CornerRadius::same(5), Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);

    let centre = track.center().x;
    let knob_x = track.left() + t * track.width();
    let filled = Rect::from_min_max(
        pos2(centre.min(knob_x), track.min.y),
        pos2(centre.max(knob_x), track.max.y),
    );
    painter.rect_filled(
        filled,
        CornerRadius::same(5),
        p.accent.gamma_multiply(alpha),
    );
    // Center notch mark
    painter.line_segment(
        [
            pos2(centre, track.top() - 4.0),
            pos2(centre, track.bottom() + 4.0),
        ],
        Stroke::new(1.5, p.text_faint.gamma_multiply(alpha)),
    );

    let knob = pos2(knob_x, track.center().y);
    let grow = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("k"), resp.hovered() || resp.dragged(), ANIM);
    painter.circle_filled(
        knob + vec2(0.0, 1.5),
        10.0 + grow * 1.5,
        Color32::from_black_alpha(45),
    );
    painter.circle_filled(knob, 10.0 + grow * 1.5, Color32::WHITE.gamma_multiply(alpha));
    painter.circle_stroke(
        knob,
        10.0 + grow * 1.5,
        Stroke::new(1.0, Color32::from_white_alpha(200)),
    );
    painter.circle_filled(knob, 4.0, p.accent.gamma_multiply(alpha));

    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Setting row with Liquid Glass icon container.
pub fn setting_row(
    ui: &mut Ui,
    p: &Palette,
    glyph: &str,
    title: &str,
    subtitle: Option<&str>,
    enabled: bool,
    trailing: impl FnOnce(&mut Ui),
) {
    let alpha = if enabled { 1.0 } else { 0.45 };
    ui.horizontal(|ui| {
        ui.set_min_height(if subtitle.is_some() { 42.0 } else { 32.0 });
        let (ir, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
        ui.painter()
            .rect_filled(ir, CornerRadius::same(10), p.surface2);
        ui.painter()
            .rect_stroke(ir, CornerRadius::same(10), Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);
        ui.painter().text(
            ir.center(),
            Align2::CENTER_CENTER,
            glyph,
            icon(16.0),
            p.text_dim.gamma_multiply(alpha),
        );
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            trailing(ui);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                if subtitle.is_some() {
                    ui.add_space(2.0);
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(title)
                            .font(regular(14.0))
                            .color(p.text.gamma_multiply(alpha)),
                    )
                    .truncate(),
                );
                if let Some(s) = subtitle {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(s)
                                .font(regular(12.0))
                                .color(p.text_faint),
                        )
                        .wrap(),
                    );
                }
            });
        });
    });
}

pub fn stacked_row(
    ui: &mut Ui,
    p: &Palette,
    glyph: &str,
    title: &str,
    value: Option<&str>,
    add: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        let (ir, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
        ui.painter()
            .rect_filled(ir, CornerRadius::same(10), p.surface2);
        ui.painter()
            .rect_stroke(ir, CornerRadius::same(10), Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);
        ui.painter().text(
            ir.center(),
            Align2::CENTER_CENTER,
            glyph,
            icon(16.0),
            p.text_dim,
        );
        ui.add_space(6.0);
        ui.label(egui::RichText::new(title).font(regular(14.0)).color(p.text));
        if let Some(v) = value {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(v)
                        .font(semibold(13.0))
                        .color(p.text_dim),
                );
            });
        }
    });
    ui.add_space(3.0);
    add(ui);
}

pub fn chip(ui: &mut Ui, p: &Palette, glyph: Option<&str>, text: &str) {
    let font = regular(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, p.text_dim);
    let icon_w = if glyph.is_some() { 16.0 } else { 0.0 };
    let size = vec2(galley.size().x + 18.0 + icon_w, 24.0);
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(r, CornerRadius::same(12), p.surface2);
    ui.painter()
        .rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);
    let mut x = r.left() + 9.0;
    if let Some(g) = glyph {
        ui.painter().text(
            pos2(x + 6.0, r.center().y),
            Align2::CENTER_CENTER,
            g,
            icon(13.0),
            p.text_dim,
        );
        x += icon_w;
    }
    ui.painter().galley(
        pos2(x, r.center().y - galley.size().y / 2.0),
        galley,
        p.text_dim,
    );
}

pub fn status_dot(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 3.5, color);
}

/// Circular battery gauge with glowing arc.
pub fn battery_ring(
    ui: &mut Ui,
    p: &Palette,
    percent: Option<u8>,
    charging: bool,
    size: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let radius = size / 2.0 - 5.0;
    let stroke_w = 5.0;
    let painter = ui.painter();
    painter.circle_stroke(c, radius, Stroke::new(stroke_w, p.track()));
    if let Some(pct) = percent {
        let t = ui
            .ctx()
            .animate_value_with_time(resp.id.with("b"), pct as f32 / 100.0, 0.5);
        let color = if charging { p.good } else { p.level(pct) };
        arc(
            painter,
            c,
            radius,
            -PI / 2.0,
            -PI / 2.0 + TAU * t,
            Stroke::new(stroke_w, color),
        );
        painter.text(
            c + vec2(0.0, if charging { -3.0 } else { 0.0 }),
            Align2::CENTER_CENTER,
            format!("{pct}%"),
            bold(15.0),
            p.text,
        );
        if charging {
            painter.text(
                c + vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                ph::LIGHTNING,
                icon(11.0),
                p.good,
            );
        }
    } else {
        painter.text(c, Align2::CENTER_CENTER, "—", semibold(15.0), p.text_dim);
    }
    resp
}

pub fn arc(painter: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let n = ((a1 - a0).abs() / TAU * 72.0).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    painter.add(Shape::line(pts.clone(), stroke));
    painter.circle_filled(pts[0], stroke.width / 2.0, stroke.color);
    painter.circle_filled(*pts.last().unwrap(), stroke.width / 2.0, stroke.color);
}

/// Headphone illustration in macOS Liquid Glass style with soft specular rims.
pub fn headphones_art(painter: &egui::Painter, rect: Rect, p: &Palette, dim: bool) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let alpha = if dim { 0.45 } else { 1.0 };
    painter.circle_filled(c, s * 0.48, p.accent_soft().gamma_multiply(alpha));
    painter.circle_stroke(
        c,
        s * 0.48,
        Stroke::new(1.0, p.glass_rim_top.gamma_multiply(alpha)),
    );

    let band_r = s * 0.27;
    let band_c = c + vec2(0.0, s * 0.05);
    let band_col = lerp_color(p.text, p.accent, 0.35).gamma_multiply(alpha);
    arc(
        painter,
        band_c,
        band_r,
        PI,
        TAU,
        Stroke::new(s * 0.055, band_col),
    );
    let cup = vec2(s * 0.15, s * 0.26);
    for side in [-1.0f32, 1.0] {
        let cx = band_c.x + side * band_r;
        let r = Rect::from_center_size(pos2(cx, band_c.y + s * 0.06), cup);
        painter.rect_filled(
            r,
            CornerRadius::same((s * 0.06) as u8),
            lerp_color(p.accent, p.accent_secondary, 0.4).gamma_multiply(alpha),
        );
        let pad = Rect::from_center_size(
            pos2(cx - side * cup.x * 0.55, r.center().y),
            vec2(cup.x * 0.35, cup.y * 0.78),
        );
        painter.rect_filled(pad, CornerRadius::same((s * 0.03) as u8), band_col);
    }
}

pub fn primary_button(ui: &mut Ui, p: &Palette, glyph: Option<&str>, text: &str) -> Response {
    let font = semibold(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, p.on_accent);
    let iw = if glyph.is_some() { 22.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 32.0 + iw, 36.0), Sense::click());
    let fill = if resp.hovered() {
        lerp_color(p.accent, Color32::WHITE, 0.12)
    } else {
        p.accent
    };
    ui.painter().rect_filled(r, CornerRadius::same(12), fill);
    ui.painter().rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0, p.glass_rim_top), egui::StrokeKind::Inside);
    let mut x = r.left() + 16.0;
    if let Some(g) = glyph {
        ui.painter().text(
            pos2(x + 8.0, r.center().y),
            Align2::CENTER_CENTER,
            g,
            icon(16.0),
            p.on_accent,
        );
        x += iw;
    }
    ui.painter().galley(
        pos2(x, r.center().y - galley.size().y / 2.0),
        galley,
        p.on_accent,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn ghost_button(ui: &mut Ui, p: &Palette, glyph: Option<&str>, text: &str) -> Response {
    let font = regular(13.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, p.text);
    let iw = if glyph.is_some() { 20.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0 + iw, 32.0), Sense::click());
    let fill = if resp.hovered() {
        p.surface2.gamma_multiply(1.3)
    } else {
        p.surface2
    };
    ui.painter().rect_filled(r, CornerRadius::same(10), fill);
    ui.painter().rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, p.glass_rim_side), egui::StrokeKind::Inside);
    let mut x = r.left() + 12.0;
    if let Some(g) = glyph {
        ui.painter().text(
            pos2(x + 7.0, r.center().y),
            Align2::CENTER_CENTER,
            g,
            icon(14.0),
            p.text_dim,
        );
        x += iw;
    }
    ui.painter().galley(
        pos2(x, r.center().y - galley.size().y / 2.0),
        galley,
        p.text,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn spinner(ui: &mut Ui, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let t = ui.input(|i| i.time) as f32;
    let a0 = t * 5.0;
    arc(
        ui.painter(),
        rect.center(),
        size / 2.0 - 2.0,
        a0,
        a0 + PI * 1.3,
        Stroke::new(2.5, p.accent),
    );
    ui.ctx().request_repaint();
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

