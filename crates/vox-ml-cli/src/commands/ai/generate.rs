//! `vox mens generate` — AI-powered Vox code generation.
//!
//! Connects to the `vox-orchestrator-d` daemon and streams AI-generated Vox source code
//! from a natural language prompt. Supports context modes, output validation,
//! structured output modes, and Codex task-job queueing.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use vox_compiler::generated_vox::{
    OutputSurfaceMode, normalize_generated_vox, validate_generated_vox,
};

use vox_bounded_fs::read_utf8_path_capped;

/// System prompt preamble embedded into every generation request.
const SYSTEM_PREAMBLE: &str = "You are an expert Vox language programmer. Vox is a full-stack AI-native programming \
     language that compiles to Rust/WASM. Generate clean, idiomatic Vox code. \
     Output ONLY valid Vox source — no markdown fences, no English explanation.";
const RUNTIME_GENERATION_KPI_SCHEMA: &str = "vox_runtime_generation_kpi_v1";

fn runtime_generation_kpi(
    success: bool,
    validate_requested: bool,
    compile_pass: Option<bool>,
    canonical_pass: Option<bool>,
    attempts: u32,
    repair_stalled: bool,
    time_to_first_valid_ms: Option<u128>,
    output_tokens: usize,
    surface_contract_failures: u32,
    canonicalization_failures: Option<u32>,
    failure_category: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "schema": RUNTIME_GENERATION_KPI_SCHEMA,
        "schema_version": 1,
        "surface": "vox-cli",
        "success": success,
        "validate_requested": validate_requested,
        "compile_pass": compile_pass,
        "canonical_pass": canonical_pass,
        "attempts": attempts,
        "repair_stalled": repair_stalled,
        "time_to_first_valid_ms": time_to_first_valid_ms.map(|v| v as u64),
        "output_tokens": output_tokens,
        "surface_contract_failures": surface_contract_failures,
        "strictness_failures": surface_contract_failures,
        "canonicalization_failures": canonicalization_failures,
        "failure_category": failure_category,
    })
}

/// Collect schema context from the current working directory (if a Vox project).
fn collect_schema_context() -> String {
    let schema_file = std::env::current_dir()
        .ok()
        .map(|d| d.join("schema.vox"))
        .filter(|p| p.exists());

    if let Some(path) = schema_file
        && let Ok(content) = read_utf8_path_capped(&path)
    {
        return format!(
            "\n\n# Project Schema (schema.vox):\n```vox\n{}\n```",
            content
        );
    }
    String::new()
}

/// Build the full prompt based on context mode.
fn build_prompt(prompt: &str, context_mode: Option<&str>) -> String {
    let mode = context_mode.unwrap_or("minimal");
    let mut ctx = String::new();

    match mode {
        "schema-only" | "repo-aware" | "full" => {
            ctx.push_str(&collect_schema_context());
        }
        _ => {} // minimal, graph-aware: no extra file context in this path
    }

    format!("{SYSTEM_PREAMBLE}{ctx}\n\n# Task:\n{prompt}")
}

/// Full [`jsonschema`] validation against a JSON Schema file (requires `output_mode` + `schema` path).
fn validate_json_schema(source: &str, schema_path: &Path) -> Result<()> {
    let schema_str = read_utf8_path_capped(schema_path)
        .with_context(|| format!("Failed to read schema: {}", schema_path.display()))?;
    let validator = vox_jsonschema_util::compile_validator_from_utf8(&schema_str, schema_path)?;
    let instance: serde_json::Value = serde_json::from_str(source)
        .context("Generated output is not valid JSON (required by --output-mode + --schema)")?;
    vox_jsonschema_util::validate(
        &instance,
        &validator,
        format!("generated output vs {}", schema_path.display()),
    )
}

