//! The single chat dispatch command.
//!
//! Before this module the composer forked in TypeScript: `task_category ==
//! 'chat'` early-returned to `chat_send_message` (4 fields) while everything
//! else went to `submit_orchestrator_task` (16). Half the composer's controls
//! therefore did nothing on a quick chat. The fork now lives here, as one
//! `match` over one struct.
//!
//! NOTE the frontend still branches — on *store lifecycle*, not on dispatch.
//! See the spec §6: `submitResolved` is the only writer of `taskToSession`,
//! which routes every live agent event to a bubble.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::control_plane::SubmitTaskInput;
use crate::commands::daemon::PersistentDaemon; // NB: commands::daemon, not crate::daemon
use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

/// Routing fields that must exist on both input structs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Execution {
    #[default]
    Sync,
    Background,
    /// GUI's `/plan` slash command: issues `vox_plan` with `require_approval:
    /// true` instead of `vox_chat_message`, and returns a `plan_session_id`/
    /// `plan_version` the frontend points `PlanPanel` at.
    Plan,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChatTurnInput {
    pub session_id: String,
    pub content: String,
    #[serde(default)]
    pub execution: Execution,
    #[serde(default)]
    pub model_override: Option<String>,
    /// Composer "Run on" tier: local|mesh|cloud|auto. NOT `cognitive_profile`.
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub clutch: Option<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub context_files: Vec<String>,
    #[serde(default)]
    pub active_skill: Option<String>,
    #[serde(default)]
    pub skill_exclusions: Vec<String>,
    #[serde(default)]
    pub grounding_check_enabled: Option<bool>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub allow_duplicate: Option<bool>,
    #[serde(default)]
    pub chat_session_id: Option<String>,
    /// Interaction mode from the composer (plan|act|verify). Was dropped
    /// end-to-end pre-fix: forwarded server-side onto
    /// `SubmitTaskInput::mode` -> `enqueue_hints.mode` (see
    /// `control_plane::submit_task_params`) on the background path only —
    /// the sync path has no `mode` concept in `vox_chat_message`.
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatTurnDto {
    /// `0` on the background branch: that path persists no assistant row.
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub task_id: Option<String>,
    pub model_id: Option<String>,
    pub latency_ms: Option<u64>,
    pub selection_reason: Option<String>,
    pub grounding_flagged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<String>,
    /// Turn events derived from tool results this turn (Phase E Task E1) —
    /// empty on the background branch, which persists no assistant row.
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
    /// Set on the `Execution::Plan` branch only — lets the frontend point
    /// `PlanPanel` at the DAG `vox_plan` just persisted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_version: Option<i64>,
}

/// Wrapper every real dispatch call site applies (`format!("LLM error: {e}")`
/// — see `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs` and
/// siblings) before a backend error string reaches the GUI. Stripped before
/// classification so detection matches the wire shape, not the raw source.
const LLM_ERROR_WRAPPER_PREFIX: &str = "LLM error: ";

/// Typed classification of a `chat_turn` dispatch failure, for a GUI toast
/// that can react to the failure kind instead of pattern-matching a raw
/// string itself. `#[serde(tag = "kind", ...)]` so a future JSON-carrying
/// error surface can round-trip this directly; today it's Rust-internal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatTurnError {
    RateLimited {
        message: String,
    },
    ContextExceeded {
        message: String,
    },
    BudgetExceeded {
        message: String,
    },
    /// Fallback: anything not recognized as one of the above.
    Backend {
        message: String,
    },
}

/// Classifies a backend error string (as it actually reaches the GUI — see
/// `LLM_ERROR_WRAPPER_PREFIX`) into a [`ChatTurnError`]. Matches against the
/// real prefixes/`Display` formats production emits:
/// - `vox_actor_runtime::llm::RATE_LIMITED_PREFIX`
/// - `vox_actor_runtime::llm::CONTEXT_EXCEEDED_PREFIX`
/// - `BudgetGuardError::Exceeded`'s Display (`"{Daily|Session} budget of $…
///   exceeded (spent $…)"`, `crates/vox-orchestrator-mcp/.../budget_guard.rs`)
pub fn classify_turn_error(text: &str) -> ChatTurnError {
    let unwrapped = text.strip_prefix(LLM_ERROR_WRAPPER_PREFIX).unwrap_or(text);
    if unwrapped.starts_with(vox_actor_runtime::llm::RATE_LIMITED_PREFIX) {
        return ChatTurnError::RateLimited {
            message: unwrapped.to_string(),
        };
    }
    if unwrapped.starts_with(vox_actor_runtime::llm::CONTEXT_EXCEEDED_PREFIX) {
        return ChatTurnError::ContextExceeded {
            message: unwrapped.to_string(),
        };
    }
    if unwrapped.starts_with("Daily budget of $") || unwrapped.starts_with("Session budget of $") {
        return ChatTurnError::BudgetExceeded {
            message: unwrapped.to_string(),
        };
    }
    ChatTurnError::Backend {
        message: text.to_string(),
    }
}

