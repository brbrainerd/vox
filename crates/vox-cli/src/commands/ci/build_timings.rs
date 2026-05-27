//! `vox ci build-timings` — rich per-crate build telemetry persisted to Arca.
//!
//! Data collected per run:
//!   - Wall-clock windows between `compiler-artifact` events → elapsed_ms per crate
//!   - `compiler-message` warnings (code, message, crate)
//!   - `cargo metadata` dep-graph fingerprint (SHA-256 of sorted `name@version` strings)
//!   - Total run duration
//!
//! All records land in the V34 `build_run / build_crate_sample / build_warning` tables.
//!
//! **`--deep` persistence:** Arca tables are written when VoxDB connects (same as before). The legacy
//! `research_metrics` fallback (`benchmark_event` / `cargo_build_metrics`) runs **only** when
//! **`VOX_BENCHMARK_TELEMETRY=1`** (see [`crate::benchmark_telemetry::record_opt_with_unit`]) so benchmark
//! rows stay opt-in. Trust SSOT: `docs/src/architecture/telemetry-trust-ssot.md`.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── Internal structs ─────────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize)]
struct BuildSummary {
    pub total_ms: i64,
    pub run_name: Option<String>,
    pub rustc_version: Option<String>,
    pub dep_fingerprint: Option<String>,
    pub profile: String,
    pub crates: Vec<CrateRecord>,
    pub warnings: Vec<WarningRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CrateRecord {
    pub name: String,
    pub version: Option<String>,
    pub elapsed_ms: Option<i64>,
    pub fresh: bool,
    pub features: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
struct WarningRecord {
    pub crate_name: String,
    pub level: String,
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DependencyShapeSummary {
    workspace_crate_count: usize,
    workspace_edge_count: usize,
    max_fan_in: usize,
    max_fan_out: usize,
    top_fan_in: Vec<(String, usize)>,
    top_fan_out: Vec<(String, usize)>,
}

// ── Dep-graph fingerprint ────────────────────────────────────────────────────

fn dep_graph_fingerprint() -> Option<String> {
    // Fix 7: remove --no-deps to catch third-party version changes
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let packages = json["packages"].as_array()?;
    let mut entries: Vec<String> = packages
        .iter()
        .filter_map(|p| {
            let name = p["name"].as_str()?;
            let ver = p["version"].as_str()?;
            Some(format!("{name}@{ver}"))
        })
        .collect();
    entries.sort();

    // Fix 2: Deterministic FNV-1a hash instead of unstable DefaultHasher
    let mut h: u64 = 0xcbf29ce484222325;
    for e in &entries {
        for b in e.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x00000100000001b3);
        }
        h ^= b'\n' as u64;
        h = h.wrapping_mul(0x00000100000001b3);
    }
    Some(format!("{h:016x}"))
}

fn compute_dependency_shape_summary() -> Option<DependencyShapeSummary> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let packages = json["packages"].as_array()?;
    let workspace_members = json["workspace_members"].as_array()?;

    let mut workspace_ids = std::collections::HashSet::new();
    for id in workspace_members {
        if let Some(s) = id.as_str() {
            workspace_ids.insert(s.to_string());
        }
    }

    let mut id_to_name = std::collections::HashMap::new();
    for p in packages {
        let id = p["id"].as_str()?;
        let name = p["name"].as_str()?;
        id_to_name.insert(id.to_string(), name.to_string());
    }

    let mut fan_out = std::collections::HashMap::<String, usize>::new();
    let mut fan_in = std::collections::HashMap::<String, usize>::new();
    let mut edge_count = 0usize;

