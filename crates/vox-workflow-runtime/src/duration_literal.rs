//! Shared `@scheduled` / `workflow_wait` duration-literal parser.
//!
//! Single source of truth for what `"5m"` / `"500ms"` / `"1d"` mean across:
//!
//! - The `vox-codegen` emit (`emit_main_boot`) — validates literals at codegen
//!   time and emits a call to this function from the generated `main()`.
//! - The interpreted workflow planner (`workflow::plan::parse_duration_ms_str`)
//!   — wraps this function and converts to `u64` milliseconds.
//!
//! Previously these two surfaces had divergent parsers (different unit support,
//! different fallback behaviour). ADR-041 §6(a, c follow-up cluster) M-7 calls
//! for one parser; M-3 calls for a real error type instead of a silent fallback
//! to 60 seconds.
//!
//! ## Accepted literals
//!
//! | Literal | Meaning |
//! |---------|---------|
//! | `"500ms"` | 500 milliseconds |
//! | `"30s"` / `"30 s"` | 30 seconds |
//! | `"5m"` | 5 minutes |
//! | `"2h"` | 2 hours |
//! | `"1d"` | 1 day |
//! | `"30"` (bare integer) | 30 seconds (ergonomic default — matches cron-style schedulers) |
//!
//! Whitespace inside the literal is trimmed; leading sign characters and
//! decimal points are rejected.

use std::time::Duration;

/// Error produced when a duration literal can't be parsed. Implements
/// `std::error::Error` so it composes with `anyhow::Result` and the codegen
/// emit's compile-time validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationParseError {
    /// Empty after trimming.
    Empty,
    /// Numeric portion is empty (e.g. just `"ms"`).
    EmptyDigits,
    /// Numeric portion failed `u64::from_str`.
    InvalidNumber(String),
    /// Unit suffix is present but isn't one of the accepted units.
    UnknownUnit(String),
}

impl std::fmt::Display for DurationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty duration literal"),
            Self::EmptyDigits => {
                write!(f, "duration literal has unit suffix but no numeric digits")
            }
            Self::InvalidNumber(s) => write!(
                f,
                "duration literal numeric portion {:?} is not a non-negative integer",
                s
            ),
            Self::UnknownUnit(u) => {
                write!(
                    f,
                    "duration literal has unknown unit {:?} — expected one of ms, s, m, h, d (or a bare integer for seconds)",
                    u
                )
            }
        }
    }
}

impl std::error::Error for DurationParseError {}

/// Parse a duration literal. See module docs for the accepted grammar.
///
/// On error returns a structured [`DurationParseError`] — callers decide
/// whether to convert to a compile-time diagnostic (`vox_codegen::emit_main_boot`)
/// or a runtime `anyhow::Error` (`workflow::plan::parse_duration_ms_str`).
pub fn parse_duration_str(s: &str) -> Result<Duration, DurationParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(DurationParseError::Empty);
    }

    // Greedy capture of the leading ASCII-digit prefix, then any remaining
    // characters form the unit. Whitespace between digits and unit is allowed
    // (`"30 s"` parses).
    let digit_len = s.bytes().take_while(|b| b.is_ascii_digit()).count();
    let digits = &s[..digit_len];
    let unit = s[digit_len..].trim();

    if digits.is_empty() {
        // No leading digits. If the remainder looks number-shaped (decimals,
        // signs), the caller intended a number — emit InvalidNumber so the
        // diagnostic blames the right thing. Otherwise it's a unit-only
        // literal like `"minutes"` — UnknownUnit.
        if unit
            .chars()
            .any(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
        {
            return Err(DurationParseError::InvalidNumber(s.to_string()));
        }
        return Err(DurationParseError::UnknownUnit(unit.to_string()));
    }

    let unit_ms: u128 = match unit {
        // Bare integer (no unit) — ergonomic default = seconds.
        "" => 1_000,
        "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 60 * 60 * 1_000,
        "d" => 24 * 60 * 60 * 1_000,
        other => {
            // Two-pronged classification: if the "unit" contains
            // number-shaped characters, the user wrote something like
            // `"1.5h"` and the digits we captured are just a prefix — the
            // whole input is best diagnosed as InvalidNumber.
            if other
                .chars()
                .any(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
            {
                return Err(DurationParseError::InvalidNumber(s.to_string()));
            }
            return Err(DurationParseError::UnknownUnit(other.to_string()));
        }
    };

    let n: u64 = digits
        .parse()
        .map_err(|_| DurationParseError::InvalidNumber(digits.to_string()))?;
    let ms = (n as u128).saturating_mul(unit_ms);
    Ok(Duration::from_millis(ms.min(u64::MAX as u128) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_milliseconds() {
        assert_eq!(
            parse_duration_str("500ms").unwrap(),
            Duration::from_millis(500)
        );
        assert_eq!(parse_duration_str("0ms").unwrap(), Duration::from_millis(0));
    }

    #[test]
    fn parses_seconds() {
        assert_eq!(parse_duration_str("30s").unwrap(), Duration::from_secs(30));
        // Whitespace tolerated.
        assert_eq!(
            parse_duration_str(" 30 s ").unwrap(),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn parses_minutes_hours_days() {
        assert_eq!(parse_duration_str("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(
            parse_duration_str("2h").unwrap(),
            Duration::from_secs(2 * 3600)
        );
        assert_eq!(
            parse_duration_str("1d").unwrap(),
            Duration::from_secs(86_400)
        );
    }

    #[test]
    fn bare_integer_means_seconds() {
        // Ergonomic default — matches cron-ish schedulers.
        assert_eq!(parse_duration_str("30").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn empty_is_error() {
        assert_eq!(
            parse_duration_str("").unwrap_err(),
            DurationParseError::Empty
        );
        assert_eq!(
            parse_duration_str("   ").unwrap_err(),
            DurationParseError::Empty
        );
    }

    #[test]
    fn empty_digits_is_error() {
        // Suffix with no digits.
        matches!(
            parse_duration_str("ms").unwrap_err(),
            DurationParseError::EmptyDigits
        );
        matches!(
            parse_duration_str(" s").unwrap_err(),
            DurationParseError::EmptyDigits
        );
    }

    #[test]
    fn unknown_unit_is_error() {
        // "minutes" is not a recognised unit suffix — caller intended "m".
        match parse_duration_str("5minutes").unwrap_err() {
            DurationParseError::UnknownUnit(u) => assert_eq!(u, "minutes"),
            other => panic!("expected UnknownUnit, got {other:?}"),
        }
    }

    #[test]
    fn invalid_number_is_error() {
        // Decimal not supported.
        match parse_duration_str("1.5h").unwrap_err() {
            DurationParseError::InvalidNumber(_) => {}
            other => panic!("expected InvalidNumber, got {other:?}"),
        }
        // Negative not supported.
        match parse_duration_str("-30s").unwrap_err() {
            DurationParseError::InvalidNumber(_) => {}
            other => panic!("expected InvalidNumber, got {other:?}"),
        }
    }

    #[test]
    fn display_error_is_user_friendly() {
        let e = parse_duration_str("5minutes").unwrap_err();
        let msg = format!("{e}");
        assert!(msg.contains("minutes"), "{msg}");
        assert!(msg.contains("expected"), "{msg}");
    }
}
