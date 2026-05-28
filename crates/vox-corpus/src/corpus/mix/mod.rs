//! Weighted merge of multiple JSONL corpus files into one training set.
//!
//! ## `record_format: asr_refine`
//!
//! Lines are JSON objects with `noisy_text` and `corrected_text` (see `mens/schemas/asr_refine_pairs.schema.json`).
//! They are rewritten to `vox_tensor::data::TrainingPair`-compatible JSON (`prompt`, `response`, optional `rating`,
//! `category`), with a fixed instruction prefix on `prompt` so Mens LoRA sees a consistent correction task.
//!
//! ## `record_format: tool_trace`
//!
//! Lines are JSON objects matching [`crate::tool_workflow_corpus::ToolTraceRecord`]: tool invocations for SFT.
//! JSON Schema: `mens/schemas/tool_trace_record.schema.json`; example JSONL: `mens/data/tool_traces.example.jsonl`.
//! They become `prompt`/`response` rows with `category` `tool_trace` (use `--context-filter tool_trace` in training
//! to select only these rows).
//!
//! ## `record_format: speech_to_code`
//!
//! Lines are JSON objects with `refined_transcript` (spoken intent) and `vox_code` (validated .vox source), optional
//! `transcript_alternatives`, `repair_metadata`, and `diagnostics_snapshot` (compiler/LSP repair loop). See `mens/schemas/speech_to_code_trace.schema.json`.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use xxhash_rust::xxh3::xxh3_64;

use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;

use crate::tool_workflow_corpus::ToolTraceRecord;

/// Parse JSON string as [`serde_json::Value`], or wrap as a JSON string if parse fails.
fn json_or_string_fragment(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

/// Prepended to `noisy_text` when normalizing `asr_refine` rows.
pub const ASR_REFINE_INSTRUCTION: &str = "Correct the following noisy transcript, preserving intent. Fix phonetic errors, restore punctuation, and normalize code identifiers.\n\n";

/// Instruction prefix for speech-to-code SFT rows (`record_format: speech_to_code`).
pub const SPEECH_TO_CODE_INSTRUCTION: &str = "Given the following spoken request, emit valid Vox source that satisfies it. Preserve identifiers and paths mentioned in the transcript.\n\nTranscript:\n";

/// One JSONL source file and its weight for [`run_mix`].
///
/// ## Weight semantics
///
/// Historically `weight` was applied as a literal line-repeat factor —
/// `weight=6.0` produced 6 physical copies of each row in the output
/// JSONL. That destroys uniqueness (the SFT corpus ended up 99% duplicate
/// lines, see audit at docs/src/architecture/2026-05-22-v1-readiness-snapshot.md)
/// without adding training signal, since the trainer already shuffles
/// and iterates over multiple epochs.
///
/// The current default is **emit each row exactly once** and stamp the
/// source's `weight` onto the row as a `mix_weight` field so downstream
/// samplers (DataLoader / WeightedRandomSampler) can honor the proportion
/// at batch-construction time. Set [`MixSource::physical_repeats`] to
/// `true` to opt in to the legacy duplication behavior.
#[derive(Debug, Deserialize)]
pub struct MixSource {
    /// Path to the JSONL file, relative to the process cwd unless absolute.
    pub path: String,
    /// Sampling weight for this source. Stamped onto each emitted row as
    /// `mix_weight: <weight>` so a downstream weighted sampler can use it.
    /// When [`MixSource::physical_repeats`] is `true` the legacy behavior
    /// (each row emitted `ceil(max(weight,0)).max(1)` times) is restored.
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// When `asr_refine`, parse each line as ASR refinement JSON and emit `prompt`/`response` training rows.
    #[serde(default)]
    pub record_format: Option<String>,
    /// When `true`, silently skip this source if the file does not exist (no warning printed).
    #[serde(default)]
    pub optional: bool,
    /// Probability (0.0 to 1.0) of including a row in the output.
    #[serde(default)]
    pub sample_rate: Option<f64>,
    /// Opt-in to the legacy "weight = literal line repeats" behavior.
    /// Default `false` (each row emitted once, weight stamped on row).
    /// Set `true` only when a downstream consumer doesn't honor
    /// `mix_weight` and you need pre-expanded duplicates. See struct docs.
    #[serde(default)]
    pub physical_repeats: bool,
}

fn default_weight() -> f64 {
    1.0
}

/// YAML shape for a corpus mix job: weighted sources and output path.
#[derive(Debug, Deserialize)]
pub struct MixConfigSchema {
    /// Ordered list of JSONL inputs and weights.
    pub sources: Vec<MixSource>,
    /// Output JSONL path (relative to cwd unless absolute).
    pub output: String,
    /// Optional lane allow-list. When set, only rows in these lanes are emitted.
    #[serde(default)]
    pub include_lanes: Vec<String>,
    /// Optional lane deny-list. Rows in these lanes are skipped.
    #[serde(default)]
    pub exclude_lanes: Vec<String>,
}

impl MixConfigSchema {
    /// Read and validate a mix YAML file from disk.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = vox_bounded_fs::read_utf8_path_capped(path)
            .with_context(|| format!("read mix config {}", path.display()))?;
        let cfg: Self = serde_yaml::from_str(&raw)
            .with_context(|| format!("parse mix YAML {}", path.display()))?;
        anyhow::ensure!(
            !cfg.sources.is_empty(),
            "mix config {}: sources must be non-empty",
            path.display()
        );
        Ok(cfg)
    }
}

