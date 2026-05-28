use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Editor/LLM anti-stub rider appended to system prompts that must emit full code.
pub const ANTI_LAZINESS_RIDER: &str = "\nCRITICAL DIRECTIVE: You must output the COMPLETE, fully-implemented replacement code. DO NOT under any circumstances use placeholders, stubs, 'TODOs', or elide implementation details. Writing partial code is a catastrophic failure.";

/// One persisted chat turn in the session transcript (also returned in history APIs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTranscriptEntry {
    /// Opaque message id (UUID/ulid string).
    pub id: String,
    /// Role label (OpenAI-compatible): user, assistant, or system.
    pub role: String,
    /// Message body after expansion (mentions resolved server-side).
    pub content: String,
    /// Epoch seconds when stored.
    pub timestamp: u64,
    /// Extra files pulled in via @mentions or explicit attachments.
    pub context_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Model id recorded for assistant turns.
    pub model_used: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Approximate token usage when available.
    pub tokens: Option<u64>,
}

/// Arguments for `vox_chat_message` (prompt + rich editor context).
#[derive(Debug, Deserialize)]
pub struct ChatMessageParams {
    /// User message text (`message` is accepted for registry / legacy clients).
    #[serde(alias = "message")]
    pub prompt: String,
    #[serde(default)]
    /// Explicit @mention or attachment paths from the client.
    pub context_files: Vec<String>,
    /// Open file paths provided by the editor for implicit context injection
    #[serde(default)]
    pub open_files: Vec<String>,
    /// Active editor file path (workspace-relative)
    #[serde(default)]
    pub active_file: Option<String>,
    /// Active editor cursor line (1-indexed)
    #[serde(default)]
    pub active_line: Option<u32>,
    /// Selected text in the active editor
    #[serde(default)]
    pub selected_text: Option<String>,
    /// Active LSP diagnostics to inject as context
    #[serde(default)]
    pub diagnostics: Vec<Value>,
    /// Optional logical grouping identifier for this chat thread.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Editor thread id (e.g. VS Code); joins the same journey envelope as `session_id`.
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Stable id for this user request across routing, storage, and telemetry (generated if omitted).
    #[serde(default)]
    pub journey_id: Option<String>,
    /// Optionally selects a specific LLM routing profile (e.g. "reasoning", "fast", "creative").
    #[serde(default)]
    pub cognitive_profile: Option<String>,
    /// If true, enforces strict JSON output from the LLM.
    #[serde(default)]
    pub json_mode: bool,
    /// Optional end-to-end trace id; forwarded to retrieval (`vox-search`) for logging and sidecar HTTP.
    #[serde(default)]
    pub trace_id: Option<String>,
    /// Optional correlation id; used for retrieval when `trace_id` is unset.
    #[serde(default)]
    pub correlation_id: Option<String>,
    /// Multimodal attachments bounded by specific MIME types.
    #[serde(default)]
    pub attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
    /// Optional temperature override (0.0–2.0).
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Optional top_p override (0.0–1.0).
    #[serde(default)]
    pub top_p: Option<f32>,
}

fn default_chat_history_session_id() -> String {
    "default".to_string()
}

/// Retrieve history for a specific session ID.
#[derive(Debug, Deserialize)]
pub struct ChatHistoryParams {
    /// Logical grouping identifier to fetch history for. Omitted JSON / `{}` deserializes to `"default"`.
    #[serde(default = "default_chat_history_session_id")]
    pub session_id: String,
    /// Optional correlation for retrieval / logging parity with `vox_chat_message`.
    #[serde(default)]
    pub trace_id: Option<String>,
}

/// Arguments for `vox_inline_edit` (range replacement inside one file).
#[derive(Debug, Deserialize)]
pub struct InlineEditParams {
    /// The edit instruction / prompt from the user (`instruction` is a legacy alias).
    #[serde(alias = "instruction")]
    pub prompt: String,
    /// Workspace-relative file path (`file_path` is a legacy alias).
    #[serde(alias = "file_path")]
    pub file: String,
    /// Start line of target range (1-indexed)
    pub start_line: u32,
    /// End line of target range (1-indexed, inclusive)
    pub end_line: u32,
    /// The current text in the range (sent by editor; `selection` is a legacy alias).
    #[serde(alias = "selection")]
    pub current_text: String,
    /// Language ID of the file
    #[serde(default)]
    pub language: Option<String>,
    /// Surrounding context lines before and after the range (0-40 lines typically)
    #[serde(default)]
    pub context_before: Option<String>,
    #[serde(default)]
    /// Optional lines after the selection for better LLM grounding.
    pub context_after: Option<String>,
    /// If true, enforces strict JSON output from the LLM (rarely used for raw code edits).
    #[serde(default)]
    pub json_mode: bool,
    /// Optional tenant/session partition key for usage attribution.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Multimodal attachments bounded by specific MIME types.
    #[serde(default)]
    pub attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
    /// Optional temperature override (0.0–2.0).
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Optional top_p override (0.0–1.0).
    #[serde(default)]
    pub top_p: Option<f32>,
}

