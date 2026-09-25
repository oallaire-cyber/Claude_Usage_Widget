//! Parser tests over the committed fixture files in `/fixtures` (see docs/FINDINGS.md).

use cuw_core::statusline::{parse_payload, Payload};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/",
            $name
        ))
    };
}

fn ids(p: &Payload) -> Vec<(String, f64, Option<i64>)> {
    match p {
        Payload::RateLimits(r) => r
            .iter()
            .map(|w| (w.id.clone(), w.used_percentage, w.resets_at))
            .collect(),
        other => panic!("expected rate limits, got {other:?}"),
    }
}

#[test]
fn documented_payload() {
    let p = parse_payload(fixture!("statusline/documented.json"));
    assert_eq!(
        ids(&p),
        vec![
            ("five_hour".into(), 23.5, Some(1738425600)),
            ("seven_day".into(), 41.2, Some(1738857600))
        ]
    );
}

#[test]
fn iso_payload_matches_unix_payload() {
    assert_eq!(
        ids(&parse_payload(fixture!("statusline/iso-resets-at.json"))),
        ids(&parse_payload(fixture!("statusline/documented.json")))
    );
}

#[test]
fn missing_rate_limits_and_missing_seven_day() {
    assert_eq!(
        parse_payload(fixture!("statusline/no-rate-limits.json")),
        Payload::NoRateLimits
    );
    let p = parse_payload(fixture!("statusline/no-seven-day.json"));
    assert_eq!(ids(&p).len(), 1);
}

#[test]
fn extra_windows() {
    let p = parse_payload(fixture!("statusline/extra-window.json"));
    let names: Vec<String> = ids(&p).into_iter().map(|w| w.0).collect();
    assert_eq!(
        names,
        ["five_hour", "seven_day", "seven_day_fable", "spend_limit"]
    );
}

#[test]
fn malformed_and_empty() {
    assert_eq!(
        parse_payload(fixture!("statusline/malformed-truncated.json")),
        Payload::Malformed
    );
    assert_eq!(
        parse_payload(fixture!("statusline/empty.json")),
        Payload::Malformed
    );
}