/// Options for [`run_mix_with_options`].
#[derive(Debug, Clone)]
pub struct MixRunOptions {
    /// When `true`, fail if a non-optional source is missing, unreadable, or has zero usable lines after normalization.
    pub strict: bool,
    /// When `true`, write a JSON report next to the output (`{output}.mix_report.json`).
    pub write_report: bool,
}

impl Default for MixRunOptions {
    fn default() -> Self {
        Self {
            strict: false,
            write_report: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MixSourceReportRow {
    pub path: String,
    pub weight: f64,
    pub optional: bool,
    pub record_format: Option<String>,
    pub resolved_path: String,
    /// Metadata for incremental skip: mtime and size.
    pub source_fingerprint: String,
    pub input_lines: usize,
    pub repeats: usize,
    pub emitted_lines: usize,
    pub skipped_reason: Option<String>,
    pub share_of_output: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MixRunReport {
    pub config_path: String,
    pub output_path: String,
    pub strict: bool,
    /// Hash of the MixConfigSchema fields and include/exclude lanes.
    pub config_fingerprint: String,
    pub sources: Vec<MixSourceReportRow>,
    pub total_emitted: usize,
}

/// Rewrite one JSONL line for mixing. Pass-through unless `record_format` requests transformation.
pub fn normalize_training_jsonl_line(
    line: &str,
    record_format: Option<&str>,
) -> Result<String, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("empty line".into());
    }
    match record_format.map(str::trim).filter(|s| !s.is_empty()) {
        Some("tool_trace") => {
            let rec: ToolTraceRecord = serde_json::from_str(trimmed)
                .map_err(|e| format!("tool_trace: invalid json: {e}"))?;
            let prompt = format!(
                "[vox_tool_supervision]\nTask:\n{}\n\nRespond with a single JSON object describing the tool call and outcome. \
                 Use keys: tool_name (string), arguments (object), result (any), success (boolean).\n",
                rec.task_prompt.trim()
            );
            let response = if let Some(f) = rec
                .followup_text
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                f.to_string()
            } else {
                let body = serde_json::json!({
                    "tool_name": rec.tool_name,
                    "arguments": json_or_string_fragment(&rec.arguments_json),
                    "result": json_or_string_fragment(&rec.result_json),
                    "success": rec.success,
                });
                body.to_string()
            };
            let mut row = serde_json::Map::new();
            row.insert("prompt".to_string(), serde_json::Value::String(prompt));
            row.insert("response".to_string(), serde_json::Value::String(response));
            row.insert(
                "category".to_string(),
                serde_json::Value::String("tool_trace".into()),
            );
            if let Some(ref sid) = rec.session_id {
                let r = sid.trim();
                if !r.is_empty() {
                    row.insert(
                        "source".to_string(),
                        serde_json::Value::String(format!("tool_trace:{r}")),
                    );
                }
            }
            serde_json::to_string(&serde_json::Value::Object(row)).map_err(|e| e.to_string())
        }
        Some("asr_refine") => {
            let v: serde_json::Value =
                serde_json::from_str(trimmed).map_err(|e| format!("invalid json: {e}"))?;
            if v.get("prompt").and_then(|x| x.as_str()).is_some()
                && v.get("response").and_then(|x| x.as_str()).is_some()
            {
                return Ok(trimmed.to_string());
            }
            let noisy = v
                .get("noisy_text")
                .and_then(|x| x.as_str())
                .ok_or_else(|| "asr_refine: missing noisy_text".to_string())?;
            let corrected = v
                .get("corrected_text")
                .and_then(|x| x.as_str())
                .ok_or_else(|| "asr_refine: missing corrected_text".to_string())?;
            let prompt = format!("{ASR_REFINE_INSTRUCTION}{noisy}");
            let mut row = serde_json::Map::new();
            row.insert("prompt".to_string(), serde_json::Value::String(prompt));
            row.insert(
                "response".to_string(),
                serde_json::Value::String(corrected.to_string()),
            );
            if let Some(c) = v.get("category").filter(|x| !x.is_null()) {
                row.insert("category".to_string(), c.clone());
            } else {
                row.insert(
                    "category".to_string(),
                    serde_json::Value::String("asr_refine".into()),
                );
            }
            if let Some(r) = v.get("rating").filter(|x| !x.is_null()) {
                row.insert("rating".to_string(), r.clone());
            }
            serde_json::to_string(&serde_json::Value::Object(row)).map_err(|e| e.to_string())
        }
        Some("speech_to_code") => {
            let v: serde_json::Value =
                serde_json::from_str(trimmed).map_err(|e| format!("invalid json: {e}"))?;
            if v.get("prompt").and_then(|x| x.as_str()).is_some()
                && v.get("response").and_then(|x| x.as_str()).is_some()
            {
                return Ok(trimmed.to_string());
            }
            let transcript = v
                .get("refined_transcript")
                .or_else(|| v.get("transcript"))
                .and_then(|x| x.as_str())
                .ok_or_else(|| "speech_to_code: missing refined_transcript".to_string())?;
            let code = v
                .get("vox_code")
                .or_else(|| v.get("code"))
                .and_then(|x| x.as_str())
                .ok_or_else(|| "speech_to_code: missing vox_code".to_string())?;
            let mut prompt = format!("{SPEECH_TO_CODE_INSTRUCTION}{transcript}");
            if let Some(serde_json::Value::Array(alts)) = v.get("transcript_alternatives") {
                let joined: Vec<String> = alts
                    .iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect();
                if !joined.is_empty() {
                    prompt.push_str("\n\nAlternatives:\n");
                    for a in joined {
                        prompt.push_str("- ");
                        prompt.push_str(&a);
                        prompt.push('\n');
                    }
                }
            }
            let mut row = serde_json::Map::new();
            row.insert("prompt".to_string(), serde_json::Value::String(prompt));
            row.insert(
                "response".to_string(),
                serde_json::Value::String(code.to_string()),
            );
            if let Some(c) = v.get("category").filter(|x| !x.is_null()) {
                row.insert("category".to_string(), c.clone());
            } else {
                row.insert(
                    "category".to_string(),
                    serde_json::Value::String("speech_to_code".into()),
                );
            }
            if let Some(r) = v.get("rating").filter(|x| !x.is_null()) {
                row.insert("rating".to_string(), r.clone());
            }
            if let Some(ds) = v.get("diagnostics_snapshot").filter(|x| !x.is_null()) {
                row.insert("diagnostics_snapshot".to_string(), ds.clone());
            }
            serde_json::to_string(&serde_json::Value::Object(row)).map_err(|e| e.to_string())
        }
        Some("workflow_trace") => {
            let rec: crate::tool_workflow_corpus::WorkflowTraceRecord =
                serde_json::from_str(trimmed)
                    .map_err(|e| format!("workflow_trace: invalid json: {e}"))?;
            let prompt = format!(
                "[vox_workflow_supervision]\nIntent: {}\n\nExecution Log Excerpt:\n{}\n\nEmit a JSON object with routing_efficiency (0.0-1.0).\n",
                rec.intent.trim(),
                rec.execution_log_excerpt.trim()
            );
            let mut row = serde_json::Map::new();
            row.insert("prompt".to_string(), serde_json::Value::String(prompt));
            let response = serde_json::json!({
                "routing_efficiency": rec.routing_efficiency.unwrap_or(1.0),
            });
            row.insert(
                "response".to_string(),
                serde_json::Value::String(response.to_string()),
            );
            row.insert(
                "category".to_string(),
                serde_json::Value::String("workflow_trace".into()),
            );
            serde_json::to_string(&serde_json::Value::Object(row)).map_err(|e| e.to_string())
        }
        _ => Ok(trimmed.to_string()),
    }
}

fn default_lane_from_row(row: &serde_json::Value) -> &'static str {
    let category = row
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if category.contains("tool_trace") {
        "vox_tooling"
    } else if category.contains("speech") || category.contains("asr_refine") {
        "vox_speech"
    } else if category.contains("documentation") {
        "vox_docs_qa"
    } else {
        "vox_codegen"
    }
}