/// Successful inline edit payload returned to the editor host.
#[derive(Debug, Serialize)]
pub struct InlineEditResult {
    /// Replacement text for the range [`start_line`, `end_line`]
    pub replacement: String,
    /// Human-readable explanation of what was changed
    pub explanation: String,
    /// Estimated token usage
    pub tokens: u64,
    /// Model that produced this edit
    pub model_used: String,
}

/// Iterative refinement policy for `vox_plan` (default: off; backward compatible).
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanLoopMode {
    #[default]
    Off,
    /// Refine when automated gap risk score exceeds `gap_risk_threshold`.
    Auto,
    /// Run up to `max_refine_rounds` refinement passes regardless of initial score.
    Force,
}

/// Target detail level for `vox_plan` (output token budget + minimum task floor).
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanDepth {
    /// Fewer tasks and lower LLM output cap.
    Minimal,
    #[default]
    Standard,
    /// More tasks expected; higher output cap; stronger adequacy expansion.
    Deep,
}

/// Arguments for `vox_plan` structured planning tool.
#[derive(Debug, Deserialize)]
pub struct PlanParams {
    /// The request / goal to plan for
    pub goal: String,
    /// Optional files to scope the plan to
    #[serde(default)]
    pub scope_files: Vec<String>,
    /// Whether to write the plan to PLAN.md in the workspace root
    #[serde(default)]
    pub write_to_disk: bool,
    /// Maximum number of tasks to generate (default: 30)
    #[serde(default)]
    pub max_tasks: Option<usize>,
    /// Optional tenant/session partition key for usage attribution.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Planning depth / verbosity tier (`standard` when omitted).
    #[serde(default)]
    pub plan_depth: Option<PlanDepth>,
    /// When not `false` (default), run bounded refinement if the plan is structurally thin even when `loop_mode` is off.
    #[serde(default)]
    pub auto_expand_thin_plan: Option<bool>,
    /// Optional iterative refinement (`off` when omitted).
    #[serde(default)]
    pub loop_mode: Option<PlanLoopMode>,
    /// Maximum refinement LLM rounds (capped server-side).
    #[serde(default)]
    pub max_refine_rounds: Option<u32>,
    /// Approximate output-token budget across refinement passes.
    #[serde(default)]
    pub refine_budget_tokens: Option<u32>,
    /// Aggregate gap risk above which `loop_mode=auto` runs refinement (0.28 default in engine).
    #[serde(default)]
    pub gap_risk_threshold: Option<f32>,
    /// Slice start when returning a window of tasks (UI pagination).
    #[serde(default)]
    pub plan_page_offset: Option<usize>,
    /// Max tasks to return after refinement; omit for full list.
    #[serde(default)]
    pub plan_page_limit: Option<usize>,
    /// When set with an attached Codex DB, upserts `plan_sessions` and records iterative telemetry.
    #[serde(default)]
    pub plan_telemetry_session_id: Option<String>,
    /// Optional link into `question_sessions.question_session_id` for unified analytics.
    #[serde(default)]
    pub question_link_session_id: Option<String>,
    /// When true, include `clarifying_questions` from gap heuristics in the tool result.
    #[serde(default)]
    pub questioning_hints_enabled: Option<bool>,
    /// Reserved: `local_first` | `cloud_first` | `balanced` for multi-answer orchestration.
    #[serde(default)]
    pub answerer_profile: Option<String>,
    /// Multimodal attachments bounded by specific MIME types.
    #[serde(default)]
    pub attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
    /// Optional temperature override (0.0–2.0).
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Optional top_p override (0.0–1.0).
    #[serde(default)]
    pub top_p: Option<f32>,
}

/// Arguments for `vox_replan` — forwards to DeI `ai.plan.replan` when `vox-orchestrator-d` is available.
#[derive(Debug, Deserialize)]
pub struct PlanReplanParams {
    /// Session id from a prior `vox_plan` or `ai.plan.new`.
    pub session_id: String,
    /// What changed since the last plan version.
    pub delta_hint: String,
    #[serde(default)]
    pub write_to_disk: bool,
    #[serde(default)]
    pub mode: Option<String>,
}

