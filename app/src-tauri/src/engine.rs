//! The engine thread: re-reads sources when they change (and every 15 s, since statuses and countdowns
//! move with time), merges them, runs notifications, updates the tray and pushes the view to windows.
//!
//! All Tauri calls that update the UI happen on this thread, never while a lock is held: Tauri runs
//! them on the main thread and waits, and the main thread may itself be waiting for a lock.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use chrono::{Datelike, Local, TimeZone, Timelike};
use cuw_core::merge::merge;
use cuw_core::notify::{evaluate, NotifyState};
use cuw_core::text::{notification_text, Lang, LocalTime};
use cuw_core::view::{build_view, View};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_notification::NotificationExt;

use crate::data::DataStore;
use crate::settings::{self, Settings};
use crate::{paths, sys, ui};

const TICK: Duration = Duration::from_secs(15);
/// Change notifications come in bursts (temp file + rename, several files): wait this long, then
/// handle them once.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// Settings writes (e.g. while dragging the pinned widget) are grouped this long.
const SAVE_DELAY: Duration = Duration::from_millis(700);

pub enum Msg {
    /// A watched file changed.
    Refresh,
    /// Settings or autostart changed: recompute and repaint.
    SettingsChanged,
    /// Settings changed in memory; write them to disk soon.
    SaveSettings,
}

/// What every window receives (`ui-state` event and `get_ui_state`).
#[derive(Debug, Clone, Serialize)]
pub struct UiState {
    pub view: View,
    pub lang: Lang,
    pub accent: Option<String>,
    pub pinned: bool,
}

pub struct Shared {
    pub settings: Mutex<Settings>,
    pub ui: Mutex<Option<UiState>>,
    pub tx: Mutex<Sender<Msg>>,
    pub system_french: bool,
}

impl Shared {
    pub fn send(&self, m: Msg) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(m);
        }
    }

    pub fn lang(&self) -> Lang {
        let pref = self.settings.lock().map(|s| s.language).unwrap_or_default();
        pref.resolve(self.system_french)
    }
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn to_local(t: i64) -> LocalTime {
    match Local.timestamp_opt(t, 0).earliest() {
        Some(dt) => LocalTime {
            weekday: dt.weekday().num_days_from_monday() as u8,
            hour: dt.hour() as u8,
            minute: dt.minute() as u8,
        },
        None => LocalTime {
            weekday: 0,
            hour: 0,
            minute: 0,
        },
    }
}

fn save_settings_now(shared: &Shared) {
    let snapshot = shared.settings.lock().ok().map(|s| s.clone());
    if let Some(s) = snapshot {
        let _ = settings::save(&paths::settings_file(), &s);
    }
}

pub fn run(app: AppHandle, rx: Receiver<Msg>) {
    let mut store = DataStore::default();
    let mut notify_state: NotifyState = settings::load_notify_state(&paths::notify_state_file());
    let mut tray = ui::TrayCache::default();
    let mut save_due: Option<Instant> = None;

    loop {
        let wait = save_due
            .map(|d| d.saturating_duration_since(Instant::now()).min(TICK))
            .unwrap_or(TICK);
        let mut recompute = true;
        match rx.recv_timeout(wait) {
            Ok(Msg::SaveSettings) => {
                save_due.get_or_insert_with(|| Instant::now() + SAVE_DELAY);
                recompute = false;
            }
            Ok(Msg::Refresh) | Ok(Msg::SettingsChanged) => {
                std::thread::sleep(DEBOUNCE);
                while let Ok(m) = rx.try_recv() {
                    if let Msg::SaveSettings = m {
                        save_due.get_or_insert_with(|| Instant::now() + SAVE_DELAY);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                // A pending save alone does not need a recompute.
                recompute = save_due.is_none_or(|d| d > Instant::now());
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let shared = app.state::<Shared>();
        if save_due.is_some_and(|d| d <= Instant::now()) {
            save_due = None;
            save_settings_now(&shared);
        }
        if !recompute {
            continue;
        }

        store.refresh();
        let now = unix_now();
        let merged = merge(&store.observations(), now);
        let settings = match shared.settings.lock() {
            Ok(s) => s.clone(),
            Err(_) => continue,
        };
        let before = notify_state.clone();
        let notes = evaluate(&merged, &settings.notifications, &mut notify_state);
        if notify_state != before {
            let _ = settings::save(&paths::notify_state_file(), &notify_state);
        }
        let view = build_view(merged, store.history(), now);
        let lang = settings.language.resolve(shared.system_french);
        let ui_state = UiState {
            view,
            lang,
            accent: sys::accent_colour(),
            pinned: settings.pinned,
        };
        if let Ok(mut u) = shared.ui.lock() {
            *u = Some(ui_state.clone());
        }

        let autostart = app.autolaunch().is_enabled().unwrap_or(false);
        tray.update(&app, &ui_state.view, lang, settings.pinned, autostart);
        let _ = app.emit("ui-state", &ui_state);

        for n in notes {
            let (title, body) = notification_text(&n, now, lang, &to_local);
            let _ = app.notification().builder().title(title).body(body).show();
        }
    }
    save_settings_now(&app.state::<Shared>());
}
