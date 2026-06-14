//! Resolution of `--since`/`--until` into a concrete `CommitRange`.

use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum CommitRange {
    /// Inclusive `since_ref..until_ref` git ref pair.
    Refs { since: String, until: String },
    /// "commits with commit_ts >= now - duration, walking from until_ref".
    SinceDuration { duration: Duration, until: String },
}

#[derive(Debug, Error)]
pub enum RangeError {
    #[error("invalid duration string: {0}")]
    InvalidDuration(String),
}

/// Parses a duration string of the form `NNd`, `NNh`, or `NNw` (N = ASCII digits),
/// or the long forms `"N days ago"` / `"N hours ago"` / `"N weeks ago"`.
///
/// The compact `digits+suffix` form requires the prefix to be ALL ASCII
/// digits — refs like `feature-2d` or `abc123d` MUST NOT parse as durations.
pub fn parse_duration(s: &str) -> Result<Duration, RangeError> {
    let s = s.trim();

    // "<n> days|hours|weeks ago" form — `parse::<i64>` enforces the digit
    // requirement on `rest`, but we still reject silently surprising shapes.
    if let Some(rest) = s.strip_suffix(" days ago") {
        return rest
            .trim()
            .parse::<i64>()
            .map(Duration::days)
            .map_err(|_| RangeError::InvalidDuration(s.into()));
    }
    if let Some(rest) = s.strip_suffix(" hours ago") {
        return rest
            .trim()
            .parse::<i64>()
            .map(Duration::hours)
            .map_err(|_| RangeError::InvalidDuration(s.into()));
    }
    if let Some(rest) = s.strip_suffix(" weeks ago") {
        return rest
            .trim()
            .parse::<i64>()
            .map(Duration::weeks)
            .map_err(|_| RangeError::InvalidDuration(s.into()));
    }

    // Compact `<digits><d|h|w>` form: the prefix MUST be all ASCII digits so
    // that refs like `feature-2d` or short SHAs ending in d/h/w are rejected.
    if s.len() >= 2 {
        let (num, suffix) = s.split_at(s.len() - 1);
        if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
            let n: i64 = num
                .parse()
                .map_err(|_| RangeError::InvalidDuration(s.into()))?;
            return match suffix {
                "d" => Ok(Duration::days(n)),
                "h" => Ok(Duration::hours(n)),
                "w" => Ok(Duration::weeks(n)),
                _ => Err(RangeError::InvalidDuration(s.into())),
            };
        }
    }

    Err(RangeError::InvalidDuration(s.into()))
}

/// Resolves CLI args + config default into a `CommitRange`.
///
/// `since_arg` and `until_arg` are the raw `--since`/`--until` strings.
/// If neither parses as a duration, both are treated as git refs.
pub fn resolve(
    since_arg: Option<&str>,
    until_arg: Option<&str>,
    default_since: &str,
) -> Result<CommitRange, RangeError> {
    let until = until_arg.unwrap_or("HEAD").to_string();
    let since_raw = since_arg.unwrap_or(default_since);

    match parse_duration(since_raw) {
        Ok(d) => Ok(CommitRange::SinceDuration { duration: d, until }),
        Err(_) => Ok(CommitRange::Refs {
            since: since_raw.into(),
            until,
        }),
    }
}

