//! Build-time metadata injection — call [`emit`] from any `build.rs`.
//!
//! Emits two `cargo:rustc-env` lines that downstream crates can consume with
//! `env!()`:
//!
//! - `VOX_BUILD_NUMBER` — git commit count on HEAD, auto-increments on every merge.
//! - `VOX_GIT_HASH`    — short SHA for diagnostics; never used in version comparison.
//!
//! The full display string produced by binaries is:
//! ```text
//! {CARGO_PKG_VERSION}+build.{VOX_BUILD_NUMBER} ({VOX_GIT_HASH})
//! ```
//!
//! This crate has no runtime dependencies — it must only be used as a
//! `[build-dependencies]` entry.

use std::process::Command;

/// Emit version metadata `cargo:rustc-env` vars from a build script.
///
/// Call this once from `build.rs`. Each binary's build script may add its own
/// `cargo:rerun-if-changed` lines after this call.
pub fn emit() {
    println!("cargo:rustc-env=VOX_BUILD_NUMBER={}", build_number());

    let git_hash =
        git_stdout(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VOX_GIT_HASH={git_hash}");

    // Invalidate when the branch tip changes.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
}

/// Resolve the build number, in precedence order, without ever inventing one.
///
/// 1. `VOX_BUILD_NUMBER` when set explicitly — the escape hatch for packagers and
///    for any build that has no git history.
/// 2. The commit count on HEAD, but ONLY in a full clone.
/// 3. `"dev"`.
///
/// The shallow check is the point. `actions/checkout` defaults to `fetch-depth: 1`,
/// so `rev-list --count HEAD` returns **1** in a release build — and every shipped
/// binary reported `0.6.0+build.1`, a number that looks precise and means nothing.
/// A build number that silently lies is worse than one that admits it is unknown.
fn build_number() -> String {
    if let Ok(explicit) = std::env::var("VOX_BUILD_NUMBER")
        && !explicit.trim().is_empty()
    {
        return explicit.trim().to_string();
    }
    if git_stdout(&["rev-parse", "--is-shallow-repository"]).as_deref() == Some("true") {
        // Shallow: the count is an artifact of clone depth, not of history.
        return "dev".to_string();
    }
    git_stdout(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "dev".to_string())
}

fn git_stdout(args: &[&str]) -> Option<String> {
    // vox-arch-check: allow git-exec
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
// `std::env::set_var`/`remove_var` are unsafe in edition 2024 (they race any
// concurrent getenv in another thread). The workspace denies `unsafe_code` via
// `-D warnings`, so the tests below need an explicit, scoped exemption rather
// than a workspace-wide relaxation. Each block is single-threaded and restores
// the variable immediately; see the SAFETY note on the first one.
#[allow(unsafe_code)]
mod tests {
    use super::*;

    #[test]
    fn explicit_env_wins_over_git() {
        // SAFETY: single-threaded test; restored immediately.
        unsafe { std::env::set_var("VOX_BUILD_NUMBER", "12345") };
        let n = build_number();
        unsafe { std::env::remove_var("VOX_BUILD_NUMBER") };
        assert_eq!(n, "12345");
    }

    #[test]
    fn blank_env_falls_through_to_git() {
        unsafe { std::env::set_var("VOX_BUILD_NUMBER", "   ") };
        let n = build_number();
        unsafe { std::env::remove_var("VOX_BUILD_NUMBER") };
        assert_ne!(
            n, "   ",
            "a blank override must not become the build number"
        );
    }

    #[test]
    fn git_stdout_with_bogus_arg_returns_none() {
        // A non-existent git subcommand must not panic; should return None.
        assert!(git_stdout(&["this-is-not-a-real-git-subcommand-xyz"]).is_none());
    }
}
