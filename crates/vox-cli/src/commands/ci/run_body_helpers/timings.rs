use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::commands::ci::{cargo_bin, nvcc_available};
use vox_bounded_fs::read_utf8_path_capped;

#[derive(serde::Serialize)]
struct TimingRecord {
    lane: &'static str,
    ok: bool,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Single source of truth: `docs/ci/build-timings/budgets.json` (see also crate-build-lanes-migration.md).
#[derive(Debug, Deserialize)]
struct BuildTimingBudgetsFile {
    lanes: std::collections::HashMap<String, u64>,
}

fn load_build_timing_budgets(root: &Path) -> Result<std::collections::HashMap<String, u128>> {
    let p = root.join("docs/ci/build-timings/budgets.json");
    let raw = read_utf8_path_capped(&p)
        .with_context(|| format!("read build timing budgets {}", p.display()))?;
    let parsed: BuildTimingBudgetsFile =
        serde_json::from_str(&raw).context("parse docs/ci/build-timings/budgets.json")?;
    Ok(parsed
        .lanes
        .into_iter()
        .map(|(k, v)| (k, u128::from(v)))
        .collect())
}

/// Soft budgets from `budgets.json`. `VOX_BUILD_TIMINGS_BUDGET_WARN=1` stderr for missing lane keys
/// and over-budget lanes. `VOX_BUILD_TIMINGS_BUDGET_FAIL=1` fails if any lane exceeded its cap (warn not required).
fn apply_build_timing_budgets(records: &[TimingRecord], root: &Path) -> Result<()> {
    let warn_resolved =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBuildTimingsBudgetWarn);
    let warn = warn_resolved.expose().unwrap_or_default() == "1";
    let fail_resolved =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBuildTimingsBudgetFail);
    let fail = fail_resolved.expose().unwrap_or_default() == "1";
    if !warn && !fail {
        return Ok(());
    }
    let budgets = load_build_timing_budgets(root)?;
    let mut any_over = false;
    for r in records {
        if !r.ok {
            continue;
        }
        match budgets.get(r.lane) {
            None => {
                if warn {
                    eprintln!(
                        "build-timings budget: lane {} has no entry in docs/ci/build-timings/budgets.json",
                        r.lane
                    );
                }
            }
            Some(max_ms) => {
                if r.duration_ms > *max_ms {
                    any_over = true;
                    if warn || fail {
                        eprintln!(
                            "build-timings budget: lane {} took {} ms (soft upper {} ms from budgets.json)",
                            r.lane, r.duration_ms, max_ms
                        );
                    }
                }
            }
        }
    }
    if any_over && fail {
        return Err(anyhow!(
            "one or more lanes exceeded soft budget (VOX_BUILD_TIMINGS_BUDGET_FAIL=1)"
        ));
    }
    Ok(())
}

fn run_cargo_lane(cargo: &Path, root: &Path, lane: &'static str, args: &[&str]) -> TimingRecord {
    let start = Instant::now();
    let st = Command::new(cargo).current_dir(root).args(args).status();
    let duration_ms = start.elapsed().as_millis();
    match st {
        Ok(s) if s.success() => TimingRecord {
            lane,
            ok: true,
            duration_ms,
            error: None,
        },
        Ok(_) => TimingRecord {
            lane,
            ok: false,
            duration_ms,
            error: Some("non-zero exit".into()),
        },
        Err(e) => TimingRecord {
            lane,
            ok: false,
            duration_ms,
            error: Some(e.to_string()),
        },
    }
}

