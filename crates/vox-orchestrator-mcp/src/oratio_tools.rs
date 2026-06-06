//! MCP tools for Vox Oratio (Candle Whisper STT).

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::llm_bridge::{McpInferRouting, mcp_infer_completion};
use crate::server_state::ServerState;
use crate::text_normalization::{
    llm_changes_well_formed, llm_confidence_field_ok, protected_tokens_preserved,
    strip_json_codeblock_fence, validate_llm_surface,
};
use crate::workspace_path;

fn resolve_audio_path(state: &ServerState, path: &str) -> PathBuf {
    workspace_path::resolve_under_repository_root(state, path)
}

/// Transcribe an audio file via the `oratio` plugin + deterministic refinement.
/// Falls back to `vox_speech::transcribe_path_detailed` for .txt/.md files.
pub(crate) fn transcribe_path_via_plugin(
    path: &std::path::Path,
    ctx: &vox_speech::refine::CorrectionContext,
    language_hint: Option<&str>,
) -> anyhow::Result<vox_speech::TranscribeDetail> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Text/markdown passthrough: vox-speech handles these without candle.
    if matches!(ext.as_str(), "txt" | "md") {
        return vox_speech::transcribe_path_detailed(path, ctx, language_hint)
            .map_err(|e| anyhow::anyhow!("{e}"));
    }

    let plugin = vox_plugin_host::cached_code_plugin("oratio")
        .map_err(|e| anyhow::anyhow!("oratio plugin load: {e}"))?;
    let stt = plugin
        .plugin
        .as_speech_to_text()
        .into_option()
        .ok_or_else(|| anyhow::anyhow!("oratio plugin missing SpeechToText accessor"))?;

    let path_str = path.to_string_lossy().to_string();
    let config_json = serde_json::json!({ "language": language_hint }).to_string();

    let transcription_json = stt
        .transcribe_path(path_str.as_str().into(), config_json.as_str().into())
        .into_result()
        .map_err(|e| anyhow::anyhow!("transcribe_path plugin: {e}"))?;

    let v: serde_json::Value = serde_json::from_str(transcription_json.as_str())
        .map_err(|e| anyhow::anyhow!("plugin returned invalid JSON: {e}"))?;
    let raw_text = v
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(vox_speech::refine_raw_text(&raw_text, ctx))
}

pub(crate) fn parse_profile(args: &Value) -> vox_speech::refine::OratioCorrectionProfile {
    match args
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or("balanced")
    {
        "conservative" => vox_speech::refine::OratioCorrectionProfile::Conservative,
        "aggressive" => vox_speech::refine::OratioCorrectionProfile::Aggressive,
        _ => vox_speech::refine::OratioCorrectionProfile::Balanced,
    }
}

pub(crate) fn parse_route_mode(args: &Value) -> vox_speech::RouteMode {
    match args
        .get("route_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("none")
    {
        "tool" => vox_speech::RouteMode::Tool,
        "chat" => vox_speech::RouteMode::Chat,
        "orchestrator" => vox_speech::RouteMode::Orchestrator,
        _ => vox_speech::RouteMode::None,
    }
}

/// `vox_oratio_transcribe`: thin STT + deterministic refine only (no session envelope, no routing).
pub fn transcribe(state: &ServerState, args: Value) -> anyhow::Result<String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing string field `path`"))?;
    let full = resolve_audio_path(state, path);
    let language_hint = args
        .get("language_hint")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned);
    let debug_parser_payload = args
        .get("debug_parser_payload")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let profile = parse_profile(&args);
    let rtc = vox_speech::OratioRuntimeConfig::resolve();
    let ctx =
        vox_speech::refine::CorrectionContext::from_runtime(&rtc, profile, debug_parser_payload);
    let detail = transcribe_path_via_plugin(&full, &ctx, language_hint.as_deref())?;
    let correlation_id = vox_speech::trace::new_correlation_id();
    let mut out = json!({
        "path": full,
        "correlation_id": correlation_id,
        "raw_text": detail.raw_text,
        "refined_text": detail.refined_text,
        "text": detail.refined_text,
        "confidence": detail.confidence,
        "clarification_recommended": vox_speech::clarification_recommended(
            detail.confidence,
            rtc.routing.tool_route_min_confidence,
        ),
    });
    if debug_parser_payload {
        out["correction_trace"] = json!(detail.correction_trace);
        out["runtime_config"] = vox_speech::runtime_config_diagnostic_json(&rtc);
    }
    if let Some(ref nb) = detail.n_best {
        out["n_best"] = json!(nb);
    }
    Ok(serde_json::to_string(&out)?)
}

