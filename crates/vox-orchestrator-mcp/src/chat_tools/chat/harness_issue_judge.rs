//! Threshold-triggered LLM judge for the harness issue scorer (Task 8). Fires
//! only when [`super::harness_issue_scorer::HarnessIssueScorer::record`] returns
//! `true` — most turns never call this. Uses the model-agnostic
//! `vox_actor_runtime::llm` boundary, matching every other LLM call in this
//! codebase (see `crates/vox-effort-audit/src/judge/mod.rs` for the pattern
//! this is adapted from). Runs fire-and-forget from the caller's turn (see
//! Task 11) rather than blocking it — blocking a chat turn on judge latency
//! would be a real UX regression for a detector that should be invisible in
//! the common case.

use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig};
use vox_actor_runtime::{ActivityOptions, ActivityResult};

/// Severities the rest of the system accepts — anything else from the judge
/// gets normalized down to `medium` rather than silently dropping the issue
/// (strict downstream validation rejects an out-of-vocabulary severity with
/// only a background warn log, which would silently lose real issues).
const KNOWN_SEVERITIES: &[&str] = &["low", "medium", "high"];

/// A judged, real harness issue. `None` is returned by [`judge`] when the
/// judge concludes the accumulated signals were not a genuine issue.
pub struct JudgedHarnessIssue {
    pub category: String,
    pub severity: String, // always one of KNOWN_SEVERITIES after normalize_severity
    pub summary: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct JudgeVerdict {
    is_issue: bool,
    #[serde(default)]
    category: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    summary: String,
}

/// Strip a leading ```` ```json ```` (or bare ` ``` `) fence and a trailing
/// ` ``` ` from a model response, if present. Models are asked not to wrap
/// their JSON in a code fence, but real-world responses do it anyway; this is
/// a minimal, tolerant unwrap — not a full markdown parser.
fn strip_markdown_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    let without_open = trimmed
        .strip_prefix("```")
        .map(|rest| {
            // Drop an optional language tag (e.g. "json") up to the first newline.
            match rest.find('\n') {
                Some(idx) if rest[..idx].trim().chars().all(|c| c.is_ascii_alphabetic()) => {
                    &rest[idx + 1..]
                }
                _ => rest,
            }
        })
        .unwrap_or(trimmed);
    without_open
        .trim()
        .strip_suffix("```")
        .map(str::trim)
        .unwrap_or_else(|| without_open.trim())
}

fn normalize_severity(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    if KNOWN_SEVERITIES.contains(&lower.as_str()) {
        lower
    } else {
        "medium".to_string()
    }
}

const JUDGE_SYSTEM_PROMPT: &str = "You are reviewing a short excerpt of recent \
tool calls from an AI coding agent's turn, because a heuristic scorer flagged \
repeated errors or retries. Decide whether this is a genuine recurring issue \
worth a human's attention (e.g. the agent kept hitting the same compiler \
error, or retried an identical failing action). Respond with ONLY a JSON \
object, no prose, matching exactly: \
{\"is_issue\": bool, \"category\": string, \"severity\": \"low\"|\"medium\"|\"high\", \"summary\": string}. \
If this looks like normal iterative debugging rather than a stuck loop, set is_issue to false.";

/// Judge a small excerpt of recent tool-call activity. Returns `None` for
/// both an explicit "not a real issue" verdict and an unparseable/failed
/// response — a judge failure must never crash or block the chat turn.
pub async fn judge(recent_activity: &str, model: &str) -> Option<JudgedHarnessIssue> {
    let messages = vec![
        LlmChatMessage {
            role: "system".into(),
            content: JUDGE_SYSTEM_PROMPT.to_string(),
            ..Default::default()
        },
        LlmChatMessage {
            role: "user".into(),
            content: recent_activity.to_string(),
            ..Default::default()
        },
    ];

    let llm_config = LlmConfig {
        provider: "auto".into(),
        model: model.to_string(),
        cost_per_1k: None,
        base_url: None,
        api_key: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(256),
        response_format: None,
        tools: None,
        tool_choice: None,
        timeout_ms: Some(15_000),
        telemetry_session_id: None,
        telemetry_user_id: None,
        telemetry_task_category: Some("HarnessIssueJudge".into()),
        telemetry_strength_tag: None,
        telemetry_trace_id: None,
        telemetry_attempt_number: Some(1),
        telemetry_skip_interaction: false,
    };

    let activity_options =
        ActivityOptions::default().with_timeout(std::time::Duration::from_secs(15));

    let infer_result =
        vox_actor_runtime::llm::infer_with_retry(&activity_options, messages, vec![llm_config])
            .await;

    let response = match infer_result {
        ActivityResult::Ok(Ok((resp, _cfg))) => resp,
        ActivityResult::Ok(Err(e)) => {
            tracing::warn!(target: "harness_issue_judge", error = %e, "judge call failed");
            return None;
        }
        ActivityResult::Failed(e) => {
            tracing::warn!(target: "harness_issue_judge", error = ?e, "judge activity failed");
            return None;
        }
        ActivityResult::Cancelled => return None,
    };

    let verdict: JudgeVerdict = match serde_json::from_str(strip_markdown_fence(&response.content))
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "harness_issue_judge", error = %e, raw = %response.content, "judge response was not valid JSON");
            return None;
        }
    };

    if !verdict.is_issue {
        return None;
    }
    Some(JudgedHarnessIssue {
        category: verdict.category,
        severity: normalize_severity(&verdict.severity),
        summary: verdict.summary,
    })
}

#[cfg(test)]
mod tests {
    use super::{JudgeVerdict, normalize_severity, strip_markdown_fence};

    #[test]
    fn parses_a_real_issue_verdict() {
        let raw = r#"{"is_issue": true, "category": "repeated_compiler_error", "severity": "medium", "summary": "stuck on E0502"}"#;
        let v: JudgeVerdict = serde_json::from_str(raw).unwrap();
        assert!(v.is_issue);
        assert_eq!(v.category, "repeated_compiler_error");
    }

    #[test]
    fn parses_a_not_an_issue_verdict() {
        let raw = r#"{"is_issue": false}"#;
        let v: JudgeVerdict = serde_json::from_str(raw).unwrap();
        assert!(!v.is_issue);
    }

    #[test]
    fn normalize_severity_passes_through_known_values() {
        assert_eq!(normalize_severity("high"), "high");
        assert_eq!(normalize_severity("Medium"), "medium");
    }

    #[test]
    fn normalize_severity_falls_back_to_medium_for_unknown_values() {
        assert_eq!(normalize_severity("Critical"), "medium");
        assert_eq!(normalize_severity(""), "medium");
    }

    #[test]
    fn strip_markdown_fence_unwraps_fenced_json_with_language_tag() {
        let raw = "```json\n{\"is_issue\": true, \"category\": \"x\"}\n```";
        let unwrapped = strip_markdown_fence(raw);
        let v: JudgeVerdict = serde_json::from_str(unwrapped).expect("parses");
        assert!(v.is_issue);
        assert_eq!(v.category, "x");
    }

    #[test]
    fn strip_markdown_fence_unwraps_bare_fence() {
        let raw = "```\n{\"is_issue\": false}\n```";
        let unwrapped = strip_markdown_fence(raw);
        let v: JudgeVerdict = serde_json::from_str(unwrapped).expect("parses");
        assert!(!v.is_issue);
    }

    #[test]
    fn strip_markdown_fence_leaves_unfenced_json_unchanged() {
        let raw = "{\"is_issue\": false}";
        assert_eq!(strip_markdown_fence(raw), raw);
    }
}