    for p in packages {
        let Some(pkg_id) = p["id"].as_str() else {
            continue;
        };
        if !workspace_ids.contains(pkg_id) {
            continue;
        }
        let Some(name) = id_to_name.get(pkg_id).cloned() else {
            continue;
        };
        let Some(deps) = p["dependencies"].as_array() else {
            continue;
        };

        let mut out_count = 0usize;
        for dep in deps {
            let Some(dep_name) = dep["name"].as_str() else {
                continue;
            };
            // Only count internal workspace dependency edges for crate-topology decisions.
            let is_workspace_dep = id_to_name.values().any(|n| n == dep_name)
                && packages.iter().any(|pkg| {
                    pkg["name"].as_str() == Some(dep_name)
                        && pkg["id"]
                            .as_str()
                            .map(|id| workspace_ids.contains(id))
                            .unwrap_or(false)
                });
            if is_workspace_dep {
                out_count += 1;
                edge_count += 1;
                *fan_in.entry(dep_name.to_string()).or_insert(0) += 1;
            }
        }
        fan_out.insert(name, out_count);
    }

    for name in fan_out.keys() {
        fan_in.entry(name.clone()).or_insert(0);
    }

    let mut top_in: Vec<(String, usize)> = fan_in.into_iter().collect();
    top_in.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_in.truncate(5);

    let mut top_out: Vec<(String, usize)> = fan_out.into_iter().collect();
    top_out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_out.truncate(5);

    Some(DependencyShapeSummary {
        workspace_crate_count: workspace_ids.len(),
        workspace_edge_count: edge_count,
        max_fan_in: top_in.first().map(|(_, v)| *v).unwrap_or(0),
        max_fan_out: top_out.first().map(|(_, v)| *v).unwrap_or(0),
        top_fan_in: top_in,
        top_fan_out: top_out,
    })
}

// ── Rustc version ────────────────────────────────────────────────────────────

