//! Which notifications to show. Each threshold fires at most once per window cycle; an optional
//! "limit reset" notification tells you when a window you were warned about has reset.
//!
//! The caller persists [`NotifyState`] (so an app restart does not repeat notifications) and turns the
//! returned [`Notification`]s into toasts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge_state::SAME_CYCLE_TOLERANCE_SECS;
use crate::merge::{Merged, Status};
use crate::window::{FIVE_HOUR, SEVEN_DAY};

/// Without reset times (Claude Desktop source), a drop of this many points means a new cycle.
pub const CYCLE_DROP_POINTS: f64 = 5.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotifyConfig {
    pub enabled: bool,
    /// Thresholds in percent, per window id. Windows not listed get no threshold notifications.
    pub thresholds: BTreeMap<String, Vec<f64>>,
    /// Notify when a window for which a threshold fired has reset.
    pub notify_reset: bool,
}

impl Default for NotifyConfig {
    /// Session 80/95 %, weekly 75/90 % (PROMPT.md Phase 5), reset notification on.
    fn default() -> Self {
        let mut thresholds = BTreeMap::new();
        thresholds.insert(FIVE_HOUR.to_string(), vec![80.0, 95.0]);
        thresholds.insert(SEVEN_DAY.to_string(), vec![75.0, 90.0]);
        NotifyConfig {
            enabled: true,
            thresholds,
            notify_reset: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowNotifyState {
    /// `resets_at` of the cycle being tracked (None when the source has no reset time).
    pub cycle: Option<i64>,
    /// Thresholds already notified in this cycle.
    pub fired: Vec<f64>,
    pub last_used: f64,
    pub reset_notified: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NotifyState {
    pub windows: BTreeMap<String, WindowNotifyState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notification {
    /// `used_percentage` crossed `threshold` (the highest one crossed, if several at once).
    Threshold {
        window: String,
        threshold: f64,
        used_percentage: f64,
        resets_at: Option<i64>,
    },
    /// The window reset after a threshold notification in the previous cycle.
    Reset { window: String },
}

fn same_cycle(a: Option<i64>, b: Option<i64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= SAME_CYCLE_TOLERANCE_SECS,
        _ => true, // unknown on either side: decided by the value-drop rule instead
    }
}

/// Evaluate the merged view against the config, updating `state`. Returns notifications to show
/// (none when notifications are disabled — state still advances, so enabling later does not replay).
pub fn evaluate(merged: &Merged, cfg: &NotifyConfig, state: &mut NotifyState) -> Vec<Notification> {
    let mut out = Vec::new();
    for w in &merged.windows {
        let first_sight = !state.windows.contains_key(&w.id);
        let st = state.windows.entry(w.id.clone()).or_default();
        if first_sight {
            st.cycle = w.resets_at;
        }

        let new_cycle = !first_sight
            && if w.resets_at.is_some() && st.cycle.is_some() {
                !same_cycle(st.cycle, w.resets_at)
            } else {
                w.status != Status::WindowReset
                    && w.used_percentage < st.last_used - CYCLE_DROP_POINTS
            };
        if new_cycle {
            if cfg.notify_reset && !st.fired.is_empty() && !st.reset_notified {
                out.push(Notification::Reset {
                    window: w.id.clone(),
                });
            }
            st.fired.clear();
            st.reset_notified = false;
        }
        if w.resets_at.is_some() {
            st.cycle = w.resets_at;
        }

        if w.status == Status::WindowReset {
            if cfg.notify_reset && !st.fired.is_empty() && !st.reset_notified {
                out.push(Notification::Reset {
                    window: w.id.clone(),
                });
            }
            st.reset_notified = true;
            continue;
        }

        st.last_used = w.used_percentage;
        let crossed: Vec<f64> = cfg
            .thresholds
            .get(&w.id)
            .map(|ts| {
                ts.iter()
                    .copied()
                    .filter(|t| w.used_percentage >= *t && !st.fired.contains(t))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(top) = crossed.iter().copied().reduce(f64::max) {
            st.fired.extend(crossed);
            out.push(Notification::Threshold {
                window: w.id.clone(),
                threshold: top,
                used_percentage: w.used_percentage,
                resets_at: w.resets_at,
            });
        }
    }
    if cfg.enabled {
        out
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::merge;
    use crate::sources::{Observation, Source};

    const NOW: i64 = 1_000_000;

    fn view(id: &str, used: f64, resets: Option<i64>, now: i64) -> Merged {
        let src = if resets.is_some() {
            Source::StatusLine
        } else {
            Source::DesktopHistory
        };
        merge(
            &[Observation {
                id: id.into(),
                used_percentage: used,
                resets_at: resets,
                as_of: now,
                source: src,
            }],
            now,
        )
    }

    fn thresholds(n: &[Notification]) -> Vec<f64> {
        n.iter()
            .filter_map(|x| match x {
                Notification::Threshold { threshold, .. } => Some(*threshold),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn each_threshold_fires_once_per_cycle() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        let r = Some(NOW + 3600);
        assert!(evaluate(&view("five_hour", 50.0, r, NOW), &cfg, &mut st).is_empty());
        assert_eq!(
            thresholds(&evaluate(&view("five_hour", 81.0, r, NOW), &cfg, &mut st)),
            [80.0]
        );
        assert!(evaluate(&view("five_hour", 85.0, r, NOW), &cfg, &mut st).is_empty());
        assert_eq!(
            thresholds(&evaluate(&view("five_hour", 95.0, r, NOW), &cfg, &mut st)),
            [95.0]
        );
        assert!(evaluate(&view("five_hour", 99.0, r, NOW), &cfg, &mut st).is_empty());
    }

    #[test]
    fn jump_over_several_thresholds_fires_once_with_highest() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        let n = evaluate(
            &view("seven_day", 92.0, Some(NOW + 86_400), NOW),
            &cfg,
            &mut st,
        );
        assert_eq!(thresholds(&n), [90.0]);
        assert!(evaluate(
            &view("seven_day", 93.0, Some(NOW + 86_400), NOW),
            &cfg,
            &mut st
        )
        .is_empty());
    }

    #[test]
    fn new_cycle_rearms_and_optionally_notifies_reset() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        evaluate(&view("five_hour", 82.0, Some(NOW + 60), NOW), &cfg, &mut st);
        // Window passes its reset time: one reset notification.
        let n = evaluate(
            &view("five_hour", 82.0, Some(NOW + 60), NOW + 61),
            &cfg,
            &mut st,
        );
        assert_eq!(
            n,
            vec![Notification::Reset {
                window: "five_hour".into()
            }]
        );
        assert!(evaluate(
            &view("five_hour", 82.0, Some(NOW + 60), NOW + 120),
            &cfg,
            &mut st
        )
        .is_empty());
        // Fresh data for the next cycle: no second reset notification; thresholds re-armed.
        let next = Some(NOW + 60 + 18_000);
        assert!(evaluate(&view("five_hour", 5.0, next, NOW + 200), &cfg, &mut st).is_empty());
        assert_eq!(
            thresholds(&evaluate(
                &view("five_hour", 80.0, next, NOW + 300),
                &cfg,
                &mut st
            )),
            [80.0]
        );
    }

    #[test]
    fn new_cycle_seen_directly_notifies_reset_once() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        evaluate(&view("five_hour", 96.0, Some(NOW + 60), NOW), &cfg, &mut st);
        let n = evaluate(
            &view("five_hour", 1.0, Some(NOW + 18_000), NOW + 100),
            &cfg,
            &mut st,
        );
        assert_eq!(
            n,
            vec![Notification::Reset {
                window: "five_hour".into()
            }]
        );
    }

    #[test]
    fn no_reset_notification_without_prior_warning_or_when_disabled() {
        let mut st = NotifyState::default();
        let cfg = NotifyConfig::default();
        evaluate(&view("five_hour", 40.0, Some(NOW + 60), NOW), &cfg, &mut st);
        assert!(evaluate(
            &view("five_hour", 40.0, Some(NOW + 60), NOW + 61),
            &cfg,
            &mut st
        )
        .is_empty());

        let cfg = NotifyConfig {
            notify_reset: false,
            ..NotifyConfig::default()
        };
        let mut st = NotifyState::default();
        evaluate(&view("five_hour", 90.0, Some(NOW + 60), NOW), &cfg, &mut st);
        assert!(evaluate(
            &view("five_hour", 90.0, Some(NOW + 60), NOW + 61),
            &cfg,
            &mut st
        )
        .is_empty());
    }

    #[test]
    fn disabled_notifications_advance_state_silently() {
        let off = NotifyConfig {
            enabled: false,
            ..NotifyConfig::default()
        };
        let mut st = NotifyState::default();
        assert!(evaluate(
            &view("five_hour", 90.0, Some(NOW + 3600), NOW),
            &off,
            &mut st
        )
        .is_empty());
        // Turning them back on does not replay the 80 % warning for this cycle.
        let on = NotifyConfig::default();
        assert!(evaluate(
            &view("five_hour", 91.0, Some(NOW + 3600), NOW),
            &on,
            &mut st
        )
        .is_empty());
    }

    #[test]
    fn desktop_source_without_reset_times_uses_value_drop() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        assert_eq!(
            thresholds(&evaluate(
                &view("five_hour", 85.0, None, NOW),
                &cfg,
                &mut st
            )),
            [80.0]
        );
        assert!(evaluate(&view("five_hour", 83.0, None, NOW), &cfg, &mut st).is_empty());
        let n = evaluate(&view("five_hour", 2.0, None, NOW), &cfg, &mut st);
        assert_eq!(
            n,
            vec![Notification::Reset {
                window: "five_hour".into()
            }]
        );
        assert_eq!(
            thresholds(&evaluate(
                &view("five_hour", 81.0, None, NOW),
                &cfg,
                &mut st
            )),
            [80.0]
        );
    }

    #[test]
    fn custom_thresholds_and_unlisted_windows() {
        let mut cfg = NotifyConfig::default();
        cfg.thresholds.insert("seven_day_fable".into(), vec![50.0]);
        let mut st = NotifyState::default();
        assert_eq!(
            thresholds(&evaluate(
                &view("seven_day_fable", 55.0, Some(NOW + 1000), NOW),
                &cfg,
                &mut st
            )),
            [50.0]
        );
        assert!(evaluate(
            &view("spend_limit", 99.0, Some(NOW + 1000), NOW),
            &cfg,
            &mut st
        )
        .is_empty());
    }

    #[test]
    fn already_high_at_first_sight_notifies() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        assert_eq!(
            thresholds(&evaluate(
                &view("seven_day", 80.0, Some(NOW + 1000), NOW),
                &cfg,
                &mut st
            )),
            [75.0]
        );
    }

    #[test]
    fn state_roundtrips_through_json() {
        let cfg = NotifyConfig::default();
        let mut st = NotifyState::default();
        evaluate(
            &view("five_hour", 81.0, Some(NOW + 3600), NOW),
            &cfg,
            &mut st,
        );
        let back: NotifyState = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
        assert_eq!(back, st);
    }
}
