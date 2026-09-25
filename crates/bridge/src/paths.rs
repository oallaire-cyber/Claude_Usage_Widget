//! File locations and the install record written by `tools/install-bridge.ps1`.

use std::path::{Path, PathBuf};

use serde_json::Value;

pub const STATE_FILE: &str = "state.json";
pub const HISTORY_FILE: &str = "history.jsonl";
pub const LOCK_FILE: &str = "state.lock";
/// Written by the install script: `{ "previous_status_line": <object|null>, ... }`.
pub const INSTALL_FILE: &str = "install.json";

/// Overrides the data directory (tests, portable use).
pub const DATA_DIR_ENV: &str = "CUW_DATA_DIR";

/// `%APPDATA%\ClaudeUsageWidget`, or `$CUW_DATA_DIR` when set.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ClaudeUsageWidget")
}

/// The previous `statusLine.command` recorded by the install script, if any.
pub fn previous_command(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(data_dir.join(INSTALL_FILE)).ok()?;
    previous_command_from(&text)
}

pub fn previous_command_from(install_json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(install_json.trim_start_matches('\u{feff}')).ok()?;
    let prev = v.get("previous_status_line")?;
    if prev
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| t != "command")
    {
        return None;
    }
    let cmd = prev.get("command")?.as_str()?.trim();
    (!cmd.is_empty()).then(|| cmd.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_command_variants() {
        assert_eq!(
            previous_command_from(
                r#"{"previous_status_line":{"type":"command","command":"~/.claude/sl.sh","padding":1}}"#
            ),
            Some("~/.claude/sl.sh".into())
        );
        assert_eq!(
            previous_command_from(r#"{"previous_status_line":null}"#),
            None
        );
        assert_eq!(
            previous_command_from(r#"{"previous_status_line":{"type":"static","command":"x"}}"#),
            None
        );
        assert_eq!(
            previous_command_from(r#"{"previous_status_line":{"command":"  "}}"#),
            None
        );
        assert_eq!(previous_command_from("garbage"), None);
    }
}
