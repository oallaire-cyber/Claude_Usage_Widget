//! Adapters turning each local source into [`Observation`]s.
//!
//! - A: the bridge's `state.json` (documented status-line data).
//! - B: `cachedUsageUtilization` in `~/.claude.json` (undocumented; absent in Claude Code 2.1.282).
//! - C: Claude Desktop's `plan-usage-history.json` (undocumented; percentages only).
//!
//! B and C are parsed defensively: any shape change yields fewer observations, never an error.
//! Only the whitelisted fields are deserialised; everything else in those files is skipped unread.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::bridge_state::BridgeState;
use crate::statusline::number;
use crate::time::parse_timestamp;
use crate::window::{normalise_id, EXTRA_USAGE, FIVE_HOUR, SEVEN_DAY};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A — Claude Code status line, via the bridge.
    StatusLine,
    /// B — Claude Code's own cache in `~/.claude.json`.
    ClaudeCodeCache,
    /// C — Claude Desktop usage history.
    DesktopHistory,
}

/// One source's view of one window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub used_percentage: f64,
    pub resets_at: Option<i64>,
    /// When the source last saw this value (Unix seconds).
    pub as_of: i64,
    pub source: Source,
}

/// A: every window stored by the bridge, dated by when a payload last reported it.
pub fn from_bridge_state(st: &BridgeState) -> Vec<Observation> {
    st.windows
        .iter()
        .map(|(id, w)| Observation {
            id: id.clone(),
            used_percentage: w.used_percentage,
            resets_at: w.resets_at,
            as_of: w.observed_at,
            source: Source::StatusLine,
        })
        .collect()
}

