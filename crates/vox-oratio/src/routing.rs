//! Two-way routing for transcript-driven loops (tool / chat / orchestrator).
//!
//! Uses deterministic intent classification with optional confidence gating aligned to
//! [`crate::runtime_config::OratioRuntimeConfig`].

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::runtime_config::OratioRuntimeConfig;

/// IDE context for grounding speech commands in the current workspace state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdeContext {
    /// Path to the currently active file in the editor.
    pub active_file: Option<String>,
    /// Line number where the cursor is currently located (1-indexed).
    pub cursor_line: Option<usize>,
    /// Recent build or lint errors relevant to the current session.
    pub recent_errors: Vec<String>,
    /// Hierarchical symbol stack (e.g. ["fn_name", "struct_name", "mod_name"]).
    pub symbol_stack: Vec<String>,
}

/// Classified intent for tool routing (deterministic scaffold; LLM disambiguation is out-of-crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntentKind {
    None,
    OratioStatus,
    CodeCreate,
    CodeEdit,
    ExplainCode,
    RunCheck,
}

/// Alphanumeric / underscore tokens (hybrid classifier: avoids `exchange` → `change ` false positives).
fn word_tokens(lower: &str) -> Vec<&str> {
    lower
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .collect()
}

fn tokens_contain_consecutive(tokens: &[&str], phrase: &[&str]) -> bool {
    if phrase.is_empty() || phrase.len() > tokens.len() {
        return false;
    }
    'outer: for start in 0..=(tokens.len() - phrase.len()) {
        for i in 0..phrase.len() {
            if tokens[start + i] != phrase[i] {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

fn classify_speech_code_intent(lower: &str, compact: &str) -> Option<(IntentKind, f32)> {
    let tokens = word_tokens(lower);

    let create_phrases: &[&[&str]] = &[
        &["create", "a", "function"],
        &["create", "function"],
        &["new", "function"],
        &["add", "a", "function"],
        &["add", "function"],
        &["write", "a", "function"],
        &["generate", "code"],
        &["create", "file"],
        &["new", "file"],
        &["add", "file"],
    ];
    for ph in create_phrases {
        if tokens_contain_consecutive(&tokens, ph) {
            return Some((IntentKind::CodeCreate, 0.74));
        }
    }
    let create_hits = [
        "create a function",
        "create function",
        "new function",
        "add a function",
        "add function",
        "write a function",
        "generate code",
        "create file",
        "new file",
        "add file",
    ];
    for h in create_hits {
        if lower.contains(h) {
            return Some((IntentKind::CodeCreate, 0.72));
        }
    }
    let create_compact = [
        "createfunction",
        "newfunction",
        "addfunction",
        "generatecode",
        "createfile",
    ];
    for h in create_compact {
        if compact.contains(h) {
            return Some((IntentKind::CodeCreate, 0.62));
        }
    }

    let edit_words = [
        "edit", "change", "modify", "update", "refactor", "rename", "replace",
    ];
    if edit_words.iter().any(|w| tokens.contains(w)) {
        return Some((IntentKind::CodeEdit, 0.7));
    }

    if tokens.first().copied() == Some("explain")
        || tokens_contain_consecutive(&tokens, &["what", "does"])
        || tokens_contain_consecutive(&tokens, &["how", "does"])
    {
        return Some((IntentKind::ExplainCode, 0.66));
    }
    let test_hits = [
        "run test",
        "run the test",
        "cargo test",
        "run checks",
        "type check",
    ];
    for h in test_hits {
        if lower.contains(h) {
            return Some((IntentKind::RunCheck, 0.7));
        }
    }
    None
}

fn classify_intent(transcript: &str, context: &IdeContext) -> (IntentKind, f32) {
    let t = transcript.trim();
    if t.is_empty() {
        return (IntentKind::None, 0.0);
    }
    let lower = t.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();

    let tokens = word_tokens(&lower);

    // Contextual bias for "edit this" / "fix this"
    if context.active_file.is_some() {
        if lower.contains("edit this") || lower.contains("change this") {
            return (IntentKind::CodeEdit, 0.85);
        }

        // Error keyword bias: if user mentions a word from a recent error, check first
        // (more specific than generic "fix" → higher confidence 0.88).
        for err in &context.recent_errors {
            let err_lower = err.to_lowercase();
            let err_tokens = word_tokens(&err_lower);
            for et in err_tokens {
                if et.len() > 3 && tokens.contains(&et) {
                    return (IntentKind::CodeEdit, 0.88);
                }
            }
        }

        if lower.contains("fix this") || !context.recent_errors.is_empty() && lower.contains("fix")
        {
            return (IntentKind::CodeEdit, 0.82);
        }
    }

    let status_needles = [
        "voxoratiostatus",
        "oratiostatus",
        "vox oratio status",
        "oratio status",
    ];
    for n in status_needles {
        let n_compact: String = n.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.contains(&n_compact) || lower.contains(n) {
            return (IntentKind::OratioStatus, 0.92);
        }
    }
    if lower == "status" || compact == "status" {
        return (IntentKind::OratioStatus, 0.78);
    }

    // Token-density hint: "status" near "oratio" / "vox"
    let has_status = tokens.contains(&"status");
    let has_oratio = tokens.iter().any(|w| w.contains("oratio"));
    let has_vox = tokens.contains(&"vox");
    if has_status && has_oratio {
        return (IntentKind::OratioStatus, 0.72);
    }
    if has_status && has_vox {
        return (IntentKind::OratioStatus, 0.55);
    }

    if let Some((k, c)) = classify_speech_code_intent(&lower, &compact) {
        return (k, c);
    }

    (IntentKind::None, 0.35)
}

fn intent_action_id(intent: IntentKind) -> &'static str {
    match intent {
        IntentKind::None => "none",
        IntentKind::OratioStatus => "oratio.status",
        IntentKind::CodeCreate => "speech.intent.code_create",
        IntentKind::CodeEdit => "speech.intent.code_edit",
        IntentKind::ExplainCode => "speech.intent.explain_code",
        IntentKind::RunCheck => "speech.intent.run_check",
    }
}

#[derive(Debug, Clone)]
struct RoutingSessionRecord {
    chat_history: Vec<String>,
    user_turns: usize,
    last_user_transcript: Option<String>,
    last_touch: Instant,
}

impl Default for RoutingSessionRecord {
    fn default() -> Self {
        Self {
            chat_history: Vec::new(),
            user_turns: 0,
            last_user_transcript: None,
            last_touch: Instant::now(),
        }
    }
}

fn routing_session_cap_ttl() -> (usize, Duration) {
    const DEF_CAP: usize = 4096;
    const DEF_TTL_SECS: u64 = 86_400;
    let cap = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioRoutingSessionCap)
        .expose()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEF_CAP);
    let ttl_secs =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioRoutingSessionTtlSecs)
            .expose()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEF_TTL_SECS);
    (cap.max(64), Duration::from_secs(ttl_secs.max(60)))
}

