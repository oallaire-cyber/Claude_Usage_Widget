//! What the app shows: the merged windows, each with its pace and tone. Sent as-is to the frontend.

use serde::{Deserialize, Serialize};

use crate::bridge_state::Sample;
use crate::merge::{Merged, MergedWindow, Status};
use crate::pace::{compute_pace, Pace};
use crate::sources::Source;
use crate::tray::{tone, Tone};
use crate::window::FIVE_HOUR;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowView {
    #[serde(flatten)]
    pub window: MergedWindow,
    /// `None` when pace cannot be computed (unknown window length, no reset time, window reset).
    pub pace: Option<Pace>,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The time this view was computed for; the frontend formats countdowns from it.
    pub now: i64,
    pub status: Status,
    pub as_of: Option<i64>,
    pub source: Option<Source>,
    pub windows: Vec<WindowView>,
}

impl View {
    pub fn session(&self) -> Option<&WindowView> {
        self.windows.iter().find(|w| w.window.id == FIVE_HOUR)
    }

    /// What the tray ring shows: session fill (0..=1) and tone. Grey and empty without a session.
    pub fn tray(&self) -> (f64, Tone) {
        match self.session() {
            Some(s) => (s.window.display_percentage / 100.0, s.tone),
            None => (0.0, Tone::Grey),
        }
    }
}

/// Add pace (from `history`) and tone to every merged window. Pace is computed for `ok` and `stale`
/// windows (the even-spend marker depends only on time); tone is grey unless the window is `ok`.
pub fn build_view(merged: Merged, history: &[Sample], now: i64) -> View {
    let windows = merged
        .windows
        .into_iter()
        .map(|w| {
            let pace = match w.status {
                Status::Ok | Status::Stale => {
                    compute_pace(&w.id, w.used_percentage, w.resets_at, now, history)
                }
                _ => None,
            };
            let tone = tone(w.status, w.used_percentage, pace.as_ref());
            WindowView {
                window: w,
                pace,
                tone,
            }
        })
        .collect();
    View {
        now,
        status: merged.status,
        as_of: merged.as_of,
        source: merged.source,
        windows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::merge;
    use crate::sources::Observation;

    const NOW: i64 = 1_000_000;

    fn obs(id: &str, p: f64, resets: Option<i64>, as_of: i64) -> Observation {
        Observation {
            id: id.into(),
            used_percentage: p,
            resets_at: resets,
            as_of,
            source: Source::StatusLine,
        }
    }

    #[test]
    fn session_drives_the_tray() {
        // Session half elapsed, 80 % used: ratio 1.6 → amber.
        let m = merge(
            &[
                obs("five_hour", 80.0, Some(NOW + 9000), NOW),
                obs("seven_day", 10.0, Some(NOW + 300_000), NOW),
            ],
            NOW,
        );
        let v = build_view(m, &[], NOW);
        assert_eq!(v.windows.len(), 2);
        let (fill, t) = v.tray();
        assert!((fill - 0.8).abs() < 1e-9);
        assert_eq!(t, Tone::Amber);
        let pace = v.session().unwrap().pace.as_ref().unwrap();
        assert!((pace.pace_ratio.unwrap() - 1.6).abs() < 1e-9);
    }

    #[test]
    fn stale_keeps_pace_but_is_grey() {
        let m = merge(&[obs("five_hour", 50.0, Some(NOW + 9000), NOW - 3600)], NOW);
        let v = build_view(m, &[], NOW);
        let s = v.session().unwrap();
        assert_eq!(s.window.status, Status::Stale);
        assert!(s.pace.is_some());
        assert_eq!(s.tone, Tone::Grey);
    }

    #[test]
    fn reset_and_no_data_are_grey_and_empty() {
        let m = merge(&[obs("five_hour", 70.0, Some(NOW - 10), NOW - 100)], NOW);
        let v = build_view(m, &[], NOW);
        assert_eq!(v.tray(), (0.0, Tone::Grey));
        assert!(v.session().unwrap().pace.is_none());

        let v = build_view(merge(&[], NOW), &[], NOW);
        assert_eq!(v.status, Status::NoData);
        assert_eq!(v.tray(), (0.0, Tone::Grey));
    }

    #[test]
    fn serialises_flat() {
        let m = merge(&[obs("five_hour", 10.0, Some(NOW + 9000), NOW)], NOW);
        let v = build_view(m, &[], NOW);
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["windows"][0]["id"], "five_hour");
        assert_eq!(j["windows"][0]["tone"], "blue");
        assert_eq!(j["windows"][0]["status"], "ok");
        assert_eq!(j["source"], "status_line");
    }
}
