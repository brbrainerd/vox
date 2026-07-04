//! `vox ci detect-rules-bench` — authoring-time F1 scorer for `contracts/code-audit/rules.v1.yaml`.
//!
//! Walks `<fixtures-root>/<parent-id>/` directories, matches positive/negative
//! fixture files against the compiled rule pack, and emits a precision/recall/F1
//! table.  Non-zero exit when any rule's F1 falls below `--min-f1`.

use anyhow::{Context, Result, bail};
use std::path::Path;
use vox_rule_pack::{RulePack, bench::run_bench};

pub fn run(rules: &Path, fixtures_root: &Path, min_f1: f64, json: bool) -> Result<()> {
    let yaml = std::fs::read_to_string(rules)
        .with_context(|| format!("cannot read rules file: {}", rules.display()))?;

    let pack = RulePack::load_from_str(&yaml)
        .with_context(|| format!("failed to parse rules file: {}", rules.display()))?;

    let report = run_bench(&pack, fixtures_root);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_table(&report, min_f1);
    }

    let failing: Vec<_> = report.rules.iter().filter(|r| r.f1 < min_f1).collect();

    if !failing.is_empty() {
        eprintln!();
        eprintln!(
            "{} rule(s) below F1 threshold ({min_f1:.2}):",
            failing.len()
        );
        for r in &failing {
            eprintln!("  {} — F1={:.3}", r.rule_id, r.f1);
        }
        bail!("detect-rules-bench: F1 threshold not met");
    }

    Ok(())
}

fn print_table(report: &vox_rule_pack::bench::BenchReport, min_f1: f64) {
    println!(
        "{:<45}  {:>5}  {:>4}  {:>4}  {:>4}  {:>6}  {:>6}  {:>6}",
        "rule_id", "pos", "pos+", "neg", "neg-", "prec", "recall", "f1"
    );
    println!("{}", "-".repeat(95));
    for r in &report.rules {
        let flag = if r.f1 < min_f1 { " ✗" } else { "" };
        println!(
            "{:<45}  {:>5}  {:>4}  {:>4}  {:>4}  {:>4.2}  {:>6.3}  {:>6.3}{}",
            r.rule_id,
            r.positive_total,
            r.positive_matched,
            r.negative_total,
            r.negative_matched,
            r.precision,
            r.recall,
            r.f1,
            flag,
        );
    }
}
