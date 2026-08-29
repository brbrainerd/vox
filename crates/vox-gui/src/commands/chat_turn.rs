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
pub const ROUTING_FIELDS: &[&str] = &[
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Execution {
    #[default]
    Sync,
    Background,
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
    ] {
        if let Some(v) = val {
            obj.insert(key.into(), serde_json::json!(v));
        }
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
        mode: None,
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
        chat_session_id: Some(input.session_id.clone()),
    }
}

#[tauri::command]
pub async fn chat_turn(
    app_handle: tauri::AppHandle,
    input: ChatTurnInput,
    pool: State<'_, GuiDbPool>,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    if input.session_id.trim().is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    if input.content.trim().is_empty() {
        return Err("content must not be empty".to_string());
    }
    match input.execution {
        Execution::Sync => run_sync(input, pool, daemon).await,
        Execution::Background => run_background(app_handle, input, daemon).await,
    }
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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
            "clutch": "genius", "risk": "low"
        }))
        .expect("input");
        let args = sync_tool_args(&input);
        assert_eq!(args["prompt"], "harden the crypto invariants");
        assert_eq!(args["model_override"], "openrouter/anthropic/claude-opus-5");
        assert_eq!(args["tier"], "cloud");
        assert_eq!(args["context_files"][0], "crates/vox-crypto/src/lib.rs");
        assert_eq!(args["active_skill"], "ponytail");
        assert_eq!(args["clutch"], "genius");
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
            "dry_run": true, "active_skill": "ponytail", "allow_duplicate": false
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
        assert!(out.task_category.is_none());
    }
}
