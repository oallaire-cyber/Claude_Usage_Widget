//! Tray icon, its menu, and the three windows: popup card, pinned widget, settings.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use cuw_core::text::{tooltip, Lang};
use cuw_core::tray::{icon_size_for_scale, render_ring, Tone};
use cuw_core::view::View;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt as _;

use crate::engine::{to_local, Msg, Shared};
use crate::placement::{popup_position, visible_on, Rect};
use crate::sys;

pub const TRAY_ID: &str = "main";
pub const POPUP: &str = "popup";
pub const PINNED: &str = "pinned";
pub const SETTINGS: &str = "settings";

pub const POPUP_WIDTH: f64 = 360.0;
const POPUP_MARGIN: f64 = 12.0;
const PINNED_SIZE: (f64, f64) = (256.0, 76.0);
/// A tray click arriving this soon after the popup lost focus is the click that closed it.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// Popup bookkeeping (short locks only, never held across Tauri calls).
pub struct PopupState {
    pub hidden_at: Option<Instant>,
    pub anchor: Option<Rect>,
    /// Content height reported by the frontend, logical px.
    pub height: f64,
}

impl Default for PopupState {
    fn default() -> Self {
        PopupState {
            hidden_at: None,
            anchor: None,
            height: 460.0,
        }
    }
}

pub struct PopupCell(pub Mutex<PopupState>);

// ---------------------------------------------------------------------------------------------
// Tray

struct MenuLabels {
    pin: &'static str,
    settings: &'static str,
    autostart: &'static str,
    quit: &'static str,
}

fn labels(lang: Lang) -> MenuLabels {
    match lang {
        Lang::En => MenuLabels {
            pin: "Pin widget",
            settings: "Settings…",
            autostart: "Start with Windows",
            quit: "Quit",
        },
        Lang::Fr => MenuLabels {
            pin: "Épingler le widget",
            settings: "Paramètres…",
            autostart: "Lancer au démarrage de Windows",
            quit: "Quitter",
        },
    }
}

pub struct MenuHandles {
    pin: CheckMenuItem<tauri::Wry>,
    autostart: CheckMenuItem<tauri::Wry>,
}

fn build_menu(
    app: &AppHandle,
    lang: Lang,
    pinned: bool,
    autostart: bool,
) -> tauri::Result<(Menu<tauri::Wry>, MenuHandles)> {
    let l = labels(lang);
    let pin = CheckMenuItem::with_id(app, "pin", l.pin, true, pinned, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", l.settings, true, None::<&str>)?;
    let auto =
        CheckMenuItem::with_id(app, "autostart", l.autostart, true, autostart, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&pin, &settings, &sep, &auto, &sep2, &quit])?;
    Ok((
        menu,
        MenuHandles {
            pin,
            autostart: auto,
        },
    ))
}

fn tray_size(app: &AppHandle) -> u32 {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    icon_size_for_scale(scale)
}

/// Last values pushed to the tray, so only real changes reach Windows. Owned by the engine thread.
#[derive(Default)]
pub struct TrayCache {
    icon: Option<(u32, i64, Tone)>,
    tooltip: String,
    menu: Option<(Lang, MenuHandles)>,
    checks: Option<(bool, bool)>,
}

