//! User-facing text produced on the Rust side: the tray tooltip and toast notifications, in English
//! and French. (The popup's text lives in the frontend.) Local time is supplied by the caller.

use serde::{Deserialize, Serialize};

use crate::merge::Status;
use crate::notify::Notification;
use crate::view::{View, WindowView};
use crate::window::{EXTRA_USAGE, FIVE_HOUR, SEVEN_DAY};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    En,
    Fr,
}

/// A Unix timestamp as seen on the local clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    /// 0 = Monday … 6 = Sunday.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

/// Converts Unix seconds to local time (the app passes one backed by the system time zone).
pub type ToLocal<'a> = &'a dyn Fn(i64) -> LocalTime;

const DAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DAYS_FR: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];

/// Name of a window, as used in notifications.
pub fn window_label(id: &str, lang: Lang) -> String {
    let fr = lang == Lang::Fr;
    match id {
        FIVE_HOUR => "Session".into(),
        SEVEN_DAY => if fr { "Hebdo" } else { "Weekly" }.into(),
        EXTRA_USAGE => if fr {
            "Usage supplémentaire"
        } else {
            "Extra usage"
        }
        .into(),
        "spend_limit" => if fr {
            "Plafond de dépense"
        } else {
            "Spend limit"
        }
        .into(),
        _ => {
            if let Some(model) = id.strip_prefix("seven_day_") {
                let head = if fr { "Hebdo" } else { "Weekly" };
                format!("{head} · {}", capitalise(model))
            } else {
                capitalise(&id.replace('_', " "))
            }
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// `3h49`, `49 min`, `< 1 min`, or `2 d 5 h` for long durations.
pub fn countdown(secs: i64, lang: Lang) -> String {
    let secs = secs.max(0);
    let mins = secs / 60;
    if mins < 1 {
        "< 1 min".into()
    } else if mins < 60 {
        format!("{mins} min")
    } else if mins < 24 * 60 {
        format!("{}h{:02}", mins / 60, mins % 60)
    } else {
        let d = if lang == Lang::Fr { "j" } else { "d" };
        format!("{} {d} {} h", mins / (24 * 60), (mins / 60) % 24)
    }
}

pub fn clock(t: LocalTime) -> String {
    format!("{:02}:{:02}", t.hour, t.minute)
}

pub fn weekday_clock(t: LocalTime, lang: Lang) -> String {
    let days = if lang == Lang::Fr { &DAYS_FR } else { &DAYS_EN };
    format!("{} {}", days[(t.weekday % 7) as usize], clock(t))
}

fn pct(p: f64, lang: Lang) -> String {
    let n = p.max(0.0).round() as i64;
    if lang == Lang::Fr {
        format!("{n}\u{a0}%")
    } else {
        format!("{n}%")
    }
}

/// When a window resets: a countdown for windows shorter than a day, else weekday and time.
fn reset_phrase(w: &WindowView, now: i64, lang: Lang, to_local: ToLocal) -> Option<String> {
    let r = w.window.resets_at?;
    let when = if r - now < 24 * 3600 {
        countdown(r - now, lang)
    } else {
        weekday_clock(to_local(r), lang)
    };
    Some(if lang == Lang::Fr {
        format!("réinit. {when}")
    } else {
        format!("resets {when}")
    })
}

fn tooltip_part(w: &WindowView, now: i64, lang: Lang, to_local: ToLocal) -> String {
    let fr = lang == Lang::Fr;
    let name = match w.window.id.as_str() {
        SEVEN_DAY if fr => "Semaine".to_string(),
        SEVEN_DAY => "Week".to_string(),
        id => window_label(id, lang),
    };
    if w.window.status == Status::WindowReset {
        return if fr {
            format!("{name} réinitialisée")
        } else {
            format!("{name} reset")
        };
    }
    let mut s = format!("{name} {}", pct(w.window.display_percentage, lang));
    if let Some(r) = reset_phrase(w, now, lang, to_local) {
        s.push_str(" · ");
        s.push_str(&r);
    }
    s
}

/// `Session 14% · resets 3h49 | Week 8% · resets Wed 06:00` + a second line `as of 14:32`.
/// Only the session and weekly windows are listed (the tooltip is limited to 127 characters).
pub fn tooltip(view: &View, lang: Lang, to_local: ToLocal) -> String {
    let fr = lang == Lang::Fr;
    if view.status == Status::NoData || view.windows.is_empty() {
        return if fr {
            "Claude Usage Widget — pas encore de données".into()
        } else {
            "Claude Usage Widget — no data yet".into()
        };
    }
    let mut parts: Vec<String> = view
        .windows
        .iter()
        .filter(|w| w.window.id == FIVE_HOUR || w.window.id == SEVEN_DAY)
        .map(|w| tooltip_part(w, view.now, lang, to_local))
        .collect();
    if parts.is_empty() {
        parts = view
            .windows
            .iter()
            .take(2)
            .map(|w| tooltip_part(w, view.now, lang, to_local))
            .collect();
    }
    let mut s = parts.join(" | ");
    if let Some(t) = view.as_of {
        let at = clock(to_local(t));
        s.push('\n');
        s.push_str(&match (fr, view.status == Status::Stale) {
            (false, false) => format!("as of {at}"),
            (false, true) => format!("as of {at} — not updating"),
            (true, false) => format!("à jour à {at}"),
            (true, true) => format!("données de {at} — pas de mise à jour"),
        });
    }
    truncate_chars(s, 127)
}

fn truncate_chars(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        s
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}

/// Title and body of a toast.
pub fn notification_text(
    n: &Notification,
    now: i64,
    lang: Lang,
    to_local: ToLocal,
) -> (String, String) {
    let fr = lang == Lang::Fr;
    match n {
        Notification::Threshold {
            window,
            threshold,
            used_percentage,
            resets_at,
        } => {
            let label = window_label(window, lang);
            let title = if fr {
                format!("Claude · {label} à {}", pct(*used_percentage, lang))
            } else {
                format!("Claude · {label} at {}", pct(*used_percentage, lang))
            };
            let mut body = if fr {
                format!("Seuil de {} atteint.", pct(*threshold, lang))
            } else {
                format!("Your {} alert was reached.", pct(*threshold, lang))
            };
            if let Some(r) = resets_at.filter(|r| *r > now) {
                let when = if r - now < 24 * 3600 {
                    format!("{} ({})", countdown(r - now, lang), clock(to_local(r)))
                } else {
                    weekday_clock(to_local(r), lang)
                };
                body.push_str(&if fr {
                    format!(" Réinitialisation : {when}.")
                } else {
                    format!(" Resets: {when}.")
                });
            }
            (title, body)
        }
        Notification::Reset { window } => {
            let label = window_label(window, lang);
            if fr {
                (
                    format!("Claude · {label} réinitialisée"),
                    format!("La limite « {label} » est de nouveau disponible."),
                )
            } else {
                (
                    format!("Claude · {label} limit reset"),
                    format!("Your {label} limit is available again."),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::merge;
    use crate::sources::{Observation, Source};
    use crate::view::build_view;

    // 2026-09-23 (a Wednesday) 12:32:00 UTC.
    const NOW: i64 = 1_790_166_720;

    /// UTC+2 (Paris in summer), computed without a time-zone database.
    fn paris(t: i64) -> LocalTime {
        let local = t + 7200;
        let days = local.div_euclid(86_400);
        let secs = local.rem_euclid(86_400);
        LocalTime {
            // 1970-01-01 was a Thursday (index 3).
            weekday: ((days + 3).rem_euclid(7)) as u8,
            hour: (secs / 3600) as u8,
            minute: ((secs / 60) % 60) as u8,
        }
    }

    fn obs(id: &str, p: f64, resets: Option<i64>, as_of: i64) -> Observation {
        Observation {
            id: id.into(),
            used_percentage: p,
            resets_at: resets,
            as_of,
            source: Source::StatusLine,
        }
    }

    fn view(o: &[Observation]) -> View {
        build_view(merge(o, NOW), &[], NOW)
    }

    #[test]
    fn local_time_helper_is_right() {
        assert_eq!(
            paris(NOW),
            LocalTime {
                weekday: 2,
                hour: 14,
                minute: 32
            }
        );
    }

    #[test]
    fn tooltip_matches_spec_shape() {
        let v = view(&[
            obs("five_hour", 14.2, Some(NOW + 3 * 3600 + 49 * 60 + 20), NOW),
            // Next Wednesday 06:00 Paris = 04:00 UTC, 6 days 15h28 from now.
            obs(
                "seven_day",
                8.0,
                Some(NOW + 6 * 86_400 + 15 * 3600 + 28 * 60),
                NOW,
            ),
        ]);
        assert_eq!(
            tooltip(&v, Lang::En, &paris),
            "Session 14% · resets 3h49 | Week 8% · resets Wed 06:00\nas of 14:32"
        );
        assert_eq!(
            tooltip(&v, Lang::Fr, &paris),
            "Session 14\u{a0}% · réinit. 3h49 | Semaine 8\u{a0}% · réinit. mer. 06:00\nà jour à 14:32"
        );
    }

    #[test]
    fn tooltip_states() {
        let none = view(&[]);
        assert_eq!(
            tooltip(&none, Lang::En, &paris),
            "Claude Usage Widget — no data yet"
        );

        let stale = view(&[obs("five_hour", 40.0, Some(NOW + 600), NOW - 3600)]);
        assert_eq!(
            tooltip(&stale, Lang::En, &paris),
            "Session 40% · resets 10 min\nas of 13:32 — not updating"
        );

        let reset = view(&[obs("five_hour", 90.0, Some(NOW - 60), NOW - 600)]);
        assert!(tooltip(&reset, Lang::En, &paris).starts_with("Session reset\n"));

        // Desktop-only data: no reset time, no countdown.
        let desk = build_view(
            merge(
                &[Observation {
                    source: Source::DesktopHistory,
                    ..obs("five_hour", 33.0, None, NOW - 60)
                }],
                NOW,
            ),
            &[],
            NOW,
        );
        assert_eq!(tooltip(&desk, Lang::En, &paris), "Session 33%\nas of 14:31");
    }

    #[test]
    fn countdowns() {
        assert_eq!(countdown(30, Lang::En), "< 1 min");
        assert_eq!(countdown(49 * 60 + 59, Lang::En), "49 min");
        assert_eq!(countdown(3600, Lang::En), "1h00");
        assert_eq!(countdown(2 * 86_400 + 5 * 3600, Lang::Fr), "2 j 5 h");
        assert_eq!(countdown(-5, Lang::En), "< 1 min");
    }

    #[test]
    fn labels() {
        assert_eq!(window_label("seven_day_fable", Lang::En), "Weekly · Fable");
        assert_eq!(window_label("spend_limit", Lang::Fr), "Plafond de dépense");
        assert_eq!(window_label("desktop_foo", Lang::En), "Desktop foo");
    }

    #[test]
    fn notification_texts() {
        let n = Notification::Threshold {
            window: "five_hour".into(),
            threshold: 80.0,
            used_percentage: 82.4,
            resets_at: Some(NOW + 72 * 60),
        };
        assert_eq!(
            notification_text(&n, NOW, Lang::En, &paris),
            (
                "Claude · Session at 82%".to_string(),
                "Your 80% alert was reached. Resets: 1h12 (15:44).".to_string()
            )
        );
        let (t, b) = notification_text(
            &Notification::Reset {
                window: "seven_day".into(),
            },
            NOW,
            Lang::Fr,
            &paris,
        );
        assert_eq!(t, "Claude · Hebdo réinitialisée");
        assert!(b.contains("de nouveau disponible"));
    }
}
