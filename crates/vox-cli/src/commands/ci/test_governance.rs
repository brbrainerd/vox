//! Governance gates built on test inventory and runtime report artifacts (`ignored-test-age`, `flake-budget`, `runtime-regress`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use vox_bounded_fs::read_utf8_path_capped;

use super::cmd_enums::GovernanceGateMode;
use super::test_inventory::{
    IgnoredTestGovernanceFinding, IgnoredTestGovernanceIssue, TestInventoryReport,
    scan_ignored_test_governance_findings,
};
use super::test_runtime_report::{
    TestRuntimeReport, compare_runtime_reports, parse_runtime_report_json,
    retry_flaky_candidate_count,
};

#[derive(Debug, Clone, Serialize)]
struct IgnoredTestAgeJson {
    schema_version: u32,
    mode: &'static str,
    total_ignored_tests: u64,
    violation_count: usize,
    inventory_expected_ignored: Option<u64>,
    inventory_count_matches_live_scan: Option<bool>,
    findings: Vec<IgnoredTestGovernanceFinding>,
}

pub(crate) fn run_ignored_test_age(
    root: &Path,
    mode: GovernanceGateMode,
    inventory: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let (total_ignored, findings) = scan_ignored_test_governance_findings(root)?;

    let mut inv_expected: Option<u64> = None;
    let mut inv_matches: Option<bool> = None;
    if let Some(inv_path) = inventory.as_ref() {
        let raw =
            fs::read_to_string(inv_path).with_context(|| format!("read {}", inv_path.display()))?;
        let inv: TestInventoryReport =
            serde_json::from_str(&raw).with_context(|| format!("parse {}", inv_path.display()))?;
        let expected = inv.summary.cargo_ignored_test_functions;
        inv_expected = Some(expected);
        let ok = expected == total_ignored;
        inv_matches = Some(ok);
        let msg = format!(
            "ignored-test-age: inventory `{}` expects {} ignored tests; live scan counted {}",
            inv_path.display(),
            expected,
            total_ignored
        );
        match mode {
            GovernanceGateMode::Warn => {
                if !ok {
                    eprintln!("warning: {msg}");
                }
            }
            GovernanceGateMode::Enforce => {
                if !ok {
                    anyhow::bail!(msg);
                }
            }
        }
    }

    if json {
        let payload = IgnoredTestAgeJson {
            schema_version: 1,
            mode: mode.label(),
            total_ignored_tests: total_ignored,
            violation_count: findings.len(),
            inventory_expected_ignored: inv_expected,
            inventory_count_matches_live_scan: inv_matches,
            findings: findings.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "ignored-test-age ({}): {} ignored test(s) scanned; {} governance violation(s)",
            mode.label(),
            total_ignored,
            findings.len()
        );
        for f in &findings {
            match &f.issue {
                IgnoredTestGovernanceIssue::BareIgnore => {
                    eprintln!(
                        "  {}:{} `{}`: bare #[ignore] (use #[ignore = \"…\"] with owner:/sunset:/date)",
                        f.path, f.line, f.test_function
                    );
                }
                IgnoredTestGovernanceIssue::MissingOwnershipMarker { reason } => {
                    eprintln!(
                        "  {}:{} `{}`: ignore reason lacks owner/sunset marker: {:?}",
                        f.path, f.line, f.test_function, reason
                    );
                }
            }
        }
    }

    if mode == GovernanceGateMode::Enforce && !findings.is_empty() {
        anyhow::bail!(
            "ignored-test-age: {} ignored test(s) missing governance markers (--mode enforce)",
            findings.len()
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct FlakeBudgetJson {
    schema_version: u32,
    mode: &'static str,
    max_candidates: usize,
    candidate_count: usize,
    exceeds_budget: bool,
}

pub(crate) fn run_flake_budget(
    root: &Path,
    mode: GovernanceGateMode,
    report_json: Option<PathBuf>,
    junit: Option<PathBuf>,
    top: usize,
    max_candidates: usize,
    json: bool,
) -> Result<()> {
    let _root = root;
    let report: TestRuntimeReport = match (&report_json, &junit) {
        (Some(p), None) => {
            let text = read_utf8_path_capped(p)
                .with_context(|| format!("read runtime report JSON {}", p.display()))?;
            parse_runtime_report_json(&text)?
        }
        (None, Some(p)) => {
            let xml =
                read_utf8_path_capped(p).with_context(|| format!("read JUnit {}", p.display()))?;
            super::test_runtime_report::build_report(&xml, top)?
        }
        (Some(_), Some(_)) => anyhow::bail!("pass only one of --report-json or --junit"),
        (None, None) => {
            anyhow::bail!("flake-budget requires --report-json <path> or --junit <path>")
        }
    };

    let n = retry_flaky_candidate_count(&report);
    let exceeds = n > max_candidates;

    if json {
        let payload = FlakeBudgetJson {
            schema_version: 1,
            mode: mode.label(),
            max_candidates,
            candidate_count: n,
            exceeds_budget: exceeds,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "flake-budget ({}): {} retry/flaky candidate row(s); budget {}",
            mode.label(),
            n,
            max_candidates
        );
        if exceeds && mode == GovernanceGateMode::Warn {
            eprintln!(
                "warning: flake-budget: candidate count {n} exceeds --max-candidates {max_candidates}"
            );
        }
    }

    if mode == GovernanceGateMode::Enforce && exceeds {
        anyhow::bail!(
            "flake-budget: {} candidate(s) > {} (--mode enforce)",
            n,
            max_candidates
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeRegressJson {
    schema_version: u32,
    mode: &'static str,
    percent_threshold: f64,
    absolute_ms_threshold: u64,
    regression_count: usize,
    missing_in_current_top_count: usize,
    regressions: super::test_runtime_report::RuntimeRegressReport,
}

pub(crate) fn run_runtime_regress(
    mode: GovernanceGateMode,
    current: PathBuf,
    baseline: PathBuf,
    percent: f64,
    absolute_ms: u64,
    json: bool,
) -> Result<()> {
    let cur_raw = read_utf8_path_capped(&current)
        .with_context(|| format!("read current {}", current.display()))?;
    let base_raw = read_utf8_path_capped(&baseline)
        .with_context(|| format!("read baseline {}", baseline.display()))?;
    let cur = parse_runtime_report_json(&cur_raw)
        .with_context(|| format!("parse current {}", current.display()))?;
    let base = parse_runtime_report_json(&base_raw)
        .with_context(|| format!("parse baseline {}", baseline.display()))?;

    let regressions = compare_runtime_reports(&base, &cur, percent, absolute_ms);
    let reg_n = regressions.regressions.len();
    let miss_n = regressions.missing_in_current_top.len();
    let has_issue = reg_n > 0 || miss_n > 0;

    if json {
        let payload = RuntimeRegressJson {
            schema_version: 1,
            mode: mode.label(),
            percent_threshold: percent,
            absolute_ms_threshold: absolute_ms,
            regression_count: reg_n,
            missing_in_current_top_count: miss_n,
            regressions: regressions.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "runtime-regress ({}): {} regression(s), {} baseline top test(s) missing from current top slice",
            mode.label(),
            reg_n,
            miss_n
        );
        for r in &regressions.regressions {
            println!(
                "  {}::{}  {:.4}s -> {:.4}s  (+{:.4}s, {:.1}% vs baseline)",
                r.classname.as_deref().unwrap_or(""),
                r.name,
                r.baseline_secs,
                r.current_secs,
                r.delta_secs,
                r.percent_over_baseline
            );
        }
        for m in &regressions.missing_in_current_top {
            println!(
                "  (missing in current top) {}::{}",
                m.classname.as_deref().unwrap_or(""),
                m.name
            );
        }
    }

    if has_issue {
        let msg =
            format!("runtime-regress: {reg_n} regression(s), {miss_n} missing-in-current-top");
        match mode {
            GovernanceGateMode::Warn => eprintln!("warning: {msg}"),
            GovernanceGateMode::Enforce => anyhow::bail!(msg),
        }
    }

    Ok(())
}
