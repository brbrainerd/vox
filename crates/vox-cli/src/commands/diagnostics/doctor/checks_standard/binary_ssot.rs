//! `vox doctor` — canonical-binary single-source-of-truth check.
//!
//! Two `vox` binaries can coexist — `~/.cargo/bin/vox` (the canonical
//! `cargo install` target) and `~/.vox/bin/vox` (voxup-managed) — and drift to
//! different build numbers. Which one "wins" then depends on `PATH` order, so a
//! stale shadowing binary silently runs outdated logic. This check enumerates
//! every `vox` on `PATH` plus the two known locations, asks each its build
//! number via `--version`, and fails if they disagree.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use super::super::common::Check;
use crate::freshness::{
    build_number_from_version_line, canonical_install_path, distinct_build_numbers, vox_binary_name,
};

const CHECK_NAME: &str = "Binary SSOT (canonical vox)";

/// Candidate `vox` executable locations: every `PATH` dir plus the two known
/// install roots. Deduplicated by canonical path.
fn candidate_paths() -> Vec<PathBuf> {
    let name = vox_binary_name();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut out = Vec::new();
    let mut push = |p: PathBuf| {
        let key = std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
        if seen.insert(key) {
            out.push(p);
        }
    };

    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            // Skip Cargo build dirs (`…/target/{debug,release}`): a developer's
            // in-progress local build is not an "installed" binary, and counting
            // it would flag a divergence on every active checkout.
            if crate::freshness::is_cargo_build_dir(&dir) {
                continue;
            }
            let cand = dir.join(name);
            if cand.is_file() {
                push(cand);
            }
        }
    }
    // The two known install roots, even if not on PATH: the canonical
    // voxup-managed `~/.vox/bin` and the `cargo install` `~/.cargo/bin`.
    push(canonical_install_path());
    push(crate::freshness::cargo_install_path());

    out.into_iter()
        .filter(|p| std::fs::metadata(p).is_ok())
        .collect()
}

/// Build number reported by `<path> --version`, or `None` if it cannot be read.
fn report_build_number(path: &std::path::Path) -> Option<u64> {
    let out = Command::new(path).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    build_number_from_version_line(text.trim())
}

pub fn run(checks: &mut Vec<Check>) {
    let candidates = candidate_paths();
    if candidates.is_empty() {
        checks.push(Check::pass(
            CHECK_NAME,
            "no installed vox binary found on PATH or in ~/.cargo/bin / ~/.vox/bin".to_string(),
        ));
        return;
    }

    let reports: Vec<(PathBuf, Option<u64>)> = candidates
        .into_iter()
        .map(|p| {
            let n = report_build_number(&p);
            (p, n)
        })
        .collect();

    let numbers: Vec<Option<u64>> = reports.iter().map(|(_, n)| *n).collect();
    let distinct = distinct_build_numbers(&numbers);
    let canonical = canonical_install_path();

    if distinct.len() <= 1 {
        let detail = match distinct.first() {
            Some(n) => format!("{} vox binar(ies) agree on build {n}", reports.len()),
            None => format!(
                "{} vox binar(ies) found; none reported a build number",
                reports.len()
            ),
        };
        checks.push(Check::pass(CHECK_NAME, detail));
        return;
    }

    let listing = reports
        .iter()
        .map(|(p, n)| {
            let marker = if std::fs::canonicalize(p).ok() == std::fs::canonicalize(&canonical).ok()
            {
                " [canonical]"
            } else {
                ""
            };
            let build = n.map_or_else(|| "unknown".to_string(), |n| n.to_string());
            format!("{} = build {build}{marker}", p.display())
        })
        .collect::<Vec<_>>()
        .join("; ");

    checks.push(Check::fail(
        CHECK_NAME,
        format!(
            "installed vox binaries disagree on build number ({listing}). Canonical is {} — \
             refresh it ({}) and remove or update any earlier-on-PATH copy.",
            canonical.display(),
            crate::freshness::refresh_guidance(),
        ),
    ));
}
