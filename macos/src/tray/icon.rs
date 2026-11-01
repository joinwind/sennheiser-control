//! Menu Bar icon and macOS squircle AppIcon rendering (`tiny-skia` + `ab_glyph`).

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayVisual {
    Disconnected,
    Connecting,
    Battery {
        percent: Option<u8>,
        charging: bool,
    },
}

pub const GOOD: [u8; 3] = [0x30, 0xD1, 0x58];
pub const WARN: [u8; 3] = [0xFF, 0x9F, 0x0A];
pub const BAD: [u8; 3] = [0xFF, 0x45, 0x3A];
pub const ACCENT: [u8; 3] = [0x0A, 0x84, 0xFF];

pub fn level_color(percent: u8) -> [u8; 3] {
    match percent {
        0..=15 => BAD,
        16..=35 => WARN,
        _ => GOOD,
    }
}

pub struct IconRenderer {
    font: Option<FontVec>,
}

fn load_font() -> Option<FontVec> {
    let macos_candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSDisplay.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ];
    for path in macos_candidates {
        if let Ok(bytes) = std::fs::read(path)
            && let Ok(f) = FontVec::try_from_vec(bytes)
        {
            return Some(f);
        }
    }
    if let Some(windir) = std::env::var_os("WINDIR").map(std::path::PathBuf::from) {
        for name in ["segoeuib.ttf", "seguisb.ttf", "segoeui.ttf"] {
            if let Ok(bytes) = std::fs::read(windir.join("Fonts").join(name))
                && let Ok(f) = FontVec::try_from_vec(bytes)
            {
                return Some(f);
            }
        }
    }
    None
}