async fn maybe_llm_polish(
    state: &ServerState,
    session: &vox_speech::OratioSessionResult,
    route_mode: vox_speech::RouteMode,
    llm_refinement: bool,
    llm_min_det_confidence: f32,
    llm_max_output_tokens: u64,
    _runtime: &vox_speech::OratioRuntimeConfig,
) -> Value {
    if !llm_refinement {
        return json!({
            "applied": false,
            "skipped": true,
            "reason": "llm_refinement_disabled",
        });
    }
    let route_requires_clarity = matches!(
        route_mode,
        vox_speech::RouteMode::Tool | vox_speech::RouteMode::Orchestrator
    );
    let entropy_high = session.correction_trace.len() > 8;
    let should_llm =
        session.confidence < llm_min_det_confidence || entropy_high || route_requires_clarity;
    if !should_llm {
        return json!({
            "skipped": true,
            "reason": "deterministic_confidence_sufficient",
            "deterministic_confidence": session.confidence,
        });
    }

    let user_prompt = vox_speech::refine::llm_correction_prompt::build_llm_correction_prompt(
        &session.raw_text,
        &session.text,
        session.confidence,
    );
    let system_prompt = vox_speech::refine::llm_correction_prompt::llm_system_prompt();

    let resolution_template = crate::llm_bridge::McpChatModelResolution {
        complexity: 1,
        ..Default::default()
    };
    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => {
            tracing::warn!(target: "vox_mcp_oratio", stage = "llm_pass", "mcp_chat_model_override: {e}");
            return json!({
                "applied": false,
                "reason": "model_pref_lock_failed",
                "error": format!("{e}"),
            });
        }
    };
    let (model, free_only) = match crate::chat_model_resolve::resolve_chat_llm_model(
        state,
        &user_prompt,
        resolution_template.clone(),
        Some(session.session_id.as_str()),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(target: "vox_mcp_oratio", stage = "llm_pass", "llm model resolve: {e}");
            return json!({
                "applied": false,
                "reason": "model_resolve_failed",
                "error": format!("{e}"),
            });
        }
    };

    let routing = McpInferRouting {
        user_prompt: &user_prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template,
        free_only,
        allow_cloud_ollama_fallback: true,
        selection_rationale: None,
        user_id: Some(session.session_id.as_str()),
    };

    let raw = match mcp_infer_completion(
        state,
        model,
        "vox_oratio_listen",
        system_prompt,
        &routing,
        llm_max_output_tokens.max(64),
        0.1,
        None,
        None,
        true,
        None,
    )
    .await
    {
        Ok((t, _, _)) => t,
        Err(e) => {
            tracing::warn!(target: "vox_mcp_oratio", stage = "llm_pass", "llm infer: {e}");
            return json!({
                "applied": false,
                "reason": "infer_failed",
                "error": format!("{e}"),
            });
        }
    };

    let trimmed = strip_json_codeblock_fence(&raw);
    let parsed: Value = match serde_json::from_str(&trimmed) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "vox_mcp_oratio", stage = "llm_pass", "llm json parse: {e}");
            let preview: String = trimmed.chars().take(512).collect();
            return json!({
                "applied": false,
                "reason": "parse_failed",
                "error": format!("{e}"),
                "raw_preview": preview,
            });
        }
    };

    if !llm_changes_well_formed(&parsed) {
        return json!({
            "applied": false,
            "reason": "changes_schema_invalid",
        });
    }
    if !llm_confidence_field_ok(&parsed) {
        return json!({
            "applied": false,
            "reason": "confidence_out_of_range",
        });
    }

    if parsed
        .get("keep_original")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return json!({"applied": false, "keep_original": true, "llm": parsed});
    }

    let corrected = match parsed.get("corrected_text").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            return json!({"applied": false, "reason": "missing_corrected_text", "llm": parsed});
        }
    };

    if !validate_llm_surface(&session.text, &corrected) {
        return json!({
            "applied": false,
            "reason": "surface_validation_failed",
        });
    }
    if !protected_tokens_preserved(&session.text, &corrected) {
        return json!({
            "applied": false,
            "reason": "protected_token_not_preserved",
        });
    }

    let conf = parsed
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|x| x as f32)
        .unwrap_or(session.confidence);

    json!({
        "applied": true,
        "llm": parsed,
        "effective_confidence": conf,
        "corrected_text": corrected,
    })
}