/// B: `~/.claude.json`. Only the `cachedUsageUtilization` key is deserialised. `file_mtime` dates the
/// values when the block carries no timestamp of its own.
///
/// Assumed shape (the block was absent when this was written — see DECISIONS.md D-13): an object
/// whose children are windows `{ "utilization" | "used_percentage" | "percent": n, "resets_at" |
/// "resetsAt": t }`, optionally wrapped in `{ "data": {...}, "timestamp": t }`.
pub fn from_claude_code_cache(json: &str, file_mtime: i64) -> Vec<Observation> {
    #[derive(Deserialize)]
    struct Doc {
        #[serde(rename = "cachedUsageUtilization", default)]
        block: Option<Value>,
    }
    let Ok(doc) = serde_json::from_str::<Doc>(json.trim_start_matches('\u{feff}')) else {
        return vec![];
    };
    let Some(Value::Object(block)) = doc.block else {
        return vec![];
    };
    let stamp = [
        "timestamp",
        "updatedAt",
        "updated_at",
        "fetchedAt",
        "lastUpdated",
        "cachedAt",
    ]
    .iter()
    .find_map(|k| block.get(*k).and_then(parse_timestamp))
    .filter(|t| *t <= file_mtime + 60)
    .unwrap_or(file_mtime);
    let windows: &Map<String, Value> = match block.get("data").or_else(|| block.get("utilization"))
    {
        Some(Value::Object(inner)) => inner,
        _ => &block,
    };
    let mut out: Vec<Observation> = windows
        .iter()
        .filter_map(|(key, w)| {
            let w = w.as_object()?;
            let used = [
                "utilization",
                "used_percentage",
                "usedPercentage",
                "percent",
            ]
            .iter()
            .find_map(|k| w.get(*k).and_then(number))?;
            let resets_at = ["resets_at", "resetsAt", "reset_at", "resetAt"]
                .iter()
                .find_map(|k| w.get(*k).and_then(parse_timestamp));
            Some(Observation {
                id: normalise_id(key),
                used_percentage: used,
                resets_at,
                as_of: stamp,
                source: Source::ClaudeCodeCache,
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// C: Claude Desktop `plan-usage-history.json` → the newest sample's windows (no reset times).
pub fn from_desktop_history(json: &str) -> Vec<Observation> {
    #[derive(Deserialize)]
    struct Doc {
        #[serde(default)]
        samples: Vec<Value>,
    }
    let Ok(doc) = serde_json::from_str::<Doc>(json.trim_start_matches('\u{feff}')) else {
        return vec![];
    };
    let newest = doc
        .samples
        .iter()
        .filter_map(|s| {
            // `t` is Unix milliseconds in Desktop's file (FINDINGS.md).
            let t = match s.get("t")? {
                Value::Number(n) => Some((n.as_f64()? / 1000.0).floor() as i64),
                other => parse_timestamp(other),
            }
            .filter(|t| *t > 0)?;
            let u = s.get("u")?.as_object()?;
            Some((t, u))
        })
        .max_by_key(|(t, _)| *t);
    let Some((t, u)) = newest else {
        return vec![];
    };
    let mut out: Vec<Observation> = u
        .iter()
        .filter_map(|(key, v)| {
            let id = match key.as_str() {
                "fh" => FIVE_HOUR.to_string(),
                "sd" => SEVEN_DAY.to_string(),
                "xu" => EXTRA_USAGE.to_string(),
                other => format!("desktop_{}", normalise_id(other)),
            };
            Some(Observation {
                id,
                used_percentage: number(v)?,
                resets_at: None,
                as_of: t,
                source: Source::DesktopHistory,
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge_state::apply_readings;
    use crate::statusline::WindowReading;

    #[test]
    fn bridge_state_adapter() {
        let st = apply_readings(
            None,
            &[WindowReading {
                id: "five_hour".into(),
                used_percentage: 12.0,
                resets_at: Some(5000),
            }],
            100,
        );
        let obs = from_bridge_state(&st);
        assert_eq!(
            obs,
            vec![Observation {
                id: "five_hour".into(),
                used_percentage: 12.0,
                resets_at: Some(5000),
                as_of: 100,
                source: Source::StatusLine
            }]
        );
    }

    #[test]
    fn desktop_fixture_takes_newest_sample() {
        let json = include_str!("../../../fixtures/desktop/plan-usage-history.redacted.json");
        let obs = from_desktop_history(json);
        let ids: Vec<_> = obs
            .iter()
            .map(|o| (o.id.as_str(), o.used_percentage))
            .collect();
        assert_eq!(
            ids,
            [
                ("extra_usage", 100.0),
                ("five_hour", 21.0),
                ("seven_day", 10.0)
            ]
        );
        assert!(obs.iter().all(|o| o.resets_at.is_none()
            && o.as_of == 1_790_302_700
            && o.source == Source::DesktopHistory));
    }

    #[test]
    fn desktop_order_independent_and_defensive() {
        let json = r#"{"version":3,"samples":[
            {"t":2000000,"org":"x","u":{"fh":50,"new_key":7}},
            {"t":1000000,"org":"x","u":{"fh":10}},
            {"t":"bad","u":{"fh":99}},
            "junk"]}"#;
        let obs = from_desktop_history(json);
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[0].id, "desktop_new_key");
        assert_eq!(obs[1].used_percentage, 50.0);
        assert_eq!(obs[1].as_of, 2000);
        for bad in ["", "{", "[]", r#"{"samples":{}}"#, r#"{"samples":[]}"#] {
            assert!(from_desktop_history(bad).is_empty(), "{bad}");
        }
    }

    #[test]
    fn claude_code_cache_shapes() {
        // Other keys (e.g. MCP configs) are never deserialised.
        let json = r#"{"mcpServers":{"x":{"env":{"KEY":"secret"}}},
            "cachedUsageUtilization":{"timestamp":1000,
              "five_hour":{"utilization":42.0,"resets_at":"2025-02-01T16:00:00Z"},
              "sevenDayFable":{"utilization":7,"resetsAt":1738857600},
              "note":"x"}}"#;
        let obs = from_claude_code_cache(json, 2000);
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[0].id, "five_hour");
        assert_eq!(obs[0].resets_at, Some(1738425600));
        assert_eq!(obs[0].as_of, 1000);
        assert_eq!(obs[1].id, "seven_day_fable");
        assert_eq!(obs[1].source, Source::ClaudeCodeCache);

        let wrapped = r#"{"cachedUsageUtilization":{"data":{"seven_day":{"used_percentage":3}}}}"#;
        let obs = from_claude_code_cache(wrapped, 500);
        assert_eq!(obs[0].id, "seven_day");
        assert_eq!(obs[0].as_of, 500);
    }

    #[test]
    fn claude_code_cache_absent_or_odd() {
        for json in [
            r#"{"numStartups":3}"#,
            r#"{"cachedUsageUtilization":null}"#,
            r#"{"cachedUsageUtilization":[1,2]}"#,
            r#"{"cachedUsageUtilization":{"five_hour":{"utilization":"n/a"}}}"#,
            "not json",
            "",
        ] {
            assert!(from_claude_code_cache(json, 1).is_empty(), "{json}");
        }
    }

    #[test]
    fn claude_code_cache_future_timestamp_falls_back_to_mtime() {
        let json =
            r#"{"cachedUsageUtilization":{"timestamp":999999999,"five_hour":{"utilization":1}}}"#;
        assert_eq!(from_claude_code_cache(json, 1000)[0].as_of, 1000);
    }
}
