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

/// Parses a duration string of the form `<digits>{d|h|w}` or "<digits> days ago"
/// / "<digits> hours ago" / "<digits> weeks ago".
///
/// The compact `<digits><suffix>` form requires the prefix to be ALL ASCII
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
