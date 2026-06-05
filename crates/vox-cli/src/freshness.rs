//! Binary-freshness self-check.
//!
//! Detects when *this* `vox` binary is older than the working tree it is being
//! run inside, by comparing the **embedded** build number (baked in at compile
//! time by [`vox_build_meta::emit`]) against the **live** build number computed
//! from the repo at run time (`git rev-list --count HEAD`).
//!
//! Motivation: a stale installed `vox.exe` runs outdated guard logic and
//! allowlists, so `vox ci *` can produce confidently-wrong verdicts that do not
//! reflect the current source. Build number (not semver) is the right signal —
//! it catches *same-version* staleness (`0.6.0+build.601` vs
//! `0.6.0+build.1917`) that a semver compare misses.
//!
//! Scope: enforcement runs only on `vox ci *` (the diagnostic surface) and as a
//! non-fatal `vox doctor` check. We deliberately do not add a git subprocess to
//! every `vox` invocation.

use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

/// Build number baked in at compile time (`git rev-list --count HEAD`), or
/// `"dev"` when git was unavailable during the build.
pub const EMBEDDED_BUILD_NUMBER: &str = env!("VOX_BUILD_NUMBER");

/// Short git hash baked in at compile time, for diagnostics only.
pub const EMBEDDED_GIT_HASH: &str = env!("VOX_GIT_HASH");

/// Environment variable that downgrades the `vox ci *` hard-fail to a note.
pub const SKIP_ENV: &str = "VOX_SKIP_FRESHNESS_CHECK";

/// Canonical refresh command shown in staleness diagnostics.
const REINSTALL_HINT: &str = "cargo install --path crates/vox-cli --force";

/// Platform basename of the `vox` executable (`vox.exe` on Windows).
pub fn vox_binary_name() -> &'static str {
    if cfg!(windows) { "vox.exe" } else { "vox" }
}

/// Canonical install location for the `vox` binary: `~/.cargo/bin/<vox>`.
///
/// This is what `cargo install -p vox-cli` (or `--path crates/vox-cli`)
/// produces and is declared the single source of truth. A second binary at
/// `~/.vox/bin` that reports a different build number is a divergence that
/// [`crate::commands::diagnostics::doctor`] flags.
pub fn canonical_install_path() -> std::path::PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".cargo")
        .join("bin")
        .join(vox_binary_name())
}

