//! Pace: how fast a window is being used compared with even spending.
//!
//! - elapsed fraction = (now − (resets_at − length)) / length
//! - pace ratio = used fraction ÷ elapsed fraction (1.0 = exactly even spend)
//! - projected exhaustion: linear fit over recent history samples, else the average rate since the
//!   window started.

use serde::{Deserialize, Serialize};

use crate::bridge_state::{Sample, SAME_CYCLE_TOLERANCE_SECS};
use crate::window::window_length_secs;

/// Pace ratio from which a window counts as "ahead" (also the tray's amber threshold).
pub const AHEAD_RATIO: f64 = 1.1;
/// Guards (PROMPT.md §4b): below this much used, pace is not meaningful yet.
pub const GUARD_MIN_USED_PCT: f64 = 20.0;
/// Guards: in the first 10 % of a window, pace is not meaningful yet.
pub const GUARD_MIN_ELAPSED: f64 = 0.10;
/// The regression looks back over this fraction of the window (1 h for 5 h, ~34 h for 7 days).
const FIT_LOOKBACK_FRACTION: i64 = 5;
const FIT_MIN_SAMPLES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaceStatus {
    OnTrack,
    Ahead,
    WillExhaust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionBasis {
    /// Linear fit over recent history samples.
    History,
    /// Average rate since the window started.
    Average,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pace {
    /// 0..=1 — also where the "even spend" marker sits on the ring.
    pub elapsed_fraction: f64,
    /// `None` at the very start of a window (nothing elapsed yet).
    pub pace_ratio: Option<f64>,
    pub status: PaceStatus,
    /// When usage reaches 100 % at the projected rate, if before the reset. Already-exhausted = `now`.
    pub projected_exhaustion_at: Option<i64>,
    pub basis: ProjectionBasis,
    /// True when the §4b guards applied (little used or window just started): colour by plain thresholds.
    pub guarded: bool,
}

/// Pace of window `id`, or `None` when it cannot be computed (unknown length, no reset time, or the
/// window has already reset). `history` may contain any samples; only this window's current cycle is used.
pub fn compute_pace(
    id: &str,
    used_percentage: f64,
    resets_at: Option<i64>,
    now: i64,
    history: &[Sample],
) -> Option<Pace> {
    let len = window_length_secs(id)?;
    let resets_at = resets_at?;
    if now >= resets_at || !used_percentage.is_finite() {
        return None;
    }
    let start = resets_at - len;
    let elapsed_fraction = ((now - start) as f64 / len as f64).clamp(0.0, 1.0);
    let used = used_percentage.max(0.0);
    let pace_ratio = (elapsed_fraction > 0.0).then(|| (used / 100.0) / elapsed_fraction);
    let guarded = used < GUARD_MIN_USED_PCT || elapsed_fraction < GUARD_MIN_ELAPSED;

    // Rate in percentage points per second.
    let (rate, basis) = match fit_rate(id, resets_at, len, now, history) {
        Some(r) => (r, ProjectionBasis::History),
        None => {
            let secs = (now - start).max(0);
            let r = if secs > 0 { used / secs as f64 } else { 0.0 };
            (r, ProjectionBasis::Average)
        }
    };

    let projected_exhaustion_at = if used >= 100.0 {
        Some(now)
    } else if rate > 0.0 {
        let t = now as f64 + (100.0 - used) / rate;
        (t < resets_at as f64).then_some(t.ceil() as i64)
    } else {
        None
    };

    let status = if used >= 100.0 {
        PaceStatus::WillExhaust
    } else if guarded {
        PaceStatus::OnTrack
    } else if projected_exhaustion_at.is_some() {
        PaceStatus::WillExhaust
    } else if pace_ratio.is_some_and(|r| r >= AHEAD_RATIO) {
        PaceStatus::Ahead
    } else {
        PaceStatus::OnTrack
    };

    Some(Pace {
        elapsed_fraction,
        pace_ratio,
        status,
        projected_exhaustion_at,
        basis,
        guarded,
    })
}

/// Least-squares slope (pct/s) over this cycle's samples in the look-back period. `None` when there
/// are too few samples or they span too short a time; a flat or falling fit yields `Some(≤ 0)`.
fn fit_rate(id: &str, resets_at: i64, len: i64, now: i64, history: &[Sample]) -> Option<f64> {
    let lookback = len / FIT_LOOKBACK_FRACTION;
    let from = (resets_at - len).max(now - lookback);
    let pts: Vec<(f64, f64)> = history
        .iter()
        .filter(|s| {
            s.w == id
                && s.r
                    .is_some_and(|r| (r - resets_at).abs() <= SAME_CYCLE_TOLERANCE_SECS)
                && s.t >= from
                && s.t <= now
                && s.p.is_finite()
        })
        .map(|s| ((s.t - from) as f64, s.p))
        .collect();
    if pts.len() < FIT_MIN_SAMPLES {
        return None;
    }
    let (tmin, tmax) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), (t, _)| {
        (a.min(*t), b.max(*t))
    });
    if tmax - tmin < (lookback / 4) as f64 {
        return None;
    }
    let n = pts.len() as f64;
    let mt = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let mp = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let cov: f64 = pts.iter().map(|(t, p)| (t - mt) * (p - mp)).sum();
    let var: f64 = pts.iter().map(|(t, _)| (t - mt).powi(2)).sum();
    (var > 0.0).then(|| cov / var)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3600;
    const RESET: i64 = 100 * H;

    /// `now` such that `f` of the 5-hour window ending at RESET has elapsed.
    fn at(f: f64) -> i64 {
        RESET - 5 * H + (f * (5 * H) as f64) as i64
    }

    fn samples(points: &[(i64, f64)]) -> Vec<Sample> {
        points
            .iter()
            .map(|(t, p)| Sample {
                t: *t,
                w: "five_hour".into(),
                p: *p,
                r: Some(RESET),
            })
            .collect()
    }

    #[test]
    fn elapsed_and_ratio() {
        let p = compute_pace("five_hour", 25.0, Some(RESET), at(0.5), &[]).unwrap();
        assert!((p.elapsed_fraction - 0.5).abs() < 1e-9);
        assert!((p.pace_ratio.unwrap() - 0.5).abs() < 1e-9);
        assert_eq!(p.status, PaceStatus::OnTrack);
        assert_eq!(p.basis, ProjectionBasis::Average);
        assert_eq!(p.projected_exhaustion_at, None);
    }

    #[test]
    fn not_computable() {
        assert!(compute_pace("spend_limit", 10.0, Some(RESET), at(0.5), &[]).is_none());
        assert!(compute_pace("five_hour", 10.0, None, at(0.5), &[]).is_none());
        assert!(compute_pace("five_hour", 10.0, Some(RESET), RESET, &[]).is_none());
    }

    #[test]
    fn average_fallback_projects_exhaustion() {
        // 60 % used at 40 % elapsed: ratio 1.5; average rate reaches 100 % at 66.7 % elapsed.
        let now = at(0.4);
        let p = compute_pace("five_hour", 60.0, Some(RESET), now, &[]).unwrap();
        assert!((p.pace_ratio.unwrap() - 1.5).abs() < 1e-9);
        assert_eq!(p.status, PaceStatus::WillExhaust);
        let expected = RESET - 5 * H + (5 * H) * 2 / 3;
        assert!((p.projected_exhaustion_at.unwrap() - expected).abs() <= 2);
    }

    #[test]
    fn guards_low_usage_and_early_window() {
        // Ratio 3.0 but only 15 % used: guarded, on track.
        let p = compute_pace("five_hour", 15.0, Some(RESET), at(0.05), &[]).unwrap();
        assert!(p.guarded);
        assert_eq!(p.status, PaceStatus::OnTrack);
        // 30 % used in the first 5 % of the window: guarded too.
        let p = compute_pace("five_hour", 30.0, Some(RESET), at(0.05), &[]).unwrap();
        assert!(p.guarded);
        assert_eq!(p.status, PaceStatus::OnTrack);
    }

    #[test]
    fn exhausted_is_always_will_exhaust() {
        let now = at(0.05);
        let p = compute_pace("five_hour", 100.0, Some(RESET), now, &[]).unwrap();
        assert_eq!(p.status, PaceStatus::WillExhaust);
        assert_eq!(p.projected_exhaustion_at, Some(now));
    }

    #[test]
    fn history_fit_slowed_down_gives_ahead() {
        // 50 % used at 40 % elapsed (ratio 1.25) but flat for the last hour: ahead, not exhausting.
        let now = at(0.4);
        let h = samples(&[
            (now - 3000, 50.0),
            (now - 2000, 50.0),
            (now - 1000, 50.0),
            (now, 50.0),
        ]);
        let p = compute_pace("five_hour", 50.0, Some(RESET), now, &h).unwrap();
        assert_eq!(p.basis, ProjectionBasis::History);
        assert_eq!(p.projected_exhaustion_at, None);
        assert_eq!(p.status, PaceStatus::Ahead);
    }

    #[test]
    fn history_fit_burst_projects_early_exhaustion() {
        // 50 % used at 50 % elapsed (ratio 1.0, even spend on average) but +30 pts in the last
        // 30 min = 60 pts/h: the remaining 50 pts go in ~50 min, well before the reset in 2.5 h.
        let now = at(0.5);
        let h = samples(&[
            (now - 1800, 20.0),
            (now - 1200, 30.0),
            (now - 600, 40.0),
            (now, 50.0),
        ]);
        let p = compute_pace("five_hour", 50.0, Some(RESET), now, &h).unwrap();
        assert_eq!(p.basis, ProjectionBasis::History);
        assert_eq!(p.status, PaceStatus::WillExhaust);
        let t = p.projected_exhaustion_at.unwrap();
        assert!((t - (now + 3000)).abs() <= 2, "{}", t - now);
    }

    #[test]
    fn history_ignores_other_windows_cycles_and_sparse_data() {
        let now = at(0.5);
        let mut h = samples(&[(now - 1800, 0.0), (now - 900, 50.0)]); // only 2 samples: too few
        h.push(Sample {
            t: now - 600,
            w: "seven_day".into(),
            p: 90.0,
            r: Some(RESET),
        });
        h.push(Sample {
            t: now - 300,
            w: "five_hour".into(),
            p: 99.0,
            r: Some(RESET - 5 * H),
        });
        let p = compute_pace("five_hour", 30.0, Some(RESET), now, &h).unwrap();
        assert_eq!(p.basis, ProjectionBasis::Average);
    }

    #[test]
    fn history_too_short_span_falls_back() {
        let now = at(0.5);
        let h = samples(&[(now - 120, 29.0), (now - 60, 29.5), (now, 30.0)]);
        let p = compute_pace("five_hour", 30.0, Some(RESET), now, &h).unwrap();
        assert_eq!(p.basis, ProjectionBasis::Average);
    }

    #[test]
    fn seven_day_window() {
        let reset = 1_000_000;
        let now = reset - 7 * 86_400 + 7 * 86_400 / 2;
        let p = compute_pace("seven_day", 50.0, Some(reset), now, &[]).unwrap();
        assert!((p.pace_ratio.unwrap() - 1.0).abs() < 1e-6);
        assert!((p.elapsed_fraction - 0.5).abs() < 1e-6);
    }
}
