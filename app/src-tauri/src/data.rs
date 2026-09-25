//! Loading the three sources (read-only) and the bridge's history. A file is re-read only when its
//! modification time changes. Only whitelisted fields are parsed (`cuw_core::sources`).
//!
//! A file caught mid-write (invalid JSON) keeps the previous values and is retried next time.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cuw_core::bridge_state::{parse_history, BridgeState, Sample};
use cuw_core::sources::{
    from_bridge_state, from_claude_code_cache, from_desktop_history, Observation,
};

use crate::paths;

#[derive(Default)]
struct Cached {
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    obs: Vec<Observation>,
}

#[derive(Default)]
pub struct DataStore {
    state: Cached,
    cache: Cached,
    desktop: Cached,
    history_mtime: Option<SystemTime>,
    history: Vec<Sample>,
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn unix(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn valid_json(s: &str) -> bool {
    serde_json::from_str::<serde::de::IgnoredAny>(s.trim_start_matches('\u{feff}')).is_ok()
}

/// Re-read `path` into `c` if it changed. `parse` gets the content and the file's mtime.
fn refresh_one(
    c: &mut Cached,
    path: Option<PathBuf>,
    parse: impl Fn(&str, i64) -> Vec<Observation>,
) -> bool {
    let m = path.as_deref().and_then(mtime);
    if path == c.path && m == c.mtime {
        return false;
    }
    let Some((p, m)) = path.clone().zip(m) else {
        // File gone: forget its values.
        let changed = !c.obs.is_empty();
        *c = Cached {
            path,
            ..Cached::default()
        };
        return changed;
    };
    let Ok(text) = std::fs::read_to_string(&p) else {
        return false; // locked or unreadable right now: retry later
    };
    if !valid_json(&text) {
        return false; // mid-write: keep previous values, retry later
    }
    c.obs = parse(&text, unix(m));
    c.path = Some(p);
    c.mtime = Some(m);
    true
}

impl DataStore {
    /// Re-read whatever changed. Returns true when any value may have changed.
    pub fn refresh(&mut self) -> bool {
        let mut changed = refresh_one(&mut self.state, Some(paths::state_file()), |s, _| {
            BridgeState::from_json(s.trim_start_matches('\u{feff}'))
                .map(|st| from_bridge_state(&st))
                .unwrap_or_default()
        });
        changed |= refresh_one(
            &mut self.cache,
            paths::claude_json(),
            from_claude_code_cache,
        );
        let newest_desktop = paths::desktop_history_candidates()
            .into_iter()
            .filter_map(|p| mtime(&p).map(|m| (m, p)))
            .max_by_key(|(m, _)| *m)
            .map(|(_, p)| p);
        changed |= refresh_one(&mut self.desktop, newest_desktop, |s, _| {
            from_desktop_history(s)
        });

        let hp = paths::history_file();
        let hm = mtime(&hp);
        if hm != self.history_mtime {
            self.history = std::fs::read_to_string(&hp)
                .map(|s| parse_history(&s))
                .unwrap_or_default();
            self.history_mtime = hm;
            changed = true;
        }
        changed
    }

    pub fn observations(&self) -> Vec<Observation> {
        let mut v = self.state.obs.clone();
        v.extend(self.cache.obs.iter().cloned());
        v.extend(self.desktop.obs.iter().cloned());
        v
    }

    pub fn history(&self) -> &[Sample] {
        &self.history
    }

    /// Folders worth watching for change notifications (those that exist).
    pub fn watch_dirs() -> Vec<PathBuf> {
        let mut dirs = vec![paths::data_dir()];
        if let Some(parent) = paths::claude_json().and_then(|p| p.parent().map(Path::to_path_buf)) {
            dirs.push(parent);
        }
        for p in paths::desktop_history_candidates() {
            if let Some(parent) = p.parent() {
                dirs.push(parent.to_path_buf());
            }
        }
        dirs.sort();
        dirs.dedup();
        dirs.retain(|d| d.is_dir());
        dirs
    }

    /// File names whose changes matter (events for other files in watched folders are ignored).
    pub fn is_relevant(path: &Path) -> bool {
        matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("state.json" | "history.jsonl" | ".claude.json" | "plan-usage-history.json")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevant_files() {
        assert!(DataStore::is_relevant(Path::new(r"C:\x\state.json")));
        assert!(DataStore::is_relevant(Path::new(
            r"C:\Users\u\.claude.json"
        )));
        assert!(!DataStore::is_relevant(Path::new(
            r"C:\Users\u\.claude.json.tmp"
        )));
        assert!(!DataStore::is_relevant(Path::new(r"C:\x\settings.json")));
    }

    #[test]
    fn partial_write_keeps_previous_values() {
        let dir = std::env::temp_dir().join(format!("cuw-data-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("plan-usage-history.json");
        std::fs::write(&p, r#"{"samples":[{"t":1790000000000,"u":{"fh":12}}]}"#).unwrap();
        let mut c = Cached::default();
        let parse = |s: &str, _| from_desktop_history(s);
        assert!(refresh_one(&mut c, Some(p.clone()), parse));
        assert_eq!(c.obs.len(), 1);
        assert!(
            !refresh_one(&mut c, Some(p.clone()), parse),
            "unchanged mtime"
        );
        // Truncated content with a new mtime: previous values kept.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&p, r#"{"samples":[{"t":17"#).unwrap();
        c.mtime = None; // force a re-read regardless of timestamp resolution
        assert!(!refresh_one(&mut c, Some(p.clone()), parse));
        assert_eq!(c.obs[0].used_percentage, 12.0);
        // File removed: values forgotten.
        std::fs::remove_file(&p).unwrap();
        c.mtime = Some(SystemTime::UNIX_EPOCH);
        assert!(refresh_one(&mut c, Some(p), parse));
        assert!(c.obs.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