fn routing_sessions() -> &'static Mutex<HashMap<String, RoutingSessionRecord>> {
    static CACHE: OnceLock<Mutex<HashMap<String, RoutingSessionRecord>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn prune_routing_sessions(map: &mut HashMap<String, RoutingSessionRecord>) {
    let (cap, ttl) = routing_session_cap_ttl();
    let now = Instant::now();
    map.retain(|_, v| now.duration_since(v.last_touch) <= ttl);
    while map.len() > cap {
        let Some(oldest_key) = map
            .iter()
            .min_by_key(|(_, v)| v.last_touch)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        map.remove(&oldest_key);
    }
}

/// Route modes for transcript-driven execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    /// No route action.
    #[default]
    None,
    /// Tool-loop route (safe local actions).
    Tool,
    /// Conversational route with session memory.
    Chat,
    /// Orchestrator envelope route.
    Orchestrator,
    /// Clarification requested from user.
    Clarify,
}

/// Route response envelope shared by CLI and MCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    /// Chosen route mode.
    pub mode: RouteMode,
    /// Stable action identifier when one is selected.
    pub action: String,
    /// Human-readable status.
    pub status: String,
    /// Structured payload for downstream consumers.
    pub payload: serde_json::Value,
}

/// Route with explicit confidence + runtime policy (CLI / MCP parity).
#[must_use]
pub fn route_transcript_with_options(
    mode: RouteMode,
    session_id: &str,
    transcript: &str,
    transcript_confidence: f32,
    runtime: &OratioRuntimeConfig,
    context: &IdeContext,
) -> RouteResponse {
    let empty = transcript.trim().is_empty();
    if empty && !matches!(mode, RouteMode::None) {
        return RouteResponse {
            mode: RouteMode::None,
            action: "none".to_string(),
            status: "no_op_empty_transcript".to_string(),
            payload: serde_json::json!({ "note": "Empty transcript; route suppressed" }),
        };
    }

    if let Ok(mut map) = routing_sessions().lock() {
        prune_routing_sessions(&mut map);
        if !transcript.trim().is_empty() {
            let rec = map.entry(session_id.to_string()).or_default();
            rec.last_touch = Instant::now();
            if let Some(prev) = rec.last_user_transcript.as_ref()
                && prev == transcript
            {
                return RouteResponse {
                    mode: RouteMode::None,
                    action: "none".to_string(),
                    status: "repetition_breaker".to_string(),
                    payload: serde_json::json!({
                        "note": "Identical consecutive transcript for session",
                        "session_id": session_id,
                    }),
                };
            }
            rec.last_user_transcript = Some(transcript.to_string());
        }
    }

    match mode {
        RouteMode::None => RouteResponse {
            mode,
            action: "none".to_string(),
            status: "no_route".to_string(),
            payload: serde_json::json!({ "note": "No route action requested" }),
        },
        RouteMode::Tool => {
            let min_c = runtime.routing.tool_route_min_confidence.clamp(0.0, 1.0);
            if transcript_confidence < min_c {
                return RouteResponse {
                    mode: RouteMode::None,
                    action: "none".to_string(),
                    status: "below_tool_confidence".to_string(),
                    payload: serde_json::json!({
                        "note": "Transcript confidence below tool route threshold",
                        "transcript_confidence": transcript_confidence,
                        "min_confidence": min_c,
                    }),
                };
            }
            let (intent, route_conf) = classify_intent(transcript, context);
            if matches!(
                intent,
                IntentKind::OratioStatus
                    | IntentKind::CodeCreate
                    | IntentKind::CodeEdit
                    | IntentKind::ExplainCode
                    | IntentKind::RunCheck
            ) {
                // Multi-turn ambiguity detection for "fix"
                if matches!(intent, IntentKind::CodeEdit)
                    && transcript.to_lowercase().contains("fix")
                    && context.recent_errors.len() > 1
                {
                    return RouteResponse {
                        mode: RouteMode::Clarify,
                        action: "oratio.clarify.multiple_errors".to_string(),
                        status: "ambiguous_target".to_string(),
                        payload: serde_json::json!({
                            "message": "Multiple errors found. Which one should I fix?",
                            "options": context.recent_errors,
                        }),
                    };
                }

                let blended = (transcript_confidence * 0.5 + route_conf * 0.5).clamp(0.0, 1.0);
                if blended < min_c {
                    return RouteResponse {
                        mode: RouteMode::None,
                        action: "none".to_string(),
                        status: "below_blended_tool_confidence".to_string(),
                        payload: serde_json::json!({
                            "note": "Blended ASR + intent confidence below threshold",
                            "blended_confidence": blended,
                            "min_confidence": min_c,
                        }),
                    };
                }
                let escalation = crate::tiering::speech_escalation_recommended(
                    route_conf,
                    transcript_confidence,
                );
                if matches!(intent, IntentKind::OratioStatus) {
                    return RouteResponse {
                        mode,
                        action: "oratio.status".to_string(),
                        status: "executed".to_string(),
                        payload: serde_json::json!({
                            "summary": crate::transcript_status(),
                            "candle": crate::candle_backend_status_json(),
                            "intent_confidence": route_conf,
                            "blended_confidence": blended,
                            "speech_escalation_recommended": escalation,
                        }),
                    };
                }
                return RouteResponse {
                    mode,
                    action: intent_action_id(intent).to_string(),
                    status: "intent_matched".to_string(),
                    payload: serde_json::json!({
                        "intent": intent_action_id(intent),
                        "transcript": transcript,
                        "intent_confidence": route_conf,
                        "blended_confidence": blended,
                        "transcript_confidence": transcript_confidence,
                        "speech_escalation_recommended": escalation,
                        "note": "Downstream MCP/planner should validate slots and invoke codegen tools",
                    }),
                };
            }
            let escalation =
                crate::tiering::speech_escalation_recommended(route_conf, transcript_confidence);
            RouteResponse {
                mode,
                action: "none".to_string(),
                status: "no_match".to_string(),
                payload: serde_json::json!({
                    "note": "No safe tool action matched transcript",
                    "intent_confidence": route_conf,
                    "transcript_confidence": transcript_confidence,
                    "speech_escalation_recommended": escalation,
                }),
            }
        }
        RouteMode::Chat => {
            let cap = runtime.routing.chat_max_messages.max(4);
            let max_turns = runtime.routing.route_max_user_turns.max(1);
            let mut map = routing_sessions()
                .lock()
                .expect("routing session mutex poisoned");
            prune_routing_sessions(&mut map);
            let rec = map.entry(session_id.to_string()).or_default();
            rec.last_touch = Instant::now();
            rec.user_turns += 1;
            let n = rec.user_turns;
            if n > max_turns {
                return RouteResponse {
                    mode: RouteMode::None,
                    action: "none".to_string(),
                    status: "guard_max_user_turns".to_string(),
                    payload: serde_json::json!({
                        "session_id": session_id,
                        "user_turns": n,
                        "max_user_turns": max_turns,
                    }),
                };
            }

            let history = &mut rec.chat_history;
            history.push(format!("user:{transcript}"));
            if history.len() > cap {
                let drain = history.len() - cap;
                history.drain(0..drain);
            }
            let response = format!("Session {session_id}: received '{transcript}'");
            history.push(format!("assistant:{response}"));
            if history.len() > cap {
                let drain = history.len() - cap;
                history.drain(0..drain);
            }
            RouteResponse {
                mode,
                action: "chat.turn".to_string(),
                status: "executed".to_string(),
                payload: serde_json::json!({
                    "session_id": session_id,
                    "turns": history.len(),
                    "response": response,
                    "user_turn": n,
                }),
            }
        }
        RouteMode::Orchestrator => {
            let min_o = runtime.routing.orchestrator_min_confidence.clamp(0.0, 1.0);
            let low = transcript_confidence < min_o;
            RouteResponse {
                mode,
                action: "orchestrator.enqueue".to_string(),
                status: if low {
                    "queued_low_confidence".to_string()
                } else {
                    "queued".to_string()
                },
                payload: serde_json::json!({
                    "session_id": session_id,
                    "intent_text": transcript,
                    "workflow_status": "queued",
                    "transcript_confidence": transcript_confidence,
                    "orchestrator_min_confidence": min_o,
                    "note": "Transcript routed to orchestrator-compatible envelope",
                }),
            }
        }
        RouteMode::Clarify => RouteResponse {
            mode,
            action: "oratio.clarify".to_string(),
            status: "clarification_required".to_string(),
            payload: serde_json::json!({
                "note": "User clarification needed for ambiguous intent",
                "session_id": session_id,
            }),
        },
    }
}