fn non_blank(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// `vox_chat_message` args. `ChatMessageParams` publishes
/// `additionalProperties: true` via a hand-written literal, so unknown keys are
/// tolerated — but they are also silently ignored until the struct AND the
/// literal at `input_schemas.rs:626-628` are updated (Task A2).
pub fn sync_tool_args(input: &ChatTurnInput) -> serde_json::Value {
    let mut args = serde_json::json!({
        "prompt": input.content,
        "session_id": input.session_id,
    });
    let obj = args.as_object_mut().expect("json! object");
    if !input.context_files.is_empty() {
        obj.insert(
            "context_files".into(),
            serde_json::json!(input.context_files),
        );
    }
    if !input.skill_exclusions.is_empty() {
        obj.insert(
            "skill_exclusions".into(),
            serde_json::json!(input.skill_exclusions),
        );
    }
    for (key, val) in [
        ("model_override", non_blank(&input.model_override)),
        ("tier", non_blank(&input.tier)),
        ("clutch", non_blank(&input.clutch)),
        ("risk", non_blank(&input.risk)),
        ("active_skill", non_blank(&input.active_skill)),
        ("priority", non_blank(&input.priority)),
    ] {
        if let Some(v) = val {
            obj.insert(key.into(), serde_json::json!(v));
        }
    }
    // `ChatMessageParams` (crates/vox-orchestrator-mcp/src/chat_tools/params.rs)
    // needs a `dry_run: Option<bool>` field for the daemon to actually read
    // this — cross-file dependency on the parallel params.rs work. Emitted
    // here regardless so the wire carries it once that lands.
    if let Some(v) = input.dry_run {
        obj.insert("dry_run".into(), serde_json::json!(v));
    }
    args
}

/// Total mapping onto the existing background dispatch input.
pub fn background_input(input: &ChatTurnInput) -> SubmitTaskInput {
    SubmitTaskInput {
        description: input.content.clone(),
        files: input.context_files.clone(),
        priority: input.priority.clone(),
        session_id: Some(input.session_id.clone()),
        mode: input.mode.clone(),
        tier: input.tier.clone(),
        // `model_hint` is dead on the wire: the daemon's SUBMIT_TASK handler
        // never reads it. Only `tier` -> enqueue_hints.model_preference works.
        model_hint: None,
        allow_duplicate: input.allow_duplicate,
        dry_run: input.dry_run,
        active_skill: input.active_skill.clone(),
        clutch: input.clutch.clone(),
        risk: input.risk.clone(),
        model_override: input.model_override.clone(),
        task_category: None,
        grounding_check_enabled: input.grounding_check_enabled,
        // The real originating chat session when the caller sent one
        // (App.tsx's `activeSessionId`) — falls back to `session_id` only
        // when absent, since `session_id` itself can be a synthetic,
        // throwaway background-dispatch id (see `newBackgroundSessionId`
        // call sites in App.tsx) that points at no real transcript.
        chat_session_id: input
            .chat_session_id
            .clone()
            .or_else(|| Some(input.session_id.clone())),
    }
}

/// The one place a raw backend error string becomes a typed [`ChatTurnError`]
/// before crossing the Tauri IPC boundary — `run_sync`/`run_background` keep
/// returning `Result<_, String>` internally (every `?` site inside them stays
/// untouched), and this command classifies the final string once, so Tauri
/// serializes the tagged JSON (`{"kind": "...", ...}`) to the frontend
/// instead of a plain display string.
#[tauri::command]
pub async fn chat_turn(
    app_handle: tauri::AppHandle,
    input: ChatTurnInput,
    pool: State<'_, GuiDbPool>,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, ChatTurnError> {
    if input.session_id.trim().is_empty() {
        return Err(classify_turn_error("session_id must not be empty"));
    }
    if input.content.trim().is_empty() {
        return Err(classify_turn_error("content must not be empty"));
    }
    let result = match input.execution {
        Execution::Sync => run_sync(input, pool, daemon).await,
        Execution::Background => run_background(app_handle, input, daemon).await,
        Execution::Plan => run_plan(input, daemon).await,
    };
    result.map_err(|e| classify_turn_error(&e))
}

async fn run_sync(
    input: ChatTurnInput,
    pool: State<'_, GuiDbPool>,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    let addr = daemon.ensure().await?;
    let client = match daemon.token().await {
        Some(token) => vox_orchestrator::orch_daemon::OrchDaemonClient::with_token(addr, token),
        None => vox_orchestrator::orch_daemon::OrchDaemonClient::new(addr),
    };
    let envelope = client
        .call(
            vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": "vox_chat_message", "args": sync_tool_args(&input) }),
        )
        .await
        .map_err(|e| e.to_string())?;
    let reply = crate::commands::chat::parse_chat_message_envelope(&envelope)?;
    let grounding_flagged = if input.grounding_check_enabled == Some(true) {
        Some(vox_orchestrator::grounding::assess_reply_confidence(&reply.content).flagged)
    } else {
        None
    };
    let db = pool.handle()?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(map_db_err)?;
    let dto = crate::commands::chat::persist_assistant_reply(
        &db,
        conv_id,
        &reply.content,
        reply.model_id.as_deref(),
        reply.latency_ms,
        reply.selection_reason.as_deref(),
        grounding_flagged,
    )
    .await?;
    Ok(ChatTurnDto {
        id: dto.id,
        role: dto.role,
        content: dto.content,
        created_at: dto.created_at,
        // Always None today: vox_chat_message's payload has no task_id.
        // Phase D adds one; do not build correlation on this yet.
        task_id: None,
        model_id: dto.model_id,
        latency_ms: dto.latency_ms,
        selection_reason: dto.selection_reason,
        grounding_flagged: dto.grounding_flagged,
        duplicate_of: None,
        events: reply.events,
        plan_session_id: None,
        plan_version: None,
    })
}

/// Dispatch only. Deliberately persists NOTHING: today's background path writes
/// no assistant row, and a persisted "Dispatched as task #N" receipt would
/// hydrate on reload in place of the live task bubble.
async fn run_background(
    app_handle: tauri::AppHandle,
    input: ChatTurnInput,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    let result = crate::commands::control_plane::submit_orchestrator_task(
        app_handle,
        background_input(&input),
        daemon,
    )
    .await?;
    Ok(ChatTurnDto {
        id: 0,
        role: "assistant".to_string(),
        content: String::new(),
        created_at: String::new(),
        task_id: result.task_id,
        model_id: None,
        latency_ms: None,
        selection_reason: None,
        grounding_flagged: None,
        duplicate_of: result.duplicate_of,
        events: vec![],
        plan_session_id: None,
        plan_version: None,
    })
}

/// `vox_plan` args for the GUI's `/plan` path. Exactly `{goal, session_id,
/// require_approval: true}` — `vox_plan`'s schema is `additionalProperties:
/// false`, so a stray `mode`/`prompt` key (present on `ChatTurnInput` for the
/// other two execution branches) is a hard reject, not a silently-ignored
/// extra.
pub fn plan_tool_args(input: &ChatTurnInput) -> serde_json::Value {
    serde_json::json!({
        "goal": input.content,
        "session_id": input.session_id,
        "require_approval": true,
    })
}

/// `vox_plan`'s `ToolResult<PlanResult>` envelope has a flat `data` object
/// (unlike `vox_chat_message`'s `data.message.content`) — a dedicated,
/// minimal parser rather than overloading `parse_chat_message_envelope` with
/// a second response shape.
fn parse_plan_envelope(envelope: &serde_json::Value) -> Result<ChatTurnDto, String> {
    let success = envelope
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !success {
        return Err(envelope
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("vox_plan failed with no error detail")
            .to_string());
    }
    let data = envelope
        .get("data")
        .ok_or_else(|| "vox_plan succeeded with no data".to_string())?;
    let plan_md = data
        .get("plan_md")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let plan_session_id = data
        .get("plan_session_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let plan_version = data.get("plan_version").and_then(|v| v.as_i64());
    Ok(ChatTurnDto {
        id: 0,
        role: "assistant".to_string(),
        content: plan_md,
        created_at: String::new(),
        task_id: None,
        model_id: None,
        latency_ms: None,
        selection_reason: None,
        grounding_flagged: None,
        duplicate_of: None,
        events: vec![],
        plan_session_id,
        plan_version,
    })
}

/// Dispatch a `/plan` turn: calls `vox_plan` (not `vox_chat_message`) and
/// returns the `plan_session_id`/`plan_version` it persisted, instead of an
/// assistant reply — there is no chat message to persist here.
async fn run_plan(
    input: ChatTurnInput,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    let addr = daemon.ensure().await?;
    let client = match daemon.token().await {
        Some(token) => vox_orchestrator::orch_daemon::OrchDaemonClient::with_token(addr, token),
        None => vox_orchestrator::orch_daemon::OrchDaemonClient::new(addr),
    };
    let envelope = client
        .call(
            vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": "vox_plan", "args": plan_tool_args(&input) }),
        )
        .await
        .map_err(|e| e.to_string())?;
    parse_plan_envelope(&envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Fields both `ChatTurnInput` and `SubmitTaskInput` must carry so routing
    /// stays in sync between the two. Only this test reads it, so it lives here
    /// rather than as a crate-public const nothing else consumes.
    const ROUTING_FIELDS: &[&str] = &[
        "priority",
        "model_override",
        "tier",
        "dry_run",
        "active_skill",
        "clutch",
        "risk",
        "allow_duplicate",
        "grounding_check_enabled",
        "chat_session_id",
    ];

    fn keys_of<T: serde::Serialize>(v: &T) -> BTreeSet<String> {
        serde_json::to_value(v)
            .expect("serializes")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    /// Reflects over BOTH structs. The first draft's version filtered
    /// SubmitTaskInput's keys BY a hand-maintained constant and then compared
    /// the result to that same constant — a tautology that passed when a field
    /// was added to one struct only. This one cannot.
    #[test]
    fn routing_fields_match_submit_task_input() {
        let turn = keys_of(&ChatTurnInput::default());
        let submit = keys_of(&crate::commands::control_plane::SubmitTaskInput::default());
        let shared: BTreeSet<String> = ROUTING_FIELDS.iter().map(|s| s.to_string()).collect();
        let missing_from_turn: Vec<_> = shared.difference(&turn).collect();
        let missing_from_submit: Vec<_> = shared.difference(&submit).collect();
        assert!(
            missing_from_turn.is_empty(),
            "ChatTurnInput is missing routing fields: {missing_from_turn:?}"
        );
        assert!(
            missing_from_submit.is_empty(),
            "SubmitTaskInput is missing routing fields: {missing_from_submit:?}"
        );
    }

    #[test]
    fn classifies_the_strings_production_actually_emits() {
        // Every error is wrapped as `format!("LLM error: {e}")` before it
        // reaches the GUI — hence `contains`, not `starts_with`.
        assert!(matches!(
            classify_turn_error("LLM error: RATE_LIMITED: openrouter free tier 50/day"),
            ChatTurnError::RateLimited { .. }
        ));
        assert!(matches!(
            classify_turn_error("LLM error: CONTEXT_LENGTH_EXCEEDED: 200000 > 128000"),
            ChatTurnError::ContextExceeded { .. }
        ));
        // Real BudgetGuardError Display, NOT the invented "budget exceeded: …".
        assert!(matches!(
            classify_turn_error("Session budget of $5.00 exceeded (spent $5.10)"),
            ChatTurnError::BudgetExceeded { .. }
        ));
        assert!(matches!(
            classify_turn_error("Daily budget of $20.00 exceeded (spent $20.03)"),
            ChatTurnError::BudgetExceeded { .. }
        ));
        assert!(matches!(
            classify_turn_error("connection refused"),
            ChatTurnError::Backend { .. }
        ));
    }

    #[test]
    fn plan_tool_args_carries_exactly_goal_session_id_require_approval() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "add a health endpoint", "mode": "plan"
        }))
        .expect("input");
        let args = plan_tool_args(&input);
        assert_eq!(args["goal"], "add a health endpoint");
        assert_eq!(args["session_id"], "s1");
        assert_eq!(args["require_approval"], true);
        let obj = args.as_object().expect("json! object");
        assert_eq!(
            obj.len(),
            3,
            "vox_plan's schema is additionalProperties:false — a stray key is a hard reject: {obj:?}"
        );
        assert!(obj.get("mode").is_none());
        assert!(obj.get("prompt").is_none());
    }

    #[test]
    fn execution_defaults_to_sync() {
        let input: ChatTurnInput =
            serde_json::from_value(serde_json::json!({"session_id":"s","content":"hi"}))
                .expect("minimal");
        assert_eq!(input.execution, Execution::Sync);
        assert!(input.context_files.is_empty());
    }

    /// The regression this whole plan exists for.
    #[test]
    fn sync_tool_args_carry_every_composer_control() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "harden the crypto invariants",
            "model_override": "openrouter/anthropic/claude-opus-5",
            "tier": "cloud",
            "context_files": ["crates/vox-crypto/src/lib.rs"],
            "active_skill": "ponytail",
            "clutch": "genius", "risk": "low",
            "priority": "urgent", "dry_run": true
        }))
        .expect("input");
        let args = sync_tool_args(&input);
        assert_eq!(args["prompt"], "harden the crypto invariants");
        assert_eq!(args["model_override"], "openrouter/anthropic/claude-opus-5");
        assert_eq!(args["tier"], "cloud");
        assert_eq!(args["context_files"][0], "crates/vox-crypto/src/lib.rs");
        assert_eq!(args["active_skill"], "ponytail");
        assert_eq!(args["clutch"], "genius");
        // Bug 2: priority/dry_run were silently dropped on the sync path.
        assert_eq!(args["priority"], "urgent");
        assert_eq!(args["dry_run"], true);
        // `cognitive_profile` must NEVER be set from the tier: its values are
        // fast|reasoning|creative, and setting it switches the turn off the
        // agent loop onto mcp_infer_completion, killing tool calls and
        // selection_reason.
        assert!(args.get("cognitive_profile").is_none());
    }

    #[test]
    fn sync_tool_args_omit_blank_optionals() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "hi", "model_override": "   ", "tier": ""
        }))
        .expect("input");
        let args = sync_tool_args(&input);
        assert!(args.get("model_override").is_none());
        assert!(args.get("tier").is_none());
    }

    #[test]
    fn background_input_maps_every_routing_field() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "refactor the parser",
            "execution": "background",
            "model_override": "m1", "tier": "mesh",
            "clutch": "efficiency", "risk": "moderate",
            "context_files": ["a.rs", "b.rs"], "priority": "urgent",
            "dry_run": true, "active_skill": "ponytail", "allow_duplicate": false,
            "mode": "act"
        }))
        .expect("input");
        let out = background_input(&input);
        assert_eq!(out.description, "refactor the parser");
        assert_eq!(out.files, vec!["a.rs".to_string(), "b.rs".to_string()]);
        assert_eq!(out.model_override.as_deref(), Some("m1"));
        assert_eq!(out.tier.as_deref(), Some("mesh"));
        assert_eq!(out.clutch.as_deref(), Some("efficiency"));
        assert_eq!(out.priority.as_deref(), Some("urgent"));
        assert_eq!(out.allow_duplicate, Some(false));
        assert_eq!(out.chat_session_id.as_deref(), Some("s1"));
        // Bug 1: mode (e.g. `/spawn`'s "act") must reach SubmitTaskInput.mode
        // -> control_plane::submit_task_params -> enqueue_hints.mode.
        assert_eq!(out.mode.as_deref(), Some("act"));
        assert!(out.task_category.is_none());
    }

    /// Bug 3 regression test. The naive `assert_eq!(out.chat_session_id.as_deref(),
    /// Some("s1"))` in the test above happens to pass even with the old
    /// `chat_session_id: Some(input.session_id.clone())` bug, because its
    /// fixture's `session_id` ("s1") IS the value under test — the bug (always
    /// echoing `session_id`, ignoring any real `chat_session_id`) is invisible
    /// unless the two differ, as they do for every real background dispatch
    /// (`session_id` is a synthetic `newBackgroundSessionId()`, `chat_session_id`
    /// is the real originating chat session).
    #[test]
    fn background_input_prefers_chat_session_id_over_session_id() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "bg-synthetic-1", "content": "spawn a sub-agent",
            "execution": "background",
            "chat_session_id": "real-chat-session-42"
        }))
        .expect("input");
        let out = background_input(&input);
        assert_eq!(out.chat_session_id.as_deref(), Some("real-chat-session-42"));

        // Fallback: a caller that never sets chat_session_id (e.g. any future
        // dispatch path that forgets it) still gets a usable value rather than
        // None.
        let input_no_chat_session: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "bg-synthetic-2", "content": "spawn a sub-agent",
            "execution": "background"
        }))
        .expect("input");
        let out2 = background_input(&input_no_chat_session);
        assert_eq!(out2.chat_session_id.as_deref(), Some("bg-synthetic-2"));
    }
}
