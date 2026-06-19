//! Best-effort "update available" footer for interactive CLI sessions.
//!
//! Non-blocking and failure-silent: a network error, a parse failure, or a
//! non-interactive/CI environment all result in NO output and NO error. This
//! must never change a command's exit code or perceptibly slow startup.

/// Returns `Some(latest)` when `latest` is a strictly newer semver than
/// `current`, else `None`. Both inputs may carry a leading `v` and/or build
/// metadata; we compare on the leading `MAJOR.MINOR.PATCH` only (pre-release
/// and `+build` suffixes are ignored for the "newer?" decision).
pub fn newer_version<'a>(current: &str, latest: &'a str) -> Option<&'a str> {
    let cur = parse_triplet(current)?;
    let lat = parse_triplet(latest)?;
    if lat > cur { Some(latest) } else { None }
}

fn parse_triplet(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    // Cut at the first non-version delimiter so "0.6.0-nightly.x+sha (hash)" works.
    let core = s
        .split(|c: char| c == '-' || c == '+' || c == ' ')
        .next()
        .unwrap_or(s);
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_newer_patch() {
        assert_eq!(newer_version("0.6.0", "0.6.1"), Some("0.6.1"));
    }

    #[test]
    fn ignores_equal_and_older() {
        assert_eq!(newer_version("0.6.1", "0.6.1"), None);
        assert_eq!(newer_version("0.6.2", "0.6.1"), None);
    }

    #[test]
    fn strips_v_prefix_and_build_metadata() {
        assert_eq!(
            newer_version("0.6.0+build.7 (abc1234)", "v0.7.0"),
            Some("v0.7.0")
        );
    }

    #[test]
    fn garbage_is_silent_none() {
        assert_eq!(newer_version("not-a-version", "0.6.1"), None);
        assert_eq!(newer_version("0.6.0", "garbage"), None);
    }
}