/// `vox_oratio_listen`: session envelope, routing, optional LLM polish (gated + validated).
pub async fn listen(state: &ServerState, args: Value) -> anyhow::Result<String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing string field `path`"))?;
    let full = resolve_audio_path(state, path);
    let rtc = vox_speech::OratioRuntimeConfig::resolve();

    let timeout_ms = args
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(rtc.session_timing.capture_timeout_ms);
    let max_duration_ms = args
        .get("max_duration_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(rtc.session_timing.max_duration_ms);
    let inference_deadline_ms = args.get("inference_deadline_ms").and_then(|v| v.as_u64());
    let heartbeat_ms = args
        .get("heartbeat_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(rtc.session_timing.heartbeat_ms);

    let language_hint = args
        .get("language_hint")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned);
    let debug_parser_payload = args
        .get("debug_parser_payload")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let profile = parse_profile(&args);
    let route_mode = parse_route_mode(&args);
    let llm_refinement = args
        .get("llm_refinement")
        .and_then(|v| v.as_bool())
        .unwrap_or(rtc.llm.llm_refinement_default);
    let llm_min_det_confidence = args
        .get("llm_min_det_confidence")
        .and_then(|v| v.as_f64())
        .map(|x| x as f32)
        .unwrap_or(rtc.llm.llm_min_det_confidence);
    let llm_max_output_tokens = args
        .get("llm_max_output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(rtc.llm.llm_max_output_tokens);

    let cfg = vox_speech::OratioSessionConfig {
        timeout_ms,
        max_duration_ms,
        inference_deadline_ms,
        language_hint: language_hint.clone(),
        correction_profile: profile,
        debug_parser_payload,
        heartbeat_ms,
        session_id: args
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
    };

    let full_for_block = full.clone();
    let rtc_block = rtc.clone();
    let mut session = tokio::task::spawn_blocking(move || {
        vox_speech::transcribe_path_session_with_runtime(&full_for_block, &cfg, &rtc_block)
    })
    .await
    .map_err(|e| anyhow::anyhow!("join: {e}"))??;

    let llm_block = maybe_llm_polish(
        state,
        &session,
        route_mode,
        llm_refinement,
        llm_min_det_confidence,
        llm_max_output_tokens,
        &rtc,
    )
    .await;

    if let Some(applied) = llm_block.get("applied").and_then(|v| v.as_bool()) {
        if applied {
            if let Some(t) = llm_block.get("corrected_text").and_then(|v| v.as_str()) {
                session.refined_text = t.to_string();
                session.text = t.to_string();
            }
            if let Some(c) = llm_block
                .get("effective_confidence")
                .and_then(|v| v.as_f64())
                .map(|x| x as f32)
            {
                session.confidence = c;
            }
        }
    }

    let route = vox_speech::route_transcript_with_options(
        route_mode,
        &session.session_id,
        &session.text,
        session.confidence,
        &rtc,
        &vox_speech::routing::IdeContext::default(),
    );
    if let Some(asr_path) = args.get("emit_asr_refine_path").and_then(|v| v.as_str()) {
        let out = resolve_audio_path(state, asr_path);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&out)?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&json!({
                "noisy_text": session.raw_text,
                "corrected_text": session.refined_text,
            }))?
        )?;
    }

    let correlation_id = vox_speech::trace::new_correlation_id();
    let intent_envelope =
        if matches!(route.mode, vox_speech::RouteMode::Tool) && route.action != "none" {
            let intent_confidence = route
                .payload
                .get("intent_confidence")
                .and_then(|v| v.as_f64())
                .map(|x| x as f32)
                .unwrap_or(0.0);
            let env = vox_speech::build_intent_envelope(
                &route.action,
                &session.text,
                intent_confidence,
                session.confidence,
                None,
            );
            let gaps = vox_speech::missing_slot_ids(&env);
            let slot_hint = vox_speech::clarification_prompt_for_slots(&env).map(str::to_string);
            Some((env, slot_hint, gaps))
        } else {
            None
        };

    let mut response = json!({
        "correlation_id": correlation_id,
        "session": session,
        "route": route,
        "clarification_recommended": vox_speech::clarification_recommended(
            session.confidence,
            rtc.routing.tool_route_min_confidence,
        ),
    });
    if let Some(nb) = &session.n_best {
        response["n_best"] = json!(nb);
    }
    if let Some((env, slot_hint, gaps)) = intent_envelope {
        response["intent_envelope"] = serde_json::to_value(&env)?;
        response["speech_escalation_recommended"] =
            json!(vox_speech::speech_escalation_recommended(
                env.intent_confidence,
                env.transcript_confidence
            ));
        if !gaps.is_empty() {
            response["intent_slot_gaps"] = json!(gaps);
        }
        if let Some(h) = slot_hint {
            response["slot_clarification"] = json!(h);
        }
    }
    response["llm_refinement"] = llm_block;
    if debug_parser_payload {
        response["runtime_config"] = vox_speech::runtime_config_diagnostic_json(&rtc);
    }
    Ok(serde_json::to_string(&response)?)
}

fn stream_ws_url() -> String {
    let host = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxDashHost)
        .expose()
        .map(String::from)
        .unwrap_or_else(|| "127.0.0.1".into());
    let port = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxDashPort)
        .expose()
        .map(String::from)
        .unwrap_or_else(|| "3847".into());
    format!("ws://{host}:{port}/api/audio/transcribe/stream")
}

/// `vox_oratio_status`: static line + Candle backend JSON (model env defaults).
pub fn status() -> String {
    serde_json::to_string(&json!({
        "summary": vox_speech::transcript_status(),
        "candle": vox_speech::candle_backend_status_json(),
        "runtime": vox_speech::runtime_config_diagnostic_json(&vox_speech::OratioRuntimeConfig::resolve()),
        "streaming": {
            "transport": "websocket",
            "stream_ws_url": stream_ws_url(),
            "input_format": "pcm_s16le_mono_16khz",
            "control_ops": ["set_language", "commit", "cancel"],
        }
    }))
    .unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string())
}
