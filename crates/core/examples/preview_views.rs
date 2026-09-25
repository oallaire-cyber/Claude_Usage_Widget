//! Prints the views used by the frontend's browser preview (`app/src/preview/views.json`), computed by
//! the real merge / pace / tone code so the screenshots show what the app would show.
//!
//! Regenerate: `node tools/gen-preview-views.mjs`

use std::collections::BTreeMap;

use cuw_core::bridge_state::Sample;
use cuw_core::merge::merge;
use cuw_core::sources::{Observation, Source};
use cuw_core::view::{build_view, View};

/// 2026-09-23 12:32:00 UTC, a Wednesday (14:32 in Paris, the screenshots' time zone).
const NOW: i64 = 1_790_166_720;
const H: i64 = 3600;
const D: i64 = 86_400;

fn a(id: &str, p: f64, resets_in: i64, age: i64) -> Observation {
    Observation {
        id: id.into(),
        used_percentage: p,
        resets_at: Some(NOW + resets_in),
        as_of: NOW - age,
        source: Source::StatusLine,
    }
}

/// Samples rising linearly to `p` now, one every 5 minutes over the last hour.
fn ramp(id: &str, p: f64, per_hour: f64, resets_in: i64) -> Vec<Sample> {
    (0..=12)
        .map(|i| {
            let back = (12 - i) * 300;
            Sample {
                t: NOW - back,
                w: id.into(),
                p: (p - per_hour * back as f64 / H as f64).max(0.0),
                r: Some(NOW + resets_in),
            }
        })
        .collect()
}

fn view(obs: &[Observation], history: &[Sample]) -> View {
    build_view(merge(obs, NOW), history, NOW)
}

fn main() {
    let session_in = 3 * H + 12 * 60;
    let week_in = 4 * D + 17 * H + 28 * 60;
    let mut out: BTreeMap<&str, View> = BTreeMap::new();

    out.insert(
        "normal",
        view(
            &[
                a("five_hour", 34.0, session_in, 60),
                a("seven_day", 22.0, week_in, 60),
            ],
            &ramp("five_hour", 34.0, 12.0, session_in),
        ),
    );
    // Session ≥ 70 % and ahead of pace; weekly ≥ 70 % but on pace.
    out.insert(
        "warn",
        view(
            &[
                a("five_hour", 74.0, H + 40 * 60, 60),
                a("seven_day", 71.0, 2 * D, 60),
            ],
            &ramp("five_hour", 74.0, 20.0, H + 40 * 60),
        ),
    );
    out.insert(
        "crit",
        view(
            &[
                a("five_hour", 93.0, 50 * 60, 30),
                a("seven_day", 91.0, D + 3 * H, 30),
            ],
            &ramp("five_hour", 93.0, 15.0, 50 * 60),
        ),
    );
    // No update for 45 minutes.
    out.insert(
        "stale",
        view(
            &[
                a("five_hour", 48.0, 2 * H, 45 * 60),
                a("seven_day", 30.0, 3 * D, 45 * 60),
            ],
            &[],
        ),
    );
    // The session window reset 5 minutes ago; weekly still running.
    out.insert(
        "reset",
        view(
            &[
                a("five_hour", 88.0, -5 * 60, 20 * 60),
                a("seven_day", 40.0, 3 * D, 20 * 60),
            ],
            &[],
        ),
    );
    out.insert("nodata", view(&[], &[]));
    // A model-scoped weekly window, if Claude Code ever sends one.
    out.insert(
        "extra",
        view(
            &[
                a("five_hour", 34.0, session_in, 60),
                a("seven_day", 22.0, week_in, 60),
                a("seven_day_fable", 12.0, week_in, 60),
            ],
            &[],
        ),
    );
    // Claude Desktop only: percentages without reset times.
    out.insert(
        "desktop",
        view(
            &[
                Observation {
                    id: "five_hour".into(),
                    used_percentage: 41.0,
                    resets_at: None,
                    as_of: NOW - 8 * 60,
                    source: Source::DesktopHistory,
                },
                Observation {
                    id: "seven_day".into(),
                    used_percentage: 19.0,
                    resets_at: None,
                    as_of: NOW - 8 * 60,
                    source: Source::DesktopHistory,
                },
            ],
            &[],
        ),
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&out).expect("views serialise")
    );
}
