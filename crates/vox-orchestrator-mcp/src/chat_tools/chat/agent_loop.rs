//! Task 1.3c (harness implementation spec §3.3): the real multi-turn, tool-calling
//! agent loop — the piece that finally wires together Tasks 1.1 (conversation
//! history), 1.2 (tool selection), 1.3a (tool_calls parsed from the wire response),
//! and 1.3b (tool_calls/tool_call_id carried on request messages).
//!
//! Finding F24 ("no code path in Vox ever passes tools to a model"): before this
//! module, every chat completion built its request with `tools: None`. This module
//! is the first that (a) selects a bounded tool subset for the turn
//! ([`crate::llm_bridge::tool_selection::select_tools_for_turn`]), (b) sends it to
//! the model via [`vox_actor_runtime::llm::llm_chat`] (the only call path that
//! carries `tools`/`tool_calls` end to end — see Task 1.3a/1.3b), (c) dispatches any
//! requested calls through [`crate::dispatch::handle_tool_call_with_mode`], and (d)
//! feeds the results back to the model as `role: "tool"` messages, looping until the
//! model stops requesting tools or a hard iteration bound is hit.
//!
//! Deliberately out of scope here (see Task 1.3c notes in the harness implementation
//! spec): per-turn skill-pin plumbing for `active_skill_id` (the caller can pass one
//! in if it has it cheaply at hand; this module does not go hunting for it), and any
//! new permission-mode taxonomy or dispatch mechanism — `permission_mode` is passed
//! straight through to `handle_tool_call_with_mode` exactly as that function already
//! requires (an authenticated transport-layer value, never LLM-composed).

use vox_actor_runtime::ActivityOptions;
use vox_actor_runtime::llm::{
    LlmChatMessage, LlmConfig, LlmResponse, LlmToolDef, llm_chat, llm_stream_activity,
};
use vox_mcp_registry::TOOL_REGISTRY;
use vox_orchestrator::events::AgentEventKind;
use vox_orchestrator::models::{ModelSpec, ProviderType};
use vox_orchestrator::types::AgentId;

use crate::input_schemas::tool_input_schema;
use crate::llm_bridge::tool_selection::{DEFAULT_MAX_TOOLS, TurnContext, select_tools_for_turn};
use crate::server_state::ServerState;

/// Serializes test-only access to the process-global `OPENROUTER_BASE_URL` /
/// `OPENROUTER_API_KEY` env vars. Any `#[cfg(test)]` module in this crate that
/// mutates those vars (to point them at a `wiremock` server) must lock this —
/// a per-file lock is not enough, since `cargo test` runs test binaries'
/// tests concurrently across modules and a private per-module lock does not
/// serialize against a sibling module's private lock, letting two tests
/// stomp each other's env var value mid-request.
#[cfg(test)]
pub(crate) static CHAT_MESSAGE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Narrow `ModelSpec -> LlmConfig` mapper (Task 1.3d, F24 wiring).
///
/// Deliberately covers only the two simplest, most common provider shapes:
///
/// - [`ProviderType::OpenRouter`]: Vox's de-facto default provider for the vast
///   majority of catalog models. Maps to [`LlmConfig::openrouter`], which already
///   resolves the API key from `vox_secrets::SecretId::OpenRouterApiKey` — no
///   fallback/vision/budget logic required.
/// - [`ProviderType::Ollama`]: local inference with a fixed, well-known URL shape
///   (`$OLLAMA_URL/v1/chat/completions`), mirroring the existing
///   `ModelRegistry::get_llm_config` conversion for the same provider type
///   (`crates/vox-orchestrator/src/models/registry.rs`).
///
/// Returns `None` for every other [`ProviderType`] (`GoogleDirect`,
/// `HuggingFaceRouter`, `VoxLocal`, `PopuliMesh`, `Anthropic`, `Mistral`,
/// `DeepSeek`, `SambaNova`, `Groq`, `Cerebras`, `Custom`) — those require the
/// provider-specific fallback chains, dedicated-endpoint resolution, or
/// unreachable-provider handling that only
/// `crate::llm_bridge::infer::mcp_infer_completion` implements, and this mapper
/// deliberately does not attempt to replicate that pipeline. Callers must fall
/// back to `mcp_infer_completion` when this returns `None`.
#[must_use]
pub(crate) fn model_spec_to_llm_config(spec: &ModelSpec) -> Option<LlmConfig> {
    match spec.provider_type {
        ProviderType::OpenRouter => Some(LlmConfig::openrouter(spec.id.clone())),
        ProviderType::Ollama => {
            let base_url = vox_secrets::resolve_secret(vox_secrets::SecretId::OllamaUrl)
                .expose()
                .filter(|s: &&str| !s.trim().is_empty())
                .map(|u: &str| format!("{}/v1/chat/completions", u.trim_end_matches('/')))?;
            Some(LlmConfig {
                provider: "ollama".to_string(),
                model: spec.id.clone(),
                cost_per_1k: None,
                base_url: Some(base_url),
                api_key: None,
                temperature: None,
                top_p: None,
                max_tokens: Some(spec.max_tokens),
                response_format: None,
                tools: None,
                tool_choice: None,
                timeout_ms: None,
                telemetry_session_id: None,
                telemetry_user_id: None,
                telemetry_task_category: None,
                telemetry_strength_tag: None,
                telemetry_trace_id: None,
                telemetry_attempt_number: None,
                telemetry_skip_interaction: true,
            })
        }
        ProviderType::GoogleDirect
        | ProviderType::HuggingFaceRouter
        | ProviderType::VoxLocal
        | ProviderType::PopuliMesh
        | ProviderType::Anthropic
        | ProviderType::Mistral
        | ProviderType::DeepSeek
        | ProviderType::SambaNova
        | ProviderType::Groq
        | ProviderType::Cerebras
        | ProviderType::Custom(_) => None,
    }
}

/// Hard bound on the number of model round-trips within a single `run_agent_turn`
/// call. Each iteration is: one `llm_chat` call, plus (if it requested tools) one
/// `handle_tool_call_with_mode` dispatch per call before looping again. 8 is chosen
/// to comfortably cover realistic tool-use chains (a typical "look something up,
/// then answer" turn is 2-3 iterations) while guaranteeing the loop cannot spin
/// forever if a model pathologically keeps requesting tools — this is a safety
/// bound, not a tuning knob callers are expected to raise routinely.
pub(crate) const DEFAULT_MAX_ITERATIONS: usize = 8;

/// Result of running one agent turn to completion (or to the iteration bound).
///
/// Called from `vox_chat_message`'s live entrypoint (`message.rs`) for the subset
/// of provider/model choices [`model_spec_to_llm_config`] can map without the
/// full `mcp_infer_completion` fallback pipeline (Task 1.3d, F24).
#[derive(Debug, Clone)]
pub struct AgentTurnOutcome {
    /// The final assistant-facing text. When the iteration bound is hit before the
    /// model stops requesting tools, this is the last assistant text seen (which may
    /// be empty) with [`AgentTurnOutcome::hit_iteration_limit`] set to `true` so
    /// callers can surface that distinctly rather than silently truncating.
    pub final_text: String,
    /// Model id reported by the last successful `llm_chat` response, or empty string
    /// if every attempt failed before any response was received.
    pub model_used: String,
    /// Total number of individual tool calls dispatched across every iteration of
    /// this turn (for observability/tests — not the same as iteration count, since
    /// one iteration's response may request several calls at once).
    pub tool_calls_made: usize,
    /// `true` if the loop stopped only because [`DEFAULT_MAX_ITERATIONS`] (or the
    /// caller-supplied `max_iterations`) was reached while the model was still
    /// requesting tools, rather than because the model returned a final answer.
    pub hit_iteration_limit: bool,
    /// Sum of `prompt_tokens + completion_tokens` reported by the wire response
    /// across every `llm_chat` round-trip made during this turn (for transcript
    /// bookkeeping — `message.rs` records this alongside the persisted turn).
    pub total_tokens: u64,
    /// Chat-turn-visible events derived from tool RESULTS during this turn (see
    /// [`turn_event_for_result`]) — e.g. a skill activation chip. Empty unless a
    /// dispatched tool call both matches a known event-worthy tool AND actually
    /// succeeded.
    pub events: Vec<serde_json::Value>,
    /// Wall-clock latency of the final iteration's `llm_chat`/`stream_final_answer` call, in
    /// ms. `None` only if every iteration failed before any response was received (in which
    /// case the turn as a whole already errored out before `AgentTurnOutcome` is built).
    pub latency_ms: Option<u64>,
    /// Task M3: time to first token, in ms, from the final iteration's response. Only real on
    /// the streaming path (`stream_tokens: true`); the non-streaming path reports
    /// `latency_ms` here (see [`LlmResponse::ttft_ms`]'s doc comment).
    pub ttft_ms: Option<u64>,
    /// Task M3: time per output token, in ms, from the final iteration's response. `None`
    /// when that iteration had zero completion tokens.
    pub tpot_ms: Option<f64>,
}

