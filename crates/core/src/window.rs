//! Window identities and lengths.

pub const FIVE_HOUR: &str = "five_hour";
pub const SEVEN_DAY: &str = "seven_day";
/// Claude Desktop's `xu` key: extra (paid) usage — see docs/FINDINGS.md.
pub const EXTRA_USAGE: &str = "extra_usage";

pub const FIVE_HOUR_SECS: i64 = 5 * 3600;
pub const SEVEN_DAY_SECS: i64 = 7 * 86_400;

/// Length of a rolling window, when known: `five_hour*` = 5 h, `seven_day*` = 7 days (including
/// model-scoped weekly windows such as `seven_day_fable`). Others (`spend_limit`, `extra_usage`) have
/// no fixed length, so no pace can be computed for them.
pub fn window_length_secs(id: &str) -> Option<i64> {
    if id.starts_with(FIVE_HOUR) {
        Some(FIVE_HOUR_SECS)
    } else if id.starts_with(SEVEN_DAY) {
        Some(SEVEN_DAY_SECS)
    } else {
        None
    }
}

/// Display order: session, weekly, other weekly (model-scoped) windows, then the rest; ties by name.
pub fn display_rank(id: &str) -> (u8, &str) {
    let rank = match id {
        FIVE_HOUR => 0,
        SEVEN_DAY => 1,
        _ if id.starts_with(SEVEN_DAY) || id.starts_with(FIVE_HOUR) => 2,
        _ => 3,
    };
    (rank, id)
}

/// `sevenDayOpus` → `seven_day_opus`; snake_case input is returned unchanged.
pub fn normalise_id(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (i, c) in key.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '-' || c == ' ' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(window_length_secs("five_hour"), Some(18_000));
        assert_eq!(window_length_secs("seven_day"), Some(604_800));
        assert_eq!(window_length_secs("seven_day_fable"), Some(604_800));
        assert_eq!(window_length_secs("spend_limit"), None);
    }

    #[test]
    fn ordering() {
        let mut ids = [
            "spend_limit",
            "seven_day_fable",
            "seven_day",
            "five_hour",
            "extra_usage",
        ];
        ids.sort_by_key(|id| display_rank(id));
        assert_eq!(
            ids,
            [
                "five_hour",
                "seven_day",
                "seven_day_fable",
                "extra_usage",
                "spend_limit"
            ]
        );
    }

    #[test]
    fn normalise() {
        assert_eq!(normalise_id("sevenDayOpus"), "seven_day_opus");
        assert_eq!(normalise_id("five_hour"), "five_hour");
        assert_eq!(normalise_id("FiveHour"), "five_hour");
    }
}