/// Extract the build number from a `vox --version` line such as
/// `vox 0.6.0+build.601 (abc1234)`. Returns `None` when the `+build.N` marker
/// is absent or non-numeric.
pub fn build_number_from_version_line(line: &str) -> Option<u64> {
    const MARKER: &str = "+build.";
    let start = line.find(MARKER)? + MARKER.len();
    let digits: String = line[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    parse_build_number(&digits)
}

/// Distinct build numbers among a set of reports (ignoring non-responders).
///
/// More than one distinct value means installed `vox` binaries disagree — a
/// PATH-shadowing divergence.
pub fn distinct_build_numbers(reports: &[Option<u64>]) -> Vec<u64> {
    let mut seen: Vec<u64> = reports.iter().flatten().copied().collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

/// Result of comparing the embedded build number against the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    /// Binary is at or ahead of the working tree — safe.
    Fresh,
    /// Binary predates the working tree — its guard logic may be outdated.
    Stale { embedded: u64, live: u64 },
    /// Could not determine (dev build, detached/no-git tree, parse failure).
    Unknown(&'static str),
}

/// Parse a build-number string into a comparable integer.
///
/// Returns `None` for the `"dev"` sentinel or any non-numeric value, which the
/// caller treats as [`Freshness::Unknown`] (never stale, never warn).
fn parse_build_number(s: &str) -> Option<u64> {
    s.trim().parse::<u64>().ok()
}

/// Pure classification of embedded-vs-live build numbers.
///
/// Only `embedded < live` is dangerous: the binary predates the source it is
/// judging. `embedded >= live` (a binary built at a newer commit than the
/// checked-out tree) is treated as fresh.
fn classify(embedded: Option<u64>, live: Option<u64>) -> Freshness {
    match (embedded, live) {
        (Some(embedded), Some(live)) => {
            if embedded < live {
                Freshness::Stale { embedded, live }
            } else {
                Freshness::Fresh
            }
        }
        (None, _) => Freshness::Unknown("binary has no numeric build number (dev build)"),
        (_, None) => {
            Freshness::Unknown("working tree build number unavailable (no git / detached)")
        }
    }
}

/// Live working-tree build number: `git -C <repo_root> rev-list --count HEAD`.
fn live_build_number(repo_root: &Path) -> Option<u64> {
    // vox-arch-check: allow git-exec
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    parse_build_number(s.trim())
}

/// Evaluate this binary's freshness against `repo_root`.
pub fn evaluate(repo_root: &Path) -> Freshness {
    classify(
        parse_build_number(EMBEDDED_BUILD_NUMBER),
        live_build_number(repo_root),
    )
}

/// Whether the operator asked to skip the freshness gate.
fn skip_requested() -> bool {
    std::env::var_os(SKIP_ENV).is_some_and(|v| !v.is_empty())
}

/// Human-readable staleness message for a verdict, or `None` when fresh/unknown.
fn staleness_message(freshness: &Freshness) -> Option<String> {
    match freshness {
        Freshness::Stale { embedded, live } => Some(format!(
            "installed vox is stale: built at commit {embedded} ({EMBEDDED_GIT_HASH}), \
             but the working tree is at commit {live}. Its guard logic and allowlists \
             may be outdated. Refresh with `{REINSTALL_HINT}`."
        )),
        Freshness::Fresh | Freshness::Unknown(_) => None,
    }
}

/// Human-readable staleness message, or `None` when fresh/unknown.
pub fn staleness_warning(repo_root: &Path) -> Option<String> {
    staleness_message(&evaluate(repo_root))
}

/// Pure gate: hard-fail on `Stale` unless `skip` is set (then warn to stderr).
fn gate(freshness: &Freshness, skip: bool) -> Result<()> {
    let Some(msg) = staleness_message(freshness) else {
        return Ok(());
    };
    if skip {
        eprintln!("warning: {msg} ({SKIP_ENV} set — running anyway)");
        return Ok(());
    }
    Err(anyhow!(
        "{msg}\n\
         `vox ci` will not run a guard with a stale binary — its verdict would not \
         reflect the current source. Set {SKIP_ENV}=1 to override."
    ))
}

/// Gate for `vox ci *`: hard-fail when the binary is stale.
///
/// A stale guard verdict is worse than no verdict, so `vox ci` refuses to run
/// on a stale binary. Set [`SKIP_ENV`] to downgrade this to a printed note.
pub fn enforce_for_ci(repo_root: &Path) -> Result<()> {
    gate(&evaluate(repo_root), skip_requested())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_dev_and_garbage() {
        assert_eq!(parse_build_number("601"), Some(601));
        assert_eq!(parse_build_number("  1917 "), Some(1917));
        assert_eq!(parse_build_number("dev"), None);
        assert_eq!(parse_build_number(""), None);
        assert_eq!(parse_build_number("12a"), None);
    }

    #[test]
    fn classify_stale_only_when_binary_older() {
        assert_eq!(
            classify(Some(601), Some(1917)),
            Freshness::Stale {
                embedded: 601,
                live: 1917
            }
        );
    }

    #[test]
    fn classify_equal_is_fresh() {
        assert_eq!(classify(Some(1917), Some(1917)), Freshness::Fresh);
    }

    #[test]
    fn classify_ahead_is_fresh() {
        // Binary built at a newer commit than the checked-out tree is not "stale".
        assert_eq!(classify(Some(2000), Some(1917)), Freshness::Fresh);
    }

    #[test]
    fn classify_unknown_when_either_missing() {
        assert!(matches!(classify(None, Some(1)), Freshness::Unknown(_)));
        assert!(matches!(classify(Some(1), None), Freshness::Unknown(_)));
        assert!(matches!(classify(None, None), Freshness::Unknown(_)));
    }

    #[test]
    fn gate_blocks_stale_without_skip() {
        let stale = Freshness::Stale {
            embedded: 601,
            live: 1917,
        };
        assert!(gate(&stale, false).is_err());
        assert!(gate(&stale, true).is_ok());
    }

    #[test]
    fn gate_allows_fresh_and_unknown() {
        assert!(gate(&Freshness::Fresh, false).is_ok());
        assert!(gate(&Freshness::Unknown("dev"), false).is_ok());
    }

    #[test]
    fn build_number_parsed_from_version_line() {
        assert_eq!(
            build_number_from_version_line("vox 0.6.0+build.601 (abc1234)"),
            Some(601)
        );
        assert_eq!(
            build_number_from_version_line("0.6.0+build.1917"),
            Some(1917)
        );
        // No marker / dev build → None.
        assert_eq!(build_number_from_version_line("vox 0.6.0 (dev)"), None);
        assert_eq!(build_number_from_version_line("vox 0.6.0+build.dev"), None);
    }

    #[test]
    fn distinct_build_numbers_ignores_nonresponders_and_dedups() {
        assert_eq!(
            distinct_build_numbers(&[Some(601), None, Some(601), Some(1917)]),
            vec![601, 1917]
        );
        assert_eq!(distinct_build_numbers(&[None, None]), Vec::<u64>::new());
        assert_eq!(distinct_build_numbers(&[Some(5)]), vec![5]);
    }
}
