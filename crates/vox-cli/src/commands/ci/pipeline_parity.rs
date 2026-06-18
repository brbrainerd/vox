//! `vox ci pipeline-parity` — umbrella gate for script→emission SSOT verification.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use vox_compiler::feature_matrix::{Feature, Support, support};
use vox_compiler::target::Target;

pub async fn run(root: &Path) -> Result<()> {
    println!("pipeline-parity: grammar SSOT…");
    super::grammar_ssot_parity::run().await?;

    println!("pipeline-parity: canonical golden ladder…");
    run_cargo_test(
        root,
        &[
            "test",
            "-p",
            "vox-compiler",
            "--test",
            "emission_ladder_test",
            "--",
            "--test-threads=1",
            "--nocapture",
        ],
    )?;

    println!("pipeline-parity: feature matrix smoke…");
    run_cargo_test(
        root,
        &[
            "test",
            "-p",
            "vox-compiler",
            "--test",
            "feature_matrix_parity_test",
            "--",
            "--nocapture",
        ],
    )?;

    println!("pipeline-parity: k-complexity budget (ladder-scoped)…");
    super::run_body::run_body_helpers::run_k_complexity_budget(root, 0.0, false)?;

    print_matrix_coverage();
    println!("pipeline-parity OK");
    Ok(())
}

fn run_cargo_test(root: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .current_dir(root)
        .args(args)
        .status()
        .with_context(|| format!("spawn cargo {}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("cargo {} failed (exit {status})", args.join(" ")))
    }
}

fn print_matrix_coverage() {
    let mut implemented = 0usize;
    let mut unverified = 0usize;
    let mut unsupported = 0usize;
    for feature in Feature::all() {
        for target in Target::ALL {
            match support(feature, target) {
                Support::Implemented => implemented += 1,
                Support::Unverified => unverified += 1,
                Support::Unsupported(_) => unsupported += 1,
            }
        }
    }
    let total = implemented + unverified + unsupported;
    let pct = |n: usize| n as f64 * 100.0 / total as f64;
    println!(
        "matrix coverage: {:.1}% Implemented, {:.1}% Unverified, {:.1}% Unsupported ({total} cells)",
        pct(implemented),
        pct(unverified),
        pct(unsupported)
    );
}
