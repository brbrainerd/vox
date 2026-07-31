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

use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, LlmToolDef, llm_chat};
use vox_actor_runtime::ActivityOptions;
use vox_mcp_registry::TOOL_REGISTRY;

use crate::input_schemas::tool_input_schema;
use crate::llm_bridge::tool_selection::{DEFAULT_MAX_TOOLS, TurnContext, select_tools_for_turn};
use crate::server_state::ServerState;

/// Hard bound on the number of model round-trips within a single `run_agent_turn`
/// call. Each iteration is: one `llm_chat` call, plus (if it requested tools) one
/// `handle_tool_call_with_mode` dispatch per call before looping again. 8 is chosen
/// to comfortably cover realistic tool-use chains (a typical "look something up,
/// then answer" turn is 2-3 iterations) while guaranteeing the loop cannot spin
/// forever if a model pathologically keeps requesting tools — this is a safety
/// bound, not a tuning knob callers are expected to raise routinely.
#[allow(dead_code)]
pub(crate) const DEFAULT_MAX_ITERATIONS: usize = 8;

/// Result of running one agent turn to completion (or to the iteration bound).
///
/// Not yet called from `vox_chat_message`'s live entrypoint (see the Task 1.3c
/// report / `message.rs` for why: wiring it in would require rebuilding
/// `message.rs`'s model-resolution/fallback-candidate logic, which is explicitly
/// out of scope for this task) — `#[allow(dead_code)]` reflects that this is a
/// deliberately-unwired-but-complete, independently-tested unit, not an oversight.
#[allow(dead_code)]
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
#[allow(dead_code, clippy::too_many_arguments)]
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
    })
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
        ServerState::test_stub(
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
            .respond_with(ResponseTemplate::new(200).set_body_json(plain_response_body(
                "hello, no tools needed here",
            )))
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
        let msgs = second_body["messages"]
            .as_array()
            .expect("messages array");
        let tool_msg = msgs
            .iter()
            .find(|m| m["role"] == "tool")
            .expect("a role:tool message must be present in the follow-up request");
        assert_eq!(tool_msg["tool_call_id"], "call_1");
        assert_eq!(tool_msg["name"], "vox_git_status");
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
}