fn rustc_version() -> Option<String> {
    let out = Command::new("rustc").args(["--version"]).output().ok()?;
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

// ── Main collector ───────────────────────────────────────────────────────────

/// Run `cargo build --message-format=json` on the given package (or the whole workspace),
/// collect per-crate timings and warnings, persist to Arca, and print a concise summary.
pub async fn bench_build_run(
    persist: bool,
    run_name: Option<String>,
    profile: Option<String>,
) -> Result<()> {
    let target_pkg = "vox-corpus"; // never self — avoids Access Denied on Windows
    let profile = profile.unwrap_or_else(|| "dev".to_string());

    let mut args = vec!["build", "--message-format=json", "-p", target_pkg];
    if profile == "release" {
        args.push("--release");
    }

    println!(
        "vox ci build-timings → cargo build {} -p {target_pkg} --message-format=json",
        if profile == "release" {
            "--release"
        } else {
            ""
        }
    );

    let dep_fp = dep_graph_fingerprint();
    let dep_shape = compute_dependency_shape_summary();
    let rustc_ver = rustc_version();
    let start = Instant::now();

    let output = Command::new("cargo")
        .args(&args)
        .output()
        .context("failed to spawn cargo build")?;

    let total_ms = start.elapsed().as_millis() as i64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Phase D: emit failure summary so the telemetry stream has a complete record.
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            let build_id = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|_| "0".into());
            vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::BuildSummary(
                vox_telemetry::BuildSummaryEvent {
                    build_id,
                    outcome: "failure".into(),
                    wall_time_ms: total_ms as u64,
                    crates_compiled: 0,
                    error_count: 1,
                    invocation_context: Some("local".into()),
                }
            ));
        }
        anyhow::bail!("cargo build failed:\n{stderr}");
    }

    let mut summary = BuildSummary {
        total_ms,
        run_name: run_name.clone(),
        rustc_version: rustc_ver.clone(),
        dep_fingerprint: dep_fp.clone(),
        profile: profile.clone(),
        ..Default::default()
    };

    // Track artifact timestamps for elapsed_ms windows
    let mut last_ts = start;
    let mut raw_warnings = Vec::new();
    let mut parsed_lines = 0usize;
    let mut skipped_lines = 0usize;

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        parsed_lines += 1;
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
            skipped_lines += 1;
            continue;
        };
        match val["reason"].as_str() {
            Some("compiler-artifact") => {
                let pkg_id = val["package_id"].as_str().unwrap_or("");
                let name = pkg_id
                    .split_whitespace()
                    .next()
                    .unwrap_or(pkg_id)
                    .to_string();
                let version = pkg_id.split_whitespace().nth(1).map(|v| v.to_string());
                let fresh = val["fresh"].as_bool().unwrap_or(false);
                let features = val["features"].as_array().map(|a| {
                    a.iter()
                        .filter_map(|f| f.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                        .join(",")
                });
                let now = Instant::now();
                let elapsed_ms = if fresh {
                    None
                } else {
                    Some(now.duration_since(last_ts).as_millis() as i64)
                };
                last_ts = now;
                summary.crates.push(CrateRecord {
                    name,
                    version,
                    elapsed_ms,
                    fresh,
                    features,
                });
            }
            Some("compiler-message") => {
                let msg = &val["message"];
                if let Some(level) = msg["level"].as_str() {
                    if level == "warning" || level == "error" {
                        let code = msg["code"]
                            .as_object()
                            .and_then(|c| c.get("code"))
                            .and_then(|c| c.as_str())
                            .map(str::to_string);
                        let message = msg["message"].as_str().unwrap_or("").to_string();

                        // Fix 13: robust crate extraction using target name if path guess fails
                        let mut crate_name = val["target"]["name"].as_str().map(|s| s.to_string());

                        if crate_name.is_none() {
                            crate_name = msg["spans"]
                                .as_array()
                                .and_then(|s| s.first())
                                .and_then(|s| s["file_name"].as_str())
                                .map(|f| {
                                    let p = Path::new(f);
                                    p.components()
                                        .find_map(|c| {
                                            c.as_os_str().to_str().filter(|s| s.starts_with("vox-"))
                                        })
                                        .unwrap_or("unknown")
                                        .to_string()
                                });
                        }

                        raw_warnings.push(WarningRecord {
                            crate_name: crate_name.unwrap_or_else(|| "unknown".into()),
                            level: level.to_string(),
                            code,
                            message,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Fix 11: Deduplicate warnings within the same run
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for w in raw_warnings {
        if seen.insert(w.clone()) {
            summary.warnings.push(w);
        }
    }

    let crate_count = summary.crates.len() as i64;
    let fresh_count = summary.crates.iter().filter(|c| c.fresh).count() as i64;
    tracing::debug!(
        target: "vox.ci.build_timings",
        parsed_lines,
        skipped_lines,
        crate_count,
        warning_count = summary.warnings.len(),
        dep_fingerprint = dep_fp.as_deref().unwrap_or(""),
        "parsed cargo --message-format=json stream"
    );

    // ── Print concise summary ───────────────────────────────────────────────
    println!(
        "✓ Build complete ({}) — {:.2}s  ({} compiled, {} cached, {} warnings)",
        profile,
        total_ms as f64 / 1000.0,
        crate_count - fresh_count,
        fresh_count,
        summary.warnings.len()
    );

    // Slowest 5 non-fresh crates
    let mut compiled: Vec<_> = summary
        .crates
        .iter()
        .filter(|c| !c.fresh && c.elapsed_ms.is_some())
        .collect();
    compiled.sort_by_key(|c| std::cmp::Reverse(c.elapsed_ms.unwrap_or(0)));
    if !compiled.is_empty() {
        println!("Slowest crates:");
        for c in compiled.iter().take(5) {
            println!("  {:40} {:>6}ms", c.name, c.elapsed_ms.unwrap_or(0));
        }
    }

    // ── Emit build summary telemetry ─────────────────────────────────────────
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let build_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis().to_string())
            .unwrap_or_else(|_| "0".into());
        vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::BuildSummary(
            vox_telemetry::BuildSummaryEvent {
                build_id,
                outcome: "success".into(),
                wall_time_ms: total_ms as u64,
                crates_compiled: (crate_count - fresh_count).max(0) as u32,
                error_count: 0,
                invocation_context: Some("local".into()),
            }
        ));
    }

    // ── Persist ─────────────────────────────────────────────────────────────
    if persist {
        // Fix 4: Resolve real repository_id instead of hardcoded "local"
        let repo =
            vox_repository::discover_repository(&std::env::current_dir().unwrap_or_default());
        let repo_id = repo
            .as_ref()
            .map(|r| r.repository_id.clone())
            .unwrap_or_else(|_| "local".into());

        // Try new Arca tables first
        let persisted = try_persist_to_arca(
            &repo_id,
            &summary,
            crate_count,
            fresh_count,
            &rustc_ver,
            &dep_fp,
            dep_shape.as_ref(),
            &run_name,
        )
        .await;

        if persisted {
            println!("Recorded metrics to VoxDB (V34 build tables).");
        } else {
            let details = serde_json::to_value(&summary).unwrap_or_default();
            let dep_shape_json = dep_shape
                .as_ref()
                .and_then(|d| serde_json::to_value(d).ok())
                .unwrap_or_else(|| serde_json::json!({}));
            crate::benchmark_telemetry::record_opt_with_unit(
                "cargo_build_metrics",
                Some(total_ms as f64 / 1000.0),
                Some("seconds"),
                Some(serde_json::json!({
                    "build_summary": details,
                    "dependency_shape": dep_shape_json,
                })),
            )
            .await;
            let benchmark_telemetry_resolved =
                vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBenchmarkTelemetry);
            if benchmark_telemetry_resolved
                .expose()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            {
                println!(
                    "Recorded fallback metrics to research_metrics (VOX_BENCHMARK_TELEMETRY)."
                );
            } else {
                println!(
                    "Skipping research_metrics fallback (set VOX_BENCHMARK_TELEMETRY=1 to record benchmark_event)."
                );
            }
        }
    }

    Ok(())
}

async fn try_persist_to_arca(
    repo_id: &str,
    summary: &BuildSummary,
    crate_count: i64,
    fresh_count: i64,
    rustc_ver: &Option<String>,
    dep_fp: &Option<String>,
    dep_shape: Option<&DependencyShapeSummary>,
    run_name: &Option<String>,
) -> bool {
    use vox_db::{DbConfig, VoxDb};

    let config = match DbConfig::resolve_canonical() {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(
                target: "vox.ci.build_timings",
                error = %e,
                "unable to resolve canonical VoxDB config for build telemetry"
            );
            return false;
        }
    };
    let Ok(db) = VoxDb::connect(config).await else {
        tracing::debug!(
            target: "vox.ci.build_timings",
            "VoxDb::connect failed while persisting build telemetry"
        );
        return false;
    };

    let run_id = match db
        .insert_build_run(
            repo_id,
            run_name.as_deref(),
            rustc_ver.as_deref(),
            &summary.profile,
            summary.total_ms,
            crate_count,
            fresh_count,
            dep_fp.as_deref(),
        )
        .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::debug!(
                target: "vox.ci.build_timings",
                error = %e,
                "insert_build_run failed"
            );
            return false;
        }
    };

    // Samples
    let samples: Vec<_> = summary
        .crates
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.version.as_deref(),
                c.elapsed_ms,
                c.fresh,
                c.features.as_deref(),
            )
        })
        .collect();
    if let Err(e) = db.insert_crate_samples(run_id, &samples).await {
        tracing::debug!(
            target: "vox.ci.build_timings",
            error = %e,
            run_id,
            "insert_crate_samples reported an error"
        );
    }

    // Warnings
    let warns: Vec<_> = summary
        .warnings
        .iter()
        .map(|w| {
            (
                w.crate_name.as_str(),
                w.level.as_str(),
                w.code.as_deref(),
                w.message.as_str(),
            )
        })
        .collect();
    if let Err(e) = db.insert_build_warnings(run_id, &warns).await {
        tracing::debug!(
            target: "vox.ci.build_timings",
            error = %e,
            run_id,
            "insert_build_warnings reported an error"
        );
    }

    if let Some(shape) = dep_shape {
        let Ok(shape_json) = serde_json::to_value(shape) else {
            tracing::debug!(
                target: "vox.ci.build_timings",
                run_id,
                "dependency shape serialization failed"
            );
            return true;
        };
        if let Err(e) = db.insert_build_dependency_shape(run_id, &shape_json).await {
            tracing::debug!(
                target: "vox.ci.build_timings",
                error = %e,
                run_id,
                "insert_build_dependency_shape reported an error"
            );
        }
    }

    true
}
