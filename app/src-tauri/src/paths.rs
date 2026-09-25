//! Where the app reads and writes. Every source is read-only; the app writes only in its data folder.

use std::path::PathBuf;

/// Same override as the bridge (tests, portable use).
pub const DATA_DIR_ENV: &str = "CUW_DATA_DIR";

/// `%APPDATA%\ClaudeUsageWidget` (shared with the bridge), or `$CUW_DATA_DIR` when set.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ClaudeUsageWidget")
}

/// Source A, written by the bridge.
pub fn state_file() -> PathBuf {
    data_dir().join("state.json")
}

pub fn history_file() -> PathBuf {
    data_dir().join("history.jsonl")
}

/// The app's own files.
pub fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn notify_state_file() -> PathBuf {
    data_dir().join("notify-state.json")
}

/// Source B: `~/.claude.json`, or `$CLAUDE_CONFIG_DIR/.claude.json` when Claude Code is configured so.
pub fn claude_json() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d).join(".claude.json"));
    }
    std::env::var_os("USERPROFILE")
        .filter(|d| !d.is_empty())
        .map(|d| PathBuf::from(d).join(".claude.json"))
}

/// Source C candidates: classic install under `%APPDATA%\Claude`, and the packaged (MSIX) install under
/// `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude`. The newer file wins (see `data.rs`).
pub fn desktop_history_candidates() -> Vec<PathBuf> {
    const FILE: &str = "plan-usage-history.json";
    let mut out = Vec::new();
    if let Some(app) = std::env::var_os("APPDATA") {
        out.push(PathBuf::from(app).join("Claude").join(FILE));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let packages = PathBuf::from(local).join("Packages");
        if let Ok(entries) = std::fs::read_dir(&packages) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("Claude_") {
                    out.push(
                        e.path()
                            .join("LocalCache")
                            .join("Roaming")
                            .join("Claude")
                            .join(FILE),
                    );
                }
            }
        }
    }
    out
}
