//! `vox mens eval-local` — evaluate trained model against heldout benchmark.

use super::eval_local_prompt::{
    PreparedBench, prepare_bench_item, sort_prepared_benches_lexicographic,
};
use anyhow::Result;
use std::path::PathBuf;

use vox_bounded_fs::read_utf8_path_capped;
const ANTI_STUB_MIN_CONSTRUCT_RICHNESS: f64 = 0.20;

pub fn run_eval_local(
    model: PathBuf,
    bench: PathBuf,
    max_tokens: usize,
    temperature: f32,
    samples: usize,
    seed_base: u64,
    output: Option<PathBuf>,
) -> Result<()> {
    use owo_colors::OwoColorize;

    if !model.exists() {
        anyhow::bail!(
            "Model checkpoint not found at {}.\n  Run `vox mens train` first.",
            model.display()
        );
    }

    let manifest_path = bench.join("manifest.json");
    let manifest: serde_json::Value = if manifest_path.exists() {
        let content = read_utf8_path_capped(&manifest_path)?;
        serde_json::from_str(&content)?
    } else {
        anyhow::bail!(
            "Benchmark manifest not found at {}",
            manifest_path.display()
        );
    };

    eprintln!("{}", "╔══════════════════════════════════════════╗".cyan());
    eprintln!("{}", "║   Vox Mens — Local Eval Harness        ║".cyan());
    eprintln!("{}", "╚══════════════════════════════════════════╝".cyan());
    eprintln!("  Model:  {}", model.display());
    eprintln!("  Bench:  {}", bench.display());
    eprintln!("  Tokens: {} per sample | temp {}", max_tokens, temperature);
    eprintln!();

    let benchmarks = manifest["benchmarks"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let model_size = std::fs::metadata(&model).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "  Checkpoint: {:.1} MB | {} benchmark samples",
        model_size as f64 / 1_048_576.0,
        benchmarks.len()
    );
    eprintln!(
        "  {} Prompt order: lexicographic by full prompt (prefix-cache friendly)",
        "ℹ".cyan()
    );
    eprintln!();

    #[cfg(feature = "gpu")]
    let _serve_cfg = crate::commands::ai::serve::ServeConfig {
        model_path: model.clone(),
        port: 0,
        host: std::net::Ipv4Addr::LOCALHOST.to_string(),
        max_tokens,
        temperature,
        system_prompt: None,
    };

    // SP3 Unit 3: candle_inference_serve deleted from vox-populi; authoritative copy lives in
    // vox-plugin-mens-candle-cuda/src/inference.rs.  vox-ml-cli drives inference through the plugin
    // host (MlBackend::generate / plugin dispatch).  Until the plugin-host wiring is plumbed into
    // eval_local, the engine is unconditionally None.
    #[cfg(feature = "gpu")]
    #[allow(unused_mut)]
    let mut engine: Option<()> = None;

    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut passed_k = 0usize;
    let mut passed_1 = 0usize;
    let mut semantic_passed_k = 0usize;
    let mut anti_stub_passed_k = 0usize;
    let mut placeholder_event_count = 0usize;
    let mut trivial_placeholder_event_count = 0usize;
    let mut construct_richness_sum = 0.0_f64;
    let mut category_stats: std::collections::HashMap<String, (usize, usize, usize)> =
        std::collections::HashMap::new();

    #[cfg(feature = "gpu")]
    let _eval_session: Option<String> = None;

    let mut prepared: Vec<PreparedBench> = benchmarks
        .iter()
        .enumerate()
        .map(|(i, item)| prepare_bench_item(&bench, i, item))
        .collect();
    sort_prepared_benches_lexicographic(&mut prepared);

    for w in prepared {
        let prompt = w.prompt;
        let id = w.id;
        let file = w.file;
        let category = w.category;
        let description = w.description;
        let context_files = w.context_files;
        let semantic_expected_contains = w.semantic_expected_contains;
        let manifest_index = w.manifest_index;

        let (pass_at_1, pass_at_k, samples_json): (bool, bool, Vec<serde_json::Value>) =
            if prompt.is_empty() || !model.exists() {
                (
                    false,
                    false,
                    vec![serde_json::json!({
                        "sample_index": 0,
                        "pass": false,
                        "error": "no prompt or model"
                    })],
                )
            } else {
                // SP3 Unit 3: InferenceEngine removed from vox-populi; engine is always None
                // until vox-ml-cli eval-local is rewired through the plugin host.
                #[cfg(feature = "gpu")]
                {
                    let _ = &engine; // suppress unused warning
                    (
                        false,
                        false,
                        vec![serde_json::json!({
                            "sample_index": 0,
                            "pass": false,
                            "error": "inference engine not yet wired to plugin host (SP3 Unit 3)"
                        })],
                    )
                }
                #[cfg(not(feature = "gpu"))]
                {
                    (
                        false,
                        false,
                        vec![serde_json::json!({
                            "sample_index": 0,
                            "pass": false,
                            "error": format!(
                                "CPU: GPU needed for inference — model {:.1}MB, prompt {} chars",
                                model_size as f64 / 1_048_576.0,
                                prompt.len()
                            )
                        })],
                    )
                }
            };

        let semantic_pass_at_k = samples_json.iter().any(|v| {
            v.get("semantic_pass")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        });
        let entry = serde_json::json!({
            "manifest_index": manifest_index,
            "id": id,
            "file": file,
            "context_files": context_files,
            "category": category,
            "description": description,
            "pass_at_1": pass_at_1,
            "pass_at_k": pass_at_k,
            "semantic_pass_at_k": semantic_pass_at_k,
            "k": samples.max(1),
            "samples": samples_json,
        });
        results.push(entry);

        let (p1, pk, t) = category_stats.entry(category.clone()).or_insert((0, 0, 0));
        *t += 1;
        if pass_at_1 {
            *p1 += 1;
            passed_1 += 1;
        }
        if pass_at_k {
            *pk += 1;
            passed_k += 1;
        }
        if semantic_pass_at_k {
            semantic_passed_k += 1;
        }
        if samples_json.iter().any(|v| {
            v.get("anti_stub_pass")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        }) {
            anti_stub_passed_k += 1;
        }
        if samples_json.iter().any(|v| {
            v.get("checks")
                .and_then(|c| c.get("placeholder_marker_hits"))
                .and_then(|x| x.as_u64())
                .unwrap_or(0)
                > 0
        }) {
            placeholder_event_count += 1;
        }
        if samples_json.iter().any(|v| {
            v.get("checks")
                .and_then(|c| c.get("trivial_placeholder_output"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        }) {
            trivial_placeholder_event_count += 1;
        }
        let construct_richness_best = samples_json
            .iter()
            .filter_map(|v| {
                v.get("checks")
                    .and_then(|c| c.get("construct_richness_score"))
                    .and_then(|x| x.as_f64())
            })
            .fold(0.0_f64, f64::max);
        construct_richness_sum += construct_richness_best;

        let icon = if pass_at_k {
            "✓".green().to_string()
        } else {
            "✗".red().to_string()
        };
        eprintln!(
            "  {} [{:8}] {} — {} (p@1={} p@k={})",
            icon, category, id, description, pass_at_1, pass_at_k
        );
    }

    results.sort_by(|a, b| {
        let ia = a
            .get("manifest_index")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let ib = b
            .get("manifest_index")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        ia.cmp(&ib)
    });

    let total = benchmarks.len();
    let pass_rate_at_1 = if total > 0 {
        passed_1 as f64 / total as f64
    } else {
        0.0
    };
    let pass_rate_at_k = if total > 0 {
        passed_k as f64 / total as f64
    } else {
        0.0
    };
    let semantic_task_success = if total > 0 {
        semantic_passed_k as f64 / total as f64
    } else {
        0.0
    };
    let anti_stub_task_success = if total > 0 {
        anti_stub_passed_k as f64 / total as f64
    } else {
        0.0
    };
    let placeholder_event_rate = if total > 0 {
        placeholder_event_count as f64 / total as f64
    } else {
        0.0
    };
    let trivial_placeholder_event_rate = if total > 0 {
        trivial_placeholder_event_count as f64 / total as f64
    } else {
        0.0
    };
    let construct_richness_mean = if total > 0 {
        construct_richness_sum / total as f64
    } else {
        0.0
    };
    eprintln!();
    eprintln!("  {}", "─".repeat(54));
    eprintln!(
        "  Overall: pass@1 {}/{} ({:.0}%) | pass@{} {}/{} ({:.0}%) | semantic {:.0}%",
        passed_1,
        total,
        pass_rate_at_1 * 100.0,
        samples.max(1),
        passed_k,
        total,
        pass_rate_at_k * 100.0,
        semantic_task_success * 100.0
    );
    for (cat, (p1, pk, t)) in &category_stats {
        eprintln!("    {:12} p@1 {}/{} p@k {}/{}", cat, p1, t, pk, t);
    }
    eprintln!();

    let report = serde_json::json!({
        "model": model.to_string_lossy(),
        "bench": bench.to_string_lossy(),
        "max_tokens": max_tokens,
        "temperature": temperature,
        "k": samples.max(1),
        "seed_base": seed_base,
        "total": total,
        "passed_at_1": passed_1,
        "passed_at_k": passed_k,
        "pass_rate_at_1": pass_rate_at_1,
        "pass_rate_at_k": pass_rate_at_k,
        "semantic_task_success": semantic_task_success,
        "anti_stub_task_success": anti_stub_task_success,
        "placeholder_event_rate": placeholder_event_rate,
        "trivial_placeholder_event_rate": trivial_placeholder_event_rate,
        "construct_richness_mean": construct_richness_mean,
        "category_stats": category_stats.iter().map(|(k, (p1, pk, t))| {
            serde_json::json!({
                "category": k,
                "passed_at_1": p1,
                "passed_at_k": pk,
                "total": t,
                "pass_rate_at_1": *p1 as f64 / (*t as f64).max(1.0),
                "pass_rate_at_k": *pk as f64 / (*t as f64).max(1.0)
            })
        }).collect::<Vec<_>>(),
        "results": results,
    });

    if let Some(out_path) = output {
        std::fs::write(&out_path, serde_json::to_string_pretty(&report)?)?;
        eprintln!("  Report saved: {}", out_path.display());
        if let Some(parent) = out_path.parent() {
            let passk_path = parent.join("benchmark_passatk.json");
            std::fs::write(
                &passk_path,
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema": "vox_mens_benchmark_passatk_v1",
                    "k": samples.max(1),
                    "seed_base": seed_base,
                    "total": total,
                    "pass_rate_at_1": pass_rate_at_1,
                    "pass_rate_at_k": pass_rate_at_k
                }))?,
            )?;
            eprintln!("  pass@k summary saved: {}", passk_path.display());
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    #[cfg(feature = "gpu")]
    vox_cli_core::benchmark_telemetry::record_opt_blocking(
        "eval_local",
        Some(pass_rate_at_k),
        Some(serde_json::json!({
            "total": total,
            "passed_at_1": passed_1,
            "passed_at_k": passed_k,
            "model": model.to_string_lossy(),
            "bench": bench.to_string_lossy(),
        })),
    );

    Ok(())
}

struct CompletionVerification {
    pass: bool,
    semantic_pass: bool,
    anti_stub_pass: bool,
    checks: serde_json::Value,
}

fn placeholder_marker_hits(source: &str) -> usize {
    let lower = source.to_ascii_lowercase();
    [
        "todo",
        "tbd",
        "placeholder",
        "stub",
        "not implemented",
        "coming soon",
    ]
    .iter()
    .filter(|m| lower.contains(**m))
    .count()
}

fn is_trivial_placeholder_output(source: &str) -> bool {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return true;
    }
    let code_lines = trimmed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .count();
    code_lines <= 1 || trimmed.eq_ignore_ascii_case("return")
}