fn default_response_mode_for_lane(lane: &str) -> &'static str {
    if lane == "vox_docs_qa" {
        "prose_only"
    } else {
        "code_only"
    }
}

fn default_task_family_from_lane(lane: &str) -> &'static str {
    match lane {
        "vox_docs_qa" => "docs_qa",
        "vox_tooling" => "tool_trace",
        "vox_speech" => "speech_to_code",
        _ => "vox_codegen",
    }
}

/// Infer corpus origin from row metadata when not explicitly set.
///
/// Research (Continual Learning §mode-collapse) proves that synthetic data
/// must never *replace* human ground-truth. Every row must carry an explicit
/// `origin` so batches can be validated against `min_human_ratio`.
fn infer_origin(obj: &serde_json::Map<String, serde_json::Value>) -> &'static str {
    if let Some(v) = obj.get("origin").and_then(|x| x.as_str()) {
        // Already set — don't overwrite.
        return match v {
            "human" => "human",
            "synthetic" => "synthetic",
            "agent" => "agent",
            _ => "synthetic", // conservative default
        };
    }
    // Heuristic: rows from extract_rs / extract_vox are human-written crate source.
    if let Some(src) = obj.get("source").and_then(|x| x.as_str()) {
        if src.contains("crates/") || src.ends_with(".rs") || src.ends_with(".vox") {
            return "human";
        }
        if src.starts_with("tool_trace:") || src.starts_with("agent:") {
            return "agent";
        }
    }
    if let Some(cat) = obj.get("category").and_then(|x| x.as_str())
        && (cat == "tool_trace" || cat == "speech_to_code")
    {
        return "agent";
    }
    "synthetic"
}

