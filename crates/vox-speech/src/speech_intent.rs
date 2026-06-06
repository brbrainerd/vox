//! Structured speech intent envelope for planner / MCP (contract scaffold).

use serde::{Deserialize, Serialize};

use crate::ast_mapper::{AstTarget, map_to_ast_target};

/// Action labels aligned with [`crate::routing`] tool-route ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SpeechIntentAction {
    /// Oratio / STT status probe.
    OratioStatus,
    /// Create new code or file.
    CodeCreate,
    /// Edit existing code.
    CodeEdit,
    /// Explain without writes.
    ExplainCode,
    /// Run tests or checks.
    RunCheck,
    /// No confident classification.
    #[default]
    None,
}

/// Slots and confidence for downstream codegen tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpeechIntentEnvelope {
    /// Classified action.
    pub action: SpeechIntentAction,
    /// 0.0..=1.0 routing confidence.
    pub intent_confidence: f32,
    /// 0.0..=1.0 ASR confidence (if available).
    pub transcript_confidence: f32,
    /// Normalized transcript text.
    pub transcript: String,
    /// Optional target path / symbol (filled by future slot-filling).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
    /// Optional identifier / function name extracted from speech.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    /// Optional AST-aware target extracted from speech to eliminate character-precise dictation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast_target: Option<AstTarget>,
}

/// Map a [`RouteResponse::action`](crate::RouteResponse) string to a planner action.
#[must_use]
pub fn speech_intent_action_from_route_action(action: &str) -> SpeechIntentAction {
    match action {
        "oratio.status" => SpeechIntentAction::OratioStatus,
        "speech.intent.code_create" => SpeechIntentAction::CodeCreate,
        "speech.intent.code_edit" => SpeechIntentAction::CodeEdit,
        "speech.intent.explain_code" => SpeechIntentAction::ExplainCode,
        "speech.intent.run_check" => SpeechIntentAction::RunCheck,
        _ => SpeechIntentAction::None,
    }
}

/// Heuristic slot fill from transcript (path in backticks, “file …”, “function …”, etc.).
#[must_use]
pub fn fill_slots_heuristic(
    transcript: &str,
    context: Option<&crate::routing::IdeContext>,
) -> (Option<String>, Option<String>) {
    let mut path = None;
    let mut symbol = None;

    let lower = transcript.to_ascii_lowercase();

    // Contextual resolution for "this file" / "this"
    if let Some(ctx) = context
        && (lower.contains("this file")
            || lower.contains("edit this")
            || lower.contains("fix this"))
        && let Some(active) = &ctx.active_file
    {
        path = Some(active.clone());
    }

    for part in transcript.split('`') {
        let p = part.trim();
        if p.contains('.') && (p.ends_with(".vox") || p.contains('/')) {
            path = Some(p.to_string());
            break;
        }
    }

    if path.is_none() {
        let lower = transcript.to_ascii_lowercase();
        for needle in ["file ", "path ", "in "] {
            if let Some(pos) = lower.find(needle) {
                let rest = transcript[pos + needle.len()..].trim();
                let token = rest.split_whitespace().next().unwrap_or("");
                if token.contains('.') || token.contains('/') {
                    path = Some(
                        token
                            .trim_matches(|c| c == '\'' || c == '"' || c == '`')
                            .to_string(),
                    );
                    break;
                }
            }
        }
    }

    let lower = transcript.to_ascii_lowercase();
    for pat in ["function ", "called ", "named ", "fn "] {
        if let Some(pos) = lower.find(pat) {
            let rest = transcript[pos + pat.len()..].trim();
            let tok = rest.split_whitespace().next().unwrap_or("");
            let tok = tok.trim_matches(|c| c == ',' || c == '.' || c == '\'' || c == '`');
            if tok.chars().all(|c| c.is_alphanumeric() || c == '_') && !tok.is_empty() {
                symbol = Some(tok.to_string());
                break;
            }
        }
    }

    (path, symbol)
}

/// Build an intent envelope for MCP / planner from routing output.
#[must_use]
pub fn build_intent_envelope(
    action: &str,
    transcript: &str,
    intent_confidence: f32,
    transcript_confidence: f32,
    context: Option<&crate::routing::IdeContext>,
) -> SpeechIntentEnvelope {
    let ia = speech_intent_action_from_route_action(action);
    let (target_path, symbol_name) = fill_slots_heuristic(transcript, context);
    let ast_target = map_to_ast_target(transcript, context);
    SpeechIntentEnvelope {
        action: ia,
        intent_confidence,
        transcript_confidence,
        transcript: transcript.to_string(),
        target_path,
        symbol_name,
        ast_target,
    }
}

/// Slot IDs still needed before safe tool execution (contract for planner/MCP).
#[must_use]
pub fn missing_slot_ids(env: &SpeechIntentEnvelope) -> Vec<&'static str> {
    match env.action {
        SpeechIntentAction::CodeCreate | SpeechIntentAction::CodeEdit => {
            if env.target_path.is_none() && env.symbol_name.is_none() {
                vec!["target_path_or_symbol"]
            } else {
                vec![]
            }
        }
        SpeechIntentAction::RunCheck => {
            if env.target_path.is_none() {
                vec!["test_scope"]
            } else {
                vec![]
            }
        }
        SpeechIntentAction::ExplainCode => {
            if env.target_path.is_none() && env.symbol_name.is_none() {
                vec!["explain_focus"]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Suggested clarification when required slots are missing for a matched tool intent.
#[must_use]
pub fn clarification_prompt_for_slots(env: &SpeechIntentEnvelope) -> Option<&'static str> {
    if missing_slot_ids(env).is_empty() {
        return None;
    }
    match env.action {
        SpeechIntentAction::CodeCreate | SpeechIntentAction::CodeEdit => Some(
            "Say which `.vox` path or symbol name to create or edit (or wrap the path in backticks).",
        ),
        SpeechIntentAction::RunCheck => Some(
            "Say which crate, test filter, or path to run—or say 'workspace' to run the full suite.",
        ),
        SpeechIntentAction::ExplainCode => {
            Some("Say which file, symbol, or code region to explain (path in backticks helps).")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_slots_path_in_backticks() {
        let (p, s) = fill_slots_heuristic("open `src/foo.vox` and add code", None);
        assert_eq!(p.as_deref(), Some("src/foo.vox"));
        assert!(s.is_none());
    }

    #[test]
    fn fill_slots_function_named() {
        let (_p, s) = fill_slots_heuristic("edit function bar_baz in the workflow", None);
        assert_eq!(s.as_deref(), Some("bar_baz"));
    }

    #[test]
    fn fill_slots_contextual_this_file() {
        let ctx = crate::routing::IdeContext {
            active_file: Some("src/lib.vox".to_string()),
            ..crate::routing::IdeContext::default()
        };
        let (p, _s) = fill_slots_heuristic("fix this file", Some(&ctx));
        assert_eq!(p.as_deref(), Some("src/lib.vox"));
    }

    #[test]
    fn clarification_when_code_create_missing_slots() {
        let env = build_intent_envelope(
            "speech.intent.code_create",
            "create a new handler",
            0.9,
            0.85,
            None,
        );
        assert!(clarification_prompt_for_slots(&env).is_some());
    }
}
