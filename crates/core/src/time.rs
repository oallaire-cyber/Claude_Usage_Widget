//! Timestamp parsing. Everything in the core is Unix seconds (`i64`); the core never reads the clock.

use serde_json::Value;

/// Values above this are taken to be milliseconds rather than seconds (year ~2286 in seconds).
const MILLIS_THRESHOLD: f64 = 1e11;

/// Parse a `resets_at`-style value: Unix seconds (int or float), Unix milliseconds, a numeric string,
/// or an ISO 8601 / RFC 3339 string. Returns `None` for anything else.
pub fn parse_timestamp(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => from_epoch_number(n.as_f64()?),
        Value::String(s) => {
            let s = s.trim();
            if let Ok(f) = s.parse::<f64>() {
                return from_epoch_number(f);
            }
            parse_iso8601(s)
        }
        _ => None,
    }
}

fn from_epoch_number(f: f64) -> Option<i64> {
    if !f.is_finite() || f <= 0.0 {
        return None;
    }
    let secs = if f > MILLIS_THRESHOLD { f / 1000.0 } else { f };
    Some(secs.floor() as i64)
}

/// Parse `YYYY-MM-DD[T ]HH:MM[:SS[.fff]][Z|±HH:MM|±HHMM]`. A missing offset is read as UTC.
pub fn parse_iso8601(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> {
        let part = s.get(from..from + len)?;
        if part.bytes().all(|c| c.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    if b.len() < 16
        || b[4] != b'-'
        || b[7] != b'-'
        || !(b[10] == b'T' || b[10] == b't' || b[10] == b' ')
        || b[13] != b':'
    {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute) = (num(11, 2)?, num(14, 2)?);
    let mut i = 16;
    let mut second = 0;
    if b.get(i) == Some(&b':') {
        second = num(i + 1, 2)?;
        i += 3;
        if b.get(i) == Some(&b'.') || b.get(i) == Some(&b',') {
            i += 1;
            while b.get(i).is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
            }
        }
    }
    let offset = match b.get(i) {
        None => 0,
        Some(b'Z') | Some(b'z') if i + 1 == b.len() => 0,
        Some(&sign @ (b'+' | b'-')) => {
            let oh = num(i + 1, 2)?;
            let rest = &s[i + 3..];
            let om = match rest.len() {
                0 => 0,
                2 => num(i + 3, 2)?,
                3 if rest.starts_with(':') => num(i + 4, 2)?,
                _ => return None,
            };
            let off = oh * 3600 + om * 60;
            if sign == b'+' {
                off
            } else {
                -off
            }
        }
        _ => return None,
    };
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unix_seconds_int_and_float() {
        assert_eq!(parse_timestamp(&json!(1738425600)), Some(1738425600));
        assert_eq!(parse_timestamp(&json!(1738425600.9)), Some(1738425600));
    }

    #[test]
    fn unix_millis_and_numeric_string() {
        assert_eq!(parse_timestamp(&json!(1738425600123_i64)), Some(1738425600));
        assert_eq!(parse_timestamp(&json!("1738425600")), Some(1738425600));
    }

    #[test]
    fn iso_variants() {
        // 2025-02-01T16:00:00Z == 1738425600
        for s in [
            "2025-02-01T16:00:00Z",
            "2025-02-01T16:00:00.000Z",
            "2025-02-01T16:00:00.123456+00:00",
            "2025-02-01T17:00:00+01:00",
            "2025-02-01T11:00:00-0500",
            "2025-02-01 16:00:00",
            "2025-02-01T16:00Z",
        ] {
            assert_eq!(parse_timestamp(&json!(s)), Some(1738425600), "{s}");
        }
    }

    #[test]
    fn rejects_garbage() {
        for v in [
            json!(null),
            json!(true),
            json!(""),
            json!("soon"),
            json!(-5),
            json!("2025-13-01T00:00:00Z"),
            json!({}),
        ] {
            assert_eq!(parse_timestamp(&v), None, "{v}");
        }
    }

    #[test]
    fn epoch_and_leap_day() {
        assert_eq!(parse_iso8601("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601("2024-02-29T00:00:00Z"), Some(1709164800));
    }
}
