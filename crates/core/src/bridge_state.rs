//! The bridge's persisted state (`state.json`) and history samples (`history.jsonl`).
//!
//! Pure functions only; the bridge binary does the file I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::statusline::WindowReading;

pub const STATE_SCHEMA: u32 = 1;

/// Two `resets_at` values closer than this belong to the same window cycle (absorbs rounding
/// differences between sessions).
pub const SAME_CYCLE_TOLERANCE_SECS: i64 = 60;

/// At most one history sample per window per this many seconds.
pub const SAMPLE_INTERVAL_SECS: i64 = 60;

/// History samples older than this are pruned.
pub const HISTORY_RETENTION_SECS: i64 = 14 * 86_400;

/// Pruning rewrites the history file, so it runs at most this often.
pub const PRUNE_INTERVAL_SECS: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredWindow {
    pub used_percentage: f64,
    #[serde(default)]
    pub resets_at: Option<i64>,
    /// Last time a payload reported this window.
    pub observed_at: i64,
    /// Last time a history sample was appended for this window.
    #[serde(default)]
    pub last_sample_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeState {
    pub schema: u32,
    /// Last time any payload with `rate_limits` arrived.
    pub updated_at: i64,
    #[serde(default)]
    pub windows: BTreeMap<String, StoredWindow>,
    #[serde(default)]
    pub last_prune_at: Option<i64>,
}

impl BridgeState {
    pub fn new(now: i64) -> Self {
        BridgeState {
            schema: STATE_SCHEMA,
            updated_at: now,
            windows: BTreeMap::new(),
            last_prune_at: None,
        }
    }

    /// Parse a state file defensively: anything unreadable yields `None` (the bridge then starts fresh).
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str::<BridgeState>(s)
            .ok()
            .filter(|st| st.schema == STATE_SCHEMA)
    }
}

fn same_cycle(a: Option<i64>, b: Option<i64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= SAME_CYCLE_TOLERANCE_SECS,
        (None, None) => true,
        _ => false,
    }
}

/// Fold one payload's readings into the previous state.
///
/// Several Claude Code sessions may run at once and each pipes its own view. Per window:
/// - same cycle (`resets_at` equal within tolerance): keep the **highest** `used_percentage`, so a
///   quieter session never drags the value down;
/// - newer cycle (later `resets_at`): replace;
/// - older cycle (a session still repeating a previous window): ignore;
/// - window absent from this payload: keep the stored one (Claude Code drops windows whose
///   `resets_at` has passed; the app turns those into `window_reset`).
pub fn apply_readings(
    prev: Option<BridgeState>,
    readings: &[WindowReading],
    now: i64,
) -> BridgeState {
    let mut st = prev.unwrap_or_else(|| BridgeState::new(now));
    st.schema = STATE_SCHEMA;
    st.updated_at = now;
    for r in readings {
        match st.windows.get_mut(&r.id) {
            Some(w) if same_cycle(w.resets_at, r.resets_at) => {
                w.used_percentage = w.used_percentage.max(r.used_percentage);
                w.observed_at = now;
                // Keep the later of two near-identical reset times.
                w.resets_at = w.resets_at.max(r.resets_at);
            }
            Some(w) if is_older(r.resets_at, w.resets_at) => {}
            Some(w) => {
                w.used_percentage = r.used_percentage;
                w.resets_at = r.resets_at;
                w.observed_at = now;
            }
            None => {
                st.windows.insert(
                    r.id.clone(),
                    StoredWindow {
                        used_percentage: r.used_percentage,
                        resets_at: r.resets_at,
                        observed_at: now,
                        last_sample_at: None,
                    },
                );
            }
        }
    }
    st
}

/// `candidate` belongs to an earlier cycle than `stored`. A reading without `resets_at` is never
/// "older" (it cannot be placed) and replaces the stored value.
fn is_older(candidate: Option<i64>, stored: Option<i64>) -> bool {
    matches!((candidate, stored), (Some(c), Some(s)) if c < s)
}

/// One line of `history.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Sample time, Unix seconds.
    pub t: i64,
    /// Window id.
    pub w: String,
    /// Used percentage (after the multi-session max).
    pub p: f64,
    /// Window `resets_at`, identifies the cycle.
    #[serde(default)]
    pub r: Option<i64>,
}

/// Samples due now (at most one per window per minute), and the state updated to record them.
pub fn due_samples(st: &mut BridgeState, now: i64) -> Vec<Sample> {
    let mut out = Vec::new();
    for (id, w) in st.windows.iter_mut() {
        if w.observed_at != now {
            continue; // not reported by this payload
        }
        if w.last_sample_at
            .is_some_and(|t| now - t < SAMPLE_INTERVAL_SECS && now >= t)
        {
            continue;
        }
        w.last_sample_at = Some(now);
        out.push(Sample {
            t: now,
            w: id.clone(),
            p: w.used_percentage,
            r: w.resets_at,
        });
    }
    out
}

/// Whether the history file should be pruned now; records the prune time when it returns true.
pub fn prune_due(st: &mut BridgeState, now: i64) -> bool {
    let due = st
        .last_prune_at
        .is_none_or(|t| now - t >= PRUNE_INTERVAL_SECS || now < t);
    if due {
        st.last_prune_at = Some(now);
    }
    due
}

