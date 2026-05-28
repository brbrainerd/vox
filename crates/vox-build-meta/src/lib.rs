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
    let build_number =
        git_stdout(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "dev".to_string());
    println!("cargo:rustc-env=VOX_BUILD_NUMBER={build_number}");

    let git_hash =
        git_stdout(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VOX_GIT_HASH={git_hash}");

    // Invalidate when the branch tip changes.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
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
