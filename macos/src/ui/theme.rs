//! macOS 27 "Liquid Glass" palette, San Francisco typography, and egui visuals.
//!
//! Designed around macOS Control Center & Liquid Glass design language:
//! - Translucent frosted materials (`surface` / `surface2`) over an ambient glowing backdrop
//! - Specular top-edge highlights (`glass_rim_top`) and subtle side borders (`glass_rim_side`)
//! - macOS System Blue (`#0A84FF` dark / `#007AFF` light), Indigo (`#5E5CE6`),
//!   Green (`#30D158`), Orange (`#FF9F0A`), Red (`#FF453A`)
//! - Native San Francisco (`SFNS.ttf`) typography with Phosphor vector icons.

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals,
    epaint::Shadow,
};

#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// Deep base window background (translucent glass canvas).
    pub bg: Color32,
    /// Primary frosted Liquid Glass card fill.
    pub surface: Color32,
    /// Elevated / interactive pill or inner well fill.
    pub surface2: Color32,
    /// Floating selected capsule fill inside segmented controls.
    pub pill_active: Color32,
    /// Specular 1px highlight along the top edge of glass cards & pills.
    pub glass_rim_top: Color32,
    /// Subtle outer glass border.
    pub glass_rim_side: Color32,
    /// Hairline divider color inside cards.
    pub border: Color32,
    /// Primary label color.
    pub text: Color32,
    /// Secondary label color.
    pub text_dim: Color32,
    /// Tertiary / caption label color.
    pub text_faint: Color32,
    /// macOS System Blue (`#0A84FF` / `#007AFF`).
    pub accent: Color32,
    /// Secondary gradient / glow accent (macOS System Indigo `#5E5CE6`).
    pub accent_secondary: Color32,
    /// Foreground color on top of `accent`.
    pub on_accent: Color32,
    /// macOS System Green (`#30D158` / `#34C759`).
    pub good: Color32,
    /// macOS System Orange (`#FF9F0A` / `#FF9500`).
    pub warn: Color32,
    /// macOS System Red (`#FF453A` / `#FF3B30`).
    pub bad: Color32,
    /// macOS Traffic Light — Close (`#FF5F56`).
    pub tl_close: Color32,
    /// macOS Traffic Light — Minimize (`#FFBD2E`).
    pub tl_min: Color32,
    /// macOS Traffic Light — Zoom/Expand (`#27C93F`).
    pub tl_zoom: Color32,
}

impl Palette {
    pub const DARK: Self = Self {
        dark: true,
        bg: Color32::from_rgba_premultiplied(15, 18, 25, 242),
        surface: Color32::from_rgba_premultiplied(32, 38, 52, 168),
        surface2: Color32::from_rgba_premultiplied(48, 56, 76, 150),
        pill_active: Color32::from_rgba_premultiplied(74, 86, 114, 195),
        glass_rim_top: Color32::from_rgba_premultiplied(255, 255, 255, 58),
        glass_rim_side: Color32::from_rgba_premultiplied(255, 255, 255, 22),
        border: Color32::from_rgba_premultiplied(255, 255, 255, 18),
        text: Color32::from_rgb(0xF5, 0xF7, 0xFA),
        text_dim: Color32::from_rgb(0xA4, 0xAE, 0xBE),
        text_faint: Color32::from_rgb(0x6C, 0x76, 0x89),
        accent: Color32::from_rgb(0x0A, 0x84, 0xFF),
        accent_secondary: Color32::from_rgb(0x5E, 0x5C, 0xE6),
        on_accent: Color32::WHITE,
        good: Color32::from_rgb(0x30, 0xD1, 0x58),
        warn: Color32::from_rgb(0xFF, 0x9F, 0x0A),
        bad: Color32::from_rgb(0xFF, 0x45, 0x3A),
        tl_close: Color32::from_rgb(0xFF, 0x5F, 0x56),
        tl_min: Color32::from_rgb(0xFF, 0xBD, 0x2E),
        tl_zoom: Color32::from_rgb(0x27, 0xC9, 0x3F),
    };

