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
use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, LlmToolDef, llm_chat};
use vox_mcp_registry::TOOL_REGISTRY;
use vox_orchestrator::models::{ModelSpec, ProviderType};

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
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_agent_turn(
    state: &ServerState,
    prior_conversation: Vec<LlmChatMessage>,
    system_prompt: String,
    user_message: String,
    permission_mode: Option<&str>,
    active_skill_id: Option<String>,
    llm_config_template: LlmConfig,
    max_iterations: usize,
) -> Result<AgentTurnOutcome, String> {
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

    for iteration in 0..max_iterations {
        let mut config = llm_config_template.clone();
        config.tools = if tool_defs.is_empty() {
            None
        } else {
            Some(tool_defs.clone())
        };

        let resp = match llm_chat(&activity_options, messages.clone(), config).await {
            vox_actor_runtime::ActivityResult::Ok(Ok(r)) => r,
            vox_actor_runtime::ActivityResult::Ok(Err(e)) => return Err(e),
            vox_actor_runtime::ActivityResult::Failed(e) => return Err(format!("{e:?}")),
            vox_actor_runtime::ActivityResult::Cancelled => {
                return Err("llm_chat activity cancelled".to_string());
            }
        };
        model_used = resp.model.clone();
        total_tokens += u64::from(resp.prompt_tokens) + u64::from(resp.completion_tokens);

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
                    let result = crate::dispatch::handle_tool_call_with_mode(
                        state,
                        &call.name,
                        call.arguments.clone(),
                        permission_mode,
                    )
                    .await;
                    let content = match result {
                        Ok(s) => s,
                        Err(e) => format!("Error: {e}"),
                    };
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
        vec![],
        "system prompt".to_string(),
        "eval-gate: loop forever please".to_string(),
        None,
        None,
        llm_config,
        max_iterations,
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
            vec![],
            "system prompt".to_string(),
            "hi".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
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
            vec![],
            "system prompt".to_string(),
            "what's the git status?".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
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
            vec![],
            "system prompt".to_string(),
            "hi".to_string(),
            None,
            None,
            config,
            DEFAULT_MAX_ITERATIONS,
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
            vec![],
            "system prompt".to_string(),
            "loop forever please".to_string(),
            None,
            None,
            config,
            max_iterations,
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
            .expect("no request with a JSON `messages` body for the resolved chat model was received");
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
            .expect("no request with a JSON `messages` body for the resolved chat model was received");
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
}