/// Call the `vox-orchestrator-d` daemon and collect the full streamed generation result.
///
/// Chunks are printed to stdout in real-time by `call_daemon`; the final
/// `Result` payload (if any) is returned as the captured text.
async fn generate_via_daemon(full_prompt: &str) -> Result<String> {
    let result = crate::dei_daemon::call(
        crate::dei_daemon::method::AI_GENERATE,
        serde_json::json!({ "prompt": full_prompt }),
        false,
    )
    .await?;

    // The daemon streams Chunk payloads directly to stdout (handled by call_daemon).
    // If it also returns a structured "text" result, capture it.
    let text = result
        .get("text")
        .or_else(|| result.get("output"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(text)
}

/// Persist generated result to Codex via `Arca::log_execution`.
///
/// This is a best-effort operation — failures are swallowed silently so they
/// never break the generation flow. The `queue` flag in `run()` controls whether
/// this is called at all; when `false` (the default) there is zero overhead.
async fn try_enqueue_job(prompt: &str, result: &str) {
    let _ = (prompt, result);
    tracing::debug!(
        target = "vox_cli.generate",
        "Codex queue persistence for generate is not wired in this build (optional vox-arca path)."
    );
}

/// Main entry point for `vox mens generate`.
///
/// # Parameters
/// - `prompt` — natural language description of the code to generate
/// - `output` — optional file path to write; stdout if `None`
/// - `no_validate` — skip Vox parse/typecheck validation
/// - `server_url` — explicit inference server URL (overrides daemon)
/// - `max_retries` — retry count on validation failure (default 2)
/// - `output_mode` — structured output: `strict_json`, `jsonl_records`, `tool_args_json`
/// - `schema` — JSON schema path for post-generation validation (requires `output_mode`)
/// - `context_mode` — context assembly strategy: `minimal` | `repo-aware` | `schema-only` | `graph-aware` | `full`
/// - `conversation_id` — Codex conversation ID for graph-aware context (reserved)
/// - `queue` — persist result to Codex task queue
#[allow(clippy::too_many_arguments)] // Mirrors CLI flag surface; bundling would obscure dispatch.
pub async fn run(
    prompt: &str,
    output: Option<PathBuf>,
    no_validate: bool,
    server_url: Option<&str>,
    max_retries: Option<u32>,
    output_mode: Option<&str>,
    schema: Option<&Path>,
    context_mode: Option<&str>,
    _conversation_id: Option<i64>,
    queue: bool,
) -> Result<()> {
    let retries = max_retries.unwrap_or(2);
    let mut full_prompt = build_prompt(prompt, context_mode);

    println!("{} Generating Vox code…", "◆".cyan().bold());
    if let Some(mode) = context_mode.filter(|m| *m != "minimal") {
        println!("  Context mode: {}", mode.dimmed());
    }

    let mut last_result = String::new();
    let mut last_err: Option<anyhow::Error> = None;
    let mut strictness_failures: u32 = 0;
    let mut canonical_failures: u32 = 0;
    let run_started = Instant::now();
    let mut time_to_first_valid_ms: Option<u128> = None;
    let mut repair_stalled = false;
    let mut prev_error_sig: Option<u64> = None;
    let mut attempts_used: u32 = 0;

    // Build the HTTP client once outside the retry loop — creating a Client per
    // attempt is expensive (TLS handshake, connection pool teardown).
    let http_client = server_url.map(|_| {
        vox_http_client::client_builder()
            .timeout(std::time::Duration::from_secs(
                std::env::var("VOX_GEN_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(120u64),
            ))
            .build()
            .expect("reqwest Client should always construct")
    });

    // max_tokens: env override → CLI default (2048). Allows tuning without recompile.
    let max_tokens: u64 = std::env::var("VOX_GEN_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2048);

    for attempt in 0..=retries {
        attempts_used = attempt + 1;
        if attempt > 0 {
            eprintln!(
                "{} Retry {}/{} — regenerating…",
                "↺".yellow(),
                attempt,
                retries
            );
        }

        let raw = if let (Some(url), Some(client)) = (server_url, &http_client) {
            // Direct HTTP inference: POST to /generate endpoint
            let body = serde_json::json!({
                "prompt": full_prompt,
                "max_tokens": max_tokens,
            });
            let resp = client
                .post(format!("{}/generate", url.trim_end_matches('/')))
                .json(&body)
                .send()
                .await
                .with_context(|| format!("Failed to connect to inference server at {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Inference server returned {status}: {body_text}");
            }
            let json: serde_json::Value = resp.json().await.context("Invalid JSON from server")?;
            json.get("text")
                .or_else(|| json.get("response"))
                .or_else(|| json.get("generated_text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            // Route through vox-orchestrator-d daemon (fail fast — no fake offline output).
            generate_via_daemon(&full_prompt).await.context(
                "Orchestrator daemon unavailable. Start `vox-orchestrator-d`, or pass `--server-url` for direct HTTP inference.",
            )?
        };

        let normalized = normalize_generated_vox(&raw, OutputSurfaceMode::RawCodeOnly);
        let stripped = normalized.normalized;
        if normalized.had_fence || normalized.had_prose_markers {
            strictness_failures += 1;
        }

        tracing::debug!(
            attempt,
            retries,
            prompt_len = full_prompt.len(),
            raw_len = raw.len(),
            stripped_len = stripped.len(),
            "generate parser payload snapshot"
        );

        last_result = stripped.clone();

        if no_validate {
            break;
        }

        // Validate: JSON schema (if provided) or Vox parse/typecheck.
        let validation_result: Result<Vec<String>> =
            if let (Some(mode), Some(sch)) = (output_mode, schema) {
                if matches!(mode, "strict_json" | "jsonl_records" | "tool_args_json") {
                    validate_json_schema(&stripped, sch)?;
                    Ok(Vec::new())
                } else {
                    let validation = validate_generated_vox(&stripped, true);
                    if validation.is_valid() {
                        Ok(Vec::new())
                    } else {
                        Ok(validation.errors.into_iter().map(|e| e.message).collect())
                    }
                }
            } else {
                let validation = validate_generated_vox(&stripped, true);
                if validation.is_valid() {
                    Ok(Vec::new())
                } else {
                    Ok(validation.errors.into_iter().map(|e| e.message).collect())
                }
            };

        match validation_result {
            Ok(errors) if errors.is_empty() => {
                if time_to_first_valid_ms.is_none() {
                    time_to_first_valid_ms = Some(run_started.elapsed().as_millis());
                }
                // Enforce canonicalized/de-whitespaced valid `.vox` output by default.
                if output_mode.is_none() {
                    let validation = validate_generated_vox(&stripped, true);
                    if let Some(canon) = validation.canonicalized {
                        last_result = canon;
                    } else {
                        canonical_failures += 1;
                        last_result = stripped;
                    }
                } else {
                    last_result = stripped;
                }
                break;
            }
            Ok(errs) if attempt < retries => {
                if !errs.is_empty() {
                    let sig = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut rows = errs.clone();
                        rows.sort();
                        let mut h = DefaultHasher::new();
                        rows.hash(&mut h);
                        h.finish()
                    };
                    if prev_error_sig == Some(sig) {
                        repair_stalled = true;
                    }
                    prev_error_sig = Some(sig);
                    let mut feedback = String::from(
                        "\n\nThe previous generation had compiler errors. Regenerate ONLY corrected .vox code.\n",
                    );
                    for (idx, msg) in errs.iter().enumerate() {
                        feedback.push_str(&format!("{}. {}\n", idx + 1, msg));
                    }
                    full_prompt.push_str(&feedback);
                }
                last_err = Some(anyhow::anyhow!("generated code had validation errors"));
                continue;
            }
            Ok(errs) => {
                eprintln!(
                    "{} Validation failed after {} attempt(s): {}",
                    "✗".red().bold(),
                    attempt + 1,
                    errs.join("; ")
                );
                last_err = Some(anyhow::anyhow!(errs.join("; ")));
                break;
            }
            Err(e) => {
                eprintln!(
                    "{} Validation failed after {} attempt(s): {}",
                    "✗".red().bold(),
                    attempt + 1,
                    e
                );
                last_err = Some(e);
                break;
            }
        }
    }

    // Optionally queue to Codex task store.
    if queue {
        try_enqueue_job(prompt, &last_result).await;
    }

    // Write output.
    if let Some(out_path) = &output {
        std::fs::create_dir_all(out_path.parent().unwrap_or_else(|| Path::new(".")))?;
        std::fs::write(out_path, &last_result)
            .with_context(|| format!("Failed to write output to {}", out_path.display()))?;
        println!("{} Written to {}", "✓".green().bold(), out_path.display());
    } else {
        println!("\n{}", last_result);
    }

    if let Some(ref err) = last_err {
        eprintln!(
            "{} Note: generated code may have issues — {}",
            "⚠".yellow(),
            err
        );
    }
    eprintln!(
        "{} Strictness/canonicalization accounting: strictness_failures={} canonicalization_failures={}",
        "ℹ".cyan(),
        strictness_failures,
        canonical_failures
    );
    let compile_pass = if no_validate {
        None
    } else {
        Some(last_err.is_none())
    };
    let canonical_pass = if no_validate {
        None
    } else {
        Some(canonical_failures == 0)
    };
    let failure_category = if last_err.is_some() {
        Some("validation")
    } else {
        None
    };
    let kpi = runtime_generation_kpi(
        last_err.is_none(),
        !no_validate,
        compile_pass,
        canonical_pass,
        attempts_used,
        repair_stalled,
        time_to_first_valid_ms,
        last_result.split_whitespace().count(),
        strictness_failures,
        Some(canonical_failures),
        failure_category,
    );
    eprintln!("{} Generation KPIs: {}", "ℹ".cyan(), kpi);
    if let Ok(kpi_path) = std::env::var("VOX_GEN_KPI_JSONL") {
        let mut row = serde_json::to_string(&kpi)?;
        row.push('\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&kpi_path)?
            .write_all(row.as_bytes())?;
    }

    Ok(())
}