/// Max chars of a model-authored string (e.g. a raw skill id) echoed into a
/// turn event. Events render in trusted, system-styled chrome, so anything
/// derived from model/tool-call content must be bounded before it reaches
/// the UI, independent of the id-shape validation below.
const TURN_EVENT_STRING_CAP: usize = 200;

/// A skill id is only ever a short slug (see `SkillManifest::id` producers —
/// installer-assigned, never user/model free text at creation time). Reject
/// anything else (path separators, `..`, whitespace, control chars) rather
/// than echoing it into system-styled UI: a model can put an arbitrary string
/// in a tool call's `id` argument (including content it read from untrusted
/// tool output earlier in the transcript), so this is a format allowlist, not
/// a registry lookup — [`run_agent_turn`]'s call site additionally confirms
/// the id resolves in the real skill registry before trusting it further.
fn is_plausible_skill_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= TURN_EVENT_STRING_CAP
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Derive a chat-turn-visible event from a dispatched tool call's RESULT —
/// never from the call's arguments alone. `success` must reflect whether the
/// dispatch actually succeeded (e.g. `!tool_json_envelope_is_error(&content)`
/// at the call site), not merely whether the model requested the call.
///
/// Security rationale (see module docs and
/// `docs/superpowers/plans/2026-08-28-chat-harness-unification.md` Phase E
/// Task E1): a model can put anything in `args`, including text an earlier,
/// untrusted tool result injected into the conversation. Gating purely on
/// `success` — which is derived from what the tool dispatcher actually did,
/// not from what the model claimed — is what keeps a denied/errored/unknown
/// call from rendering a trusted-looking "skill activated" chip.
///
/// `result_content` is the raw dispatch response body (a [`crate::params::ToolResult`]
/// JSON envelope, e.g. `{"success":true,"data":{"agent_id":7,...}}`) — needed for
/// the delegation event (Phase D Task D3) because the spawned `agent_id` is
/// server-generated and only exists in the RESULT, never in `args`.
///
/// Returns `None` when `success` is `false`, or when `tool_name` is not a
/// tool this function knows how to turn into an event.
pub(crate) fn turn_event_for_result(
    tool_name: &str,
    args: &serde_json::Value,
    result_content: &str,
    success: bool,
) -> Option<serde_json::Value> {
    if !success {
        return None;
    }
    match tool_name {
        "vox_skill_use" => {
            let raw_id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let skill_id = if is_plausible_skill_id(raw_id) {
                raw_id.to_string()
            } else {
                "unknown".to_string()
            };
            Some(serde_json::json!({
                "kind": "skill_activated",
                "skill_id": skill_id,
            }))
        }
        "vox_spawn_agent" | "vox_submit_task" => {
            // Phase D Task D3: a "delegation" transcript row correlating this
            // turn to the agent/task it spawned. `agent_id` is always present
            // on both tools' success payloads (`SpawnAgentParams`'s handler and
            // `SubmitTaskResponse`); `task_id` only on `vox_submit_task`.
            let envelope: serde_json::Value = serde_json::from_str(result_content).ok()?;
            let data = envelope.get("data")?;
            let agent_id = data.get("agent_id").and_then(serde_json::Value::as_u64)?;
            Some(serde_json::json!({
                "kind": "delegation_spawned",
                "tool": tool_name,
                "agent_id": agent_id,
                "task_id": data.get("task_id").and_then(serde_json::Value::as_u64),
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod turn_event_tests {
    use super::turn_event_for_result;
    use serde_json::json;

    #[test]
    fn skill_event_comes_from_the_result_not_the_call() {
        // Emitting from args would render "skill activated · X" for a call that
        // was denied, unknown, or errored — letting injected content assert in
        // system-styled UI that a trusted skill ran.
        assert!(
            turn_event_for_result("vox_skill_use", &json!({"id":"ponytail"}), "", true).is_some()
        );
        assert!(
            turn_event_for_result("vox_skill_use", &json!({"id":"ponytail"}), "", false).is_none()
        );
    }

    #[test]
    fn unknown_skill_ids_are_labelled_unknown() {
        let ev =
            turn_event_for_result("vox_skill_use", &json!({"id":"../../etc/passwd"}), "", true)
                .expect("event");
        assert_eq!(ev["skill_id"], "unknown");
    }

    /// Phase D Task D3: a successful `vox_spawn_agent` call must produce a
    /// transcript-visible delegation event carrying the spawned `agent_id` —
    /// read from the dispatch RESULT (never from `args`, which a model
    /// controls and which never contains the server-assigned id anyway).
    #[test]
    fn spawn_agent_success_emits_delegation_event_with_agent_id() {
        let result = r#"{"success":true,"data":{"agent_id":42,"name":"child","dynamic":true}}"#;
        let ev = turn_event_for_result("vox_spawn_agent", &json!({}), result, true)
            .expect("delegation event");
        assert_eq!(ev["kind"], "delegation_spawned");
        assert_eq!(ev["tool"], "vox_spawn_agent");
        assert_eq!(ev["agent_id"], 42);
        assert!(ev["task_id"].is_null());
    }

    #[test]
    fn submit_task_success_emits_delegation_event_with_task_and_agent_id() {
        let result = r#"{"success":true,"data":{"task_id":9,"agent_id":3}}"#;
        let ev = turn_event_for_result("vox_submit_task", &json!({}), result, true)
            .expect("delegation event");
        assert_eq!(ev["agent_id"], 3);
        assert_eq!(ev["task_id"], 9);
    }

    #[test]
    fn failed_spawn_emits_no_delegation_event() {
        // Same rationale as the skill test above: a denied/errored call must
        // not render a trusted-looking "delegated" chip.
        let result = r#"{"success":false,"error":"boom"}"#;
        assert!(turn_event_for_result("vox_spawn_agent", &json!({}), result, false).is_none());
    }

    #[test]
    fn malformed_result_body_yields_no_event_rather_than_panicking() {
        assert!(turn_event_for_result("vox_spawn_agent", &json!({}), "not json", true).is_none());
    }
}

/// Task G1: attempt one iteration's model call via [`llm_stream_activity`],
/// emitting `AgentEventKind::TokenStreamed { session_id, .. }` on the existing
/// agent-events bus as each content chunk arrives, and returning a synthetic
/// [`LlmResponse`] built from the accumulated text. Returns `None` — meaning
/// "the caller should fall back to a plain, non-streaming `llm_chat` call for
/// this iteration" — whenever streaming can't be trusted to have captured the
/// real answer:
///
/// - the stream failed to connect or errored mid-stream, or
/// - it produced no text at all, which is the observable signature of the
///   model requesting a tool call instead of answering directly:
///   `vox_llm_egress::wire::stream_once` only parses `delta.content` out of
///   the SSE frames (see its doc comment), so `delta.tool_calls` fragments a
///   provider sends while the model is composing a tool call are silently
///   dropped rather than surfaced — there's no way to distinguish "the model
///   is calling a tool" from "the model streamed nothing" from inside this
///   function today. Treating an empty stream as "assume it needs the
///   non-streaming fallback" costs one extra round-trip on tool-call turns
///   (which weren't streaming usefully anyway) while keeping tool dispatch
///   exactly as correct as it was before this task.
///
/// The synthetic response has no real `prompt_tokens`/provider-reported
/// `completion_tokens` (streaming discards usage accounting) or `tool_calls`
/// (by construction — a non-empty stream is exactly the case this function
/// treats as "not a tool call"); `completion_tokens` is estimated the same
/// way `chat/message.rs` already estimates attention-budget spend elsewhere
/// in this crate.
///
/// ponytail: this ceiling (fallback-on-empty rather than genuinely streaming
/// tool-call turns) is what keeps this task's diff to one call site instead
/// of teaching `vox-llm-egress` to parse `delta.tool_calls`. Revisit if the
/// duplicate-call cost on tool-heavy chat turns becomes measurable.
async fn stream_final_answer(
    state: &ServerState,
    activity_options: &ActivityOptions,
    messages: &[LlmChatMessage],
    config: LlmConfig,
    session_id: Option<&str>,
) -> Option<LlmResponse> {
    use futures_util::StreamExt;

    let model = config.model.clone();
    let mut stream = match llm_stream_activity(activity_options, messages.to_vec(), config).await {
        vox_actor_runtime::ActivityResult::Ok(s) => s,
        vox_actor_runtime::ActivityResult::Failed(_)
        | vox_actor_runtime::ActivityResult::Cancelled => return None,
    };

    let bus = state.orchestrator.event_bus();
    let agent_id = AgentId(0); // chat's pseudo-agent id, matching message.rs's convention.
    let start = std::time::Instant::now();
    let mut ttft_ms: Option<u64> = None;
    let mut content = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                if chunk.is_empty() {
                    continue;
                }
                // Task M3: the FIRST non-empty chunk is the only meaningful "time to first
                // token" reading — later chunks are generation throughput, not connect+queue
                // latency.
                if ttft_ms.is_none() {
                    ttft_ms = Some(start.elapsed().as_millis() as u64);
                }
                content.push_str(&chunk);
                bus.emit(AgentEventKind::TokenStreamed {
                    agent_id,
                    text: chunk,
                    session_id: session_id.map(str::to_string),
                });
            }
            // Mid-stream transport failure: fall back rather than surfacing a
            // partial/garbled answer as final.
            Err(_) => return None,
        }
    }
    if content.trim().is_empty() {
        return None;
    }
    // `latency_ms` used to be hardcoded `0` here (Task M3) -- same class of bug Task M0
    // fixed for the non-streaming path's `infer_with_retry`, just undiscovered until this
    // task went looking for a place to measure TTFT.
    let latency_ms = start.elapsed().as_millis() as u64;
    let completion_tokens =
        vox_orchestrator::compaction::CompactionEngine::estimate_tokens(&content) as u32;
    // Generation-phase throughput: time AFTER the first token arrived, divided across the
    // tokens that followed it. `ttft_ms` is always `Some` here (content is non-empty, so the
    // loop above set it on the first chunk).
    let tpot_ms = ttft_ms.and_then(|t| {
        (completion_tokens > 0)
            .then(|| latency_ms.saturating_sub(t) as f64 / completion_tokens as f64)
    });
    Some(LlmResponse {
        content,
        prompt_tokens: 0,
        completion_tokens,
        model,
        cost_usd: None,
        tool_calls: None,
        latency_ms,
        cache_read_tokens: 0,
        ttft_ms,
        tpot_ms,
    })
}

/// Run one user turn of the tool-calling agent loop to completion.
///
/// `messages` passed to the first `llm_chat` call is `[system] + prior_conversation
/// + [user]`; callers build `prior_conversation` via
/// [`super::conversation::load_conversation`] and the system prompt via
/// [`crate::chat_tools::build_system_prompt_with_skill`] — this function does not
///   duplicate either concern, it only takes the already-assembled strings/history.
///
/// Loops: select tools for the turn, call `llm_chat` with them, and if the response
/// carries `tool_calls`, dispatch each one and append the assistant tool-call
/// message plus one `role: "tool"` result message per call, then loop. Stops as
/// soon as a response has no tool_calls (returns its `content` as the final
/// answer), or after `max_iterations` model round-trips (whichever comes first —
/// see [`DEFAULT_MAX_ITERATIONS`] for why this bound must be real and finite).
///
/// `stream_tokens`: Task G1 (chat-harness-unification plan, Phase G). When
/// `true`, each iteration first tries [`llm_stream_activity`] instead of
/// [`llm_chat`], emitting `AgentEventKind::TokenStreamed { session_id, .. }`
/// on `state.orchestrator.event_bus()` — the SAME bus `orch_daemon` already
/// forwards to the GUI's `vox://agent-events` listener for background-task
/// streaming — as each content chunk arrives, so the GUI's quick-chat bubble
/// fills in progressively instead of only updating once the whole reply is
/// back. `vox_llm_egress::wire::stream_once` only parses `delta.content`, not
/// `delta.tool_calls`, so a turn where the model requests a tool call instead
/// of answering directly streams zero chunks; that (or any other stream
/// error) falls back to one ordinary non-streaming `llm_chat` call so tool
/// dispatch is never affected — see [`stream_final_answer`]'s doc comment.
/// `false` (every pre-existing caller) reproduces the exact prior behavior:
/// one plain `llm_chat` call per iteration, no bus emissions.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_agent_turn(
    state: &ServerState,
    session_id: Option<&str>,
    prior_conversation: Vec<LlmChatMessage>,
    system_prompt: String,
    user_message: String,
    permission_mode: Option<&str>,
    active_skill_id: Option<String>,
    llm_config_template: LlmConfig,
    max_iterations: usize,
    stream_tokens: bool,
) -> Result<AgentTurnOutcome, String> {
    let mut harness_scorer = super::harness_issue_scorer::HarnessIssueScorer::new();
    // Read once per turn, not per tool call — the flag cannot change mid-turn
    // (no code path mutates it during a single `run_agent_turn` invocation).
    let harness_detection_enabled = {
        let cfg_handle = state.orchestrator.config_handle();
        crate::sync_poison::poison_rw_read(cfg_handle.read(), "orchestrator config")
            .map(|cfg| cfg.harness_issue_detection_enabled)
            .unwrap_or(false)
    };
    let mut messages: Vec<LlmChatMessage> = Vec::with_capacity(prior_conversation.len() + 2);
    messages.push(LlmChatMessage {
        role: "system".into(),
        content: system_prompt,
        ..Default::default()
    });
    messages.extend(prior_conversation);
    messages.push(LlmChatMessage {
        role: "user".into(),
        content: user_message,
        ..Default::default()
    });

    let turn_ctx = TurnContext {
        permission_mode: permission_mode.map(str::to_string),
        lanes: vec!["ai", "app"],
        active_skill_id,
        max_tools: DEFAULT_MAX_TOOLS,
        // Never offer chat-turn-entry tools (`vox_chat_*`, e.g. `vox_chat_message`)
        // as callable tools inside an agent turn. `select_tools_for_turn`'s
        // general-purpose skill-permission filter
        // (`skill_permissions::is_skill_infrastructure_tool`) deliberately
        // whitelists `vox_chat_*` through *skill* restrictions — that's correct for
        // other, non-recursive callers, so this exclusion is supplied here at the
        // `run_agent_turn` call site rather than being hardcoded in
        // `tool_selection.rs`. Without it, a model could dispatch
        // `vox_chat_message` via `handle_tool_call_with_mode`, which re-enters
        // `chat_message -> try_run_agent_turn -> run_agent_turn` — each re-entrant
        // call gets its own fresh `max_iterations` budget, so
        // `DEFAULT_MAX_ITERATIONS` alone would not bound the recursion depth.
        //
        // This is applied INSIDE `select_tools_for_turn`, before the `max_tools`
        // cap, so a `vox_chat_*` entry sitting within the first `DEFAULT_MAX_TOOLS`
        // registry-order entries no longer consumes a cap slot only to be
        // discarded afterward (which used to leave turns with fewer than
        // `DEFAULT_MAX_TOOLS` usable tools and could push a genuinely useful tool
        // just past the cap boundary out of reach).
        exclude_name_prefixes: vec!["vox_chat_"],
        // Registry order is alphabetical, so the plain `.take(max_tools)`
        // truncation made the delegation tools unreachable on every chat turn:
        // after the ai/app lane filter `vox_spawn_agent` is candidate ~166 of
        // ~188 and the cut lands at 40. Pin them so a chat turn can actually
        // delegate.
        //
        // This is not free: the cap is fixed, so each pin evicts the tool that
        // previously held the last slot. These three pins displaced
        // `vox_check_file_owner`, `vox_check_mood`, and `vox_check_workspace`
        // (previously slots 38-40, the cut ending at `vox_check_workspace`).
        // Every further pin costs another tool the same way — the real fix is
        // relevance ranking before truncation, not a longer pin list.
        pin_names: vec!["vox_spawn_agent", "vox_submit_task", "vox_task_status"],
    };
    let selected = select_tools_for_turn(TOOL_REGISTRY, &state.skill_registry, &turn_ctx);
    let tool_defs: Vec<LlmToolDef> = selected
        .iter()
        .map(|entry| LlmToolDef {
            name: entry.name.to_string(),
            description: Some(entry.description.to_string()),
            parameters: serde_json::Value::Object(tool_input_schema(entry.name)),
        })
        .collect();

    let activity_options = ActivityOptions::new();
    let mut model_used = String::new();
    let mut tool_calls_made = 0usize;
    let mut total_tokens = 0u64;
    let mut events: Vec<serde_json::Value> = Vec::new();
    let mut latency_ms: Option<u64> = None;
    let mut ttft_ms: Option<u64> = None;
    let mut tpot_ms: Option<f64> = None;

    for iteration in 0..max_iterations {
        let mut config = llm_config_template.clone();
        config.tools = if tool_defs.is_empty() {
            None
        } else {
            Some(tool_defs.clone())
        };

        let resp = if stream_tokens {
            match stream_final_answer(
                state,
                &activity_options,
                &messages,
                config.clone(),
                session_id,
            )
            .await
            {
                Some(r) => r,
                None => match llm_chat(&activity_options, messages.clone(), config).await {
                    vox_actor_runtime::ActivityResult::Ok(Ok(r)) => r,
                    vox_actor_runtime::ActivityResult::Ok(Err(e)) => return Err(e),
                    vox_actor_runtime::ActivityResult::Failed(e) => return Err(format!("{e:?}")),
                    vox_actor_runtime::ActivityResult::Cancelled => {
                        return Err("llm_chat activity cancelled".to_string());
                    }
                },
            }
        } else {
            match llm_chat(&activity_options, messages.clone(), config).await {
                vox_actor_runtime::ActivityResult::Ok(Ok(r)) => r,
                vox_actor_runtime::ActivityResult::Ok(Err(e)) => return Err(e),
                vox_actor_runtime::ActivityResult::Failed(e) => return Err(format!("{e:?}")),
                vox_actor_runtime::ActivityResult::Cancelled => {
                    return Err("llm_chat activity cancelled".to_string());
                }
            }
        };
        model_used = resp.model.clone();
        total_tokens += u64::from(resp.prompt_tokens) + u64::from(resp.completion_tokens);
        latency_ms = Some(resp.latency_ms);
        ttft_ms = resp.ttft_ms;
        tpot_ms = resp.tpot_ms;

        match resp.tool_calls {
            Some(calls) if !calls.is_empty() => {
                messages.push(LlmChatMessage {
                    role: "assistant".into(),
                    content: resp.content,
                    tool_calls: Some(calls.clone()),
                    ..Default::default()
                });

                for call in &calls {
                    tool_calls_made += 1;
                    // Phase D Task D1: inject the chat session id (and this
                    // call's own provider id as the delegation "origin turn")
                    // into the OUTGOING dispatch args only — never into
                    // `call.arguments` itself, which is what gets recorded into
                    // `messages`/`events`/the harness scorer below. Only
                    // `vox_spawn_agent`/`vox_submit_task` params structs declare
                    // these fields; every other tool's `deny_unknown_fields`
                    // schema would reject them, so this is scoped to the two
                    // delegation tools rather than injected unconditionally.
                    let mut dispatch_args = call.arguments.clone();
                    if matches!(call.name.as_str(), "vox_spawn_agent" | "vox_submit_task") {
                        if let Some(session) = session_id.filter(|s| !s.is_empty()) {
                            if dispatch_args.is_null() {
                                dispatch_args = serde_json::Value::Object(serde_json::Map::new());
                            }
                            if let Some(obj) = dispatch_args.as_object_mut() {
                                obj.insert(
                                    "chat_session_id".to_string(),
                                    serde_json::Value::String(session.to_string()),
                                );
                                obj.insert(
                                    "origin_turn_id".to_string(),
                                    serde_json::Value::String(call.id.clone()),
                                );
                            }
                        }
                    }
                    let result = crate::dispatch::handle_tool_call_with_mode(
                        state,
                        &call.name,
                        dispatch_args,
                        permission_mode,
                    )
                    .await;
                    let dispatch_ok = result.is_ok();
                    let content = match result {
                        Ok(s) => s,
                        Err(e) => format!("Error: {e}"),
                    };
                    let call_succeeded = dispatch_ok
                        && !content.starts_with("Error:")
                        && !crate::server_state::tool_json_envelope_is_error(&content);
                    // Derived from the RESULT (`call_succeeded`, computed above from
                    // what dispatch actually did), never from `call.arguments` alone
                    // — see `turn_event_for_result`'s doc comment for why that
                    // distinction is security-load-bearing.
                    if let Some(ev) =
                        turn_event_for_result(&call.name, &call.arguments, &content, call_succeeded)
                    {
                        events.push(ev);
                    }

                    if harness_detection_enabled {
                        let is_error = content.starts_with("Error:")
                            || crate::server_state::tool_json_envelope_is_error(&content);
                        // Redact before it ever enters the scorer's activity
                        // buffer — that buffer is later sent to the judge LLM
                        // and stored in evidence_json verbatim, so redaction
                        // must happen at the point of recording, not only
                        // when building the single-call summary that used to
                        // be sent (see recent_activity() below).
                        let redacted_args = vox_redact::redact_args(&call.arguments).to_string();
                        let redacted_content = if is_error {
                            vox_redact::redact_owned(&content)
                        } else {
                            String::new()
                        };
                        let crossed =
                            harness_scorer.record(&call.name, &redacted_args, &redacted_content);
                        if crossed {
                            let recent_activity = harness_scorer.recent_activity();
                            let db = state.db.clone();
                            let session_key = session_id.map(str::to_string);
                            // The judge's own model must be a real, resolved id — a
                            // literal "auto" is not a recognized provider and every
                            // real call would fail silently (judge() swallows LLM
                            // errors and returns None). Resolve it the same way
                            // propose_harness_issue_fix does.
                            let judge_model =
                                vox_orchestrator::models::select_with_default_registry(
                                    &vox_orchestrator::models::SelectionIntent::review(),
                                )
                                .map(|o| o.model_id)
                                .unwrap_or_else(|| "google/gemini-3.1-pro".to_string());
                            tokio::spawn(async move {
                                let Some(issue) = super::harness_issue_judge::judge(
                                    &recent_activity,
                                    &judge_model,
                                )
                                .await
                                else {
                                    return;
                                };
                                let Some(db) = db else {
                                    return;
                                };
                                // The scorer can re-cross threshold more than once
                                // per turn on the same stuck-loop signature; dedup
                                // against any still-pending issue for this
                                // session/category so one incident doesn't flood
                                // the review queue with duplicate rows.
                                if let Some(session_key) = session_key.as_deref() {
                                    match db
                                        .has_pending_harness_issue_for_session(
                                            session_key,
                                            &issue.category,
                                        )
                                        .await
                                    {
                                        Ok(true) => return,
                                        Ok(false) => {}
                                        Err(e) => {
                                            tracing::warn!(
                                                target: "harness_issue_judge",
                                                error = %e,
                                                "failed to check for a pending duplicate harness issue"
                                            );
                                        }
                                    }
                                }
                                let insert_result = db
                                    .insert_harness_issue(vox_db::NewHarnessIssue {
                                        source: "chat_session",
                                        session_key: session_key.as_deref(),
                                        target_path: None,
                                        detected_at_ms: chrono::Utc::now().timestamp_millis(),
                                        category: &issue.category,
                                        severity: &issue.severity,
                                        summary: &issue.summary,
                                        evidence_json: &serde_json::json!({
                                            "excerpt": recent_activity
                                        })
                                        .to_string(),
                                    })
                                    .await;
                                if let Err(e) = insert_result {
                                    // The has_pending_harness_issue_for_session check
                                    // above is a fast-path only — the database's own
                                    // partial unique index on (session_key, category)
                                    // WHERE status='pending' AND source='chat_session'
                                    // is the actual dedup enforcement, closing the race
                                    // between two concurrently-spawned judge tasks that
                                    // both passed the check before either inserted.
                                    // That expected race outcome (not a real failure)
                                    // surfaces as a unique-constraint violation here.
                                    if e.to_string().to_ascii_lowercase().contains("unique") {
                                        tracing::debug!(
                                            target: "harness_issue_judge",
                                            "duplicate harness issue insert raced with another judge task; dropped"
                                        );
                                    } else {
                                        tracing::warn!(
                                            target: "harness_issue_judge",
                                            error = %e,
                                            "failed to insert detected harness issue"
                                        );
                                    }
                                }
                            });
                            harness_scorer.reset();
                        }
                    }

                    messages.push(LlmChatMessage {
                        role: "tool".into(),
                        content,
                        tool_call_id: Some(call.id.clone()),
                        name: Some(call.name.clone()),
                        ..Default::default()
                    });
                }

                if iteration + 1 == max_iterations {
                    return Ok(AgentTurnOutcome {
                        final_text: String::new(),
                        model_used,
                        tool_calls_made,
                        hit_iteration_limit: true,
                        total_tokens,
                        events,
                        latency_ms,
                        ttft_ms,
                        tpot_ms,
                    });
                }
                // Otherwise loop: ask the model again with the tool results appended.
            }
            _ => {
                return Ok(AgentTurnOutcome {
                    final_text: resp.content,
                    model_used,
                    tool_calls_made,
                    hit_iteration_limit: false,
                    total_tokens,
                    events,
                    latency_ms,
                    ttft_ms,
                    tpot_ms,
                });
            }
        }
    }

    // Unreachable in practice: the `iteration + 1 == max_iterations` check above
    // always returns before the loop would fall through here. Kept as a defensive
    // fallback so the function has no silent infinite-loop path even if the loop
    // bound logic above is ever refactored incorrectly.
    Ok(AgentTurnOutcome {
        final_text: String::new(),
        model_used,
        tool_calls_made,
        hit_iteration_limit: true,
        total_tokens,
        events,
        latency_ms,
        ttft_ms,
        tpot_ms,
    })
}

/// Purpose-built for `vox harness eval`'s `agent-loop-terminates` golden task
/// (`crates/vox-cli/src/commands/harness/eval.rs`): stands up a wiremock model
/// server that always returns a tool call (never a final answer) and runs
/// [`run_agent_turn`] against it, asserting the loop genuinely stops at its
/// `max_iterations` bound rather than recursing forever — the property
/// [`DEFAULT_MAX_ITERATIONS`] exists to guarantee. `pub` (not `pub(crate)`)
/// specifically so `vox-cli`'s eval gate, in a different crate, can call it.
/// Hermetic: the mock server is entirely local (no real network egress), and
/// the `ServerState` built here (via [`ServerState::hermetic_stub`]) does no
/// I/O.
///
/// Gated behind the `eval-gate` feature (default OFF, enabled by `vox-cli`
/// only) because it needs `wiremock` at runtime, in normal non-`#[cfg(test)]`
/// code — see the `eval-gate` feature and the `wiremock` dependency comment
/// in `Cargo.toml` for why that crate must not leak into every consumer of
/// `vox-orchestrator-mcp` (`vox-server`, `vox-gui`, ...) by default.
#[cfg(feature = "eval-gate")]
pub async fn eval_gate_agent_loop_terminates_check() -> Result<(), String> {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let tool_call_body = serde_json::json!({
        "id": "chatcmpl-eval-gate",
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "vox_git_status",
                        "arguments": "{}",
                    },
                }],
            },
            "finish_reason": "tool_calls",
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5},
    });
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_call_body))
        .mount(&server)
        .await;

    let cfg = OrchestratorConfig::for_testing();
    let orch_cfg = cfg.clone();
    let groups = AffinityGroupRegistry::new(vec![]);
    let session_cfg = SessionConfig {
        persist: false,
        sessions_dir: std::env::temp_dir().join("vox-harness-eval-agent-loop-sessions"),
        ..SessionConfig::default()
    };
    let session_manager =
        SessionManager::new(session_cfg).map_err(|e| format!("session manager: {e}"))?;
    let repository = RepositoryContext {
        root: PathBuf::from("."),
        git_root: None,
        repository_id: "harness-eval-agent-loop".into(),
        origin_url: None,
        capabilities: RepoCapabilities {
            vox_project: false,
            cargo_workspace: false,
            cargo_package: false,
            node_workspace: false,
            python_project: false,
            go_module: false,
            git: false,
        },
        has_vox_agents_dir: false,
        vox_toml: None,
    };
    let state = ServerState::hermetic_stub(
        cfg,
        repository,
        Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
        Arc::new(Mutex::new(session_manager)),
        new_registry_arc(),
    );

    let llm_config = LlmConfig {
        provider: "openrouter".into(),
        model: "test-model".into(),
        cost_per_1k: None,
        base_url: Some(format!("{}/chat/completions", server.uri())),
        api_key: Some("test-key".into()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        response_format: None,
        tools: None,
        tool_choice: None,
        timeout_ms: None,
        telemetry_session_id: None,
        telemetry_user_id: None,
        telemetry_task_category: None,
        telemetry_strength_tag: None,
        telemetry_trace_id: None,
        telemetry_attempt_number: None,
        telemetry_skip_interaction: true,
    };

    let max_iterations = 3;
    let outcome = run_agent_turn(
        &state,
        Some("eval-gate"),
        vec![],
        "system prompt".to_string(),
        "eval-gate: loop forever please".to_string(),
        None,
        None,
        llm_config,
        max_iterations,
        false,
    )
    .await?;

    if !outcome.hit_iteration_limit {
        return Err("expected the loop to hit its iteration cap and stop".to_string());
    }
    if outcome.tool_calls_made != max_iterations {
        return Err(format!(
            "expected {max_iterations} tool calls (one per iteration), got {}",
            outcome.tool_calls_made
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_state() -> ServerState {
        let cfg = OrchestratorConfig::for_testing();
        let orch_cfg = cfg.clone();
        let groups = AffinityGroupRegistry::new(vec![]);
        let session_cfg = SessionConfig {
            persist: false,
            sessions_dir: std::env::temp_dir().join("vox-mcp-agent-loop-test-sessions"),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg).expect("session manager");
        let repository = RepositoryContext {
            root: PathBuf::from("."),
            git_root: None,
            repository_id: "agent-loop-test".into(),
            origin_url: None,
            capabilities: RepoCapabilities {
                vox_project: false,
                cargo_workspace: false,
                cargo_package: false,
                node_workspace: false,
                python_project: false,
                go_module: false,
                git: false,
            },
            has_vox_agents_dir: false,
            vox_toml: None,
        };
        ServerState::hermetic_stub(
            cfg,
            repository,
            Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
            Arc::new(Mutex::new(session_manager)),
            new_registry_arc(),
        )
    }

    fn test_config(base_url: String) -> LlmConfig {
        LlmConfig {
            provider: "openrouter".into(),
            model: "test-model".into(),
            cost_per_1k: None,
            base_url: Some(base_url),
            api_key: Some("test-key".into()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: None,
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: None,
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: true,
        }
    }

    fn plain_response_body(content: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "chatcmpl-test",
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": content},
                "finish_reason": "stop",
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5},
        })
    }

    fn tool_call_response_body() -> serde_json::Value {
        serde_json::json!({
            "id": "chatcmpl-test",
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "vox_git_status",
                            "arguments": "{}",
                        },
                    }],
                },
                "finish_reason": "tool_calls",
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5},
        })
    }

    /// A single-turn response with no tool_calls must return immediately with that
    /// text, without ever touching tool dispatch.
    #[tokio::test]
    async fn no_tool_calls_returns_immediately_with_final_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(plain_response_body("hello, no tools needed here")),
            )
            .expect(1)
            .mount(&server)
            .await;

        let state = test_state();
        let config = test_config(format!("{}/chat/completions", server.uri()));
        let outcome = run_agent_turn(
            &state,
            None,
            vec![],
            "system prompt".to_string(),
            "hi".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
            false,
        )
        .await
        .expect("run_agent_turn should succeed");

        assert_eq!(outcome.final_text, "hello, no tools needed here");
        assert_eq!(outcome.tool_calls_made, 0);
        assert!(!outcome.hit_iteration_limit);
    }

    /// A response with tool_calls must dispatch through `handle_tool_call_with_mode`
    /// (here pointed at the real, read-only, side-effect-free `vox_git_status` tool)
    /// and the *next* model call must receive a `role: "tool"` message correlated by
    /// `tool_call_id` — proved by asserting on the second request body the mock
    /// server actually received.
    #[tokio::test]
    async fn tool_call_dispatches_and_feeds_result_back_with_matching_call_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(tool_call_response_body()))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(plain_response_body("done, saw the tool result")),
            )
            .mount(&server)
            .await;

        let state = test_state();
        let config = test_config(format!("{}/chat/completions", server.uri()));
        let outcome = run_agent_turn(
            &state,
            None,
            vec![],
            "system prompt".to_string(),
            "what's the git status?".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
            false,
        )
        .await
        .expect("run_agent_turn should succeed");

        assert_eq!(outcome.final_text, "done, saw the tool result");
        assert_eq!(outcome.tool_calls_made, 1);
        assert!(!outcome.hit_iteration_limit);

        // Inspect the second request the mock server received: it must carry a
        // `role: "tool"` message with `tool_call_id: "call_1"` correlating back to
        // the call the first response requested.
        let requests = server.received_requests().await.expect("received requests");
        assert_eq!(requests.len(), 2, "expected exactly two model round-trips");
        let second_body: serde_json::Value =
            serde_json::from_slice(&requests[1].body).expect("second request body is JSON");
        let msgs = second_body["messages"].as_array().expect("messages array");
        let tool_msg = msgs
            .iter()
            .find(|m| m["role"] == "tool")
            .expect("a role:tool message must be present in the follow-up request");
        assert_eq!(tool_msg["tool_call_id"], "call_1");
        assert_eq!(tool_msg["name"], "vox_git_status");
    }

    /// Recursion-safety regression: `vox_chat_message` (and any other `vox_chat_*`
    /// tool) must never appear in the `tools` array sent to the model from within
    /// `run_agent_turn`. `select_tools_for_turn`'s general-purpose skill-permission
    /// filter (`skill_permissions::is_skill_infrastructure_tool`) whitelists
    /// `vox_chat_*` through *skill* restrictions — correct for other, non-recursive
    /// callers — so without an explicit exclusion here, a model could be offered
    /// `vox_chat_message` as a callable tool, dispatch it via
    /// `handle_tool_call_with_mode`, and re-enter `chat_message ->
    /// try_run_agent_turn -> run_agent_turn`, where each re-entrant call gets its
    /// own fresh `max_iterations` budget (no real recursion-depth bound).
    #[tokio::test]
    async fn vox_chat_message_is_never_offered_as_a_tool() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(plain_response_body("no tools needed")),
            )
            .mount(&server)
            .await;

        let state = test_state();
        let config = test_config(format!("{}/chat/completions", server.uri()));
        run_agent_turn(
            &state,
            None,
            vec![],
            "system prompt".to_string(),
            "hi".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
            false,
        )
        .await
        .expect("run_agent_turn should succeed");

        let requests = server.received_requests().await.expect("received requests");
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value =
            serde_json::from_slice(&requests[0].body).expect("request body is JSON");
        let tools = body
            .get("tools")
            .and_then(|t| t.as_array())
            .expect("tools array must be present");
        assert!(
            !tools.is_empty(),
            "sanity check: the turn should still offer some non-chat tools"
        );
        assert!(
            tools.iter().all(|t| {
                let name = t["function"]["name"].as_str().unwrap_or_default();
                !name.starts_with("vox_chat_")
            }),
            "no vox_chat_* tool (e.g. vox_chat_message) may ever be offered inside \
             run_agent_turn — doing so would allow unbounded re-entrant recursion; \
             tools sent: {tools:?}"
        );
    }

    /// If the model always returns tool_calls, the loop must still terminate at
    /// `max_iterations` rather than looping forever — the hard safety bound
    /// described on [`DEFAULT_MAX_ITERATIONS`] must be real.
    #[tokio::test]
    async fn max_iterations_bound_actually_stops_the_loop() {
        let server = MockServer::start().await;
        // Every response requests a tool call — never a final answer.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(tool_call_response_body()))
            .mount(&server)
            .await;

        let state = test_state();
        let config = test_config(format!("{}/chat/completions", server.uri()));
        let max_iterations = 3;
        let outcome = run_agent_turn(
            &state,
            None,
            vec![],
            "system prompt".to_string(),
            "loop forever please".to_string(),
            None,
            None,
            config,
            max_iterations,
            false,
        )
        .await
        .expect("run_agent_turn should return Ok even when the bound is hit");

        assert!(
            outcome.hit_iteration_limit,
            "loop must report that it stopped due to the iteration bound"
        );
        // One tool call dispatched per iteration.
        assert_eq!(outcome.tool_calls_made, max_iterations);

        let requests = server.received_requests().await.expect("received requests");
        assert_eq!(
            requests.len(),
            max_iterations,
            "loop must make exactly max_iterations model round-trips, not hang"
        );
    }

    /// A repeated-error tool call body: the model keeps calling an unknown tool
    /// name (so `handle_tool_call_with_mode` deterministically fails with
    /// `Error: Unknown tool: ...`, without needing any real side-effecting tool
    /// to error) with identical arguments every time — exactly the pattern
    /// [`super::harness_issue_scorer::HarnessIssueScorer`] is designed to flag.
    fn repeated_error_tool_call_response_body() -> serde_json::Value {
        serde_json::json!({
            "id": "chatcmpl-test",
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "vox_definitely_not_a_real_tool",
                            "arguments": "{}",
                        },
                    }],
                },
                "finish_reason": "tool_calls",
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5},
        })
    }

    /// Proves the `harness_issue_detection_enabled` config gate actually gates
    /// something: with it turned OFF, feeding `run_agent_turn` enough
    /// error-shaped tool results to normally cross the scorer's `THRESHOLD`
    /// must not produce any `scientia_harness_issues` row. Uses a real
    /// in-memory `VoxDb` (unlike `test_state()`'s hermetic, DB-less default) so
    /// the assertion is a genuine "no row was written" check, not just "no
    /// panic occurred".
    #[tokio::test]
    async fn detection_disabled_gate_prevents_any_harness_issue_row() {
        let server = MockServer::start().await;
        // Every response requests the same failing tool call, enough times to
        // cross THRESHOLD (3) if the scorer were active.
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(repeated_error_tool_call_response_body()),
            )
            .mount(&server)
            .await;

        let mut state = test_state();
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("open in-memory db");
        state.db = Some(std::sync::Arc::new(db));

        {
            let cfg_handle = state.orchestrator.config_handle();
            let mut cfg = cfg_handle.write().expect("config lock");
            cfg.harness_issue_detection_enabled = false;
        }

        let config = test_config(format!("{}/chat/completions", server.uri()));
        let max_iterations = 5;
        let outcome = run_agent_turn(
            &state,
            Some("gate-test-session"),
            vec![],
            "system prompt".to_string(),
            "keep hitting the same broken tool".to_string(),
            None,
            None,
            config,
            max_iterations,
            false,
        )
        .await
        .expect("run_agent_turn should succeed even though every tool call errors");

        assert_eq!(outcome.tool_calls_made, max_iterations);

        // Give the (should-never-have-been-spawned) fire-and-forget judge task a
        // moment to run, in case the gate were broken and it fired anyway.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = state.db.as_ref().expect("db attached");
        let rows = db
            .list_harness_issues(None, None, 10)
            .await
            .expect("list_harness_issues");
        assert!(
            rows.is_empty(),
            "harness_issue_detection_enabled = false must prevent any \
             scientia_harness_issues row from being written, got {rows:?}"
        );
    }

    /// The `vox harness eval` `agent-loop-terminates` golden task body itself must
    /// succeed — i.e. the check it performs (loop terminates at the iteration
    /// bound against a model that always requests tools) must actually hold.
    /// Only compiled with `--features eval-gate` (matching the function under
    /// test) — run `cargo test -p vox-orchestrator-mcp --lib --features
    /// eval-gate` to exercise it.
    #[cfg(feature = "eval-gate")]
    #[tokio::test]
    async fn eval_gate_agent_loop_terminates_check_reports_ok_when_loop_is_bounded() {
        let result = eval_gate_agent_loop_terminates_check().await;
        assert!(result.is_ok(), "{result:?}");
    }

    fn model_spec(provider_type: vox_orchestrator::models::ProviderType, id: &str) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: id.to_string(),
            provider: "test".to_string(),
            provider_type,
            max_tokens: 8192,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
            is_free: false,
            strengths: Vec::new(),
            capabilities: vox_orchestrator::models::ModelCapabilities::default(),
            supported_parameters: Vec::new(),
        }
    }

    /// Task 1.3d mapper test: an OpenRouter-routed [`ModelSpec`] must convert to a
    /// working [`LlmConfig`] (this is the case `message.rs` now routes through
    /// `run_agent_turn` instead of `mcp_infer_completion`).
    #[test]
    fn model_spec_to_llm_config_maps_openrouter() {
        let spec = model_spec(ProviderType::OpenRouter, "openrouter/some-model");
        let cfg = model_spec_to_llm_config(&spec).expect("openrouter must map");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "openrouter/some-model");
    }

    /// Deliberately-unhandled case: `ProviderType::GoogleDirect` requires the
    /// `apply_gemini_policy`/direct-Gemini-endpoint handling that lives in
    /// `mcp_infer_completion`'s provider-adapter dispatch, not a narrow mapper —
    /// so this must return `None` and let the caller fall back to that pipeline.
    #[test]
    fn model_spec_to_llm_config_returns_none_for_google_direct() {
        let spec = model_spec(ProviderType::GoogleDirect, "gemini-2.0-flash");
        assert!(model_spec_to_llm_config(&spec).is_none());
    }

    /// Task 1.3d end-to-end proof that F24 is fixed for the mapped-provider case:
    /// calling the real, live `vox_chat_message` handler (`chat_message`, exactly
    /// what the MCP server dispatches to) for an OpenRouter-routed model results in
    /// an actual HTTP request that carries a non-empty `tools` array — i.e. a real
    /// chat message now causes a real tool-bearing request, closing the gap
    /// Finding F24 named ("no code path in Vox ever passes tools to a model").
    #[tokio::test]
    #[allow(unsafe_code)] // env var mutation under a process-wide lock, like existing env tests
    #[allow(clippy::await_holding_lock)] // intentional: the std Mutex must stay held for the
    // entire test body to serialize access to the process-global OPENROUTER_BASE_URL /
    // OPENROUTER_API_KEY env vars against any other test that might touch them; this test
    // never runs concurrently with itself, so there's no deadlock risk from the held guard.
    async fn chat_message_default_path_sends_tools_bearing_request() {
        let _env_guard = CHAT_MESSAGE_ENV_LOCK.lock().expect("env lock");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(plain_response_body("no tools needed")),
            )
            .mount(&server)
            .await;

        let prev_base = std::env::var("OPENROUTER_BASE_URL").ok();
        let prev_key = std::env::var("OPENROUTER_API_KEY").ok();
        unsafe {
            std::env::set_var("OPENROUTER_BASE_URL", server.uri());
            std::env::set_var("OPENROUTER_API_KEY", "test-key");
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        let state = test_state();
        let model_id = "test-openrouter-model";
        {
            let handle = state.orchestrator.models_handle();
            let mut registry = handle.write().expect("models registry lock");
            registry.register(model_spec(ProviderType::OpenRouter, model_id));
        }
        // Sticky override: deterministically select the model we just registered,
        // bypassing scorer heuristics that are out of scope for this test.
        *state.mcp_chat_model_override.write() = Some(model_id.to_string());

        let params: crate::chat_tools::params::ChatMessageParams =
            serde_json::from_value(serde_json::json!({ "prompt": "hello there" }))
                .expect("chat message params");

        let response_json = super::super::message::chat_message(&state, params).await;

        unsafe {
            match prev_base {
                Some(v) => std::env::set_var("OPENROUTER_BASE_URL", v),
                None => std::env::remove_var("OPENROUTER_BASE_URL"),
            }
            match prev_key {
                Some(v) => std::env::set_var("OPENROUTER_API_KEY", v),
                None => std::env::remove_var("OPENROUTER_API_KEY"),
            }
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        assert!(
            !response_json.contains("\"error\""),
            "chat_message should succeed via the mapped agent-loop path: {response_json}"
        );

        let requests = server.received_requests().await.expect("received requests");
        assert!(
            !requests.is_empty(),
            "vox_chat_message must have made at least one HTTP request to the model"
        );
        // Find the actual chat-completion POST body sent for `model_id` (parses to
        // JSON with a `messages` array and `model` == the resolved chat model)
        // rather than assuming it's `requests[0]` or the first request with a
        // `messages` array — the same mock server also receives unrelated internal
        // LLM calls (e.g. the autonomous-research "gap analyst" follow-up-query
        // generator, which runs against `openrouter/auto` and never carries tools)
        // triggered earlier in `chat_message`, before the real tool-bearing
        // completion request this test is asserting on.
        let body = requests
            .iter()
            .find_map(|r| {
                let v: serde_json::Value = serde_json::from_slice(&r.body).ok()?;
                v.get("messages")?.as_array()?;
                if v.get("model").and_then(|m| m.as_str()) != Some(model_id) {
                    return None;
                }
                Some(v)
            })
            .expect(
                "no request with a JSON `messages` body for the resolved chat model was received",
            );
        let tools = body
            .get("tools")
            .and_then(|t| t.as_array())
            .expect("real vox_chat_message call must send a non-null `tools` array — F24");
        assert!(
            !tools.is_empty(),
            "tools array must be non-empty — a real chat message must offer real tools"
        );
    }

    /// Regression test: `params.temperature`/`params.top_p` must reach the wire
    /// request on the mapped (`run_agent_turn`) path exactly as they already do on
    /// the `call_llm` fallback path — `try_run_agent_turn` must not silently drop
    /// a caller's sampling overrides for the providers this task newly wires up.
    #[tokio::test]
    #[allow(unsafe_code)] // env var mutation under a process-wide lock, like existing env tests
    #[allow(clippy::await_holding_lock)] // see chat_message_default_path_sends_tools_bearing_request
    async fn chat_message_default_path_honors_temperature_and_top_p() {
        let _env_guard = CHAT_MESSAGE_ENV_LOCK.lock().expect("env lock");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(plain_response_body("no tools needed")),
            )
            .mount(&server)
            .await;

        let prev_base = std::env::var("OPENROUTER_BASE_URL").ok();
        let prev_key = std::env::var("OPENROUTER_API_KEY").ok();
        unsafe {
            std::env::set_var("OPENROUTER_BASE_URL", server.uri());
            std::env::set_var("OPENROUTER_API_KEY", "test-key");
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        let state = test_state();
        let model_id = "test-openrouter-model-temp";
        {
            let handle = state.orchestrator.models_handle();
            let mut registry = handle.write().expect("models registry lock");
            registry.register(model_spec(ProviderType::OpenRouter, model_id));
        }
        *state.mcp_chat_model_override.write() = Some(model_id.to_string());

        let params: crate::chat_tools::params::ChatMessageParams = serde_json::from_value(
            serde_json::json!({ "prompt": "hello there", "temperature": 0.11, "top_p": 0.42 }),
        )
        .expect("chat message params");

        let response_json = super::super::message::chat_message(&state, params).await;

        unsafe {
            match prev_base {
                Some(v) => std::env::set_var("OPENROUTER_BASE_URL", v),
                None => std::env::remove_var("OPENROUTER_BASE_URL"),
            }
            match prev_key {
                Some(v) => std::env::set_var("OPENROUTER_API_KEY", v),
                None => std::env::remove_var("OPENROUTER_API_KEY"),
            }
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        assert!(
            !response_json.contains("\"error\""),
            "chat_message should succeed via the mapped agent-loop path: {response_json}"
        );

        let requests = server.received_requests().await.expect("received requests");
        // See `chat_message_default_path_sends_tools_bearing_request` — the mock
        // server also receives the autonomous-research "gap analyst" follow-up-query
        // request (`model: "openrouter/auto"`, no sampling overrides) before the
        // real completion request for `model_id`, so the first `messages`-bearing
        // body is not necessarily the one under test.
        let body = requests
            .iter()
            .find_map(|r| {
                let v: serde_json::Value = serde_json::from_slice(&r.body).ok()?;
                v.get("messages")?.as_array()?;
                if v.get("model").and_then(|m| m.as_str()) != Some(model_id) {
                    return None;
                }
                Some(v)
            })
            .expect(
                "no request with a JSON `messages` body for the resolved chat model was received",
            );
        assert_eq!(
            body.get("temperature").and_then(serde_json::Value::as_f64),
            Some(0.11_f64),
            "params.temperature must reach the wire request on the mapped path: {body}"
        );
        assert_eq!(
            body.get("top_p").and_then(serde_json::Value::as_f64),
            Some(0.42_f64),
            "params.top_p must reach the wire request on the mapped path: {body}"
        );
    }

    // -----------------------------------------------------------------------
    // Task G1: sync-path token streaming
    // -----------------------------------------------------------------------

    /// `stream_tokens: true` against a real SSE response: `TokenStreamed`
    /// events carrying the turn's `session_id` must appear on the existing
    /// agent-events bus (`state.orchestrator.event_bus()` — the same bus
    /// `orch_daemon` forwards to the GUI) as chunks arrive, concatenating to
    /// the same text `run_agent_turn`'s own return value reports, and exactly
    /// one HTTP round-trip must occur (no non-streaming fallback needed).
    #[tokio::test]
    async fn stream_tokens_emits_token_streamed_events_with_session_id() {
        let server = MockServer::start().await;
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Hi \"}}]}\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"there\"}}]}\n\
                   data: [DONE]\n\n";
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse),
            )
            .expect(1)
            .mount(&server)
            .await;

        let state = test_state();
        let mut rx = state.orchestrator.event_bus().subscribe();
        let config = test_config(format!("{}/chat/completions", server.uri()));
        let outcome = run_agent_turn(
            &state,
            Some("sess-g1"),
            vec![],
            "system prompt".to_string(),
            "hi".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
            true,
        )
        .await
        .expect("run_agent_turn should succeed");

        assert_eq!(outcome.final_text, "Hi there");
        assert_eq!(outcome.tool_calls_made, 0);
        assert!(
            outcome.latency_ms.is_some(),
            "Task M3: streaming latency_ms must no longer be the hardcoded 0"
        );
        assert!(
            outcome.ttft_ms.is_some(),
            "Task M3: a genuinely streamed reply must report a real time-to-first-token"
        );
        assert!(
            outcome.tpot_ms.is_some(),
            "Task M3: a genuinely streamed reply with completion tokens must report tpot_ms"
        );

        let mut streamed_text = String::new();
        let mut saw_session_id = false;
        while let Ok(evt) = rx.try_recv() {
            if let AgentEventKind::TokenStreamed {
                text, session_id, ..
            } = evt.kind
            {
                streamed_text.push_str(&text);
                assert_eq!(
                    session_id.as_deref(),
                    Some("sess-g1"),
                    "every TokenStreamed frame from the sync path must carry this turn's session_id"
                );
                saw_session_id = true;
            }
        }
        assert!(
            saw_session_id,
            "expected at least one TokenStreamed event on the bus"
        );
        assert_eq!(streamed_text, "Hi there");
    }

    /// `stream_tokens: true` against a plain (non-SSE) JSON response — the
    /// observable shape of a turn where the model requested a tool call
    /// instead of streaming text (`vox_llm_egress::wire::stream_once` doesn't
    /// parse `delta.tool_calls`, so that turn streams zero content chunks).
    /// Must fall back to one ordinary non-streaming `llm_chat` call and reach
    /// the exact same outcome `stream_tokens: false` would, proving tool
    /// dispatch is unaffected by this task.
    #[tokio::test]
    async fn stream_tokens_falls_back_to_non_streaming_when_stream_yields_no_content() {
        let server = MockServer::start().await;
        // `up_to_n_times(2)`: iteration 1 costs two requests under
        // `stream_tokens: true` — the empty streaming attempt, then the
        // non-streaming fallback that actually carries the tool_calls.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(tool_call_response_body()))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(plain_response_body("done, saw the tool result")),
            )
            .mount(&server)
            .await;

        let state = test_state();
        let config = test_config(format!("{}/chat/completions", server.uri()));
        let outcome = run_agent_turn(
            &state,
            Some("sess-g1-fallback"),
            vec![],
            "system prompt".to_string(),
            "what's the git status?".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
            true,
        )
        .await
        .expect("run_agent_turn should succeed via the non-streaming fallback");

        assert_eq!(outcome.final_text, "done, saw the tool result");
        assert_eq!(
            outcome.tool_calls_made, 1,
            "the tool call from the first (streaming-attempted, empty) iteration must still \
             dispatch via the non-streaming fallback"
        );

        // Two iterations, each of which first attempts streaming (empty/no
        // SSE body -> falls back) then makes its real non-streaming call:
        // iteration 1 = [stream-attempt, fallback-that-returns-tool_calls],
        // iteration 2 = [stream-attempt, fallback-that-returns-final-text].
        let requests = server.received_requests().await.expect("received requests");
        assert_eq!(
            requests.len(),
            4,
            "each of the two iterations costs one wasted stream-attempt request plus one real \
             non-streaming request while the model is calling tools"
        );
    }
}
