//! LLM-assisted evidence / conclusion suggestions for a claim (P3 Phase 4).
//!
//! This is an **advisory** feature: it produces a list of suggestions a human
//! reviews. It NEVER auto-mutates a review decision or an assertion. Every LLM
//! call is routed STRICTLY through the model-agnostic
//! [`vox_actor_runtime::llm`] facade — there is no OpenRouter hostname or SDK in
//! this module. Any failure (missing API key, network error, parse junk)
//! degrades to an empty `Vec`, so the review flow is never broken by this path.
//!
//! The CLI (`vox scientia evidence-assist`) and the GUI Tauri command
//! (`suggest_evidence_improvements`) both call the SAME [`suggest`] function for
//! parity.

use serde::{Deserialize, Serialize};
use vox_actor_runtime::ActivityResult;
use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, llm_chat};
use vox_actor_runtime::llm_result::maybe_strip_markdown_json_fences;

/// One advisory suggestion for improving a claim's evidence or conclusion.
///
/// `kind` is one of `evidence_gap` | `conclusion_refinement` | `novelty_check`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceSuggestion {
    /// Suggestion category: `evidence_gap` | `conclusion_refinement` | `novelty_check`.
    pub kind: String,
    /// A short, human-readable summary of the suggestion.
    pub summary: String,
    /// The reasoning behind the suggestion.
    pub rationale: String,
}

/// Build the (pure) chat prompt for the evidence-assist call.
///
/// The first message is a `system` message instructing the model to emit a JSON
/// array of suggestion objects; the second is a `user` message embedding the
/// claim text and any known verdict/confidence. Pure + unit-testable: no I/O.
pub fn build_prompt(
    claim_text: &str,
    verdict: Option<&str>,
    confidence: Option<f64>,
) -> Vec<LlmChatMessage> {
    let system = "You are a research-review assistant. Given a single extracted \
claim and its (optional) verification verdict and confidence, propose concrete, \
actionable suggestions for strengthening the evidence or refining the \
conclusion. Respond with ONLY a JSON array (no prose, no markdown fences) of \
objects, each shaped exactly: \
{\"kind\": <one of \"evidence_gap\"|\"conclusion_refinement\"|\"novelty_check\">, \
\"summary\": <short string>, \"rationale\": <string>}. Return between 1 and 5 \
items. If you have nothing useful to add, return an empty array []."
        .to_string();

    let verdict_line = match verdict {
        Some(v) => format!("Verdict: {v}"),
        None => "Verdict: (none recorded)".to_string(),
    };
    let confidence_line = match confidence {
        Some(c) => format!("Confidence: {c:.2}"),
        None => "Confidence: (none recorded)".to_string(),
    };
    let user = format!("Claim: {claim_text}\n{verdict_line}\n{confidence_line}");

    vec![
        LlmChatMessage {
            role: "system".to_string(),
            content: system,
        },
        LlmChatMessage {
            role: "user".to_string(),
            content: user,
        },
    ]
}

/// Parse a raw LLM response into a suggestion list.
///
/// Strips Markdown code fences first, then parses a JSON array. Returns
/// `Vec::new()` on any parse failure (never panics) — this is advisory output.
pub fn parse_suggestions(raw: &str) -> Vec<EvidenceSuggestion> {
    let clean = maybe_strip_markdown_json_fences(raw);
    serde_json::from_str::<Vec<EvidenceSuggestion>>(clean).unwrap_or_default()
}

/// Produce advisory evidence/conclusion suggestions for a claim.
///
/// Routes through the [`vox_actor_runtime::llm`] facade. ANY error (no key,
/// network failure, unparseable output) degrades to `Vec::new()` so the review
/// flow is never broken by this advisory path.
pub async fn suggest(
    claim_text: &str,
    verdict: Option<&str>,
    confidence: Option<f64>,
) -> Vec<EvidenceSuggestion> {
    let messages = build_prompt(claim_text, verdict, confidence);
    let mut config = LlmConfig::openrouter("anthropic/claude-3.5-sonnet");
    config.temperature = Some(0.2);
    config.max_tokens = Some(800);

    let options = vox_actor_runtime::ActivityOptions::default();
    match llm_chat(&options, messages, config).await {
        ActivityResult::Ok(Ok(response)) => parse_suggestions(&response.content),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_claim_and_is_system_first() {
        let messages = build_prompt("The sky is blue", Some("Supported"), Some(0.9123));
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        let user = &messages[1].content;
        assert!(user.contains("The sky is blue"), "claim text must appear");
        assert!(user.contains("Supported"), "verdict must appear");
        // Confidence is formatted to two decimals.
        assert!(
            user.contains("0.91"),
            "formatted confidence must appear, got: {user}"
        );
    }

    #[test]
    fn parse_tolerates_fences_and_junk() {
        let fenced =
            "```json\n[{\"kind\":\"evidence_gap\",\"summary\":\"s\",\"rationale\":\"r\"}]\n```";
        let parsed = parse_suggestions(fenced);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].kind, "evidence_gap");

        assert!(parse_suggestions("not json").is_empty());
    }
}