/// Back-compat wrapper: uses [`crate::runtime_config::resolved_runtime_config`] and assumes
/// saturated transcript confidence (callers pre-gating tool mode should prefer
/// [`route_transcript_with_options`]).
#[must_use]
pub fn route_transcript(mode: RouteMode, session_id: &str, transcript: &str) -> RouteResponse {
    route_transcript_with_options(
        mode,
        session_id,
        transcript,
        1.0,
        crate::runtime_config::resolved_runtime_config(),
        &IdeContext::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_mode_matches_status_with_confidence() {
        let rt = OratioRuntimeConfig::default();
        let ctx = IdeContext::default();
        let out =
            route_transcript_with_options(RouteMode::Tool, "s1", "oratio status", 0.9, &rt, &ctx);
        assert_eq!(out.action, "oratio.status");
        let out_low =
            route_transcript_with_options(RouteMode::Tool, "s2", "oratio status", 0.1, &rt, &ctx);
        assert_eq!(out_low.status, "below_tool_confidence");
    }

    #[test]
    fn tool_mode_matches_code_create_intent() {
        let rt = OratioRuntimeConfig::default();
        let out = route_transcript_with_options(
            RouteMode::Tool,
            "sid",
            "please create a function called main",
            0.9,
            &rt,
            &IdeContext::default(),
        );
        assert_eq!(out.action, "speech.intent.code_create");
        assert_eq!(out.status, "intent_matched");
        assert!(
            out.payload.get("speech_escalation_recommended").is_some(),
            "payload should surface tiering hint for MCP/hosts: {:?}",
            out.payload
        );
    }

    #[test]
    fn exchange_does_not_false_trigger_code_edit() {
        let rt = OratioRuntimeConfig::default();
        let out = route_transcript_with_options(
            RouteMode::Tool,
            "ex",
            "exchange rate helper in the module",
            0.95,
            &rt,
            &IdeContext::default(),
        );
        assert_ne!(out.action, "speech.intent.code_edit");
    }

    #[test]
    fn explicit_change_triggers_code_edit() {
        let rt = OratioRuntimeConfig::default();
        let out = route_transcript_with_options(
            RouteMode::Tool,
            "ch",
            "change the return type to int",
            0.95,
            &rt,
            &IdeContext::default(),
        );
        assert_eq!(out.action, "speech.intent.code_edit");
    }

    #[test]
    fn repetition_breaker_triggers() {
        let rt = OratioRuntimeConfig::default();
        let ctx = IdeContext::default();
        let _ = route_transcript_with_options(RouteMode::Chat, "rpt", "hello", 1.0, &rt, &ctx);
        let out2 = route_transcript_with_options(RouteMode::Chat, "rpt", "hello", 1.0, &rt, &ctx);
        assert_eq!(out2.status, "repetition_breaker");
    }

    #[test]
    fn chat_max_user_turns_guard() {
        let mut rt = OratioRuntimeConfig::default();
        rt.routing.route_max_user_turns = 2;
        let sid = "max-turn-unit-test";
        let ctx = IdeContext::default();
        let _ = route_transcript_with_options(RouteMode::Chat, sid, "a", 1.0, &rt, &ctx);
        let _ = route_transcript_with_options(RouteMode::Chat, sid, "b", 1.0, &rt, &ctx);
        let out = route_transcript_with_options(RouteMode::Chat, sid, "c", 1.0, &rt, &ctx);
        assert_eq!(out.status, "guard_max_user_turns");
    }

    #[test]
    fn test_contextual_edit_this() {
        let rt = OratioRuntimeConfig::default();
        let ctx = IdeContext {
            active_file: Some("src/main.rs".to_string()),
            ..IdeContext::default()
        };

        let out = route_transcript_with_options(
            RouteMode::Tool,
            "ctx1",
            "edit this function",
            0.9,
            &rt,
            &ctx,
        );
        assert_eq!(out.action, "speech.intent.code_edit");
        assert_eq!(out.status, "intent_matched");
    }

    #[test]
    fn test_error_keyword_bias() {
        let rt = OratioRuntimeConfig::default();
        let mut ctx = IdeContext {
            active_file: Some("src/lib.rs".to_string()),
            ..IdeContext::default()
        };
        ctx.recent_errors
            .push("unresolved import 'vox_core'".to_string());

        // "vox_core" is in the error, user says "fix vox_core"
        let out =
            route_transcript_with_options(RouteMode::Tool, "ctx2", "fix vox_core", 0.9, &rt, &ctx);
        assert_eq!(out.action, "speech.intent.code_edit");
        assert!(out.payload["intent_confidence"].as_f64().unwrap() >= 0.85);
    }

    #[test]
    fn test_clarify_multiple_errors() {
        let rt = OratioRuntimeConfig::default();
        let mut ctx = IdeContext {
            active_file: Some("src/lib.rs".to_string()),
            ..IdeContext::default()
        };
        ctx.recent_errors.push("Error 1".to_string());
        ctx.recent_errors.push("Error 2".to_string());

        let out =
            route_transcript_with_options(RouteMode::Tool, "sid", "fix the error", 0.9, &rt, &ctx);
        assert_eq!(out.mode, RouteMode::Clarify);
        assert_eq!(out.action, "oratio.clarify.multiple_errors");
    }
}
