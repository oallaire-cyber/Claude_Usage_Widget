//! Merging sources A/B/C into one view, and the status of each window.
//!
//! Per window: the status line (A) wins whenever it is current; otherwise the freshest valid value
//! from any source. A value becomes meaningless once its `resets_at` passes (`window_reset`).

use serde::{Deserialize, Serialize};

use crate::bridge_state::SAME_CYCLE_TOLERANCE_SECS;
use crate::sources::{Observation, Source};
use crate::window::{display_rank, FIVE_HOUR, SEVEN_DAY};

/// No update for longer than this while the window is still active = `stale`.
pub const STALE_AFTER_SECS: i64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Stale,
    /// `now` is past `resets_at`: shown as 0 % "reset — waiting for fresh data".
    WindowReset,
    NoData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergedWindow {
    pub id: String,
    /// Last reported value (kept for reference even after a reset).
    pub used_percentage: f64,
    /// What to display: 0 once the window has reset.
    pub display_percentage: f64,
    pub resets_at: Option<i64>,
    pub as_of: i64,
    pub source: Source,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Merged {
    /// Ordered: session, weekly, model-scoped weekly, others.
    pub windows: Vec<MergedWindow>,
    pub status: Status,
    /// Freshest `as_of` across windows ("data as of HH:MM").
    pub as_of: Option<i64>,
    /// Source of the session window (else of the first window).
    pub source: Option<Source>,
}

fn source_priority(s: Source) -> u8 {
    match s {
        Source::StatusLine => 0,
        Source::ClaudeCodeCache => 1,
        Source::DesktopHistory => 2,
    }
}

fn is_current(o: &Observation, now: i64) -> bool {
    now - o.as_of <= STALE_AFTER_SECS && o.resets_at.is_none_or(|r| now < r)
}

pub fn window_status(resets_at: Option<i64>, as_of: i64, now: i64) -> Status {
    if resets_at.is_some_and(|r| now >= r) {
        Status::WindowReset
    } else if now - as_of > STALE_AFTER_SECS {
        Status::Stale
    } else {
        Status::Ok
    }
}

/// Pick one observation for a window from all sources' candidates.
fn select(candidates: &[&Observation], now: i64) -> Option<Observation> {
    let valid: Vec<&Observation> = candidates
        .iter()
        .copied()
        .filter(|o| o.used_percentage.is_finite() && o.used_percentage >= 0.0)
        .collect();
    // The latest reset already passed, as known from any source. A value without a reset time that
    // was observed before it belongs to the previous cycle and is discarded.
    let passed_reset = valid
        .iter()
        .filter_map(|o| o.resets_at)
        .filter(|r| *r <= now)
        .max();
    let valid: Vec<&Observation> = valid
        .into_iter()
        .filter(|o| {
            !(o.resets_at.is_none()
                && passed_reset.is_some_and(|r| o.as_of < r - SAME_CYCLE_TOLERANCE_SECS))
        })
        .collect();

    if let Some(a) = valid
        .iter()
        .filter(|o| o.source == Source::StatusLine && is_current(o, now))
        .max_by_key(|o| o.as_of)
    {
        return Some((*a).clone());
    }
    valid
        .iter()
        .max_by(|x, y| {
            x.as_of
                .cmp(&y.as_of)
                .then(source_priority(y.source).cmp(&source_priority(x.source)))
        })
        .map(|o| (*o).clone())
}

pub fn merge(observations: &[Observation], now: i64) -> Merged {
    let mut ids: Vec<&str> = observations.iter().map(|o| o.id.as_str()).collect();
    ids.sort_by_key(|id| display_rank(id));
    ids.dedup();

    let windows: Vec<MergedWindow> = ids
        .into_iter()
        .filter_map(|id| {
            let cands: Vec<&Observation> = observations.iter().filter(|o| o.id == id).collect();
            let o = select(&cands, now)?;
            let status = window_status(o.resets_at, o.as_of, now);
            Some(MergedWindow {
                display_percentage: if status == Status::WindowReset {
                    0.0
                } else {
                    o.used_percentage
                },
                id: o.id,
                used_percentage: o.used_percentage,
                resets_at: o.resets_at,
                as_of: o.as_of,
                source: o.source,
                status,
            })
        })
        .collect();

    // The overall status follows the session and weekly windows when there are any: a side window
    // that rarely updates (e.g. Claude Desktop's extra usage) must not mark everything stale.
    let primary: Vec<&MergedWindow> = windows
        .iter()
        .filter(|w| w.id == FIVE_HOUR || w.id == SEVEN_DAY)
        .collect();
    let judged: Vec<&MergedWindow> = if primary.is_empty() {
        windows.iter().collect()
    } else {
        primary
    };
    let status = if windows.is_empty() {
        Status::NoData
    } else if judged.iter().any(|w| w.status == Status::Stale) {
        Status::Stale
    } else if judged.iter().all(|w| w.status == Status::WindowReset) {
        Status::WindowReset
    } else {
        Status::Ok
    };
    Merged {
        as_of: windows.iter().map(|w| w.as_of).max(),
        source: windows.first().map(|w| w.source),
        windows,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    fn obs(id: &str, p: f64, resets: Option<i64>, as_of: i64, source: Source) -> Observation {
        Observation {
            id: id.into(),
            used_percentage: p,
            resets_at: resets,
            as_of,
            source,
        }
    }
    use Source::*;

    #[test]
    fn no_data() {
        let m = merge(&[], NOW);
        assert_eq!(m.status, Status::NoData);
        assert!(m.windows.is_empty());
        assert_eq!(m.as_of, None);
    }

    #[test]
    fn current_status_line_wins_even_if_other_source_is_fresher() {
        let m = merge(
            &[
                obs("five_hour", 30.0, Some(NOW + 3600), NOW - 600, StatusLine),
                obs("five_hour", 35.0, None, NOW - 10, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, StatusLine);
        assert_eq!(m.windows[0].used_percentage, 30.0);
        assert_eq!(m.status, Status::Ok);
    }

    #[test]
    fn stale_status_line_loses_to_fresher_source() {
        let m = merge(
            &[
                obs(
                    "five_hour",
                    30.0,
                    Some(NOW + 3600),
                    NOW - 3 * 3600,
                    StatusLine,
                ),
                obs("five_hour", 44.0, None, NOW - 60, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, DesktopHistory);
        assert_eq!(m.windows[0].resets_at, None);
        assert_eq!(m.windows[0].status, Status::Ok);
    }

    #[test]
    fn stale_status_line_still_wins_over_older_sources() {
        let m = merge(
            &[
                obs(
                    "seven_day",
                    30.0,
                    Some(NOW + 86_400),
                    NOW - 3 * 3600,
                    StatusLine,
                ),
                obs("seven_day", 20.0, None, NOW - 9 * 3600, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, StatusLine);
        assert_eq!(m.windows[0].status, Status::Stale);
        assert_eq!(m.status, Status::Stale);
    }

    #[test]
    fn b_is_used_when_a_is_absent() {
        let m = merge(
            &[
                obs(
                    "seven_day_fable",
                    12.0,
                    Some(NOW + 86_400),
                    NOW - 100,
                    ClaudeCodeCache,
                ),
                obs("seven_day_fable", 10.0, None, NOW - 1000, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, ClaudeCodeCache);
    }

    #[test]
    fn tie_prefers_a_then_b_then_c() {
        let m = merge(
            &[
                obs("five_hour", 1.0, None, NOW - 5000, DesktopHistory),
                obs("five_hour", 2.0, None, NOW - 5000, ClaudeCodeCache),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, ClaudeCodeCache);
    }

    #[test]
    fn window_reset_shows_zero() {
        let m = merge(
            &[obs("five_hour", 88.0, Some(NOW - 5), NOW - 600, StatusLine)],
            NOW,
        );
        let w = &m.windows[0];
        assert_eq!(w.status, Status::WindowReset);
        assert_eq!(w.display_percentage, 0.0);
        assert_eq!(w.used_percentage, 88.0);
        assert_eq!(m.status, Status::WindowReset);
    }

    #[test]
    fn reset_takes_precedence_over_stale() {
        let m = merge(
            &[obs(
                "five_hour",
                50.0,
                Some(NOW - 5),
                NOW - 6 * 3600,
                StatusLine,
            )],
            NOW,
        );
        assert_eq!(m.windows[0].status, Status::WindowReset);
    }

    #[test]
    fn value_from_before_a_known_reset_is_discarded() {
        // A says the window reset 10 min ago; Desktop's sample predates that reset.
        let m = merge(
            &[
                obs("five_hour", 90.0, Some(NOW - 600), NOW - 3600, StatusLine),
                obs("five_hour", 91.0, None, NOW - 1200, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, StatusLine);
        assert_eq!(m.windows[0].status, Status::WindowReset);
        // A Desktop sample taken after the reset is fresh data for the new cycle.
        let m = merge(
            &[
                obs("five_hour", 90.0, Some(NOW - 600), NOW - 3600, StatusLine),
                obs("five_hour", 3.0, None, NOW - 60, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[0].source, DesktopHistory);
        assert_eq!(m.windows[0].display_percentage, 3.0);
    }

    #[test]
    fn stale_after_thirty_minutes() {
        assert_eq!(
            window_status(Some(NOW + 1), NOW - STALE_AFTER_SECS, NOW),
            Status::Ok
        );
        assert_eq!(
            window_status(Some(NOW + 1), NOW - STALE_AFTER_SECS - 1, NOW),
            Status::Stale
        );
        assert_eq!(window_status(None, NOW - 7200, NOW), Status::Stale);
    }

    #[test]
    fn overall_status_and_order() {
        let m = merge(
            &[
                obs("spend_limit", 5.0, Some(NOW + 10), NOW, StatusLine),
                obs("seven_day", 40.0, Some(NOW + 86_400), NOW - 60, StatusLine),
                obs("five_hour", 10.0, Some(NOW - 1), NOW - 60, StatusLine),
            ],
            NOW,
        );
        let ids: Vec<_> = m.windows.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["five_hour", "seven_day", "spend_limit"]);
        assert_eq!(m.status, Status::Ok); // session reset, but the others are live
        assert_eq!(m.as_of, Some(NOW));
        assert_eq!(m.source, Some(StatusLine));
    }

    #[test]
    fn stale_side_window_does_not_make_everything_stale() {
        let m = merge(
            &[
                obs("five_hour", 30.0, Some(NOW + 3600), NOW, StatusLine),
                obs("seven_day", 20.0, Some(NOW + 86_400), NOW, StatusLine),
                obs("extra_usage", 100.0, None, NOW - 12 * 3600, DesktopHistory),
            ],
            NOW,
        );
        assert_eq!(m.windows[2].status, Status::Stale);
        assert_eq!(m.status, Status::Ok);
        // Without session/weekly windows, the side windows decide.
        let m = merge(
            &[obs(
                "extra_usage",
                100.0,
                None,
                NOW - 12 * 3600,
                DesktopHistory,
            )],
            NOW,
        );
        assert_eq!(m.status, Status::Stale);
    }

    #[test]
    fn invalid_values_are_skipped() {
        let m = merge(&[obs("five_hour", f64::NAN, None, NOW, StatusLine)], NOW);
        assert_eq!(m.status, Status::NoData);
    }
}