fn rgba(c: [u8; 3], a: f32) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn draw_headphones(pm: &mut Pixmap, ox: f32, oy: f32, s: f32, color: Color) {
    let stroke_w = (s * 0.11).max(1.4);
    let (cx, cy, r) = (ox + s * 0.5, oy + s * 0.55, s * 0.34);
    let mut pb = PathBuilder::new();
    let k = 0.552_284_8 * r;
    pb.move_to(cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    if let Some(path) = pb.finish() {
        let stroke = Stroke {
            width: stroke_w,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
    }
    let (cw, ch) = (s * 0.24, s * 0.38);
    let cup_y = oy + s * 0.49;
    for x in [cx - r - cw * 0.40, cx + r - cw * 0.60] {
        if let Some(p) = rounded_rect(x, cup_y, cw, ch, cw * 0.42) {
            pm.fill_path(
                &p,
                &paint(color),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

fn blend_px(pm: &mut Pixmap, x: i32, y: i32, c: [u8; 3], alpha: f32) {
    if x < 0 || y < 0 || x >= pm.width() as i32 || y >= pm.height() as i32 || alpha <= 0.0 {
        return;
    }
    let i = ((y as u32 * pm.width() + x as u32) * 4) as usize;
    let d = pm.data_mut();
    let a = alpha.min(1.0);
    for k in 0..3 {
        d[i + k] = (c[k] as f32 * a + d[i + k] as f32 * (1.0 - a)).round() as u8;
    }
    d[i + 3] = (255.0 * a + d[i + 3] as f32 * (1.0 - a)).round() as u8;
}

impl IconRenderer {
    pub fn new() -> Self {
        Self { font: load_font() }
    }

    fn draw_text(&self, pm: &mut Pixmap, text: &str, area: (f32, f32, f32, f32), color: [u8; 3]) {
        let (bx, by, bw, bh) = area;
        let Some(font) = &self.font else { return };
        let layout = |px: f32| {
            let sf = font.as_scaled(PxScale::from(px));
            let mut x = 0.0;
            let mut out = Vec::new();
            let mut prev = None;
            for ch in text.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    x += sf.kern(p, id);
                }
                out.push(id.with_scale_and_position(px, point(x, 0.0)));
                x += sf.h_advance(id);
                prev = Some(id);
            }
            out
        };
        let bounds = |glyphs: &[ab_glyph::Glyph]| {
            let mut b: Option<ab_glyph::Rect> = None;
            for g in glyphs {
                if let Some(o) = font.outline_glyph(g.clone()) {
                    let r = o.px_bounds();
                    b = Some(match b {
                        None => r,
                        Some(a) => ab_glyph::Rect {
                            min: point(a.min.x.min(r.min.x), a.min.y.min(r.min.y)),
                            max: point(a.max.x.max(r.max.x), a.max.y.max(r.max.y)),
                        },
                    });
                }
            }
            b
        };
        let mut px = bh * 1.3;
        let Some(b) = bounds(&layout(px)) else { return };
        let fit = (bw / b.width()).min(bh / b.height());
        px *= fit;
        let glyphs = layout(px);
        let Some(b) = bounds(&glyphs) else { return };
        let dx = bx + (bw - b.width()) / 2.0 - b.min.x;
        let dy = by + (bh - b.height()) / 2.0 - b.min.y;
        for g in glyphs {
            if let Some(o) = font.outline_glyph(g) {
                let r = o.px_bounds();
                let (ox, oy) = ((r.min.x + dx).round() as i32, (r.min.y + dy).round() as i32);
                o.draw(|x, y, cov| blend_px(pm, ox + x as i32, oy + y as i32, color, cov));
            }
        }
    }

    /// Render the macOS Menu Bar icon at `size`×`size` pixels.
    pub fn tray(&self, visual: TrayVisual, size: u32, light_menubar: bool) -> Vec<u8> {
        let s = size as f32;
        let mut pm = Pixmap::new(size, size).expect("pixmap");
        let fg: [u8; 3] = if light_menubar {
            [0x14, 0x16, 0x1F]
        } else {
            [0xF5, 0xF7, 0xFA]
        };
        match visual {
            TrayVisual::Disconnected => draw_headphones(&mut pm, 0.0, 0.0, s, rgba(fg, 0.88)),
            TrayVisual::Connecting => {
                draw_headphones(&mut pm, 0.0, 0.0, s, rgba(fg, 0.45));
                let d = (s * 0.28).max(4.0);
                if let Some(p) = PathBuilder::from_circle(s - d / 2.0, s - d / 2.0, d / 2.0) {
                    pm.fill_path(
                        &p,
                        &paint(rgba(ACCENT, 1.0)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            TrayVisual::Battery { percent: None, .. } => {
                draw_headphones(&mut pm, 0.0, 0.0, s, rgba(fg, 0.88))
            }
            TrayVisual::Battery {
                percent: Some(p),
                charging,
            } => {
                // macOS Menu Bar battery pill: rounded capsule outline + level bar + crisp number
                let bar_h = (s * 0.14).round().max(2.0);
                let gap = (s * 0.06).round().max(1.0);
                let text_h = s - bar_h - gap;
                let color = if charging {
                    GOOD
                } else if p <= 15 {
                    BAD
                } else {
                    fg
                };
                self.draw_text(&mut pm, &p.to_string(), (0.0, 0.0, s, text_h), color);
                let bar_y = s - bar_h;
                let inset = (s * 0.06).round();
                if let Some(t) = rounded_rect(inset, bar_y, s - 2.0 * inset, bar_h, bar_h / 2.0) {
                    pm.fill_path(
                        &t,
                        &paint(rgba(fg, 0.26)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                let fill_w = ((s - 2.0 * inset) * p.min(100) as f32 / 100.0).max(bar_h);
                let fill_c = if charging { GOOD } else { level_color(p) };
                if let Some(f) = rounded_rect(inset, bar_y, fill_w, bar_h, bar_h / 2.0) {
                    pm.fill_path(
                        &f,
                        &paint(rgba(fill_c, 1.0)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        to_straight(pm)
    }
}

const LOGO_SVG: &str = include_str!("../../../assets/sennheiser-logo.svg");

/// macOS 27 squircle AppIcon with Liquid Glass rim highlight and white Sennheiser mark.
pub fn app_icon(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut pm = Pixmap::new(size, size).expect("pixmap");
    let pad = (s * 0.06).round();
    let box_s = s - pad * 2.0;
    let radius = box_s * 0.225;

    // Deep obsidian squircle tile
    if let Some(bg) = rounded_rect(pad, pad, box_s, box_s, radius) {
        pm.fill_path(
            &bg,
            &paint(Color::from_rgba8(0x11, 0x15, 0x20, 255)),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        // Subtle Liquid Glass specular border
        let stroke = Stroke {
            width: (s * 0.018).max(1.0),
            ..Default::default()
        };
        pm.stroke_path(
            &bg,
            &paint(Color::from_rgba8(0x78, 0xA8, 0xFF, 90)),
            &stroke,
            Transform::identity(),
            None,
        );
    }

    if let Some(mark) = logo_path() {
        let b = mark.bounds();
        let k = box_s * 0.78 / b.width();
        let t = Transform::from_row(
            k,
            0.0,
            0.0,
            k,
            (s - b.width() * k) / 2.0 - b.left() * k,
            (s - b.height() * k) / 2.0 - b.top() * k,
        );
        if let (Some(mut holes), Some(inner)) = (
            tiny_skia::Mask::new(size, size),
            Rect::from_ltrb(
                b.left() + 0.4,
                b.top() + 0.4,
                b.right() - 0.4,
                b.bottom() - 0.4,
            ),
        ) {
            holes.fill_path(&mark, FillRule::EvenOdd, true, t);
            holes.invert();
            pm.fill_rect(inner, &paint(Color::WHITE), t, Some(&holes));
        }
    }
    to_straight(pm)
}

fn logo_path() -> Option<tiny_skia::Path> {
    let d = LOGO_SVG.split(" d=\"").nth(1)?.split('"').next()?;
    parse_svg_path(d)
}

fn parse_svg_path(d: &str) -> Option<tiny_skia::Path> {
    let b = d.as_bytes();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
            *i += 1;
        }
    };
    let starts_number =
        |i: usize| i < b.len() && (b[i].is_ascii_digit() || matches!(b[i], b'.' | b'-' | b'+'));
    let number = |i: &mut usize| -> Option<f32> {
        skip(i);
        let start = *i;
        if *i < b.len() && matches!(b[*i], b'-' | b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < b.len() {
            match b[*i] {
                c if c.is_ascii_digit() => *i += 1,
                b'.' if !dot => {
                    dot = true;
                    *i += 1;
                }
                b'e' | b'E' => {
                    *i += 1;
                    if *i < b.len() && matches!(b[*i], b'-' | b'+') {
                        *i += 1;
                    }
                    while *i < b.len() && b[*i].is_ascii_digit() {
                        *i += 1;
                    }
                    break;
                }
                _ => break,
            }
        }
        d.get(start..*i)?.parse().ok()
    };

    let mut pb = PathBuilder::new();
    let (mut cx, mut cy, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    loop {
        skip(&mut i);
        if i >= b.len() {
            break;
        }
        let cmd = b[i];
        i += 1;
        let rel = cmd.is_ascii_lowercase();
        let mut first = true;
        loop {
            skip(&mut i);
            if !first && !starts_number(i) {
                break;
            }
            match cmd.to_ascii_uppercase() {
                b'M' => {
                    let (x, y) = (number(&mut i)?, number(&mut i)?);
                    let (x, y) = if rel { (cx + x, cy + y) } else { (x, y) };
                    if first {
                        pb.move_to(x, y);
                        (sx, sy) = (x, y);
                    } else {
                        pb.line_to(x, y);
                    }
                    (cx, cy) = (x, y);
                }
                b'L' => {
                    let (x, y) = (number(&mut i)?, number(&mut i)?);
                    let (x, y) = if rel { (cx + x, cy + y) } else { (x, y) };
                    pb.line_to(x, y);
                    (cx, cy) = (x, y);
                }
                b'H' => {
                    let x = number(&mut i)? + if rel { cx } else { 0.0 };
                    pb.line_to(x, cy);
                    cx = x;
                }
                b'V' => {
                    let y = number(&mut i)? + if rel { cy } else { 0.0 };
                    pb.line_to(cx, y);
                    cy = y;
                }
                b'C' => {
                    let o = if rel { (cx, cy) } else { (0.0, 0.0) };
                    let mut v = [0.0f32; 6];
                    for n in &mut v {
                        *n = number(&mut i)?;
                    }
                    pb.cubic_to(
                        v[0] + o.0,
                        v[1] + o.1,
                        v[2] + o.0,
                        v[3] + o.1,
                        v[4] + o.0,
                        v[5] + o.1,
                    );
                    (cx, cy) = (v[4] + o.0, v[5] + o.1);
                }
                b'Z' => {
                    pb.close();
                    (cx, cy) = (sx, sy);
                    break;
                }
                _ => return None,
            }
            first = false;
        }
    }
    pb.finish()
}

fn to_straight(pm: Pixmap) -> Vec<u8> {
    let mut v = pm.take();
    for px in v.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    v
}

/// Dev aid (`--render-icons DIR`): writes a standard macOS `.iconset` directory for `iconutil -c icns`.
pub fn dev_render(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    let entries = [
        ("icon_16x16.png", 16u32),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];
    for (name, px) in entries {
        if let Some(pm) =
            Pixmap::from_vec(app_icon(px), tiny_skia::IntSize::from_wh(px, px).unwrap())
        {
            let _ = pm.save_png(dir.join(name));
        }
    }
}

