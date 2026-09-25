//! Claude Usage Widget: tray icon, popup card, pinned widget, notifications.
//!
//! The app reads local files only (PROMPT.md §3) and makes no network requests.

mod data;
mod engine;
mod paths;
mod placement;
mod settings;
mod sys;
mod ui;

use std::sync::Mutex;

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};

use engine::{Msg, Shared, UiState};
use settings::Settings;
use ui::{PopupCell, PINNED, POPUP};

/// Passed by the autostart entry, so a login start stays quiet (no popup).
const AUTOSTART_ARG: &str = "--autostart";

/// Keeps the file watcher alive for the app's lifetime.
struct WatcherCell(#[allow(dead_code)] Mutex<Option<notify::RecommendedWatcher>>);

#[tauri::command]
async fn get_ui_state(shared: tauri::State<'_, Shared>) -> Result<Option<UiState>, ()> {
    Ok(shared.ui.lock().ok().and_then(|u| u.clone()))
}

#[tauri::command]
async fn hide_popup(app: AppHandle) {
    ui::hide_popup(&app);
}

#[tauri::command]
async fn open_popup(app: AppHandle) {
    ui::show_popup(&app, None);
}

#[tauri::command]
async fn popup_resize(app: AppHandle, height: f64) {
    ui::resize_popup(&app, height);
}

#[tauri::command]
async fn open_settings(app: AppHandle) {
    ui::hide_popup(&app);
    ui::open_settings(&app);
}

#[tauri::command]
async fn set_pinned(app: AppHandle, pinned: bool) {
    ui::set_pinned(&app, pinned);
}

#[derive(Serialize)]
struct SettingsDto {
    settings: Settings,
    autostart: bool,
    lang: cuw_core::text::Lang,
}

#[tauri::command]
async fn get_settings(app: AppHandle) -> Result<SettingsDto, ()> {
    let shared = app.state::<Shared>();
    let settings = shared.settings.lock().map_err(|_| ())?.clone();
    Ok(SettingsDto {
        settings,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        lang: shared.lang(),
    })
}

/// Apply settings from the settings view. Only language and notifications come from the view;
/// the pinned state and position are owned by the app.
#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings, autostart: bool) -> Result<(), String> {
    let shared = app.state::<Shared>();
    let snapshot = {
        let mut s = shared.settings.lock().map_err(|e| e.to_string())?;
        s.language = settings.language;
        s.notifications = settings.notifications;
        s.sanitise();
        s.clone()
    };
    settings::save(&paths::settings_file(), &snapshot).map_err(|e| e.to_string())?;
    let al = app.autolaunch();
    if al.is_enabled().unwrap_or(false) != autostart {
        let r = if autostart { al.enable() } else { al.disable() };
        r.map_err(|e| e.to_string())?;
    }
    shared.send(Msg::SettingsChanged);
    Ok(())
}

fn start_watcher(tx: std::sync::mpsc::Sender<Msg>) -> Option<notify::RecommendedWatcher> {
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if ev.paths.iter().any(|p| data::DataStore::is_relevant(p)) {
                let _ = tx.send(Msg::Refresh);
            }
        }
    })
    .ok()?;
    for d in data::DataStore::watch_dirs() {
        let _ = w.watch(&d, RecursiveMode::NonRecursive);
    }
    Some(w)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = std::fs::create_dir_all(paths::data_dir());
    let mut initial: Settings = settings::load(&paths::settings_file());
    initial.sanitise();
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();

    let app = tauri::Builder::default()
        // Must be first: a second launch hands over to the running instance and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            ui::show_popup(app, None);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]),
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(Shared {
            settings: Mutex::new(initial),
            ui: Mutex::new(None),
            tx: Mutex::new(tx.clone()),
            system_french: sys::ui_language_is_french(),
        })
        .manage(PopupCell(Mutex::new(ui::PopupState::default())))
        .invoke_handler(tauri::generate_handler![
            get_ui_state,
            hide_popup,
            open_popup,
            popup_resize,
            open_settings,
            set_pinned,
            get_settings,
            save_settings
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            ui::create_tray(&handle)?;
            ui::create_popup(&handle)?;
            let pinned = app
                .state::<Shared>()
                .settings
                .lock()
                .map(|s| s.pinned)
                .unwrap_or(false);
            if pinned {
                ui::show_pinned(&handle)?;
            }
            app.manage(WatcherCell(Mutex::new(start_watcher(tx.clone()))));
            let engine_handle = handle.clone();
            std::thread::Builder::new()
                .name("cuw-engine".into())
                .spawn(move || engine::run(engine_handle, rx))?;
            let _ = tx.send(Msg::Refresh);

            // Started by hand (not at login): show the card once so the app is visibly running.
            if !std::env::args().any(|a| a == AUTOSTART_ARG) {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                    ui::show_popup(&h, None);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (POPUP, WindowEvent::Focused(false)) => ui::hide_popup(window.app_handle()),
            (POPUP, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                ui::hide_popup(window.app_handle());
            }
            (PINNED, WindowEvent::Moved(pos)) => {
                let shared = window.state::<Shared>();
                if let Ok(mut s) = shared.settings.lock() {
                    s.pinned_position = Some((pos.x, pos.y));
                }
                shared.send(Msg::SaveSettings);
            }
            (PINNED, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                ui::set_pinned(window.app_handle(), false);
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building the app");

    app.run(|_app, event| {
        // A tray app keeps running when its last window closes; only "Quit" exits.
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
