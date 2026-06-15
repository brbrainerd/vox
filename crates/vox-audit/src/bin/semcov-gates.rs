//! CI coverage gates: runs the no-NEW-regression gates against their committed
//! grandfather baselines and exits non-zero if either reports findings beyond the
//! baseline.
//!
//! - silent-drop: `catch_all_swallow` + `cross_crate_dup` vs
//!   `contracts/toestub/silent-drop-baseline.v1.json`
//! - weak-test:   `weak_test` (touch tests) vs
//!   `contracts/toestub/weak-test-baseline.v1.json`
//!
//! Both detectors are advisory heuristics; these gates guard only NEW findings
//! (e.g. introduced by Phase-3 coverage waves), not the grandfathered suite.
//! Run from the repo root: `cargo run -q -p vox-audit --bin coverage-gates`.

use std::path::PathBuf;
use std::process::ExitCode;
use vox_audit::core_gates::{run_silent_drop_gate, run_weak_test_gate};

fn main() -> ExitCode {
    let root = std::env::current_dir().expect("current dir");
    let crates = root.join("crates");
    if !crates.is_dir() {
        eprintln!(
            "coverage-gates: run from the repo root (no `crates/` under {})",
            root.display()
        );
        return ExitCode::FAILURE;
    }
    let sd_baseline = baseline_if_exists(&root, "contracts/toestub/silent-drop-baseline.v1.json");
    let wt_baseline = baseline_if_exists(&root, "contracts/toestub/weak-test-baseline.v1.json");

    let sd = run_silent_drop_gate(&crates, sd_baseline);
    let wt = run_weak_test_gate(&crates, wt_baseline);

    println!(
        "silent-drop: ok={} {}",
        sd.ok,
        sd.detail.unwrap_or_default()
    );
    println!(
        "weak-test:   ok={} {}",
        wt.ok,
        wt.detail.unwrap_or_default()
    );

    if sd.ok && wt.ok {
        println!("coverage-gates: OK (no new silent-drop or touch-test findings)");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "::error::coverage-gates: NEW finding(s) beyond baseline — add a real assertion / fix the silent drop, or (if intentional) regenerate the baseline."
        );
        ExitCode::FAILURE
    }
}

fn baseline_if_exists(root: &PathBuf, rel: &str) -> Option<PathBuf> {
    let p = root.join(rel);
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("coverage-gates: baseline missing ({rel}); running ungrandfathered");
        None
    }
}
