//! App settings and notification memory, persisted as JSON in the data folder.
//! "Start with Windows" is not stored here: the registry entry managed by the autostart plugin is the
//! single source of truth.

use std::path::Path;

use cuw_core::notify::{NotifyConfig, NotifyState};
use cuw_core::text::Lang;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePref {
    /// Follow the Windows display language.
    #[default]
    Auto,
    En,
    Fr,
}

impl LanguagePref {
    pub fn resolve(self, system_is_french: bool) -> Lang {
        match self {
            LanguagePref::En => Lang::En,
            LanguagePref::Fr => Lang::Fr,
            LanguagePref::Auto if system_is_french => Lang::Fr,
            LanguagePref::Auto => Lang::En,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: LanguagePref,
    pub notifications: NotifyConfig,
    pub pinned: bool,
    /// Physical position of the pinned widget.
    pub pinned_position: Option<(i32, i32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: LanguagePref::Auto,
            notifications: NotifyConfig::default(),
            pinned: false,
            pinned_position: None,
        }
    }
}

impl Settings {
    /// Keep thresholds sane whatever the settings view or a hand-edited file sends: finite, within
    /// 1..=100, sorted, no duplicates.
    pub fn sanitise(&mut self) {
        for ts in self.notifications.thresholds.values_mut() {
            ts.retain(|t| t.is_finite() && (1.0..=100.0).contains(t));
            ts.sort_by(|a, b| a.total_cmp(b));
            ts.dedup();
        }
    }
}

/// Read a JSON file; a missing or unreadable file yields the default.
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

/// Serialise first, then write a temporary file next to the destination and rename it over: the
/// previous file stays intact until the new one is complete.
pub fn save<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)
}

pub fn load_notify_state(path: &Path) -> NotifyState {
    load(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"pinned":true}"#).unwrap();
        assert!(s.pinned);
        assert_eq!(s.language, LanguagePref::Auto);
        assert_eq!(s.notifications, NotifyConfig::default());
    }

    #[test]
    fn sanitise_thresholds() {
        let mut s = Settings::default();
        s.notifications.thresholds.insert(
            "five_hour".into(),
            vec![95.0, 0.0, 80.0, 150.0, 80.0, f64::NAN],
        );
        s.sanitise();
        assert_eq!(s.notifications.thresholds["five_hour"], vec![80.0, 95.0]);
    }

    #[test]
    fn language() {
        assert_eq!(LanguagePref::Auto.resolve(true), Lang::Fr);
        assert_eq!(LanguagePref::Auto.resolve(false), Lang::En);
        assert_eq!(LanguagePref::Fr.resolve(false), Lang::Fr);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("cuw-settings-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let s = Settings {
            pinned: true,
            pinned_position: Some((-100, 200)),
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        save(&path, &s).unwrap(); // overwrite works
        assert_eq!(load::<Settings>(&path), s);
        std::fs::write(&path, "{ broken").unwrap();
        assert_eq!(load::<Settings>(&path), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