    pub const LIGHT: Self = Self {
        dark: false,
        bg: Color32::from_rgba_premultiplied(238, 242, 249, 246),
        surface: Color32::from_rgba_premultiplied(255, 255, 255, 198),
        surface2: Color32::from_rgba_premultiplied(226, 232, 243, 175),
        pill_active: Color32::from_rgba_premultiplied(255, 255, 255, 242),
        glass_rim_top: Color32::from_rgba_premultiplied(255, 255, 255, 225),
        glass_rim_side: Color32::from_rgba_premultiplied(15, 23, 42, 20),
        border: Color32::from_rgba_premultiplied(15, 23, 42, 16),
        text: Color32::from_rgb(0x12, 0x16, 0x21),
        text_dim: Color32::from_rgb(0x52, 0x5C, 0x6E),
        text_faint: Color32::from_rgb(0x84, 0x8E, 0xA0),
        accent: Color32::from_rgb(0x00, 0x7A, 0xFF),
        accent_secondary: Color32::from_rgb(0x58, 0x56, 0xD6),
        on_accent: Color32::WHITE,
        good: Color32::from_rgb(0x28, 0xCD, 0x41),
        warn: Color32::from_rgb(0xFF, 0x95, 0x00),
        bad: Color32::from_rgb(0xFF, 0x3B, 0x30),
        tl_close: Color32::from_rgb(0xFF, 0x5F, 0x56),
        tl_min: Color32::from_rgb(0xFF, 0xBD, 0x2E),
        tl_zoom: Color32::from_rgb(0x27, 0xC9, 0x3F),
    };

    pub fn accent_soft(&self) -> Color32 {
        let [r, g, b, _] = self.accent.to_array();
        Color32::from_rgba_unmultiplied(r, g, b, if self.dark { 46 } else { 32 })
    }

    pub fn track(&self) -> Color32 {
        if self.dark {
            Color32::from_rgba_premultiplied(64, 74, 98, 175)
        } else {
            Color32::from_rgba_premultiplied(198, 206, 222, 185)
        }
    }

    pub fn level(&self, percent: u8) -> Color32 {
        match percent {
            0..=15 => self.bad,
            16..=35 => self.warn,
            _ => self.good,
        }
    }
}

pub const SEMIBOLD: &str = "semibold";
pub const BOLD: &str = "bold";


pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}
pub fn icon(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("phosphor".into()))
}

pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // Prefer macOS San Francisco (SFNS), with Helvetica Neue and Windows Segoe UI fallbacks.
    let regular_candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSDisplay.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    let semibold_candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSDisplay.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        r"C:\Windows\Fonts\seguisb.ttf",
    ];
    let bold_candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSDisplay.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        r"C:\Windows\Fonts\segoeuib.ttf",
    ];

    for (key, paths, primary) in [
        ("sf-reg", &regular_candidates[..], true),
        (SEMIBOLD, &semibold_candidates[..], false),
        (BOLD, &bold_candidates[..], false),
    ] {
        if let Some(bytes) = paths.iter().find_map(|p| std::fs::read(p).ok()) {
            fonts
                .font_data
                .insert(key.into(), Arc::new(FontData::from_owned(bytes)));
            if primary {
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, key.into());
            } else {
                fonts
                    .families
                    .insert(FontFamily::Name(key.into()), vec![key.into()]);
            }
        } else if !primary {
            let prop = fonts
                .families
                .get(&FontFamily::Proportional)
                .cloned()
                .unwrap_or_default();
            fonts.families.insert(FontFamily::Name(key.into()), prop);
        }
    }

    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &Context, p: &Palette) {
    let mut style = (*ctx.style()).clone();
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.glass_rim_top);
    v.window_corner_radius = CornerRadius::same(18);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 32 }),
    };
    v.popup_shadow = v.window_shadow;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent_soft();
    v.selection.stroke = Stroke::new(1.5, p.accent);
    v.hyperlink_color = p.accent;
    v.extreme_bg_color = p.surface2;
    v.faint_bg_color = p.surface;

    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = p.surface2;
        w.weak_bg_fill = p.surface2;
        w.bg_stroke = Stroke::new(1.0, p.glass_rim_side);
        w.fg_stroke = Stroke::new(1.0, p.text);
        w.corner_radius = CornerRadius::same(12);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.glass_rim_top);
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.on_accent);

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating_width = 6.0;
    style.visuals = v;
    ctx.set_style(style);
}