fn enrich_lane_metadata(line: &str) -> Result<(String, String), String> {
    let mut v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("invalid training row json: {e}"))?;
    let inferred_lane = default_lane_from_row(&v).to_string();
    let obj = v
        .as_object_mut()
        .ok_or_else(|| "training row must be JSON object".to_string())?;
    let existing = obj.get("lane").and_then(|x| x.as_str()).map(str::to_string);
    // Lane override rule: if the row carries an explicit, non-default lane
    // label, trust it. If the row's lane is the generic catch-all
    // (`vox_codegen`) BUT the row's category suggests a more specific lane
    // (e.g. `documentation` → `vox_docs_qa`, `tool_trace` → `vox_tooling`),
    // override with the specific lane. This fixes the upstream-tagging bug
    // where `validated_mixed.jsonl` stamps every row `lane: vox_codegen`
    // regardless of its category, causing `include_lanes` filters in
    // mix.yaml to never emit `vox_docs_qa` / `vox_tooling` rows.
    let lane = match existing {
        Some(ref tag) if tag != "vox_codegen" => tag.clone(),
        Some(_) if inferred_lane != "vox_codegen" => inferred_lane.clone(),
        Some(tag) => tag,
        None => inferred_lane.clone(),
    };
    obj.insert("lane".to_string(), serde_json::Value::String(lane.clone()));
    if !obj.contains_key("response_mode") {
        obj.insert(
            "response_mode".to_string(),
            serde_json::Value::String(default_response_mode_for_lane(&lane).to_string()),
        );
    }
    if !obj.contains_key("task_family") {
        obj.insert(
            "task_family".to_string(),
            serde_json::Value::String(default_task_family_from_lane(&lane).to_string()),
        );
    }
    // Task 2.3.2: ensure every row carries an origin tag for batch validation.
    if !obj.contains_key("origin") {
        let origin = infer_origin(obj);
        obj.insert(
            "origin".to_string(),
            serde_json::Value::String(origin.to_string()),
        );
    }

    // Ensure assistant-token density: every training pair MUST have an assistant response.
    // If using 'messages', the last message MUST be from the 'assistant'.
    if let Some(serde_json::Value::Array(msgs)) = obj.get("messages") {
        if let Some(last) = msgs.last() {
            let role = last.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                // If the last message isn't an assistant turn, try to append 'response'/'output' if it exists
                let resp_val = obj.get("response").or_else(|| obj.get("output"));
                if let Some(resp) = resp_val.and_then(|v| v.as_str()) {
                    let mut msgs = msgs.clone();
                    let mut turn = serde_json::Map::new();
                    turn.insert(
                        "role".to_string(),
                        serde_json::Value::String("assistant".into()),
                    );
                    turn.insert(
                        "content".to_string(),
                        serde_json::Value::String(resp.into()),
                    );
                    msgs.push(serde_json::Value::Object(turn));
                    obj.insert("messages".to_string(), serde_json::Value::Array(msgs));
                } else {
                    return Err("messages array does not end with assistant turn and no response/output field found".into());
                }
            }
        } else {
            return Err("messages array is empty".into());
        }
    } else {
        let has_resp = obj
            .get("response")
            .or_else(|| obj.get("output"))
            .and_then(|v| v.as_str())
            .is_some();
        if !has_resp {
            return Err("neither messages nor response/output field present".into());
        }
    }

    serde_json::to_string(&v)
        .map(|s| (s, lane))
        .map_err(|e| e.to_string())
}