impl TrayCache {
    pub fn update(
        &mut self,
        app: &AppHandle,
        view: &View,
        lang: Lang,
        pinned: bool,
        autostart: bool,
    ) {
        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return;
        };
        let size = tray_size(app);
        let (fill, tone) = view.tray();
        let key = (size, (fill * 1000.0).round() as i64, tone);
        if self.icon != Some(key) {
            let rgba = render_ring(size, fill, tone);
            if tray.set_icon(Some(Image::new(&rgba, size, size))).is_ok() {
                self.icon = Some(key);
            }
        }
        let tip = tooltip(view, lang, &to_local);
        if tip != self.tooltip {
            let _ = tray.set_tooltip(Some(&tip));
            self.tooltip = tip;
        }
        if self.menu.as_ref().map(|(l, _)| *l) != Some(lang) {
            if let Ok((menu, handles)) = build_menu(app, lang, pinned, autostart) {
                let _ = tray.set_menu(Some(menu));
                self.menu = Some((lang, handles));
                self.checks = Some((pinned, autostart));
            }
        }
        // Clicking a check item toggles it by itself; re-sync it to the real state every time.
        if let Some((_, h)) = &self.menu {
            if self.checks != Some((pinned, autostart))
                || h.pin.is_checked().ok() != Some(pinned)
                || h.autostart.is_checked().ok() != Some(autostart)
            {
                let _ = h.pin.set_checked(pinned);
                let _ = h.autostart.set_checked(autostart);
                self.checks = Some((pinned, autostart));
            }
        }
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let size = tray_size(app);
    let rgba = render_ring(size, 0.0, Tone::Grey);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::new(&rgba, size, size))
        .tooltip("Claude Usage Widget")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let pos = rect.position.to_physical::<i32>(1.0);
                let size = rect.size.to_physical::<u32>(1.0);
                let anchor = Rect {
                    x: pos.x,
                    y: pos.y,
                    w: size.width as i32,
                    h: size.height as i32,
                };
                toggle_popup(tray.app_handle(), Some(anchor));
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "pin" => {
                let pinned = app
                    .state::<Shared>()
                    .settings
                    .lock()
                    .map(|s| s.pinned)
                    .unwrap_or(false);
                set_pinned(app, !pinned);
            }
            "settings" => open_settings(app),
            "autostart" => {
                let al = app.autolaunch();
                let _ = if al.is_enabled().unwrap_or(false) {
                    al.disable()
                } else {
                    al.enable()
                };
                app.state::<Shared>().send(Msg::SettingsChanged);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)
}

// ---------------------------------------------------------------------------------------------
// Popup

pub fn create_popup(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let h = app
        .state::<PopupCell>()
        .0
        .lock()
        .map(|p| p.height)
        .unwrap_or(460.0);
    let w = WebviewWindowBuilder::new(app, POPUP, WebviewUrl::App("index.html".into()))
        .title("Claude Usage Widget")
        .inner_size(POPUP_WIDTH, h)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false)
        .focused(false)
        .build()?;
    round(&w);
    Ok(w)
}

fn round<R: Runtime>(w: &WebviewWindow<R>) {
    #[cfg(windows)]
    if let Ok(hwnd) = w.hwnd() {
        sys::round_corners(hwnd.0);
    }
    #[cfg(not(windows))]
    let _ = w;
}

fn rect_of(pos: PhysicalPosition<i32>, size: tauri::PhysicalSize<u32>) -> Rect {
    Rect {
        x: pos.x,
        y: pos.y,
        w: size.width as i32,
        h: size.height as i32,
    }
}

/// The tray icon's rectangle, or a point in the bottom-right corner of the primary work area.
fn default_anchor(app: &AppHandle) -> Option<Rect> {
    if let Some(r) = app
        .tray_by_id(TRAY_ID)
        .and_then(|t| t.rect().ok().flatten())
    {
        let p = r.position.to_physical::<i32>(1.0);
        let s = r.size.to_physical::<u32>(1.0);
        if s.width > 0 {
            return Some(rect_of(p, s));
        }
    }
    let m = app.primary_monitor().ok().flatten()?;
    let wa = m.work_area();
    Some(Rect {
        x: wa.position.x + wa.size.width as i32 - 2,
        y: wa.position.y + wa.size.height as i32 - 2,
        w: 1,
        h: 1,
    })
}

fn position_popup(app: &AppHandle, w: &WebviewWindow, anchor: Rect) {
    let (cx, cy) = anchor.centre();
    let Some(m) = app
        .monitor_from_point(cx as f64, cy as f64)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())
    else {
        return;
    };
    let scale = m.scale_factor();
    let height = app
        .state::<PopupCell>()
        .0
        .lock()
        .map(|p| p.height)
        .unwrap_or(460.0);
    let size = (
        (POPUP_WIDTH * scale).round() as i32,
        (height * scale).round() as i32,
    );
    let monitor = rect_of(*m.position(), *m.size());
    let wa = m.work_area();
    let work = rect_of(wa.position, wa.size);
    let (x, y) = popup_position(monitor, work, anchor, size, (POPUP_MARGIN * scale) as i32);
    // Moving to a monitor with another scale makes Windows resize the window: set the logical size
    // again after the move, then the final position.
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.set_size(LogicalSize::new(POPUP_WIDTH, height));
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