pub(crate) fn run_build_timings(root: &Path, json: bool, crates: bool) -> Result<()> {
    let cargo = cargo_bin();
    let mut records: Vec<TimingRecord> = Vec::new();

    let lanes: &[(&str, &[&str])] = &[
        ("check_vox_cli_default", &["check", "-p", "vox-cli"]),
        (
            "check_vox_cli_gpu_stub",
            &["check", "-p", "vox-ml-cli", "--features", "gpu,mens-qlora"],
        ),
    ];

    for (lane, args) in lanes {
        records.push(run_cargo_lane(&cargo, root, lane, args));
    }

    if crates {
        let crate_lanes: &[(&str, &[&str])] = &[
            (
                "check_vox_cli_no_default_features",
                &["check", "-p", "vox-cli", "--no-default-features"],
            ),
            ("check_vox_db", &["check", "-p", "vox-db"]),
            ("check_vox_oratio", &["check", "-p", "vox-speech"]),
            (
                "check_vox_mens_train",
                &["check", "-p", "vox-populi", "--features", "mens,mens-train"],
            ),
            (
                "check_vox_cli_populi_oratio",
                &["check", "-p", "vox-cli", "--features", "oratio"],
            ),
            ("check_vox_mcp", &["check", "-p", "vox-mcp"]),
        ];
        for (lane, args) in crate_lanes {
            records.push(run_cargo_lane(&cargo, root, lane, args));
        }
    }

    // Optional CUDA lane (same policy as `cuda-features`).
    let skip_cuda_resolved =
        vox_secrets::resolve_secret(vox_secrets::SecretId::SkipCudaFeatureCheck);
    if skip_cuda_resolved.expose().unwrap_or_default() != "1" {
        let nvcc_ok = nvcc_available();
        if nvcc_ok {
            records.push(run_cargo_lane(
                &cargo,
                root,
                "check_vox_cli_gpu_populi_candle_cuda",
                &[
                    "check",
                    "-p",
                    "vox-ml-cli",
                    "--features",
                    "gpu,mens-candle-cuda",
                ],
            ));
        }
    }

    if json {
        for r in &records {
            let line = serde_json::to_string(r).with_context(|| {
                format!(
                    "serialize build-timings JSON for lane {} (TimingRecord)",
                    r.lane
                )
            })?;
            println!("{line}");
        }
    } else {
        println!(
            "vox ci build-timings (wall-clock cargo check){}",
            if crates { " + per-crate lanes" } else { "" }
        );
        for r in &records {
            println!(
                "  {:<40} {}  {} ms",
                r.lane,
                if r.ok { "ok" } else { "FAIL" },
                r.duration_ms
            );
            if let Some(ref e) = r.error {
                println!("    ({e})");
            }
        }
        let skip_cuda_resolved =
            vox_secrets::resolve_secret(vox_secrets::SecretId::SkipCudaFeatureCheck);
        if skip_cuda_resolved.expose().unwrap_or_default() == "1" {
            println!("  (CUDA lane skipped: SKIP_CUDA_FEATURE_CHECK=1)");
        } else if !records
            .iter()
            .any(|r| r.lane == "check_vox_cli_gpu_populi_candle_cuda")
        {
            println!("  (CUDA lane skipped: nvcc not on PATH)");
        }
    }

    if records.iter().any(|r| !r.ok) {
        return Err(anyhow!("one or more build-timings lanes failed"));
    }
    apply_build_timing_budgets(&records, root)?;
    let total_ms: u128 = records.iter().filter(|r| r.ok).map(|r| r.duration_ms).sum();
    let total_seconds = (total_ms as f64) / 1000.0;
    let details = serde_json::json!({
        "crates": crates,
        "total_ms": total_ms,
        "lanes": records.iter().map(|r| {
            serde_json::json!({"lane": r.lane, "ok": r.ok, "ms": r.duration_ms})
        }).collect::<Vec<_>>(),
    });
    tracing::debug!(
        target: "vox.ci.build_timings",
        lane_count = records.len(),
        total_ms = total_ms,
        "recording ci_build_timings benchmark_event"
    );
    crate::benchmark_telemetry::record_opt_with_unit_blocking(
        "ci_build_timings",
        Some(total_seconds),
        Some("seconds"),
        Some(details),
    );
    Ok(())
}

#[cfg(test)]
mod build_timing_budget_tests {
    use std::path::PathBuf;

    use super::load_build_timing_budgets;

    /// Keep in sync with `run_build_timings` lane ids + `docs/ci/build-timings/budgets.json`.
    const EXPECTED_BUDGET_LANES: &[&str] = &[
        "check_vox_cli_default",
        "check_vox_cli_gpu_stub",
        "check_vox_cli_gpu_populi_candle_cuda",
        "check_vox_cli_no_default_features",
        "check_vox_db",
        "check_vox_oratio",
        "check_vox_mens_train",
        "check_vox_cli_populi_oratio",
        "check_vox_mcp",
    ];

    #[test]
    fn budgets_json_loads_and_defines_all_timing_lanes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let m = load_build_timing_budgets(&root).expect("load budgets.json");
        for lane in EXPECTED_BUDGET_LANES {
            assert!(
                m.contains_key(*lane),
                "docs/ci/build-timings/budgets.json missing lane `{lane}`"
            );
        }
    }
}