/// Stamp the source's `weight` onto a normalized training row as a top-level
/// `mix_weight: f64` field so a downstream weighted sampler can honor the
/// requested source-mix proportions without us physically duplicating rows.
///
/// No-op if `weight == 1.0` (the default; keeps the JSONL diff minimal for
/// the common single-source case). Existing `mix_weight` is preserved if
/// already set — caller intent wins over mix-time defaults.
fn stamp_mix_weight(line: &str, weight: f64) -> Result<String, String> {
    if (weight - 1.0).abs() < f64::EPSILON {
        return Ok(line.to_string());
    }
    let mut v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("invalid training row json: {e}"))?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| "training row must be JSON object".to_string())?;
    if !obj.contains_key("mix_weight") {
        obj.insert(
            "mix_weight".to_string(),
            serde_json::Number::from_f64(weight)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

/// Calculate a fingerprint for a source file based on path, mtime, and size.
fn calculate_file_fingerprint(p: &Path) -> String {
    let Ok(meta) = std::fs::metadata(p) else {
        return "missing".into();
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let size = meta.len();
    format!(
        "{:x}",
        xxh3_64(format!("{}:{mtime}:{size}", p.display()).as_bytes())
    )
}

/// Calculate a composite fingerprint for the mix configuration variables.
fn calculate_config_fingerprint(cfg: &MixConfigSchema) -> String {
    let mut s = String::new();
    s.push_str(&cfg.output);
    for lane in &cfg.include_lanes {
        s.push_str(lane);
    }
    for lane in &cfg.exclude_lanes {
        s.push_str(lane);
    }
    for src in &cfg.sources {
        s.push_str(&src.path);
        s.push_str(&src.weight.to_string());
        if let Some(f) = &src.record_format {
            s.push_str(f);
        }
        if let Some(sr) = &src.sample_rate {
            s.push_str(&sr.to_string());
        }
        s.push_str(&src.optional.to_string());
        // Include physical_repeats so toggling weight semantics invalidates
        // the incremental-skip cache (otherwise the stale 6× duplicated
        // train_mixed.jsonl would be reused even after switching to
        // emit-once mode).
        s.push_str(&src.physical_repeats.to_string());
    }
    format!("{:x}", xxh3_64(s.as_bytes()))
}

/// Same as [`run_mix_with_options`] with [`MixRunOptions::default`] (lenient; writes report).
///
/// Resolves relative `output` / source paths in the YAML against [`std::env::current_dir`].
pub fn run_mix(config_path: &Path) -> anyhow::Result<()> {
    run_mix_with_options(config_path, None, MixRunOptions::default())
}

/// Concatenate sources in order, repeating each file's lines proportional to `weight` (rounded up to one copy minimum).
///
/// Relative paths in the mix YAML are resolved against `path_base` when set (e.g. Cargo workspace root),
/// otherwise against [`std::env::current_dir`].
pub fn run_mix_with_options(
    config_path: &Path,
    path_base: Option<&Path>,
    options: MixRunOptions,
) -> anyhow::Result<()> {
    let cfg = MixConfigSchema::load(config_path)?;
    let base_buf = path_base.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())
    });
    let cwd = base_buf.as_path();
    let out_path = cwd.join(&cfg.output);

    let MixRunOptions {
        strict,
        write_report,
    } = options;

    let config_fp = calculate_config_fingerprint(&cfg);

    // Incremental skip check
    if write_report {
        let report_name = out_path
            .file_stem()
            .map(|s| format!("{}.mix_report.json", s.to_string_lossy()))
            .unwrap_or_else(|| "mix_report.json".into());
        let report_path = out_path.with_file_name(report_name);
        if report_path.is_file()
            && out_path.is_file()
            && let Ok(raw) = std::fs::read_to_string(&report_path)
            && let Ok(report) = serde_json::from_str::<MixRunReport>(&raw)
            && report.config_fingerprint == config_fp
        {
            let mut all_match = true;
            for src_report in &report.sources {
                let p = cwd.join(&src_report.path);
                if calculate_file_fingerprint(&p) != src_report.source_fingerprint {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                tracing::info!("  [mix] Incremental skip: {} is fresh", out_path.display());
                return Ok(());
            }
        }
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut total_out = 0usize;
    let include_lanes: Arc<HashSet<String>> = Arc::new(if cfg.include_lanes.is_empty() {
        HashSet::from(["vox_codegen".to_string()])
    } else {
        cfg.include_lanes.iter().cloned().collect()
    });
    let exclude_lanes: Arc<HashSet<String>> = Arc::new(cfg.exclude_lanes.iter().cloned().collect());
    let lane_counts = Arc::new(Mutex::new(
        std::collections::BTreeMap::<String, usize>::new(),
    ));

    let out_file = File::create(&out_path)
        .with_context(|| format!("create mix output {}", out_path.display()))?;
    let out_file = Arc::new(Mutex::new(std::io::BufWriter::new(out_file)));

    let mut report_rows: Vec<MixSourceReportRow> = Vec::with_capacity(cfg.sources.len());

    for src in &cfg.sources {
        let p = cwd.join(&src.path);
        let src_fp = calculate_file_fingerprint(&p);

        if !p.is_file() || src.weight <= 0.0 {
            let mut row = MixSourceReportRow {
                path: src.path.clone(),
                weight: src.weight,
                optional: src.optional,
                record_format: src.record_format.clone(),
                resolved_path: p.display().to_string(),
                source_fingerprint: src_fp,
                input_lines: 0,
                repeats: 0,
                emitted_lines: 0,
                skipped_reason: None,
                share_of_output: 0.0,
            };
            if !p.is_file() {
                if src.optional {
                    row.skipped_reason = Some("missing_file_optional".into());
                } else {
                    row.skipped_reason = Some("missing_file".into());
                    if !strict {
                        eprintln!("  [mix] ⚠ Missing required source: {}", p.display());
                    } else {
                        anyhow::bail!("[mix] strict: required source missing: {}", p.display());
                    }
                }
            } else {
                row.skipped_reason = Some("weight_zero".into());
            }
            report_rows.push(row);
            continue;
        }

        // Repeat semantics: default `physical_repeats = false` → emit once,
        // stamp `mix_weight` on the row for downstream samplers. Set
        // `physical_repeats: true` per-source to restore the legacy
        // duplication behavior. See `MixSource` docs for the rationale.
        let repeats = if src.physical_repeats {
            (src.weight.max(0.0)).ceil().max(1.0) as usize
        } else {
            1
        };
        let row_weight = src.weight.max(0.0);
        let sample_rate = src.sample_rate.unwrap_or(1.0).clamp(0.0, 1.0);

        // Use a persistent reader for the source
        let file = File::open(&p).with_context(|| format!("open {}", p.display()))?;
        let reader = BufReader::new(file);

        let mut emitted_this_src = 0usize;
        let mut input_lines_count = 0usize;

        // Process in chunks to balance parallelism vs memory
        let chunk_size = 10_000;
        let mut lines_iter = reader.lines();

        loop {
            let mut chunk = Vec::with_capacity(chunk_size);
            for _ in 0..chunk_size {
                if let Some(Ok(l)) = lines_iter.next() {
                    chunk.push(l);
                } else {
                    break;
                }
            }
            if chunk.is_empty() {
                break;
            }

            input_lines_count += chunk.len();

            // Parallel normalization and filtering
            let record_format = src.record_format.clone();
            let include_lanes = Arc::clone(&include_lanes);
            let exclude_lanes = Arc::clone(&exclude_lanes);
            let lane_counts = Arc::clone(&lane_counts);

            let processed_chunk: Vec<String> = chunk
                .into_par_iter()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        return None;
                    }

                    // Sampling logic
                    if sample_rate < 1.0 {
                        let mut rng = rand::thread_rng();
                        use rand::Rng;
                        if !rng.gen_bool(sample_rate) {
                            return None;
                        }
                    }

                    let normalized =
                        match normalize_training_jsonl_line(trimmed, record_format.as_deref()) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("  [mix] skip line error: {e}");
                                return None;
                            }
                        };
                    let (normalized, lane) = match enrich_lane_metadata(&normalized) {
                        Ok(pair) => pair,
                        Err(e) => {
                            eprintln!("  [mix] skip line meta error: {e}");
                            return None;
                        }
                    };
                    if !include_lanes.contains(&lane) || exclude_lanes.contains(&lane) {
                        return None;
                    }

                    // Stamp `mix_weight` so a downstream WeightedRandomSampler
                    // can honor source-mix proportions without us inflating
                    // the JSONL via line duplication.
                    let normalized = match stamp_mix_weight(&normalized, row_weight) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("  [mix] skip line stamp error: {e}");
                            return None;
                        }
                    };

                    let mut counts = lane_counts.lock().unwrap();
                    *counts.entry(lane).or_insert(0) += 1;

                    Some(normalized)
                })
                .collect();

            if !processed_chunk.is_empty() {
                let mut out = out_file.lock().unwrap();
                for _ in 0..repeats {
                    for row in &processed_chunk {
                        writeln!(out, "{row}")?;
                        total_out += 1;
                        emitted_this_src += 1;
                    }
                }
            }
        }

        report_rows.push(MixSourceReportRow {
            path: src.path.clone(),
            weight: src.weight,
            optional: src.optional,
            record_format: src.record_format.clone(),
            resolved_path: p.display().to_string(),
            source_fingerprint: src_fp,
            input_lines: input_lines_count,
            repeats,
            emitted_lines: emitted_this_src,
            skipped_reason: if emitted_this_src == 0 && input_lines_count > 0 {
                Some("no_lines_passed_filters".into())
            } else if input_lines_count == 0 {
                Some("empty_file".into())
            } else {
                None
            },
            share_of_output: 0.0,
        });
    }

    // Flush the writer
    {
        let mut out = out_file.lock().unwrap();
        out.flush()?;
    }

    if strict {
        if total_out == 0 {
            anyhow::bail!("[mix] strict: mixed output would be empty (check sources and weights)");
        }
        for row in &report_rows {
            if row.optional
                || row.skipped_reason.as_deref() == Some("weight_zero")
                || row.skipped_reason.as_deref() == Some("missing_file_optional")
            {
                continue;
            }
            if row.emitted_lines == 0 {
                anyhow::bail!(
                    "[mix] strict: required source {:?} contributed zero rows after mix (input_lines={}, reason={:?})",
                    row.path,
                    row.input_lines,
                    row.skipped_reason
                );
            }
        }
    }

    for row in report_rows.iter_mut() {
        row.share_of_output = if total_out > 0 {
            row.emitted_lines as f64 / total_out as f64
        } else {
            0.0
        };
    }

    if write_report {
        let report_name = out_path
            .file_stem()
            .map(|s| format!("{}.mix_report.json", s.to_string_lossy()))
            .unwrap_or_else(|| "mix_report.json".into());
        let report_path = out_path.with_file_name(report_name);
        let report = MixRunReport {
            config_path: config_path.display().to_string(),
            output_path: out_path.display().to_string(),
            strict,
            config_fingerprint: config_fp,
            sources: report_rows,
            total_emitted: total_out,
        };
        std::fs::write(
            &report_path,
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()),
        )
        .with_context(|| format!("write mix report {}", report_path.display()))?;
        eprintln!("  [mix] report → {}", report_path.display());
    }

    let lane_counts = lane_counts.lock().unwrap();
    if !lane_counts.is_empty() {
        eprintln!("  [mix] lane distribution: {:?}", *lane_counts);
    }
    eprintln!("  [mix] wrote {} lines → {}", total_out, out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests;