pub fn show_popup(app: &AppHandle, anchor: Option<Rect>) {
    let Some(w) = app.get_webview_window(POPUP) else {
        return;
    };
    let anchor = anchor.or_else(|| default_anchor(app));
    if let Ok(mut p) = app.state::<PopupCell>().0.lock() {
        p.anchor = anchor;
    }
    if let Some(a) = anchor {
        position_popup(app, &w, a);
    }
    let _ = w.show();
    let _ = w.set_focus();
    let _ = w.emit("popup-shown", ());
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(POPUP) {
        if let Ok(mut p) = app.state::<PopupCell>().0.lock() {
            p.hidden_at = Some(Instant::now());
        }
        let _ = w.hide();
    }
}

pub fn toggle_popup(app: &AppHandle, anchor: Option<Rect>) {
    let Some(w) = app.get_webview_window(POPUP) else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        hide_popup(app);
        return;
    }
    let just_closed = app
        .state::<PopupCell>()
        .0
        .lock()
        .ok()
        .and_then(|p| p.hidden_at)
        .is_some_and(|t| t.elapsed() < REOPEN_GUARD);
    if !just_closed {
        show_popup(app, anchor);
    }
}

/// The frontend reports its content height; resize (and re-anchor when visible).
pub fn resize_popup(app: &AppHandle, height: f64) {
    let height = height.clamp(120.0, 900.0);
    let anchor = match app.state::<PopupCell>().0.lock() {
        Ok(mut p) => {
            if (p.height - height).abs() < 0.5 {
                return;
            }
            p.height = height;
            p.anchor
        }
        Err(_) => return,
    };
    let Some(w) = app.get_webview_window(POPUP) else {
        return;
    };
    let _ = w.set_size(LogicalSize::new(POPUP_WIDTH, height));
    if w.is_visible().unwrap_or(false) {
        if let Some(a) = anchor {
            position_popup(app, &w, a);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Pinned widget

/// Creating a WebView from an event handler on the main thread can deadlock on Windows: do it from a
/// worker thread.
fn off_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || f(&app));
}

pub fn set_pinned(app: &AppHandle, on: bool) {
    let shared = app.state::<Shared>();
    if let Ok(mut s) = shared.settings.lock() {
        s.pinned = on;
    }
    shared.send(Msg::SaveSettings);
    shared.send(Msg::SettingsChanged);
    if on {
        off_main(app, |app| {
            let _ = show_pinned(app);
        });
    } else if let Some(w) = app.get_webview_window(PINNED) {
        let _ = w.destroy();
    }
}

pub fn show_pinned(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(PINNED) {
        w.show()?;
        return Ok(());
    }
    let w = WebviewWindowBuilder::new(app, PINNED, WebviewUrl::App("index.html".into()))
        .title("Claude Usage Widget")
        .inner_size(PINNED_SIZE.0, PINNED_SIZE.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .build()?;

    let saved = app
        .state::<Shared>()
        .settings
        .lock()
        .ok()
        .and_then(|s| s.pinned_position);
    let works: Vec<Rect> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| rect_of(m.work_area().position, m.work_area().size))
        .collect();
    let size = w
        .outer_size()
        .map(|s| (s.width as i32, s.height as i32))
        .unwrap_or((256, 76));
    let pos = match saved {
        Some(p) if visible_on(p, size, &works) => Some(p),
        _ => app.primary_monitor().ok().flatten().map(|m| {
            let wa = m.work_area();
            let margin = (24.0 * m.scale_factor()) as i32;
            (
                wa.position.x + wa.size.width as i32 - size.0 - margin,
                wa.position.y + wa.size.height as i32 - size.1 - margin,
            )
        }),
    };
    if let Some((x, y)) = pos {
        w.set_position(PhysicalPosition::new(x, y))?;
    }
    w.show()?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Settings

pub fn open_settings(app: &AppHandle) {
    off_main(app, |app| {
        if let Some(w) = app.get_webview_window(SETTINGS) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
            return;
        }
        let title = match app.state::<Shared>().lang() {
            Lang::En => "Settings — Claude Usage Widget",
            Lang::Fr => "Paramètres — Claude Usage Widget",
        };
        let _ = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(460.0, 640.0)
            .min_inner_size(400.0, 480.0)
            .center()
            .focused(true)
            .build();
    });
}
