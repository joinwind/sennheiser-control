//! macOS Menu Bar menu (`NSMenu`): built from the current state, ids are parsed back into actions.

use engine::{Action, AppState, LinkState, NoiseMode};
use protocol::{AntiWind, PromptMode, eq::PRESETS};
use tray_icon::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::settings::Settings;
use crate::shared::{
    COMFORT_CALLS_TITLE, PROMPTS_TITLE, SIDETONE_LABELS, SIDETONE_TITLE, Shared, UiCmd, WIND_ORDER,
    custom_level_label,
};

const CUSTOM_LEVELS: [u8; 5] = [0, 25, 50, 75, 100];

fn check(id: &str, text: &str, checked: bool) -> CheckMenuItem {
    CheckMenuItem::with_id(id, text, true, checked, None)
}

fn item(id: &str, text: &str, enabled: bool) -> MenuItem {
    MenuItem::with_id(id, text, enabled, None)
}

fn submenu(text: &str, items: &[&dyn IsMenuItem]) -> Submenu {
    submenu_enabled(text, items, true)
}

fn submenu_enabled(text: &str, items: &[&dyn IsMenuItem], enabled: bool) -> Submenu {
    let s = Submenu::new(text, enabled);
    for i in items {
        let _ = s.append(*i);
    }
    s
}

pub fn header_text(state: &AppState) -> String {
    let name = state
        .active_device()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "Headset".into());
    match (&state.link, state.live()) {
        (LinkState::Connected, Some(h)) => {
            let mut s = name;
            if let Some(p) = h.battery {
                s.push_str(&format!(" — {p}%"));
            }
            if h.charging_now() {
                s.push_str(" · Charging");
            }
            s
        }
        (LinkState::Connecting, _) => format!("{name} — Connecting…"),
        (LinkState::Retrying(_), _) => format!("{name} — Reconnecting…"),
        _ if state.connected_devices().next().is_none() => "No Headset Connected".into(),
        _ => name,
    }
}

pub fn tooltip(state: &AppState) -> String {
    let mut t = header_text(state);
    if let Some(h) = state.live()
        && let Some(m) = h.noise_mode()
    {
        t.push_str(" · ");
        t.push_str(m.label());
        if m == NoiseMode::Custom
            && let Some(level) = h.transparency
        {
            t.push_str(&format!(" ({})", custom_level_label(level)));
        }
    }
    t
}

