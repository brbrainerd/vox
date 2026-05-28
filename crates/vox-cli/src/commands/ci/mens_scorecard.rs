//! Mens model-vs-model scorecard harness and decision gates.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use vox_compiler::generated_vox::{
    OutputSurfaceMode, normalize_generated_vox, validate_generated_vox,
};

use vox_bounded_fs::read_utf8_path_capped;

const SCHEMA_REL: &str = "contracts/eval/mens-scorecard.schema.json";
const SUMMARY_SCHEMA_REL: &str = "contracts/eval/mens-scorecard-summary.schema.json";
const EVENT_SCHEMA_REL: &str = "contracts/eval/mens-scorecard-event.schema.json";
const ANTI_STUB_MIN_CONSTRUCT_RICHNESS: f64 = 0.20;
const ANTI_STUB_MIN_PASS_RATE: f64 = 0.92;
const ANTI_STUB_MAX_PLACEHOLDER_EVENT_RATE: f64 = 0.08;
const ANTI_STUB_MAX_TRIVIAL_PLACEHOLDER_RATE: f64 = 0.08;

#[derive(Debug, Clone, Deserialize)]
struct ScorecardSpec {
    schema_version: u32,
    prompt_pack_version: String,
    max_retries: u32,
    conditions: Vec<ModelCondition>,
    tasks: Vec<BenchmarkTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConditionMode {
    HttpGenerate,
    FixtureFile,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelCondition {
    id: String,
    label: String,
    mode: ConditionMode,
    #[serde(default)]
    backend: BackendTag,
    model_id: Option<String>,
    adapter_revision: Option<String>,
    server_url: Option<String>,
    fixture_dir: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum BackendTag {
    Base,
    Qlora,
    Burn,
    #[default]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
struct BenchmarkTask {
    id: String,
    prompt: String,
    #[serde(default)]
    expected_contains: Vec<String>,
    #[serde(default)]
    semantic_expected_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskEvent {
    condition_id: String,
    backend: BackendTag,
    task_id: String,
    attempts: u32,
    compile_pass_1: bool,
    compile_pass_n: bool,
    repair_depth: Option<u32>,
    canonical_pass: bool,
    voxelized_strictness: bool,
    task_success: bool,
    semantic_task_success: bool,
    hir_error_count: u32,
    latency_ms: u128,
    time_to_first_valid_ms: Option<u128>,
    repair_stalled: bool,
    tokens_in: usize,
    tokens_out: usize,
    placeholder_marker_hits: usize,
    trivial_placeholder_output: bool,
    construct_richness_score: f64,
    anti_stub_pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConditionSummary {
    id: String,
    label: String,
    backend: BackendTag,
    model_id: Option<String>,
    adapter_revision: Option<String>,
    total_tasks: usize,
    compile_pass_at_1: f64,
    compile_pass_at_n: f64,
    canonical_pass: f64,
    voxelized_strictness: f64,
    task_success: f64,
    semantic_task_success: f64,
    repair_depth_mean: f64,
    repair_stall_rate: f64,
    time_to_first_valid_p50_ms: u128,
    latency_p50_ms: u128,
    latency_p95_ms: u128,
    tokens_in_total: usize,
    tokens_out_total: usize,
    placeholder_hits_total: usize,
    placeholder_event_rate: f64,
    trivial_placeholder_rate: f64,
    construct_richness_mean: f64,
    anti_stub_pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KpiContractAlignment {
    /// Same `schema` string as `vox_runtime_generation_kpi_v1` payloads from CLI/MCP code generation.
    runtime_generation_kpi_schema: String,
    /// JSON Schema `$id` for per-task scorecard events (stable crosswalk for eval tooling).
    mens_scorecard_event_schema_id: String,
}

/// Crosswalk from Mens anti-stub metrics to the unified completion-quality SSOT / telemetry chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompletionPolicyCrosswalk {
    /// Repo-relative path to the completion policy contract.
    policy_contract_rel: String,
    /// Human-readable note for eval consumers mapping scorecard rates to `vox ci completion-*` + VoxDB ingest.
    gate_integration_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScorecardSummary {
    schema: String,
    schema_version: u32,
    prompt_pack_version: String,
    git_sha: String,
    run_started_utc: String,
    spec_path: String,
    conditions: Vec<ConditionSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kpi_contract_alignment: Option<KpiContractAlignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completion_policy: Option<CompletionPolicyCrosswalk>,
}

#[derive(Debug, Clone, Serialize)]
struct DecisionReport {
    recommended: String,
    reason: String,
    qlora_plateau: bool,
    strictness_unmet: bool,
    semantic_unmet: bool,
    anti_stub_unmet: bool,
}

fn resolve_path(root: &Path, p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

fn is_voxelized_strict(raw: &str, normalized: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.contains("```")
        && !trimmed.to_ascii_lowercase().contains("here is")
        && normalized.len() <= trimmed.len() + 4
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

fn compile_and_canonicalize_ok(source: &str) -> (bool, bool, u32, Vec<String>) {
    let validation = validate_generated_vox(source, true);
    if !validation.is_valid() {
        let errors = validation
            .errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>();
        return (false, false, errors.len() as u32, errors);
    }
    (
        true,
        validation.canonical_ok,
        u32::from(!validation.canonical_ok),
        Vec::new(),
    )
}

fn condition_output_dir(base: &Path, condition_id: &str) -> PathBuf {
    base.join(condition_id)
}

fn write_jsonl_event(path: &Path, evt: &TaskEvent) -> Result<()> {
    let line = serde_json::to_string(evt)?;
    let mut existing = String::new();
    if path.is_file() {
        existing = std::fs::read_to_string(path).unwrap_or_default();
    }
    existing.push_str(&line);
    existing.push('\n');
    std::fs::write(path, existing)?;
    Ok(())
}

fn percentile_u128(values: &mut [u128], pct: f64) -> u128 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let idx = ((values.len() - 1) as f64 * pct).round() as usize;
    values[idx.min(values.len() - 1)]
}

fn parse_spec(root: &Path, spec_path: &Path) -> Result<ScorecardSpec> {
    let spec_full = resolve_path(root, spec_path);
    let raw = read_utf8_path_capped(&spec_full)?;
    let spec: ScorecardSpec = serde_json::from_str(&raw)
        .with_context(|| format!("parse scorecard spec {}", spec_full.display()))?;
    Ok(spec)
}

fn compile_validator(root: &Path, schema_rel: &str) -> Result<vox_jsonschema_util::Validator> {
    let schema_path = root.join(schema_rel);
    let schema_src = read_utf8_path_capped(&schema_path)?;
    vox_jsonschema_util::compile_validator_from_utf8(&schema_src, &schema_path)
}

pub fn run_verify(root: &Path, spec_path: &Path) -> Result<()> {
    let schema_path = root.join(SCHEMA_REL);
    let spec_full = resolve_path(root, spec_path);
    let spec_src = read_utf8_path_capped(&spec_full)?;
    let spec_val: serde_json::Value = serde_json::from_str(&spec_src)
        .with_context(|| format!("parse {}", spec_full.display()))?;
    let validator = compile_validator(root, SCHEMA_REL)?;
    vox_jsonschema_util::validate(
        &spec_val,
        &validator,
        format!("{} vs {}", spec_full.display(), schema_path.display()),
    )?;
    println!(
        "OK: {} matches {}",
        spec_full.display(),
        schema_path.display()
    );
    Ok(())
}

async fn generate_http(server_url: &str, prompt: &str, max_tokens: u64) -> Result<String> {
    let body = serde_json::json!({
        "prompt": prompt,
        "max_tokens": max_tokens
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/generate", server_url.trim_end_matches('/')))
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST /generate to {server_url}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        anyhow::bail!("inference server returned {status}: {txt}");
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .context("invalid JSON from inference server")?;
    Ok(v.get("text")
        .or_else(|| v.get("response"))
        .or_else(|| v.get("generated_text"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string())
}

async fn evaluate_task(
    root: &Path,
    condition: &ModelCondition,
    task: &BenchmarkTask,
    max_retries: u32,
    max_tokens: u64,
) -> Result<(TaskEvent, String)> {
    let start = Instant::now();
    let mut compile_pass_1 = false;
    let mut compile_pass_n = false;
    let mut canonical_pass = false;
    let mut repair_depth = None;
    let mut hir_error_count = 0_u32;
    let mut attempts = 0_u32;
    let mut raw_surface_best = String::new();
    let mut raw_best = String::new();
    let mut normalized_best = String::new();
    let mut current_prompt = task.prompt.clone();
    let tokens_in = task.prompt.split_whitespace().count();
    let mut time_to_first_valid_ms: Option<u128> = None;
    let mut repair_stalled = false;
    let mut prev_err_sig: Option<u64> = None;

    for attempt in 0..=max_retries {
        attempts = attempt + 1;
        let raw = match condition.mode {
            ConditionMode::FixtureFile => {
                let fixture_rel = condition
                    .fixture_dir
                    .as_ref()
                    .context("fixture mode requires fixture_dir")?;
                let fixture = resolve_path(root, &PathBuf::from(fixture_rel))
                    .join(format!("{}.vox", task.id));
                read_utf8_path_capped(&fixture)
                    .with_context(|| format!("read fixture output {}", fixture.display()))?
            }
            ConditionMode::HttpGenerate => {
                let url = condition
                    .server_url
                    .as_ref()
                    .context("http_generate mode requires server_url")?;
                generate_http(url, &current_prompt, max_tokens).await?
            }
        };
        let normalized = normalize_generated_vox(&raw, OutputSurfaceMode::RawCodeOnly);
        let stripped = normalized.normalized;
        raw_surface_best = raw.clone();
        raw_best = stripped.clone();
        normalized_best = stripped.clone();

        let (compile_ok, canon_ok, err_count, errs) = compile_and_canonicalize_ok(&stripped);
        hir_error_count = err_count;
        if attempt == 0 {
            compile_pass_1 = compile_ok;
        }
        if compile_ok {
            compile_pass_n = true;
            canonical_pass = canon_ok;
            repair_depth = Some(attempt);
            time_to_first_valid_ms = Some(start.elapsed().as_millis());
            break;
        }
        if matches!(condition.mode, ConditionMode::FixtureFile) {
            break;
        }
        if attempt < max_retries {
            let mut sorted_errs = errs.clone();
            sorted_errs.sort();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            sorted_errs.hash(&mut h);
            let sig = h.finish();
            if prev_err_sig == Some(sig) {
                repair_stalled = true;
            }
            prev_err_sig = Some(sig);
            let mut feedback = String::from(
                "\n\nThe previous generation had compiler errors. Regenerate ONLY corrected .vox code.\n",
            );
            for (idx, e) in errs.iter().enumerate() {
                feedback.push_str(&format!("{}. {}\n", idx + 1, e));
            }
            current_prompt.push_str(&feedback);
        }
    }

    let task_success = compile_pass_n
        && task
            .expected_contains
            .iter()
            .all(|needle| raw_best.contains(needle));
    let semantic_task_success = compile_pass_n
        && task
            .semantic_expected_contains
            .iter()
            .all(|needle| raw_best.contains(needle));
    let placeholder_hits = placeholder_marker_hits(&raw_best);
    let trivial_placeholder_output = is_trivial_placeholder_output(&raw_best);
    let construct_richness_score = vox_compiler::ast_eval(&raw_best).coverage_score();
    let anti_stub_pass = placeholder_hits == 0
        && !trivial_placeholder_output
        && construct_richness_score >= ANTI_STUB_MIN_CONSTRUCT_RICHNESS;
    let voxelized_strictness = is_voxelized_strict(&raw_surface_best, &normalized_best);
    let tokens_out = raw_best.split_whitespace().count();
    let event = TaskEvent {
        condition_id: condition.id.clone(),
        backend: condition.backend,
        task_id: task.id.clone(),
        attempts,
        compile_pass_1,
        compile_pass_n,
        repair_depth,
        canonical_pass,
        voxelized_strictness,
        task_success,
        semantic_task_success,
        hir_error_count,
        latency_ms: start.elapsed().as_millis(),
        time_to_first_valid_ms,
        repair_stalled,
        tokens_in,
        tokens_out,
        placeholder_marker_hits: placeholder_hits,
        trivial_placeholder_output,
        construct_richness_score,
        anti_stub_pass,
    };
    Ok((event, raw_best))
}

pub async fn run_execute(root: &Path, spec_path: &Path, out_dir: Option<&Path>) -> Result<()> {
    run_verify(root, spec_path)?;
    let spec = parse_spec(root, spec_path)?;
    let start_utc = chrono::Utc::now();
    let ts = start_utc.format("%Y%m%dT%H%M%SZ").to_string();
    let out = out_dir
        .map(|p| resolve_path(root, p))
        .unwrap_or_else(|| root.join("mens").join("eval").join("runs").join(ts));
    std::fs::create_dir_all(&out)?;

    let max_tokens: u64 = std::env::var("VOX_MENS_SCORECARD_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2048);

    let mut summaries = Vec::<ConditionSummary>::new();
    let mut all_events = Vec::<TaskEvent>::new();

    for cond in &spec.conditions {
        let cond_dir = condition_output_dir(&out, &cond.id);
        std::fs::create_dir_all(&cond_dir)?;
        let events_path = cond_dir.join("events.jsonl");
        let outputs_dir = cond_dir.join("outputs");
        std::fs::create_dir_all(&outputs_dir)?;

        let mut events = Vec::<TaskEvent>::new();
        for task in &spec.tasks {
            let (evt, output) = evaluate_task(root, cond, task, spec.max_retries, max_tokens)
                .await
                .with_context(|| format!("evaluate task {} for {}", task.id, cond.id))?;
            write_jsonl_event(&events_path, &evt)?;
            std::fs::write(outputs_dir.join(format!("{}.vox", task.id)), output)?;
            events.push(evt.clone());
            all_events.push(evt);
        }

        let total = events.len().max(1);
        let compile_1 = events.iter().filter(|e| e.compile_pass_1).count() as f64 / total as f64;
        let compile_n = events.iter().filter(|e| e.compile_pass_n).count() as f64 / total as f64;
        let canon = events.iter().filter(|e| e.canonical_pass).count() as f64 / total as f64;
        let strict = events.iter().filter(|e| e.voxelized_strictness).count() as f64 / total as f64;
        let success = events.iter().filter(|e| e.task_success).count() as f64 / total as f64;
        let semantic_success =
            events.iter().filter(|e| e.semantic_task_success).count() as f64 / total as f64;
        let repair_stall_rate =
            events.iter().filter(|e| e.repair_stalled).count() as f64 / total as f64;
        let repair_depth_vals: Vec<f64> = events
            .iter()
            .filter_map(|e| e.repair_depth.map(|v| v as f64))
            .collect();
        let repair_depth_mean = if repair_depth_vals.is_empty() {
            0.0
        } else {
            repair_depth_vals.iter().sum::<f64>() / repair_depth_vals.len() as f64
        };
        let mut lats: Vec<u128> = events.iter().map(|e| e.latency_ms).collect();
        let mut first_valid: Vec<u128> = events
            .iter()
            .filter_map(|e| e.time_to_first_valid_ms)
            .collect();
        let p50 = percentile_u128(&mut lats.clone(), 0.5);
        let p95 = percentile_u128(&mut lats, 0.95);
        let first_valid_p50 = percentile_u128(&mut first_valid, 0.5);
        let tokens_in_total = events.iter().map(|e| e.tokens_in).sum();
        let tokens_out_total = events.iter().map(|e| e.tokens_out).sum();
        let placeholder_hits_total = events.iter().map(|e| e.placeholder_marker_hits).sum();
        let placeholder_event_rate = events
            .iter()
            .filter(|e| e.placeholder_marker_hits > 0)
            .count() as f64
            / total as f64;
        let trivial_placeholder_rate = events
            .iter()
            .filter(|e| e.trivial_placeholder_output)
            .count() as f64
            / total as f64;
        let construct_richness_mean = events
            .iter()
            .map(|e| e.construct_richness_score)
            .sum::<f64>()
            / total as f64;
        let anti_stub_pass_rate =
            events.iter().filter(|e| e.anti_stub_pass).count() as f64 / total as f64;
        summaries.push(ConditionSummary {
            id: cond.id.clone(),
            label: cond.label.clone(),
            backend: cond.backend,
            model_id: cond.model_id.clone(),
            adapter_revision: cond.adapter_revision.clone(),
            total_tasks: events.len(),
            compile_pass_at_1: compile_1,
            compile_pass_at_n: compile_n,
            canonical_pass: canon,
            voxelized_strictness: strict,
            task_success: success,
            semantic_task_success: semantic_success,
            repair_depth_mean,
            repair_stall_rate,
            time_to_first_valid_p50_ms: first_valid_p50,
            latency_p50_ms: p50,
            latency_p95_ms: p95,
            tokens_in_total,
            tokens_out_total,
            placeholder_hits_total,
            placeholder_event_rate,
            trivial_placeholder_rate,
            construct_richness_mean,
            anti_stub_pass_rate,
        });
    }

    let summary = ScorecardSummary {
        schema: "vox_mens_scorecard_summary_v1".to_string(),
        schema_version: spec.schema_version,
        prompt_pack_version: spec.prompt_pack_version,
        git_sha: option_env!("VOX_GIT_HASH").unwrap_or("unknown").to_string(),
        run_started_utc: start_utc.to_rfc3339(),
        spec_path: resolve_path(root, spec_path).display().to_string(),
        conditions: summaries,
        kpi_contract_alignment: Some(KpiContractAlignment {
            runtime_generation_kpi_schema: "vox_runtime_generation_kpi_v1".to_string(),
            mens_scorecard_event_schema_id:
                "https://voxlang.org/schemas/eval/mens-scorecard-event.schema.json".to_string(),
        }),
        completion_policy: Some(CompletionPolicyCrosswalk {
            policy_contract_rel: super::completion_quality::COMPLETION_POLICY_REL.to_string(),
            gate_integration_note: "Mens anti-stub metrics (placeholder/trivial_placeholder/construct_richness/anti_stub_pass) feed the same governance chain as vox ci completion-audit|gates and contracts/telemetry/completion-*.schema.json.".to_string(),
        }),
    };

    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    std::fs::write(
        out.join("events.all.jsonl"),
        all_events
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n")
            + "\n",
    )?;

    let summary_validator = compile_validator(root, SUMMARY_SCHEMA_REL)?;
    let event_validator = compile_validator(root, EVENT_SCHEMA_REL)?;
    let summary_val = serde_json::to_value(&summary)?;
    vox_jsonschema_util::validate(
        &summary_val,
        &summary_validator,
        format!("summary artifact vs {SUMMARY_SCHEMA_REL}"),
    )?;
    for evt in &all_events {
        let evt_val = serde_json::to_value(evt)?;
        vox_jsonschema_util::validate(
            &evt_val,
            &event_validator,
            format!("event artifact vs {EVENT_SCHEMA_REL}"),
        )?;
    }

    println!("mens-scorecard: wrote {}", out.display());
    Ok(())
}

pub async fn run_ingest_trust(root: &Path, summary_path: &Path) -> Result<()> {
    let full = resolve_path(root, summary_path);
    let raw =
        read_utf8_path_capped(&full).with_context(|| format!("read summary {}", full.display()))?;
    let repository_id = vox_repository::compute_repository_id(root, None);
    let db = vox_db::VoxDb::connect_default()
        .await
        .map_err(|e| anyhow::anyhow!("connect VoxDb: {e}"))?;
    let artifact_ref = full
        .strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| full.display().to_string());
    let n = db
        .ingest_mens_scorecard_summary_json(&raw, &repository_id, &artifact_ref)
        .await
        .map_err(|e| anyhow::anyhow!("ingest trust observations: {e}"))?;
    println!(
        "mens-scorecard ingest-trust: recorded {} observation(s) from {}",
        n,
        full.display()
    );
    Ok(())
}

fn load_summary(path: &Path) -> Result<ScorecardSummary> {
    let raw = read_utf8_path_capped(path)?;
    let v: ScorecardSummary =
        serde_json::from_str(&raw).with_context(|| format!("parse summary {}", path.display()))?;
    Ok(v)
}

pub fn run_decide(root: &Path, summaries: &[PathBuf], json: bool) -> Result<()> {
    let mut merged = Vec::<ConditionSummary>::new();
    for p in summaries {
        let full = resolve_path(root, p);
        let s = load_summary(&full)?;
        merged.extend(s.conditions);
    }
    if merged.is_empty() {
        anyhow::bail!("no condition summaries provided");
    }
    let best = merged
        .iter()
        .max_by(|a, b| a.task_success.total_cmp(&b.task_success))
        .cloned()
        .context("no best condition")?;
    let qlora_like: Vec<&ConditionSummary> = merged
        .iter()
        .filter(|c| c.backend == BackendTag::Qlora)
        .collect();
    let qlora_plateau = if qlora_like.len() >= 2 {
        let mut vals: Vec<f64> = qlora_like.iter().map(|c| c.task_success).collect();
        vals.sort_by(f64::total_cmp);
        vals[vals.len() - 1] - vals[0] < 0.05
    } else {
        false
    };
    let strictness_unmet = best.voxelized_strictness < 0.95 || best.compile_pass_at_1 < 0.9;
    let semantic_unmet = best.semantic_task_success < 0.85;
    let anti_stub_unmet = best.anti_stub_pass_rate < ANTI_STUB_MIN_PASS_RATE
        || best.trivial_placeholder_rate > ANTI_STUB_MAX_TRIVIAL_PLACEHOLDER_RATE
        || best.placeholder_event_rate > ANTI_STUB_MAX_PLACEHOLDER_EVENT_RATE
        || best.construct_richness_mean < ANTI_STUB_MIN_CONSTRUCT_RICHNESS;
    let report = DecisionReport {
        recommended: if qlora_plateau && (strictness_unmet || semantic_unmet || anti_stub_unmet) {
            "invest_in_custom_model_rnd".to_string()
        } else {
            "continue_qlora_first".to_string()
        },
        reason: format!(
            "best={} task_success={:.3} semantic={:.3} compile@1={:.3} strictness={:.3} anti_stub={:.3} placeholder_rate={:.3} trivial_rate={:.3} richness={:.3} ttfv_p50={}ms repair_stall={:.3}",
            best.id,
            best.task_success,
            best.semantic_task_success,
            best.compile_pass_at_1,
            best.voxelized_strictness,
            best.anti_stub_pass_rate,
            best.placeholder_event_rate,
            best.trivial_placeholder_rate,
            best.construct_richness_mean,
            best.time_to_first_valid_p50_ms,
            best.repair_stall_rate
        ),
        qlora_plateau,
        strictness_unmet,
        semantic_unmet,
        anti_stub_unmet,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("decision: {}", report.recommended);
        println!("{}", report.reason);
        println!(
            "gates: qlora_plateau={} strictness_unmet={}",
            report.qlora_plateau, report.strictness_unmet
        );
        println!(
            "semantic_unmet={} anti_stub_unmet={}",
            report.semantic_unmet, report.anti_stub_unmet
        );
    }
    Ok(())
}

pub fn run_burn_rnd(
    root: &Path,
    qlora_summary: &Path,
    burn_summary: Option<&Path>,
    json: bool,
) -> Result<()> {
    let q = load_summary(&resolve_path(root, qlora_summary))?;
    let burn = burn_summary
        .map(|p| load_summary(&resolve_path(root, p)))
        .transpose()?;
    let q_best = q
        .conditions
        .iter()
        .max_by(|a, b| a.task_success.total_cmp(&b.task_success))
        .context("qlora summary had no conditions")?;
    let (recommended, reason) = if let Some(b) = &burn {
        let b_best = b
            .conditions
            .iter()
            .max_by(|a, b| a.task_success.total_cmp(&b.task_success))
            .context("burn summary had no conditions")?;
        if b_best.task_success > q_best.task_success + 0.05
            && b_best.voxelized_strictness >= q_best.voxelized_strictness
        {
            (
                "expand_burn_rnd",
                format!(
                    "burn outperformed qlora by {:.3} task_success with comparable strictness",
                    b_best.task_success - q_best.task_success
                ),
            )
        } else {
            (
                "keep_burn_experimental",
                "burn does not show decisive downstream win over qlora on current scorecard"
                    .to_string(),
            )
        }
    } else {
        (
            "collect_burn_baseline",
            "no burn summary provided; run at least one burn/scratch scorecard condition first"
                .to_string(),
        )
    };
    let out = serde_json::json!({
        "recommended": recommended,
        "reason": reason,
        "qlora_best_task_success": q_best.task_success,
        "qlora_best_strictness": q_best.voxelized_strictness
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "burn-rnd: {}",
            out["recommended"].as_str().unwrap_or("unknown")
        );
        println!("{}", out["reason"].as_str().unwrap_or(""));
    }
    Ok(())
}