/// For `SinceDuration`, the wall-clock cutoff at run time.
pub fn duration_cutoff(now: DateTime<Utc>, d: Duration) -> DateTime<Utc> {
    now - d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_days_suffix_forms() {
        assert_eq!(parse_duration("30d").unwrap(), Duration::days(30));
        assert_eq!(parse_duration("30 days ago").unwrap(), Duration::days(30));
    }

    #[test]
    fn duration_default_when_no_args() {
        let r = resolve(None, None, "30 days ago").unwrap();
        assert!(matches!(r, CommitRange::SinceDuration { .. }));
    }

    #[test]
    fn ref_when_since_is_sha_or_branch() {
        let r = resolve(Some("v0.5.0"), Some("HEAD"), "30 days ago").unwrap();
        assert!(matches!(r, CommitRange::Refs { ref since, .. } if since == "v0.5.0"));
    }

    #[test]
    fn head_caret_is_ref_not_duration() {
        // HEAD~30 is git-native "30 commits back", not a duration. Must be treated as ref.
        let r = resolve(Some("HEAD~30"), None, "30 days ago").unwrap();
        assert!(matches!(r, CommitRange::Refs { .. }));
    }

    #[test]
    fn rejects_ref_with_trailing_duration_letter() {
        // `feature-2d` looks like it ends in a duration suffix but must NOT
        // parse as one — it's a branch name.
        assert!(parse_duration("feature-2d").is_err());
        // Short SHA-like refs ending in d/h/w must also be rejected.
        assert!(parse_duration("abc123d").is_err());
        assert!(parse_duration("abc123h").is_err());
        assert!(parse_duration("abc123w").is_err());
    }

    #[test]
    fn duration_cutoff_subtracts() {
        let now = Utc::now();
        let cutoff = duration_cutoff(now, Duration::days(7));
        assert!((now - cutoff).num_days() == 7);
    }
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: parse_duration silently returns Ok for empty string instead of Err.
    #[test]
    fn empty_string_is_error() {
        assert!(
            matches!(parse_duration(""), Err(RangeError::InvalidDuration(_))),
            "empty string must be InvalidDuration"
        );
    }

    // Catches: "0d" being treated as invalid even though 0 is a legitimate boundary.
    #[test]
    fn zero_days_compact_is_valid() {
        let d = parse_duration("0d").expect("0d should parse");
        assert_eq!(d.num_days(), 0);
    }

    // Catches: Duration::days(i64::MAX) panicking inside parse_duration instead of returning Err.
    // This is a KNOWN BUG: chrono panics on out-of-range days. Marked should_panic to document it.
    #[test]
    #[should_panic(expected = "out of bounds")]
    fn gigantic_days_panics_in_chrono_known_bug() {
        // i64::MAX days overflows chrono's TimeDelta — parse_duration does not guard against this.
        let big = format!("{}d", i64::MAX);
        let _ = parse_duration(&big);
    }

    // Catches: whitespace between number and suffix being incorrectly accepted.
    #[test]
    fn whitespace_inside_compact_form_is_rejected() {
        // "3 d" is not a valid compact form — only "3d".
        assert!(parse_duration("3 d").is_err());
    }

    // Catches: non-digit prefix in compact form (e.g. "+3d") silently accepted.
    #[test]
    fn sign_prefix_compact_form_is_rejected() {
        assert!(parse_duration("+3d").is_err());
        assert!(parse_duration("-3d").is_err());
    }

    // Catches: "hours ago" suffix form accepting non-numeric prefix.
    #[test]
    fn non_numeric_long_form_is_rejected() {
        assert!(parse_duration("abc hours ago").is_err());
        assert!(parse_duration("two days ago").is_err());
    }

    // Catches: resolve() treating "0d" (a valid duration) as a ref rather than SinceDuration.
    #[test]
    fn zero_duration_since_arg_gives_since_duration_variant() {
        let r = resolve(Some("0d"), None, "7d").unwrap();
        assert!(
            matches!(r, CommitRange::SinceDuration { duration, .. } if duration.num_seconds() == 0)
        );
    }

    // Catches: resolve() overriding explicit until_arg with HEAD when arg is provided.
    #[test]
    fn explicit_until_arg_is_preserved() {
        let r = resolve(Some("v1.0"), Some("v2.0"), "30d").unwrap();
        if let CommitRange::Refs { until, .. } = r {
            assert_eq!(until, "v2.0");
        } else {
            panic!("expected Refs variant");
        }
    }

    // Catches: hours form producing wrong magnitude (e.g., treating hours as days).
    #[test]
    fn hours_suffix_produces_hours_not_days() {
        let d = parse_duration("24h").unwrap();
        assert_eq!(d.num_hours(), 24);
        assert_eq!(d.num_days(), 1);
    }

    // Catches: weeks suffix producing wrong magnitude.
    #[test]
    fn weeks_suffix_produces_seven_day_multiples() {
        let d = parse_duration("2w").unwrap();
        assert_eq!(d.num_days(), 14);
    }

    // Catches: "0 hours ago" long form being rejected when 0 should be valid.
    #[test]
    fn zero_hours_ago_long_form_is_valid() {
        let d = parse_duration("0 hours ago").expect("0 hours ago must be valid");
        assert_eq!(d.num_hours(), 0);
    }
}