pub fn build(state: &AppState, settings: &Settings, autostart: bool) -> Menu {
    let menu = Menu::new();
    let header = item("header", &header_text(state), false);
    let _ = menu.append(&header);
    let _ = menu.append(&PredefinedMenuItem::separator());

    if let Some(h) = state.live() {
        let mode = h.noise_mode();
        let mut modes = vec![NoiseMode::Adaptive, NoiseMode::Custom];
        if h.caps.anc_off {
            modes.push(NoiseMode::Off);
        }
        let items: Vec<CheckMenuItem> = modes
            .iter()
            .map(|m| {
                check(
                    &format!("noise:{}", m.label().to_lowercase()),
                    m.label(),
                    mode == Some(*m),
                )
            })
            .collect();
        for i in &items {
            let _ = menu.append(i);
        }
        let custom = mode == Some(NoiseMode::Custom);
        let levels: Vec<CheckMenuItem> = CUSTOM_LEVELS
            .iter()
            .map(|l| {
                check(
                    &format!("level:{l}"),
                    &custom_level_label(*l),
                    custom && h.transparency == Some(*l),
                )
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem> = levels.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = menu.append(&submenu("Custom Balance", &refs));
        let winds: Vec<CheckMenuItem> = WIND_ORDER
            .iter()
            .map(|w| {
                check(
                    &format!("wind:{}", w.to_u8()),
                    w.label(),
                    h.anti_wind == Some(*w),
                )
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem> = winds.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = menu.append(&submenu_enabled(
            "Wind Noise Reduction",
            &refs,
            mode != Some(NoiseMode::Off),
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());

        if h.caps.eq != protocol::EqGen::Unsupported && h.eq.known {
            use crate::ui::eq::EqChoice;
            let choice = crate::ui::eq::eq_choice(h, &h.eq.bands, &settings.custom_presets, false);
            let is = |c: EqChoice| choice.as_ref() == Some(&c);
            let mut presets = vec![check("eq:neutral", "Neutral", is(EqChoice::Neutral))];
            presets.extend(PRESETS.iter().enumerate().map(|(i, (name, _))| {
                check(
                    &format!("preset:{i}"),
                    name,
                    is(EqChoice::Preset(name.to_string())),
                )
            }));
            presets.extend(settings.custom_presets.iter().enumerate().map(|(i, p)| {
                check(
                    &format!("upreset:{i}"),
                    &p.name,
                    is(EqChoice::Preset(p.name.clone())),
                )
            }));
            presets.push(check("eq:custom", "Custom", is(EqChoice::Custom)));
            let refs: Vec<&dyn IsMenuItem> = presets.iter().map(|i| i as &dyn IsMenuItem).collect();
            let title = match &choice {
                Some(c) => format!("EQ Preset ({})", c.label()),
                None => "EQ Preset".to_string(),
            };
            let _ = menu.append(&submenu_enabled(&title, &refs, !h.podcast()));
            let _ = menu.append(&CheckMenuItem::with_id(
                "bass",
                "Bass Boost",
                h.eq_active(),
                h.eq.bass_boost && h.eq_active(),
                None,
            ));
        }
        let _ = menu.append(&check("podcast", "Podcast Mode", h.podcast()));
        let _ = menu.append(&PredefinedMenuItem::separator());

        let s = Submenu::new("Headset Settings", true);
        let c = &h.caps;
        if c.on_head_detection {
            let _ = s.append(&check(
                "onhead",
                "On-Head Detection",
                h.on_head_detection == Some(true),
            ));
            let on_head = h.on_head_detection == Some(true);
            let _ = s.append(&CheckMenuItem::with_id(
                "smartpause",
                "Smart Pause",
                on_head,
                h.smart_pause == Some(true),
                None,
            ));
            if c.auto_answer_calls {
                let _ = s.append(&CheckMenuItem::with_id(
                    "autocall",
                    "Auto-Answer Calls",
                    on_head,
                    h.auto_call == Some(true),
                    None,
                ));
            }
        }
        let _ = s.append(&check(
            "comfort",
            COMFORT_CALLS_TITLE,
            h.comfort_call == Some(true),
        ));
        if c.touch_lock {
            let _ = s.append(&check(
                "touchlock",
                "Lock Touch Controls",
                h.touch_lock == Some(true),
            ));
        }
        let _ = s.append(&PredefinedMenuItem::separator());
        if c.auto_power_off {
            let offs: Vec<CheckMenuItem> = protocol::AUTO_POWER_OFF_CHOICES
                .iter()
                .map(|secs| {
                    let label = if *secs == 0 {
                        "Never".to_string()
                    } else {
                        format!("{} minutes", secs / 60)
                    };
                    check(
                        &format!("autooff:{secs}"),
                        &label,
                        h.auto_off_seconds == Some(*secs),
                    )
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem> = offs.iter().map(|i| i as &dyn IsMenuItem).collect();
            let _ = s.append(&submenu("Auto Power-Off", &refs));
        }
        if c.voice_prompts {
            let prompts: Vec<CheckMenuItem> = PromptMode::ALL
                .iter()
                .map(|m| {
                    check(
                        &format!("prompts:{}", m.to_u8()),
                        m.label(),
                        h.prompt_mode == Some(*m),
                    )
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem> = prompts.iter().map(|i| i as &dyn IsMenuItem).collect();
            let _ = s.append(&submenu(PROMPTS_TITLE, &refs));
        }
        let side: Vec<CheckMenuItem> = SIDETONE_LABELS
            .iter()
            .enumerate()
            .map(|(i, l)| check(&format!("sidetone:{i}"), l, h.sidetone == Some(i as u8)))
            .collect();
        let refs: Vec<&dyn IsMenuItem> = side.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = s.append(&submenu(SIDETONE_TITLE, &refs));
        let _ = menu.append(&s);

        if !h.paired_hosts.is_empty() {
            let full = h.paired_max_connections.is_some_and(|max| {
                h.paired_hosts.iter().filter(|x| x.connected).count() >= usize::from(max)
            });
            let hosts: Vec<CheckMenuItem> = h
                .paired_hosts
                .iter()
                .map(|host| {
                    let own = h.paired_own_index == Some(host.index);
                    let label = if own {
                        format!("{} (This Mac)", host.name)
                    } else {
                        host.name.clone()
                    };
                    CheckMenuItem::with_id(
                        format!("host:{}", host.index),
                        label,
                        !own && (host.connected || !full),
                        host.connected,
                        None,
                    )
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem> = hosts.iter().map(|i| i as &dyn IsMenuItem).collect();
            let _ = menu.append(&submenu("Connections", &refs));
        }
    }

    let connected: Vec<_> = state.connected_devices().collect();
    if connected.len() > 1 {
        let items: Vec<CheckMenuItem> = connected
            .iter()
            .map(|d| {
                check(
                    &format!("device:{}", d.id),
                    &state.display_name(d),
                    state.active.as_deref() == Some(d.id.as_str()),
                )
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem> = items.iter().map(|i| i as &dyn IsMenuItem).collect();
        let _ = menu.append(&submenu("Headset", &refs));
    }
    if state.live().is_some() || connected.len() > 1 {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    if state.connected_devices().next().is_none() {
        let _ = menu.append(&item("btsettings", "Bluetooth Settings…", true));
    }
    let _ = menu.append(&item("open", "Show Sennheiser Control", true));
    let _ = menu.append(&item("settings", "Preferences…", true));
    let _ = menu.append(&check("autostart", "Open at Login", autostart));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item("quit", "Quit Sennheiser Control", true));
    menu
}

pub fn handle(id: &str, shared: &Shared) {
    let state = shared.engine.snapshot();
    let h = state.live();
    let (key, arg) = id.split_once(':').unwrap_or((id, ""));
    let num = || arg.parse::<u32>().ok();
    let act = |a: Action| shared.dispatch(a);
    match key {
        "open" => shared.send_ui(UiCmd::Show),
        "settings" => shared.send_ui(UiCmd::OpenSettings),
        "quit" => shared.send_ui(UiCmd::Quit),
        "btsettings" => crate::platform::open_bluetooth_settings(),
        "autostart" => {
            let _ = crate::platform::set_autostart(!crate::platform::autostart_enabled());
            shared.wake_all();
        }
        "noise" => act(Action::SetNoiseMode(match arg {
            "custom" => NoiseMode::Custom,
            "off" => NoiseMode::Off,
            _ => NoiseMode::Adaptive,
        })),
        "level" => {
            if let Some(l) = num() {
                act(Action::SetTransparency(l.min(100) as u8));
            }
        }
        "wind" => {
            if let Some(w) = num().and_then(|n| AntiWind::from_u8(n as u8)) {
                act(Action::SetAntiWind(w));
            }
        }
        "preset" => {
            if let Some((_, bands)) = num().and_then(|i| PRESETS.get(i as usize)) {
                act(Action::SetEqBands(*bands));
            }
        }
        "upreset" => {
            if let Some(p) =
                num().and_then(|i| shared.settings().custom_presets.get(i as usize).cloned())
            {
                act(Action::SetEqBands(p.bands));
            }
        }
        "eq" => match arg {
            "neutral" => act(Action::SetEqEnabled(false)),
            "custom" => act(Action::SetEqBands(
                shared
                    .settings()
                    .custom_eq
                    .unwrap_or([0.0; protocol::eq::BANDS]),
            )),
            _ => {}
        },
        "bass" => act(Action::SetBassBoost(
            !h.is_some_and(|h| h.eq.bass_boost && h.eq_active()),
        )),
        "podcast" => act(Action::SetPodcast(!h.is_some_and(|h| h.podcast()))),
        "onhead" => act(Action::SetOnHeadDetection(
            h.and_then(|h| h.on_head_detection) != Some(true),
        )),
        "smartpause" => act(Action::SetSmartPause(
            h.and_then(|h| h.smart_pause) != Some(true),
        )),
        "autocall" => act(Action::SetAutoCall(
            h.and_then(|h| h.auto_call) != Some(true),
        )),
        "comfort" => act(Action::SetComfortCall(
            h.and_then(|h| h.comfort_call) != Some(true),
        )),
        "touchlock" => act(Action::SetTouchLock(
            h.and_then(|h| h.touch_lock) != Some(true),
        )),
        "autooff" => {
            if let Some(s) = num() {
                act(Action::SetAutoOff(s as u16));
            }
        }
        "prompts" => {
            if let Some(m) = num().and_then(|n| PromptMode::from_u8(n as u8)) {
                act(Action::SetPromptMode(m));
            }
        }
        "sidetone" => {
            if let Some(v) = num() {
                act(Action::SetSidetone(v as u8));
            }
        }
        "host" => {
            if let Some(i) = num().map(|i| i as u8)
                && let Some(host) = h.and_then(|h| h.paired_hosts.iter().find(|x| x.index == i))
            {
                act(if host.connected {
                    Action::DisconnectHost(i)
                } else {
                    Action::ConnectHost(i)
                });
            }
        }
        "device" => act(Action::SelectDevice(arg.to_string())),
        _ => {}
    }
    shared.wake_all();
}

