//! macOS platform integration: LaunchAgents (Open at Login), single-instance Unix socket,
//! Appearance detection, Notification Center alerts, and Menu Bar popover geometry.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::shared::{Shared, UiCmd};

const LAUNCH_AGENT_LABEL: &str = "com.sennheiser-control.app";

// ----------------------------------------------------------------------------------------------- Open at Login (LaunchAgent)

fn launch_agent_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(
        home.join("Library")
            .join("LaunchAgents")
            .join(format!("{LAUNCH_AGENT_LABEL}.plist")),
    )
}

pub fn autostart_enabled() -> bool {
    launch_agent_path().is_some_and(|p| p.is_file())
}

pub fn set_autostart(enable: bool) -> std::io::Result<()> {
    let Some(plist_path) = launch_agent_path() else {
        return Ok(());
    };
    if enable {
        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let exe = std::env::current_exe()?;
        let exe_str = exe.display().to_string();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_str}</string>
        <string>--minimized</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#
        );
        std::fs::write(plist_path, plist)
    } else {
        match std::fs::remove_file(plist_path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

// ----------------------------------------------------------------------------------------------- Appearance (Dark / Light)

struct ThemeCache {
    checked_at: Option<Instant>,
    is_dark: bool,
}

static THEME_CACHE: Mutex<ThemeCache> = Mutex::new(ThemeCache {
    checked_at: None,
    is_dark: true,
});

fn macos_is_dark() -> bool {
    let mut cache = THEME_CACHE.lock();
    if let Some(t) = cache.checked_at
        && t.elapsed() < Duration::from_secs(3)
    {
        return cache.is_dark;
    }
    let dark = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .eq_ignore_ascii_case("Dark")
        })
        .unwrap_or(true);
    cache.checked_at = Some(Instant::now());
    cache.is_dark = dark;
    dark
}

/// Whether macOS is currently using the Light appearance.
pub fn apps_are_light() -> bool {
    !macos_is_dark()
}

/// Whether the macOS Menu Bar uses dark glyphs on a light backdrop.
pub fn menubar_is_light() -> bool {
    !macos_is_dark()
}

// ----------------------------------------------------------------------------------------------- Single Instance (Unix Socket)

pub enum Instance {
    Primary,
    Secondary,
}

#[cfg(unix)]
pub fn claim_single_instance(quit: bool, sim: bool, shared_slot: Arc<Mutex<Option<Arc<Shared>>>>) -> Instance {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};

    let dir = crate::settings::dir();
    let _ = std::fs::create_dir_all(&dir);
    let sock_path = dir.join(if sim {
        "instance-sim.sock"
    } else {
        "instance.sock"
    });

    if let Ok(mut stream) = UnixStream::connect(&sock_path) {
        let msg = if quit { b"quit\n" as &[u8] } else { b"show\n" };
        let _ = stream.write_all(msg);
        return Instance::Secondary;
    }
    if quit {
        return Instance::Secondary;
    }
    let _ = std::fs::remove_file(&sock_path);
    if let Ok(listener) = UnixListener::bind(&sock_path) {
        std::thread::Builder::new()
            .name("instance-ipc".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let mut line = String::new();
                    let _ = BufReader::new(stream).read_line(&mut line);
                    if let Some(shared) = shared_slot.lock().clone() {
                        if line.trim() == "quit" {
                            shared.send_ui(UiCmd::Quit);
                            break;
                        } else {
                            shared.send_ui(UiCmd::Show);
                        }
                    }
                }
            })
            .ok();
    }
    Instance::Primary
}

#[cfg(not(unix))]
pub fn claim_single_instance(_quit: bool, _sim: bool, _shared_slot: Arc<Mutex<Option<Arc<Shared>>>>) -> Instance {
    Instance::Primary
}

// ----------------------------------------------------------------------------------------------- System Actions & Notifications

pub fn open_bluetooth_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.BluetoothSettings")
            .spawn();
    }
}

pub fn notify_low_battery(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"Submarine\"",
            esc(body),
            esc(title)
        );
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        log::info!("notification: {title} — {body}");
    }
}

// ----------------------------------------------------------------------------------------------- macOS Screen & Window Styling

/// Returns the visible screen frame `(left, top, right, bottom)` in logical points (top-left origin)
/// so the popover window can anchor neatly below the macOS Menu Bar.
#[cfg(target_os = "macos")]
pub fn screen_work_area() -> Option<(f32, f32, f32, f32)> {
    use std::ffi::{CString, c_char, c_void};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        origin: NSPoint,
        size: NSSize,
    }

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc", kind = "dylib")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    unsafe {
        let cls_c = CString::new("NSScreen").ok()?;
        let screen_cls = objc_getClass(cls_c.as_ptr());
        if screen_cls.is_null() {
            return None;
        }
        let main_sel = sel_registerName(c"mainScreen".as_ptr());
        let msg_id: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let screen = msg_id(screen_cls, main_sel);
        if screen.is_null() {
            return None;
        }
        let msg_rect: unsafe extern "C" fn(Id, Sel) -> NSRect =
            std::mem::transmute(objc_msgSend as *const c_void);
        let full = msg_rect(screen, sel_registerName(c"frame".as_ptr()));
        let vis = msg_rect(screen, sel_registerName(c"visibleFrame".as_ptr()));

        // Convert from Cocoa bottom-left origin to top-left origin
        let left = vis.origin.x as f32;
        let right = (vis.origin.x + vis.size.width) as f32;
        let top = (full.size.height - (vis.origin.y + vis.size.height)) as f32;
        let bottom = (full.size.height - vis.origin.y) as f32;
        Some((left, top, right, bottom))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn screen_work_area() -> Option<(f32, f32, f32, f32)> {
    Some((0.0, 28.0, 1512.0, 940.0))
}

/// Apply native macOS NSWindow polish (shadow, movable by background).
pub fn style_macos_window(_ns_view_ptr: usize) {
    #[cfg(target_os = "macos")]
    if _ns_view_ptr != 0 {
        use std::ffi::c_void;
        type Id = *mut c_void;
        type Sel = *mut c_void;

        #[link(name = "objc", kind = "dylib")]
        unsafe extern "C" {
            fn sel_registerName(name: *const i8) -> Sel;
            fn objc_msgSend();
        }

        unsafe {
            let view = _ns_view_ptr as Id;
            let msg_id: unsafe extern "C" fn(Id, Sel) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            let window = msg_id(view, sel_registerName(c"window".as_ptr()));
            if !window.is_null() {
                let msg_bool: unsafe extern "C" fn(Id, Sel, u8) =
                    std::mem::transmute(objc_msgSend as *const c_void);
                msg_bool(window, sel_registerName(c"setHasShadow:".as_ptr()), 1);
                msg_bool(
                    window,
                    sel_registerName(c"setMovableByWindowBackground:".as_ptr()),
                    1,
                );
            }
        }
    }
}

