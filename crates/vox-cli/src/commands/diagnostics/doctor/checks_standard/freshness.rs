//! `vox doctor` — installed-binary freshness vs the working tree.
//!
//! Surfaces the same signal that `vox ci *` hard-fails on (see
//! [`crate::freshness`]), but as a non-fatal advisory row.

use super::super::common::Check;
use crate::commands::ci::repo_root;
use crate::freshness::{self, Freshness};

const CHECK_NAME: &str = "Binary freshness";

pub fn run(checks: &mut Vec<Check>) {
    let root = repo_root();
    match freshness::evaluate(&root) {
        Freshness::Fresh => checks.push(Check::pass(
            CHECK_NAME,
            format!(
                "installed vox build {} matches the working tree",
                freshness::EMBEDDED_BUILD_NUMBER
            ),
        )),
        Freshness::Stale { embedded, live } => {
            let guidance = freshness::refresh_guidance();
            checks.push(Check::fail(
                CHECK_NAME,
                format!(
                    "stale: built at commit {embedded}, working tree is at {live} — \
                     refresh: {guidance}"
                ),
            ))
        }
        // Dev builds / non-git trees are not a problem to report.
        Freshness::Unknown(reason) => checks.push(Check::pass(
            CHECK_NAME,
            format!("freshness not applicable ({reason})"),
        )),
    }
}

#[cfg(test)]
mod tests {
    /// This check runs unconditionally on every plain `vox doctor` (see
    /// `checks_standard/mod.rs`'s `run_checks`), so its stale-binary message
    /// must be persona-aware (spec §9.1): it must delegate to
    /// [`freshness::refresh_guidance`] rather than hardcoding a `cargo
    /// install`/`crates/`-relative remedy that only applies to a contributor.
    /// This guards against the message drifting back to a hardcoded literal.
    #[test]
    fn stale_message_delegates_to_persona_aware_refresh_guidance() {
        let src = include_str!("freshness.rs");
        assert!(
            src.contains("freshness::refresh_guidance()"),
            "expected the stale-binary message to use the persona-aware \
             refresh_guidance() helper instead of a hardcoded cargo/crates \
             literal"
        );
    }
}
