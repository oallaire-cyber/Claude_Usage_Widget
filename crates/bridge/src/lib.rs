//! `cuw-bridge`: Claude Code's `statusLine` command.
//!
//! 1. Reads the JSON payload on stdin; if it carries `rate_limits`, folds it into `state.json`
//!    (atomic write under a lock) and appends history samples.
//! 2. Prints the status line: the output of the previously configured status-line command when the
//!    install script recorded one (same stdin, 2 s timeout), else a compact default line.
//!
//! It must never break Claude Code: every failure degrades to printing the default line, and the
//! process always exits 0. It makes no network requests.

pub mod chain;
pub mod fsio;
pub mod paths;
#[cfg(windows)]
mod win;

use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cuw_core::bridge_state::{apply_readings, due_samples, prune_due, prune_history, BridgeState};
use cuw_core::statusline::{parse_payload, Payload};

/// Stdin is capped: the payload is a few KB; anything larger is not a status-line payload.
const MAX_STDIN: u64 = 4 * 1024 * 1024;
pub const CHAIN_TIMEOUT: Duration = Duration::from_secs(2);

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn read_stdin() -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = std::io::stdin()
        .lock()
        .take(MAX_STDIN)
        .read_to_end(&mut buf);
    buf
}

/// Update `state.json` / `history.jsonl` from one payload. Returns the resulting state (or the stored
/// one when the payload carried no `rate_limits`), used to render the default line.
pub fn record(data_dir: &Path, input: &[u8], now: i64) -> Option<BridgeState> {
    let text = String::from_utf8_lossy(input);
    let state_path = data_dir.join(paths::STATE_FILE);
    let readings = match parse_payload(&text) {
        Payload::RateLimits(r) => r,
        Payload::NoRateLimits | Payload::Malformed => {
            return std::fs::read_to_string(&state_path)
                .ok()
                .and_then(|s| BridgeState::from_json(&s));
        }
    };
    std::fs::create_dir_all(data_dir).ok()?;
    let _lock = fsio::FileLock::acquire(&data_dir.join(paths::LOCK_FILE));
    let prev = std::fs::read_to_string(&state_path)
        .ok()
        .and_then(|s| BridgeState::from_json(&s));
    let mut st = apply_readings(prev, &readings, now);
    let samples = due_samples(&mut st, now);
    let history_path = data_dir.join(paths::HISTORY_FILE);
    if prune_due(&mut st, now) {
        if let Ok(content) = std::fs::read_to_string(&history_path) {
            let _ = fsio::write_atomic(&history_path, prune_history(&content, now).as_bytes());
        }
    }
    if !samples.is_empty() {
        let mut lines = String::new();
        for s in &samples {
            if let Ok(l) = serde_json::to_string(s) {
                lines.push_str(&l);
                lines.push('\n');
            }
        }
        let _ = fsio::append(&history_path, lines.as_bytes());
    }
    if let Ok(json) = serde_json::to_vec_pretty(&st) {
        let _ = fsio::write_atomic(&state_path, &json);
    }
    Some(st)
}

/// Compact default line, e.g. `5h 14% · 7d 8%`. Windows whose reset time has passed show `0%`.
pub fn default_line(st: Option<&BridgeState>, now: i64) -> String {
    let pct = |id: &str| -> String {
        match st.and_then(|s| s.windows.get(id)) {
            Some(w) if w.resets_at.is_some_and(|r| now >= r) => "0%".into(),
            Some(w) => format!("{:.0}%", w.used_percentage),
            None => "--".into(),
        }
    };
    format!("5h {} \u{b7} 7d {}", pct("five_hour"), pct("seven_day"))
}

/// Whole bridge run. Returns the bytes to print.
pub fn run(input: &[u8], data_dir: &Path, now: i64) -> Vec<u8> {
    let st = record(data_dir, input, now);
    if std::env::var_os(chain::DEPTH_ENV).is_none() {
        if let Some(cmd) = paths::previous_command(data_dir) {
            if let Ok(out) = chain::run_chained(&cmd, input, CHAIN_TIMEOUT) {
                if !out.iter().all(u8::is_ascii_whitespace) {
                    return out;
                }
            }
        }
    }
    let mut line = default_line(st.as_ref(), now).into_bytes();
    line.push(b'\n');
    line
}

/// Entry point used by `main`: never panics outward, never fails.
pub fn main_entry() {
    std::panic::set_hook(Box::new(|_| {}));
    let input = read_stdin();
    let out = std::panic::catch_unwind(|| {
        let data_dir = paths::data_dir();
        run(&input, &data_dir, now_secs())
    })
    .unwrap_or_else(|_| b"5h -- \xc2\xb7 7d --\n".to_vec());
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(&out);
    let _ = stdout.flush();
}
