//! Gate evaluation against run artifacts.

use anyhow::Result;
use std::path::Path;

use super::io::{read_jsonl_nonempty_lines, read_utf8_path_capped};
use super::policy::{load_policy, mcp_tool_schema_metrics_path};

/// Gate check result.
#[derive(Debug)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
    pub block: bool,
}

/// Run gate checks against a run directory.
pub fn check_run(run_dir: &Path, policy_path: &Path) -> Result<Vec<GateResult>> {
    let policy = load_policy(policy_path)?;
    let mut results = Vec::new();

    // Trainer writes `training_manifest.json`; `manifest.json` is legacy/hand-written.
    // Prefer the canonical trainer output; fall back to the legacy name for older runs.
    let manifest_path = {
        let canonical = run_dir.join("training_manifest.json");
        let legacy = run_dir.join("manifest.json");
        if canonical.exists() {
            canonical
        } else {
            legacy
        }
    };
    let manifest: Option<serde_json::Value> = if manifest_path.exists() {
        let content = read_utf8_path_capped(&manifest_path)?;
        serde_json::from_str(&content).ok()
    } else {
        None
    };

    // Trainer writes `telemetry.jsonl`; `metrics.jsonl` is the legacy alias.
    // Mirror the fallback used in `vox mens status` (status.rs).
    let metrics_path = {
        let legacy = run_dir.join("metrics.jsonl");
        let canonical = run_dir.join("telemetry.jsonl");
        if legacy.exists() { legacy } else { canonical }
    };
    let metrics_lines: Vec<String> = if metrics_path.exists() {
        read_jsonl_nonempty_lines(&metrics_path)?
    } else {
        Vec::new()
    };

    let avg_tokens_per_sec = if metrics_lines.is_empty() {
        None
    } else {
        let mut sum = 0.0;
        let mut count = 0;
        let mut used_alias = false;
        for line in &metrics_lines {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let t = v
                    .get("tokens_per_sec")
                    .and_then(|x| x.as_f64())
                    .or_else(|| {
                        used_alias = true;
                        v.get("steps_per_sec_ema").and_then(|x| x.as_f64())
                    });
                if let Some(tps) = t {
                    sum += tps;
                    count += 1;
                }
            }
        }
        if used_alias {
            eprintln!(
                "warning: eval-gate consumed deprecated throughput alias `steps_per_sec_ema`; emit canonical `tokens_per_sec`"
            );
        }
        if count > 0 {
            Some(sum / count as f64)
        } else {
            None
        }
    };

    let device_profile = manifest
        .as_ref()
        .and_then(|m| m.get("device_profile"))
        .and_then(|v| v.as_str())
        .unwrap_or("cpu");

    if (policy.supervised_ratio.block || policy.supervised_ratio.min_pct > 0.0)
        && !metrics_lines.is_empty()
    {
        let sup_values: Vec<f64> = metrics_lines
            .iter()
            .filter_map(|l| {
                serde_json::from_str::<serde_json::Value>(l)
                    .ok()
                    .and_then(|v| {
                        v.get("supervised_ratio_pct")
                            .and_then(|x| x.as_f64())
                            .or_else(|| {
                                let valid = v.get("valid_tokens").and_then(|x| x.as_u64())?;
                                let theoretical =
                                    v.get("theoretical_tokens").and_then(|x| x.as_u64())?;
                                if theoretical == 0 {
                                    Some(0.0)
                                } else {
                                    Some((valid as f64 / theoretical as f64) * 100.0)
                                }
                            })
                    })
            })
            .collect();
        if !sup_values.is_empty() {
            let sup_pct = sup_values.iter().sum::<f64>() / sup_values.len() as f64;
            let passed = sup_pct >= policy.supervised_ratio.min_pct;
            results.push(GateResult {
                name: "supervised_ratio".to_string(),
                passed,
                message: format!(
                    "supervised_ratio {:.1}% {} {:.1}%",
                    sup_pct,
                    if passed { ">=" } else { "<" },
                    policy.supervised_ratio.min_pct
                ),
                block: policy.supervised_ratio.block,
            });
        }
    }

    fn normalize_device_profile(profile: &str) -> String {
        let lower = profile.to_lowercase();
        if lower.contains("cpu") {
            "cpu".to_string()
        } else if lower.contains("4080") {
            "rtx_4080_super".to_string()
        } else if lower.contains("4090") {
            "rtx_4090".to_string()
        } else {
            lower.replace([' ', '-'], "_")
        }
    }

    let device_profile_normalized = normalize_device_profile(device_profile);

    if (policy.truncation.block || policy.truncation.max_fraction < 1.0)
        && manifest
            .as_ref()
            .is_some_and(|m| m.get("truncation_fraction").is_some())
    {
        let frac = manifest
            .as_ref()
            .and_then(|m| m.get("truncation_fraction").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        let passed = frac <= policy.truncation.max_fraction;
        results.push(GateResult {
            name: "truncation".to_string(),
            passed,
            message: format!(
                "truncation_fraction {:.2} {} {:.2}",
                frac,
                if passed { "<=" } else { ">" },
                policy.truncation.max_fraction
            ),
            block: policy.truncation.block,
        });
    }

    if let Some(tps) = avg_tokens_per_sec {
        let min_tps = policy
            .throughput
            .min_tokens_per_sec
            .get(&device_profile_normalized)
            .copied()
            .or_else(|| policy.throughput.min_tokens_per_sec.get("cpu").copied())
            .unwrap_or(0.0);
        let passed = tps >= min_tps;
        if min_tps > 0.0 {
            results.push(GateResult {
                name: "throughput".to_string(),
                passed,
                message: format!(
                    "throughput {:.0} tok/s {} {:.0} (profile={})",
                    tps,
                    if passed { ">=" } else { "<" },
                    min_tps,
                    device_profile_normalized
                ),
                block: policy.throughput.block,
            });
        }
    }

    let eval_results_path = run_dir.join("eval_results.json");
    // Parse eval_results.json once and share it across all gates that need it (B2).
    let eval_json: Option<serde_json::Value> = if eval_results_path.exists() {
        let content = read_utf8_path_capped(&eval_results_path)?;
        serde_json::from_str(&content).ok()
    } else {
        None
    };

    if (policy.eval_local.block || policy.eval_local.min_parse_rate > 0.0)
        && let Some(ref eval) = eval_json
    {
        let parse_rate = eval
            .get("vox_parse_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        // construct_coverage_pct is stored as 0–100 in eval_results.json (e.g. 35.0)
        // but min_coverage_pct in the YAML is 0.0–1.0 (e.g. 0.35). Normalise (Q4).
        let coverage_pct_json = eval
            .get("construct_coverage_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let coverage = coverage_pct_json / 100.0;
        let passed = parse_rate >= policy.eval_local.min_parse_rate
            && coverage >= policy.eval_local.min_coverage_pct;
        results.push(GateResult {
            name: "eval_local".to_string(),
            passed,
            message: format!(
                "parse_rate={:.2} coverage={:.1}% (min={:.2} / {:.0}%)",
                parse_rate,
                coverage_pct_json,
                policy.eval_local.min_parse_rate,
                policy.eval_local.min_coverage_pct * 100.0,
            ),
            block: policy.eval_local.block,
        });
    }

    if policy.rust_compile_rate.block || policy.rust_compile_rate.min_pct > 0.0 {
        // Absent key = no rust rows in this corpus → gate not applicable (pass, don't fail).
        // This avoids a blocking rust gate misreading a missing metric as 0%.
        let (passed, msg) = match eval_json.as_ref().and_then(|e| e.get("rust_compile_rate")).and_then(|v| v.as_f64()) {
            Some(rate) => {
                let ok = rate >= policy.rust_compile_rate.min_pct;
                (ok, format!("rust_compile_rate={:.2} (min={:.2})", rate, policy.rust_compile_rate.min_pct))
            }
            None => (true, "rust_compile_rate not applicable (no rust_authoring rows)".to_string()),
        };
        results.push(GateResult {
            name: "rust_compile_rate".to_string(),
            passed,
            message: msg,
            block: policy.rust_compile_rate.block,
        });
    }

    if policy.clippy_clean_rate.block || policy.clippy_clean_rate.min_pct > 0.0 {
        let (passed, msg) = match eval_json.as_ref().and_then(|e| e.get("clippy_clean_rate")).and_then(|v| v.as_f64()) {
            Some(rate) => {
                let ok = rate >= policy.clippy_clean_rate.min_pct;
                (ok, format!("clippy_clean_rate={:.2} (min={:.2})", rate, policy.clippy_clean_rate.min_pct))
            }
            None => (true, "clippy_clean_rate not applicable (no rust_authoring rows)".to_string()),
        };
        results.push(GateResult {
            name: "clippy_clean_rate".to_string(),
            passed,
            message: msg,
            block: policy.clippy_clean_rate.block,
        });
    }

    // Agentic spoke gates — read from eval_results.json (same file, different keys).
    if policy.tool_call_valid_json_rate.block || policy.tool_call_valid_json_rate.min_pct > 0.0 {
        let (passed, msg) = match &eval_json {
            Some(eval) => {
                let rate = eval
                    .get("tool_call_valid_json_rate")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let ok = rate >= policy.tool_call_valid_json_rate.min_pct;
                (ok, format!("tool_call_valid_json_rate={:.2} (min={:.2})", rate, policy.tool_call_valid_json_rate.min_pct))
            }
            None => (false, "eval_results.json not found — run `vox mens eval` first".to_string()),
        };
        results.push(GateResult {
            name: "tool_call_valid_json_rate".to_string(),
            passed,
            message: msg,
            block: policy.tool_call_valid_json_rate.block,
        });
    }

    if policy.tool_name_exists_rate.block || policy.tool_name_exists_rate.min_pct > 0.0 {
        let (passed, msg) = match &eval_json {
            Some(eval) => {
                let rate = eval
                    .get("tool_name_exists_rate")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let ok = rate >= policy.tool_name_exists_rate.min_pct;
                (ok, format!("tool_name_exists_rate={:.2} (min={:.2})", rate, policy.tool_name_exists_rate.min_pct))
            }
            None => (false, "eval_results.json not found — run `vox mens eval` first".to_string()),
        };
        results.push(GateResult {
            name: "tool_name_exists_rate".to_string(),
            passed,
            message: msg,
            block: policy.tool_name_exists_rate.block,
        });
    }

    // anti_stub metrics (anti_stub_task_success, placeholder_event_rate,
    // trivial_placeholder_event_rate, construct_richness_mean) are written by
    // `vox mens eval-local` into eval_local_report.json — NOT by the corpus
    // evaluator (eval_results.json) or the QLoRA trainer.  Read from the
    // canonical eval-local output file so the gate can actually be satisfied.
    // Mirrors the pass_at_k pattern: missing file → failing GateResult whose
    // block flag is taken from the policy (block: true in full policy, false in
    // the post-train bootstrap policy).
    {
        let anti_stub_cfg = &policy.anti_stub;
        let anti_stub_active = anti_stub_cfg.block
            || anti_stub_cfg.min_pass_rate > 0.0
            || anti_stub_cfg.max_placeholder_event_rate > 0.0
            || anti_stub_cfg.max_trivial_placeholder_event_rate > 0.0
            || anti_stub_cfg.min_construct_richness_mean > 0.0;
        if anti_stub_active {
            let eval_local_report_path = run_dir.join("eval_local_report.json");
            if !eval_local_report_path.exists() {
                results.push(GateResult {
                    name: "anti_stub".to_string(),
                    passed: false,
                    message: format!(
                        "eval_local_report.json missing — run `vox mens eval-local \
                         --model {}/candle_qlora_adapter.safetensors \
                         --bench mens/data/heldout_bench \
                         -o {}/eval_local_report.json` first",
                        run_dir.display(),
                        run_dir.display()
                    ),
                    block: anti_stub_cfg.block,
                });
            } else {
                let content = read_utf8_path_capped(&eval_local_report_path)?;
                if let Ok(el) = serde_json::from_str::<serde_json::Value>(&content) {
                    let anti_stub_pass = el
                        .get("anti_stub_task_success")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let placeholder_rate = el
                        .get("placeholder_event_rate")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let trivial_placeholder_rate = el
                        .get("trivial_placeholder_event_rate")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let construct_richness_mean = el
                        .get("construct_richness_mean")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let pass_rate_ok = anti_stub_pass >= anti_stub_cfg.min_pass_rate;
                    let placeholder_ok = anti_stub_cfg.max_placeholder_event_rate <= 0.0
                        || placeholder_rate <= anti_stub_cfg.max_placeholder_event_rate;
                    let trivial_ok = anti_stub_cfg.max_trivial_placeholder_event_rate <= 0.0
                        || trivial_placeholder_rate
                            <= anti_stub_cfg.max_trivial_placeholder_event_rate;
                    let richness_ok = anti_stub_cfg.min_construct_richness_mean <= 0.0
                        || construct_richness_mean >= anti_stub_cfg.min_construct_richness_mean;
                    let passed = pass_rate_ok && placeholder_ok && trivial_ok && richness_ok;
                    results.push(GateResult {
                        name: "anti_stub".to_string(),
                        passed,
                        message: format!(
                            "anti_stub_pass={:.3} (min={:.3}) placeholder_rate={:.3} \
                             (max={:.3}) trivial_rate={:.3} (max={:.3}) \
                             richness_mean={:.3} (min={:.3})",
                            anti_stub_pass,
                            anti_stub_cfg.min_pass_rate,
                            placeholder_rate,
                            anti_stub_cfg.max_placeholder_event_rate,
                            trivial_placeholder_rate,
                            anti_stub_cfg.max_trivial_placeholder_event_rate,
                            construct_richness_mean,
                            anti_stub_cfg.min_construct_richness_mean
                        ),
                        block: anti_stub_cfg.block,
                    });
                }
            }
        }
    }

    // --- per_context gates ------------------------------------------------------
    // eval_results.json already parsed above — reuse eval_json.
    // For each context key in the policy, check parse_rate and scope_compliance_rate.
    // If the context key is present in policy but absent from JSON, emit a failing
    // GateResult so CI doesn't silently pass (B5).
    if !policy.per_context.is_empty() {
        if let Some(ref eval) = eval_json {
            let breakdown_obj = eval.get("context_breakdown").and_then(|v| v.as_object());
            for (ctx_name, gate) in &policy.per_context {
                let slice = breakdown_obj.and_then(|b| b.get(ctx_name));
                match slice {
                    None => {
                        // Context key configured in policy but not found in eval output.
                        // Emit a failing gate so this doesn't silently pass in CI.
                        results.push(GateResult {
                            name: format!("per_context[{}]", ctx_name),
                            passed: false,
                            message: format!(
                                "context '{}' not found in eval_results.json — \
                                 0 samples or not evaluated",
                                ctx_name
                            ),
                            block: gate.block,
                        });
                    }
                    Some(slice) => {
                        let parse_rate = slice
                            .get("parse_rate")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        let scope_rate = slice
                            .get("scope_compliance_rate")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(1.0);
                        let parse_ok =
                            gate.min_parse_rate == 0.0 || parse_rate >= gate.min_parse_rate;
                        let scope_ok = gate.min_scope_compliance_rate == 0.0
                            || scope_rate >= gate.min_scope_compliance_rate;
                        let passed = parse_ok && scope_ok;
                        results.push(GateResult {
                            name: format!("per_context[{}]", ctx_name),
                            passed,
                            message: format!(
                                "parse={:.1}% (min={:.0}%) scope={:.1}% (min={:.0}%)",
                                100.0 * parse_rate,
                                100.0 * gate.min_parse_rate,
                                100.0 * scope_rate,
                                100.0 * gate.min_scope_compliance_rate,
                            ),
                            block: gate.block,
                        });
                    }
                }
            }
        } else if !policy.per_context.is_empty() {
            // No eval_results.json at all — emit failing gates for all configured contexts.
            for (ctx_name, gate) in &policy.per_context {
                results.push(GateResult {
                    name: format!("per_context[{}]", ctx_name),
                    passed: false,
                    message: "eval_results.json not found — run `vox mens eval` first".to_string(),
                    block: gate.block,
                });
            }
        }
    }

    // --- modal_mix gate ----------------------------------------------------------
    // Prefers modal_breakdown from eval_json (already parsed). Falls back to
    // mix_report.json if eval_results.json is absent or lacks the field.
    if policy.modal_mix.max_voice_fraction > 0.0 {
        let modal_report: Option<serde_json::Value> = {
            // First try eval_json (already in memory)
            let has_modal = eval_json
                .as_ref()
                .and_then(|v| v.get("modal_breakdown"))
                .is_some();
            if has_modal {
                eval_json.clone()
            } else {
                // Fallback: mix_report.json
                let mix_report_path = run_dir.join("mix_report.json");
                if mix_report_path.exists() {
                    let content = read_utf8_path_capped(&mix_report_path)?;
                    serde_json::from_str(&content).ok()
                } else {
                    None
                }
            }
        };
        if let Some(ref report) = modal_report
            && let Some(breakdown) = report.get("modal_breakdown").and_then(|v| v.as_object())
        {
            let total: u64 = breakdown.values().filter_map(|v| v.as_u64()).sum();
            let voice = breakdown.get("voice").and_then(|v| v.as_u64()).unwrap_or(0);
            if total > 0 {
                let voice_frac = voice as f64 / total as f64;
                let passed = voice_frac <= policy.modal_mix.max_voice_fraction;
                results.push(GateResult {
                    name: "modal_mix[voice]".to_string(),
                    passed,
                    message: format!(
                        "voice fraction {:.1}% {} {:.0}%",
                        100.0 * voice_frac,
                        if passed { "<=" } else { ">" },
                        100.0 * policy.modal_mix.max_voice_fraction,
                    ),
                    block: policy.modal_mix.block,
                });
            }
        }
    }

    // --- mcp_tool_schema gate (MCP tool-arg JSON Schema KPI) -------------------
    let mcp_cfg = &policy.mcp_tool_schema;
    let mcp_active = mcp_cfg.min_strict_validity_rate > 0.0 || mcp_cfg.block;
    if mcp_active {
        let path = mcp_tool_schema_metrics_path(run_dir, &mcp_cfg.metrics_file);
        if !path.exists() {
            results.push(GateResult {
                name: "mcp_tool_schema".to_string(),
                passed: false,
                message: format!(
                    "metrics file missing: {} — save `tool_arg_schema_kpi` from vox-mcp diagnostics here",
                    path.display()
                ),
                block: mcp_cfg.block,
            });
        } else {
            let content = read_utf8_path_capped(&path)?;
            let v: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("{}: invalid JSON ({})", path.display(), e))?;
            let enabled = v
                .get("validation_enabled")
                .and_then(|x| x.as_bool())
                .unwrap_or(true);
            if !enabled {
                results.push(GateResult {
                    name: "mcp_tool_schema".to_string(),
                    passed: true,
                    message:
                        "MCP tool-arg schema validation disabled (VOX_MCP_TOOL_ARG_SCHEMA_VALIDATE)"
                            .to_string(),
                    block: false,
                });
            } else {
                let checks = v.get("checks_total").and_then(|x| x.as_u64()).unwrap_or(0);
                let rate_direct = v.get("strict_validity_rate").and_then(|x| x.as_f64());
                let sp = v.get("strict_passed").and_then(|x| x.as_u64()).unwrap_or(0);
                let sf = v.get("strict_failed").and_then(|x| x.as_u64()).unwrap_or(0);
                let decided = sp + sf;
                let rate_computed = if decided > 0 {
                    Some(sp as f64 / decided as f64)
                } else {
                    None
                };
                let effective_rate = rate_direct.or(rate_computed);
                let (passed, msg) = match effective_rate {
                    None if checks == 0 => (
                        false,
                        "checks_total=0 and no strict_validity_rate / pass-fail counts".to_string(),
                    ),
                    None => (
                        false,
                        "could not derive strict_validity_rate (missing field and pass+fail=0)"
                            .to_string(),
                    ),
                    Some(r) => {
                        let ok = r >= mcp_cfg.min_strict_validity_rate;
                        (
                            ok,
                            format!(
                                "strict_validity_rate={:.4} (min={:.4}) checks_total={}",
                                r, mcp_cfg.min_strict_validity_rate, checks
                            ),
                        )
                    }
                };
                results.push(GateResult {
                    name: "mcp_tool_schema".to_string(),
                    passed,
                    message: msg,
                    block: mcp_cfg.block,
                });
            }
        }
    }
    // -------------------------------------------------------------------------

    let passk_cfg = &policy.pass_at_k;
    let passk_active = passk_cfg.block
        || passk_cfg.min_pass_rate_at_1 > 0.0
        || passk_cfg.min_pass_rate_at_k > 0.0
        || passk_cfg.baseline_file.is_some();
    if passk_active {
        let metrics_name = std::path::Path::new(&passk_cfg.metrics_file)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let metrics_path = run_dir.join(if metrics_name.is_empty() {
            "benchmark_passatk.json"
        } else {
            &metrics_name
        });
        if !metrics_path.exists() {
            results.push(GateResult {
                name: "pass_at_k".to_string(),
                passed: false,
                message: format!(
                    "metrics file missing: {} — run `vox mens eval-local \
                     --model {}/candle_qlora_adapter.safetensors \
                     --bench mens/data/heldout_bench \
                     --samples <k> -o {}/eval_local_report.json` first",
                    metrics_path.display(),
                    run_dir.display(),
                    run_dir.display()
                ),
                block: passk_cfg.block,
            });
        } else {
            let content = read_utf8_path_capped(&metrics_path)?;
            let v: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("{}: invalid JSON ({})", metrics_path.display(), e))?;
            let p1 = v
                .get("pass_rate_at_1")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            let pk = v
                .get("pass_rate_at_k")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            let mut pass = p1 >= passk_cfg.min_pass_rate_at_1 && pk >= passk_cfg.min_pass_rate_at_k;
            let mut msg = format!(
                "pass@1={:.3} (min={:.3}) pass@k={:.3} (min={:.3})",
                p1, passk_cfg.min_pass_rate_at_1, pk, passk_cfg.min_pass_rate_at_k
            );
            if let Some(baseline_name) = passk_cfg.baseline_file.as_deref() {
                let baseline_path = run_dir.join(
                    std::path::Path::new(baseline_name)
                        .file_name()
                        .unwrap_or_default(),
                );
                if baseline_path.exists() {
                    let bcontent = read_utf8_path_capped(&baseline_path)?;
                    if let Ok(bv) = serde_json::from_str::<serde_json::Value>(&bcontent) {
                        let b1 = bv
                            .get("pass_rate_at_1")
                            .and_then(|x| x.as_f64())
                            .unwrap_or(0.0);
                        let bk = bv
                            .get("pass_rate_at_k")
                            .and_then(|x| x.as_f64())
                            .unwrap_or(0.0);
                        let drop1 = b1 - p1;
                        let dropk = bk - pk;
                        if drop1 > passk_cfg.max_regression_drop
                            || dropk > passk_cfg.max_regression_drop
                        {
                            pass = false;
                        }
                        msg.push_str(&format!(
                            " baseline(pass@1={:.3},pass@k={:.3}) max_drop={:.3}",
                            b1, bk, passk_cfg.max_regression_drop
                        ));
                    }
                } else {
                    msg.push_str(" baseline file missing (skipped regression check)");
                }
            }
            results.push(GateResult {
                name: "pass_at_k".to_string(),
                passed: pass,
                message: msg,
                block: passk_cfg.block,
            });
        }
    }

    let rr = &policy.review_recurrence;
    let rr_active = rr.block
        || rr.max_repeated_finding_rate > 0.0
        || rr.max_post_training_regression_rate > 0.0
        || rr.min_recurrence_delta > 0.0;
    if rr_active {
        let metrics_path = run_dir.join("review_metrics.json");
        if !metrics_path.exists() {
            results.push(GateResult {
                name: "review_recurrence".to_string(),
                passed: false,
                message: format!("metrics file missing: {}", metrics_path.display()),
                block: rr.block,
            });
        } else {
            let content = read_utf8_path_capped(&metrics_path)?;
            let v: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("{}: invalid JSON ({})", metrics_path.display(), e))?;
            let repeated = v
                .get("repeated_finding_rate")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            let post_reg = v
                .get("post_training_regression_rate")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            let delta = v
                .get("recurrence_delta")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);

            let repeated_ok =
                rr.max_repeated_finding_rate <= 0.0 || repeated <= rr.max_repeated_finding_rate;
            let post_reg_ok = rr.max_post_training_regression_rate <= 0.0
                || post_reg <= rr.max_post_training_regression_rate;
            let delta_ok = rr.min_recurrence_delta <= 0.0 || delta >= rr.min_recurrence_delta;
            results.push(GateResult {
                name: "review_recurrence".to_string(),
                passed: repeated_ok && post_reg_ok && delta_ok,
                message: format!(
                    "repeated={:.3} (max={:.3}) post_reg={:.3} (max={:.3}) delta={:.3} (min={:.3})",
                    repeated,
                    rr.max_repeated_finding_rate,
                    post_reg,
                    rr.max_post_training_regression_rate,
                    delta,
                    rr.min_recurrence_delta
                ),
                block: rr.block,
            });
        }
    }

    Ok(results)
}