/// Arguments for `vox_plan_status` — forwards to DeI `ai.plan.status`.
#[derive(Debug, Deserialize)]
pub struct PlanStatusParams {
    /// Plan session id to query.
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanListSessionsParams {
    #[serde(default)]
    pub session_id_filter: Option<String>,
    #[serde(default)]
    pub status_filter: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanResumeParams {
    pub session_id: String,
    #[serde(default)]
    pub write_to_disk: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// One row inside a generated plan (dependencies + complexity estimate).
pub struct PlanTask {
    /// Monotonic task index inside the plan.
    pub id: usize,
    /// Short imperative description.
    pub description: String,
    /// The specific task category, mapping to `TaskCategory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Related file paths for affinity routing.
    pub files: Vec<String>,
    /// Heuristic difficulty on a 1-10 scale.
    pub estimated_complexity: u8, // 1-10
    /// Task ids that should complete first.
    pub depends_on: Vec<usize>,
}

#[derive(Debug, Serialize)]
/// Full structured plan returned to the IDE / LLM.
pub struct PlanResult {
    /// Echo of the user goal string.
    pub goal: String,
    /// Ordered task breakdown.
    pub tasks: Vec<PlanTask>,
    /// One-line executive summary.
    pub summary: String,
    /// Markdown document (may mirror on-disk `PLAN.md`).
    pub plan_md: String,
    /// Whether `PLAN.md` was written under the workspace root.
    pub written_to_disk: bool,
    /// Total tasks after refinement (before pagination slice).
    #[serde(default)]
    pub plan_total_tasks: usize,
    /// Pagination offset applied to `tasks` in this payload.
    #[serde(default)]
    pub plan_page_offset: usize,
    /// Iterative loop policy applied (`off`, `auto`, `force`).
    #[serde(default)]
    pub loop_mode_effective: String,
    /// Number of LLM refinement passes executed.
    #[serde(default)]
    pub refinement_rounds: u32,
    /// Terminal loop / stop reason when refinement ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_stop_reason: Option<String>,
    /// Aggregate risk from the last gap pass (0..1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_aggregate_gap_risk: Option<f32>,
    /// Structured gap + adequacy report for analytics (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_report: Option<serde_json::Value>,
    /// Plan adequacy score 0..1 (higher = better structural fit for goal complexity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_adequacy_score: Option<f32>,
    /// True when tier-1 heuristics recommend more decomposition or verification tasks.
    #[serde(default)]
    pub plan_too_thin: bool,
    /// Orchestrator-style adequacy reason codes (`too_few_tasks`, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adequacy_reason_codes: Vec<String>,
    /// Effective planning depth (snake_case string).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub plan_depth_effective: String,
    /// Ranked clarification prompts derived from gap codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clarifying_questions: Vec<String>,
    /// Typed content blocks for structured rendering.
    ///
    /// Mirrors `plan_md` but pre-split for CLI (`render.rs`) and webview (shiki).
    /// Additive: consumers that do not read this field see no change — the field
    /// is omitted from the wire when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_blocks: Vec<vox_orchestrator::planning::ContentBlock>,
}

/// Parameters for the `vox_ghost_text` MCP tool.
#[derive(Debug, Deserialize)]
pub struct GhostTextParams {
    /// Source code prefix (up to 20 lines before cursor).
    pub prefix: String,
    /// Source code suffix (up to 5 lines after cursor).
    pub suffix: String,
    /// VS Code language ID (e.g. "vox", "rust", "typescript").
    #[serde(default)]
    pub language: Option<String>,
    /// Workspace-relative file path for context.
    #[serde(default)]
    pub file_path: Option<String>,
    /// Maximum tokens to generate. Defaults to 128 for low latency.
    #[serde(default)]
    pub max_tokens: Option<u64>,
    /// Optional tenant/session partition key for usage attribution.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Optional temperature override (0.0–2.0).
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Optional top_p override (0.0–1.0).
    #[serde(default)]
    pub top_p: Option<f32>,
}

/// Response from `vox_ghost_text`.
#[derive(Debug, Serialize)]
pub struct GhostTextResult {
    /// The generated completion text.
    pub completion: String,
    /// Model that produced this completion.
    pub model_used: String,
    /// Approximate token count.
    pub tokens: u64,
    /// Latency to first token (milliseconds, best-effort).
    pub latency_ms: u64,
}

/// Parameters for `vox_ambient_state`.
#[derive(Debug, Deserialize)]
pub struct AmbientStateParams {
    /// Optional workspace-relative path filter. Returns only decorations for this path prefix.
    #[serde(default)]
    pub path_prefix: Option<String>,
    /// Maximum number of decorations to return. Defaults to 100.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[cfg(test)]
mod chat_history_params_tests {
    use super::ChatHistoryParams;

    #[test]
    fn chat_history_empty_object_defaults_session() {
        let p: ChatHistoryParams = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(p.session_id, "default");
        assert!(p.trace_id.is_none());
    }

    #[test]
    fn chat_history_explicit_session_round_trips() {
        let p: ChatHistoryParams =
            serde_json::from_str(r#"{"session_id":"vscode-sidebar","trace_id":"t1"}"#)
                .expect("deserialize");
        assert_eq!(p.session_id, "vscode-sidebar");
        assert_eq!(p.trace_id.as_deref(), Some("t1"));
    }
}