/// Keep only well-formed lines younger than the retention period. Returns the new file content.
pub fn prune_history(content: &str, now: i64) -> String {
    let mut out = String::with_capacity(content.len());
    for line in content.lines() {
        if let Ok(s) = serde_json::from_str::<Sample>(line) {
            if now - s.t <= HISTORY_RETENTION_SECS {
                out.push_str(line.trim_end());
                out.push('\n');
            }
        }
    }
    out
}

/// Parse `history.jsonl`, skipping malformed lines.
pub fn parse_history(content: &str) -> Vec<Sample> {
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: &str, p: f64, resets: Option<i64>) -> WindowReading {
        WindowReading {
            id: id.into(),
            used_percentage: p,
            resets_at: resets,
        }
    }

    #[test]
    fn multi_session_keeps_max_within_same_cycle() {
        let st = apply_readings(None, &[r("five_hour", 30.0, Some(10_000))], 100);
        // A quieter session reports a lower value for the same window: ignored.
        let st = apply_readings(Some(st), &[r("five_hour", 12.0, Some(10_000))], 110);
        assert_eq!(st.windows["five_hour"].used_percentage, 30.0);
        assert_eq!(st.windows["five_hour"].observed_at, 110);
        // A busier session raises it.
        let st = apply_readings(Some(st), &[r("five_hour", 31.5, Some(10_030))], 120);
        assert_eq!(st.windows["five_hour"].used_percentage, 31.5);
        assert_eq!(st.windows["five_hour"].resets_at, Some(10_030));
    }

    #[test]
    fn new_cycle_replaces_and_old_cycle_is_ignored() {
        let st = apply_readings(None, &[r("five_hour", 90.0, Some(10_000))], 100);
        let st = apply_readings(Some(st), &[r("five_hour", 2.0, Some(28_000))], 10_100);
        assert_eq!(st.windows["five_hour"].used_percentage, 2.0);
        assert_eq!(st.windows["five_hour"].resets_at, Some(28_000));
        // A lagging session still reporting the previous cycle.
        let st = apply_readings(Some(st), &[r("five_hour", 95.0, Some(10_000))], 10_200);
        assert_eq!(st.windows["five_hour"].used_percentage, 2.0);
    }

    #[test]
    fn absent_window_is_kept() {
        let st = apply_readings(
            None,
            &[r("five_hour", 10.0, Some(1)), r("seven_day", 40.0, Some(2))],
            100,
        );
        let st = apply_readings(Some(st), &[r("five_hour", 11.0, Some(1))], 200);
        assert_eq!(st.windows["seven_day"].used_percentage, 40.0);
        assert_eq!(st.windows["seven_day"].observed_at, 100);
        assert_eq!(st.updated_at, 200);
    }

    #[test]
    fn extra_window_tracked_independently() {
        let st = apply_readings(
            None,
            &[
                r("five_hour", 10.0, Some(1)),
                r("seven_day_fable", 3.0, Some(5)),
            ],
            100,
        );
        assert_eq!(st.windows.len(), 2);
        assert_eq!(st.windows["seven_day_fable"].used_percentage, 3.0);
    }

    #[test]
    fn samples_at_most_once_per_minute_per_window() {
        let mut st = apply_readings(
            None,
            &[r("five_hour", 10.0, Some(1)), r("seven_day", 1.0, Some(2))],
            1000,
        );
        assert_eq!(due_samples(&mut st, 1000).len(), 2);
        let mut st = apply_readings(Some(st), &[r("five_hour", 11.0, Some(1))], 1030);
        assert!(due_samples(&mut st, 1030).is_empty());
        let mut st = apply_readings(Some(st), &[r("five_hour", 12.0, Some(1))], 1060);
        let s = due_samples(&mut st, 1060);
        assert_eq!(
            s,
            vec![Sample {
                t: 1060,
                w: "five_hour".into(),
                p: 12.0,
                r: Some(1)
            }]
        );
    }

    #[test]
    fn prune_drops_old_and_malformed_lines() {
        let now = 20 * 86_400;
        let content = format!(
            "{}\n{}\nnot json\n\n{}\n",
            r#"{"t":0,"w":"five_hour","p":1.0,"r":5}"#,
            format_args!(r#"{{"t":{},"w":"five_hour","p":2.0,"r":5}}"#, now - 3600),
            format_args!(r#"{{"t":{},"w":"seven_day","p":3.0}}"#, now - 13 * 86_400),
        );
        let out = prune_history(&content, now);
        let kept = parse_history(&out);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].p, 2.0);
        assert_eq!(kept[1].r, None);
    }

    #[test]
    fn prune_runs_daily() {
        let mut st = BridgeState::new(0);
        assert!(prune_due(&mut st, 100));
        assert!(!prune_due(&mut st, 100 + 3600));
        assert!(prune_due(&mut st, 100 + 86_400));
    }

    #[test]
    fn state_roundtrip_and_defensive_parse() {
        let st = apply_readings(None, &[r("five_hour", 10.0, Some(1))], 100);
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(BridgeState::from_json(&json), Some(st));
        assert_eq!(BridgeState::from_json("{"), None);
        assert_eq!(
            BridgeState::from_json(r#"{"schema":99,"updated_at":1}"#),
            None
        );
    }
}
