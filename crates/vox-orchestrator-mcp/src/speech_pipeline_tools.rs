//! End-to-end speech-to-code orchestration: transcribe (optional) then `vox_generate_code` with shared session/correlation metadata.

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::compiler_tools;
use crate::oratio_tools::{parse_profile, parse_route_mode};
use crate::server_state::ServerState;
use crate::workspace_path;

fn resolve_audio_path(state: &ServerState, path: &str) -> PathBuf {
    workspace_path::resolve_under_repository_root(state, path)
}

fn normalize_speech_trace_failure_category(raw: &str) -> String {
    match raw {
        "acoustic" | "lexical" | "syntactic" | "semantic" | "orchestration" | "unknown" => {
            raw.to_string()
        }
        // KPI / repair taxonomy → speech_trace.schema.json enum
        "surface_contract" => "syntactic".to_string(),
        "repair_stall" => "orchestration".to_string(),
        _ => "unknown".to_string(),
    }
}

/// `vox_speech_to_code`: exactly one of `path` (workspace-relative audio/transcript) or `prompt` (plain text).
/// Runs Oratio when `path` is set, then invokes the same codegen path as `vox_generate_code`.
/// Optionally emits one line of JSONL matching `speech_trace.schema.json` core fields to `emit_trace_path`.
pub async fn speech_to_code(state: &ServerState, args: Value) -> anyhow::Result<String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let prompt_in = args
        .get("prompt")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let correlation = vox_speech::trace::new_correlation_id();
    let session_trace = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| correlation.clone());

    let language_hint = args
        .get("language_hint")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned);

    let (raw_text, refined_text, confidence, n_best, from_path) = match (path, prompt_in) {
        (None, None) => {
            anyhow::bail!("provide exactly one of `path` or `prompt`");
        }
        (Some(_), Some(_)) => {
            anyhow::bail!("provide only one of `path` or `prompt`, not both");
        }
        (Some(p), None) => {
            let full = resolve_audio_path(state, p);
            let rtc = vox_speech::OratioRuntimeConfig::resolve();
            let debug_parser = args
                .get("debug_parser_payload")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let ctx = vox_speech::refine::CorrectionContext::from_runtime(
                &rtc,
                parse_profile(&args),
                debug_parser,
            );
            let detail = super::oratio_tools::transcribe_path_via_plugin(
                &full,
                &ctx,
                language_hint.as_deref(),
            )?;
            (
                detail.raw_text,
                detail.refined_text,
                detail.confidence,
                detail.n_best,
                true,
            )
        }
        (None, Some(pr)) => (
            pr.to_string(),
            pr.to_string(),
            1.0_f32,
            None::<Vec<String>>,
            false,
        ),
    };

    let rtc = vox_speech::OratioRuntimeConfig::resolve();
    let route = if from_path
        && args
            .get("include_route")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    {
        Some(vox_speech::route_transcript_with_options(
            parse_route_mode(&args),
            session_trace.as_str(),
            refined_text.as_str(),
            confidence,
            &rtc,
            &vox_speech::routing::IdeContext::default(),
        ))
    } else {
        None
    };

    let mut gen_map = serde_json::Map::new();
    gen_map.insert("prompt".to_string(), Value::String(refined_text.clone()));
    if let Some(v) = args.get("validate") {
        gen_map.insert("validate".to_string(), v.clone());
    }
    if let Some(v) = args.get("max_retries") {
        gen_map.insert("max_retries".to_string(), v.clone());
    }
    if let Some(v) = args.get("session_id") {
        gen_map.insert("session_id".to_string(), v.clone());
    }
    if let Some(v) = args.get("output_surface_mode") {
        gen_map.insert("output_surface_mode".to_string(), v.clone());
    }

    let gen_json = compiler_tools::generate_vox_code(state, Value::Object(gen_map.clone())).await;
    let gen_parsed: Value =
        serde_json::from_str(&gen_json).unwrap_or_else(|_| json!({ "raw": gen_json }));

    let compile_ok = gen_parsed
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let failure_category = if compile_ok {
        None
    } else if from_path && confidence < rtc.routing.tool_route_min_confidence {
        Some("acoustic")
    } else {
        Some("orchestration")
    };

    let repair_attempts = gen_parsed
        .pointer("/meta/runtime_generation_kpi/attempts")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            gen_parsed
                .pointer("/meta/repair/attempts")
                .and_then(|v| v.as_u64())
        })
        .map(|u| u as i64)
        .unwrap_or(0);

    let kpi_failure = gen_parsed
        .pointer("/meta/runtime_generation_kpi/failure_category")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let diagnostics_snapshot = gen_parsed
        .pointer("/meta/diagnostics_snapshot")
        .cloned()
        .filter(|v| !v.is_null());

    let trace_failure = if compile_ok {
        None
    } else {
        let merged = kpi_failure
            .or_else(|| failure_category.map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        Some(normalize_speech_trace_failure_category(&merged))
    };

    let mut trace = json!({
        "schema_version": "1",
        "session_id": session_trace,
        "correlation_id": correlation,
        "raw_transcript": raw_text,
        "refined_transcript": refined_text,
        "transcript_alternatives": n_best,
        "intent_action": route.as_ref().map(|r| r.action.clone()),
        "repair_attempts": repair_attempts,
        "compile_ok": compile_ok,
    });
    if let Some(ds) = diagnostics_snapshot {
        trace["diagnostics_snapshot"] = ds;
    }
    if let Some(fc) = trace_failure {
        trace["failure_category"] = json!(fc);
    }
    if let Some(evidence) = gen_parsed.get("meta") {
        trace["codegen_meta"] = evidence.clone();
    }

    if let Some(out) = args
        .get("emit_trace_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let dest = workspace_path::resolve_under_repository_root(state, out);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&dest)?;
        writeln!(f, "{}", trace)?;
    }

    let mut response = json!({
        "correlation_id": correlation,
        "transcript": {
            "raw_text": raw_text,
            "refined_text": refined_text,
            "confidence": confidence,
            "n_best": n_best,
        },
        "route": route,
        "generate": gen_parsed,
        "speech_trace": trace,
    });

    if !compile_ok {
        response["note"] =
            json!("Codegen failed; see generate.error and speech_trace.failure_category");
    }

    Ok(serde_json::to_string(&response)?)
}
