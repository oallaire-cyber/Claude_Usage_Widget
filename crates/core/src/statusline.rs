//! Source A: the JSON Claude Code pipes to the `statusLine` command.
//!
//! Only `rate_limits` is read. Every child object of `rate_limits` that carries a numeric
//! `used_percentage` is a window: `five_hour`, `seven_day`, the documented `spend_limit`, and any
//! window Claude Code adds later (e.g. a model-scoped weekly limit).

use serde_json::Value;

use crate::time::parse_timestamp;

/// One window as reported by one status-line payload.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowReading {
    pub id: String,
    pub used_percentage: f64,
    pub resets_at: Option<i64>,
}

/// Outcome of parsing a payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    /// Not JSON, or not a JSON object.
    Malformed,
    /// Valid JSON without a usable `rate_limits` object (non-subscriber, or before the first API response).
    NoRateLimits,
    /// `rate_limits` present; may hold zero windows.
    RateLimits(Vec<WindowReading>),
}

/// Parse raw stdin. Never panics.
pub fn parse_payload(input: &str) -> Payload {
    let input = input.trim_start_matches('\u{feff}');
    match serde_json::from_str::<Value>(input) {
        Ok(v @ Value::Object(_)) => match rate_limits(&v) {
            Some(readings) => Payload::RateLimits(readings),
            None => Payload::NoRateLimits,
        },
        _ => Payload::Malformed,
    }
}

/// Extract windows from a parsed payload. `None` when `rate_limits` is absent or not an object.
pub fn rate_limits(payload: &Value) -> Option<Vec<WindowReading>> {
    let obj = payload.get("rate_limits")?.as_object()?;
    let mut out: Vec<WindowReading> = obj
        .iter()
        .filter_map(|(id, w)| {
            let used = number(w.get("used_percentage")?)?;
            Some(WindowReading {
                id: id.clone(),
                used_percentage: used,
                resets_at: w.get("resets_at").and_then(parse_timestamp),
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Some(out)
}

/// A finite, non-negative number, also accepted as a numeric string.
pub(crate) fn number(v: &Value) -> Option<f64> {
    let f = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().trim_end_matches('%').parse().ok()?,
        _ => return None,
    };
    (f.is_finite() && f >= 0.0).then_some(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readings(input: &str) -> Vec<WindowReading> {
        match parse_payload(input) {
            Payload::RateLimits(r) => r,
            other => panic!("expected RateLimits, got {other:?}"),
        }
    }

    #[test]
    fn documented_example() {
        let r = readings(
            r#"{"model":{"display_name":"Opus"},"rate_limits":{
                "five_hour":{"used_percentage":23.5,"resets_at":1738425600},
                "seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#,
        );
        assert_eq!(r.len(), 2);
        assert_eq!(
            r[0],
            WindowReading {
                id: "five_hour".into(),
                used_percentage: 23.5,
                resets_at: Some(1738425600)
            }
        );
        assert_eq!(r[1].id, "seven_day");
        assert_eq!(r[1].resets_at, Some(1738857600));
    }

    #[test]
    fn iso_resets_at() {
        let r = readings(
            r#"{"rate_limits":{"five_hour":{"used_percentage":10,"resets_at":"2025-02-01T16:00:00Z"}}}"#,
        );
        assert_eq!(r[0].resets_at, Some(1738425600));
    }

    #[test]
    fn missing_rate_limits() {
        assert_eq!(
            parse_payload(r#"{"model":{"id":"x"}}"#),
            Payload::NoRateLimits
        );
        assert_eq!(
            parse_payload(r#"{"rate_limits":null}"#),
            Payload::NoRateLimits
        );
    }

    #[test]
    fn missing_seven_day() {
        let r = readings(
            r#"{"rate_limits":{"five_hour":{"used_percentage":5,"resets_at":1738425600}}}"#,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "five_hour");
    }

    #[test]
    fn extra_windows_are_kept() {
        let r = readings(
            r#"{"rate_limits":{
                "five_hour":{"used_percentage":5,"resets_at":1},
                "seven_day_fable":{"used_percentage":12.5,"resets_at":1738857600},
                "spend_limit":{"used_percentage":104.2,"resets_at":1740787200},
                "not_a_window":{"foo":1},
                "scalar":3}}"#,
        );
        let ids: Vec<_> = r.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["five_hour", "seven_day_fable", "spend_limit"]);
        assert_eq!(r[2].used_percentage, 104.2);
    }

    #[test]
    fn empty_rate_limits_object() {
        assert_eq!(
            parse_payload(r#"{"rate_limits":{}}"#),
            Payload::RateLimits(vec![])
        );
    }

    #[test]
    fn malformed_and_empty_input() {
        for input in ["", "   ", "{", "not json", "[1,2]", "42", "\"rate_limits\""] {
            assert_eq!(parse_payload(input), Payload::Malformed, "{input:?}");
        }
    }

    #[test]
    fn bom_and_bad_values() {
        let r = readings("\u{feff}{\"rate_limits\":{\"five_hour\":{\"used_percentage\":\"7\",\"resets_at\":\"never\"},\"seven_day\":{\"used_percentage\":-3}}}");
        assert_eq!(
            r,
            vec![WindowReading {
                id: "five_hour".into(),
                used_percentage: 7.0,
                resets_at: None
            }]
        );
    }
}