fn verify_completion(
    completion: &str,
    bench_root: &std::path::Path,
    file_hint: &str,
    sample_id: &str,
    manifest_index: usize,
    semantic_expected_contains: &[String],
) -> CompletionVerification {
    let non_empty = !completion.trim().is_empty();
    let mut parse_ok = false;
    let mut typecheck_ok = false;
    let parse_error: Option<String>;
    let mut diag_errors = 0usize;

    let candidate_path = if !file_hint.is_empty() {
        bench_root.join(file_hint)
    } else {
        bench_root.join(format!("eval_local_{manifest_index}_{sample_id}.vox"))
    };
    tracing::debug!(
        target: "vox_eval::mens_local",
        sample_id,
        manifest_index,
        file_hint = file_hint,
        payload_len = completion.len(),
        payload_preview = %completion.chars().take(200).collect::<String>(),
        "eval-local verifier payload"
    );
    let frontend = crate::pipeline::run_frontend_str(completion, &candidate_path, false);
    match frontend {
        Ok(res) => {
            parse_ok = true;
            diag_errors = res.error_count();
            typecheck_ok = !res.has_errors();
            parse_error = None;
        }
        Err(err) => {
            parse_error = Some(err.to_string());
        }
    }

    let pass = non_empty && parse_ok && typecheck_ok;
    let placeholder_hits = placeholder_marker_hits(completion);
    let trivial_placeholder = is_trivial_placeholder_output(completion);
    let construct_richness = vox_compiler::ast_eval(completion).coverage_score();
    let anti_stub_pass = placeholder_hits == 0
        && !trivial_placeholder
        && construct_richness >= ANTI_STUB_MIN_CONSTRUCT_RICHNESS;
    let semantic_pass = pass
        && semantic_expected_contains
            .iter()
            .all(|needle| completion.contains(needle));
    CompletionVerification {
        pass: pass && anti_stub_pass,
        semantic_pass,
        anti_stub_pass,
        checks: serde_json::json!({
            "non_empty": non_empty,
            "parse_ok": parse_ok,
            "typecheck_ok": typecheck_ok,
            "diag_errors": diag_errors,
            "parse_error": parse_error,
            "placeholder_marker_hits": placeholder_hits,
            "trivial_placeholder_output": trivial_placeholder,
            "construct_richness_score": construct_richness,
            "anti_stub_pass": anti_stub_pass,
            "semantic_expected_contains": semantic_expected_contains,
            "semantic_pass": semantic_pass
        }),
    }
}
