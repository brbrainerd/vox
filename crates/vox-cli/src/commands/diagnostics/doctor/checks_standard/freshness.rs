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
        Freshness::Stale { embedded, live } => checks.push(Check::fail(
            CHECK_NAME,
            format!(
                "stale: built at commit {embedded}, working tree is at {live} — \
                 refresh with `cargo install --path crates/vox-cli --force`"
            ),
        )),
        // Dev builds / non-git trees are not a problem to report.
        Freshness::Unknown(reason) => checks.push(Check::pass(
            CHECK_NAME,
            format!("freshness not applicable ({reason})"),
        )),
    }
}
