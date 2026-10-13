//! Input checks for what the page sends to the shell. The page is not trusted (it only reaches the
//! shell through the commands in commands.rs), so everything it passes is re-validated here, in plain
//! functions that have their own tests.

use serde_json::{json, Value};

/// Same numbers as twitch_radio/lookahead.py (tests/test_radio_lookahead.py checks they agree).
pub const LOOKAHEAD_MIN: i64 = 1;
pub const LOOKAHEAD_MAX: i64 = 15;
pub const LOOKAHEAD_DEFAULT: i64 = 5;
pub const LOOKAHEAD_DEFAULT_ENABLED: bool = true;

pub const MAX_QUERY_LENGTH: usize = 300;
pub const THEMES: [&str; 3] = ["system", "light", "dark"];

/// The colour styles, shared with the page (app/src/lib/skins.ts imports the same file).
const SKIN_NAMES_JSON: &str = include_str!("../../shared/skin-names.json");

pub fn skin_names() -> Vec<String> {
    serde_json::from_str(SKIN_NAMES_JSON).unwrap_or_else(|_| vec!["default".to_string()])
}

pub fn is_theme(value: &str) -> bool {
    THEMES.contains(&value)
}

pub fn is_skin(value: &str) -> bool {
    skin_names().iter().any(|name| name == value)
}

/// "#rrggbb" and nothing else.
pub fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// A song search or link typed in the dashboard: one tidy line, or None when there is nothing usable.
pub fn clean_query(value: &Value) -> Option<String> {
    let text = value.as_str()?;
    let query = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // JavaScript counted UTF-16 units; keep the same limit for the same input.
    if query.is_empty() || query.encode_utf16().count() > MAX_QUERY_LENGTH {
        return None;
    }
    Some(query)
}

/// What JavaScript's parseInt(text, 10) reads: optional spaces, an optional sign, then digits (the rest is ignored).
pub fn parse_int_prefix(text: &str) -> Option<i64> {
    let trimmed = text.trim_start();
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let end = digits.bytes().take_while(u8::is_ascii_digit).count();
    if end == 0 {
        return None;
    }
    let value: i64 = digits[..end].parse().ok()?;
    Some(if negative { -value } else { value })
}

/// parseInt() of a JSON value: numbers are cut to whole numbers, strings are read, anything else is nothing.
pub fn parse_int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)),
        Value::String(s) => parse_int_prefix(s),
        _ => None,
    }
}

/// The radio-lookahead setting from anything (a saved file, the page): always usable, always in range.
pub fn normalize_lookahead(value: &Value) -> Value {
    let enabled = value.get("enabled").and_then(Value::as_bool).unwrap_or(LOOKAHEAD_DEFAULT_ENABLED);
    let count = value
        .get("count")
        .and_then(parse_int_value)
        .map_or(LOOKAHEAD_DEFAULT, |n| n.clamp(LOOKAHEAD_MIN, LOOKAHEAD_MAX));
    json!({ "enabled": enabled, "count": count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_query_tidies_one_line_and_refuses_nothing_or_too_much() {
        assert_eq!(clean_query(&json!("  daft   punk\none more time ")).as_deref(), Some("daft punk one more time"));
        assert_eq!(
            clean_query(&json!("https://www.youtube.com/watch?v=abc")).as_deref(),
            Some("https://www.youtube.com/watch?v=abc")
        );
        for bad in [json!(""), json!("   "), json!("\n\t"), json!(null), json!(42), json!({}), json!(["x"])] {
            assert_eq!(clean_query(&bad), None, "{bad}");
        }
        assert_eq!(clean_query(&json!("x".repeat(MAX_QUERY_LENGTH + 1))), None);
        assert_eq!(clean_query(&json!("x".repeat(MAX_QUERY_LENGTH))).unwrap().len(), MAX_QUERY_LENGTH);
    }

    #[test]
    fn normalize_lookahead_always_returns_a_usable_in_range_setting() {
        let default = json!({ "enabled": true, "count": 5 });
        assert_eq!(normalize_lookahead(&json!({})), default);
        assert_eq!(normalize_lookahead(&json!(null)), default);
        assert_eq!(normalize_lookahead(&json!("junk")), default);
        assert_eq!(normalize_lookahead(&json!({ "enabled": false, "count": 9 })), json!({ "enabled": false, "count": 9 }));
        assert_eq!(normalize_lookahead(&json!({ "enabled": "no", "count": "7" })), json!({ "enabled": true, "count": 7 }));
        assert_eq!(normalize_lookahead(&json!({ "count": 0 }))["count"], LOOKAHEAD_MIN);
        assert_eq!(normalize_lookahead(&json!({ "count": -4 }))["count"], LOOKAHEAD_MIN);
        assert_eq!(normalize_lookahead(&json!({ "count": 99 }))["count"], LOOKAHEAD_MAX);
        for bad in [json!(null), json!([]), json!({}), json!(true), json!("many")] {
            assert_eq!(normalize_lookahead(&json!({ "count": bad }))["count"], LOOKAHEAD_DEFAULT, "{bad}");
        }
    }

    #[test]
    fn parse_int_reads_what_javascript_reads() {
        assert_eq!(parse_int_prefix("12"), Some(12));
        assert_eq!(parse_int_prefix("  -7x"), Some(-7));
        assert_eq!(parse_int_prefix("+3"), Some(3));
        assert_eq!(parse_int_prefix("abc"), None);
        assert_eq!(parse_int_prefix(""), None);
        assert_eq!(parse_int_value(&json!(9.9)), Some(9));
        assert_eq!(parse_int_value(&json!(true)), None);
    }

    #[test]
    fn is_theme_accepts_exactly_the_three_appearance_choices() {
        for ok in ["system", "light", "dark"] {
            assert!(is_theme(ok));
        }
        for bad in ["Light", "", "auto"] {
            assert!(!is_theme(bad));
        }
    }

    #[test]
    fn colour_styles_only_known_names_are_accepted() {
        assert!(is_skin("default") && is_skin("midnight"));
        for bad in ["", "Midnight", "constructor", "__proto__"] {
            assert!(!is_skin(bad), "{bad}");
        }
    }

    #[test]
    fn hex_colours_are_exactly_hash_and_six_digits() {
        assert!(is_hex_color("#0b0b10") && is_hex_color("#FFFFFF"));
        for bad in ["", "#fff", "0b0b10", "#0b0b1", "#0b0b1g", "#0b0b100", "red"] {
            assert!(!is_hex_color(bad), "{bad}");
        }
    }
}
