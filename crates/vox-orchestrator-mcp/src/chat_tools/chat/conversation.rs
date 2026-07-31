//! Conversation state for chat: turns persisted history into the multi-turn message
//! array the model actually needs to see.
//!
//! Finding F25 (harness implementation spec, Task 1.1): `chat_message` persisted and
//! returned full transcript history to the caller for GUI display, but never sent any
//! of it to the LLM — every turn was a fresh single-shot completion. This module is the
//! seam that fixes that: [`load_conversation`] reads the same persisted/hydrated history
//! used for display (`context_history_or_hydrate`) and maps it into
//! [`vox_actor_runtime::llm::LlmChatMessage`]s suitable for a real multi-turn request,
//! bounded by an approximate-token budget rather than a raw message-count FIFO.
//!
//! Persisted/display history and model-context history are deliberately different
//! concerns with different bounds: [`MAX_PERSISTED_HISTORY_ENTRIES`] /
//! [`trim_persisted_history`] bound what gets written back to `chat_history:{session_id}`
//! (and therefore what the GUI can show for the session), while [`ConversationBudget`]
//! bounds only the subset of that history threaded into the next LLM call. Shrinking one
//! must never shrink the other.

use vox_actor_runtime::llm::LlmChatMessage;

use super::super::params::ChatTranscriptEntry;
use super::hydrate::context_history_or_hydrate;
use crate::server_state::ServerState;

/// Cap on messages retained in the persisted/display transcript (`chat_history:{session_id}`).
/// This bounds storage/GUI-transcript size only — it has no bearing on how much history is
/// sent to the model for a given turn (see [`ConversationBudget`]).
pub(crate) const MAX_PERSISTED_HISTORY_ENTRIES: usize = 100;

/// Approximate characters-per-token used to estimate prompt size without a real
/// tokenizer (out of scope for this task — see Task 1.1 notes).
const CHARS_PER_TOKEN_ESTIMATE: usize = 4;

/// Default token budget for history threaded into a single chat completion request.
/// Chosen conservatively so history plus the active context preamble and system
/// prompt still leave headroom under typical model context windows.
pub(crate) const DEFAULT_CONVERSATION_TOKEN_BUDGET: usize = 4_000;

/// Token-aware budget for how much prior conversation is sent to the model on a turn,
/// following the same shape as [`vox_actor_runtime::retrieval::ContextBudget`]
/// (a plain `Copy` struct with a `Default` derived from a named constant, rather than
/// a magic number inline at each call site).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConversationBudget {
    /// Maximum approximate tokens (char_count / [`CHARS_PER_TOKEN_ESTIMATE`]) of
    /// message content to keep, most-recent-first.
    pub max_tokens: usize,
}

impl Default for ConversationBudget {
    fn default() -> Self {
        Self {
            max_tokens: DEFAULT_CONVERSATION_TOKEN_BUDGET,
        }
    }
}

/// Approximate token count for a string using a character-count heuristic.
/// Deliberately crude — a full tokenizer is out of scope for Task 1.1.
fn approx_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(CHARS_PER_TOKEN_ESTIMATE)
}

/// Keep only the most recent messages that fit under `budget.max_tokens`, dropping the
/// oldest messages first when over budget. Returned in chronological (oldest-first) order.
pub(crate) fn bound_messages_by_tokens(
    messages: Vec<LlmChatMessage>,
    budget: ConversationBudget,
) -> Vec<LlmChatMessage> {
    let mut kept_rev: Vec<LlmChatMessage> = Vec::new();
    let mut used = 0usize;
    for msg in messages.into_iter().rev() {
        let cost = approx_tokens(&msg.content);
        if used + cost > budget.max_tokens && !kept_rev.is_empty() {
            // Stop once adding the next (older) message would exceed budget — but always
            // keep at least the single most recent message even if it alone is over budget,
            // so a pathologically long last turn doesn't wipe out all history.
            break;
        }
        used += cost;
        kept_rev.push(msg);
    }
    kept_rev.reverse();
    kept_rev
}

/// Trim the persisted/display transcript in place to [`MAX_PERSISTED_HISTORY_ENTRIES`].
/// This is the storage/GUI bound — independent of [`ConversationBudget`], which bounds
/// only what is sent to the model for a given turn.
pub(crate) fn trim_persisted_history(history: &mut Vec<ChatTranscriptEntry>) {
    if history.len() > MAX_PERSISTED_HISTORY_ENTRIES {
        let trim_to = history.len() - MAX_PERSISTED_HISTORY_ENTRIES;
        history.drain(0..trim_to);
    }
}

fn transcript_entry_to_llm_message(entry: &ChatTranscriptEntry) -> LlmChatMessage {
    LlmChatMessage {
        role: entry.role.clone(),
        content: entry.content.clone(),
    }
}

/// Load this session's persisted conversation and map it into the message array the
/// model actually needs to see a real multi-turn conversation, bounded by
/// [`ConversationBudget::default`].
///
/// Reads via the same `context_history_or_hydrate` used for the GUI's full transcript
/// (so this never sees a truncated view due to display bounds), then applies the
/// token-aware budget independently.
pub(crate) async fn load_conversation(state: &ServerState, session_id: &str) -> Vec<LlmChatMessage> {
    let history_key = format!("chat_history:{session_id}");
    let history = context_history_or_hydrate(state, history_key.as_str(), session_id).await;
    let messages: Vec<LlmChatMessage> = history.iter().map(transcript_entry_to_llm_message).collect();
    bound_messages_by_tokens(messages, ConversationBudget::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(role: &str, content: &str, ts: u64) -> ChatTranscriptEntry {
        ChatTranscriptEntry {
            id: format!("id-{ts}"),
            role: role.to_string(),
            content: content.to_string(),
            timestamp: ts,
            context_files: vec![],
            model_used: None,
            tokens: None,
        }
    }

    #[test]
    fn transcript_entry_maps_role_and_content_only() {
        let e = entry("assistant", "hello there", 42);
        let m = transcript_entry_to_llm_message(&e);
        assert_eq!(m.role, "assistant");
        assert_eq!(m.content, "hello there");
    }

    #[test]
    fn bound_messages_keeps_most_recent_when_over_budget() {
        // Each message ~= 40 chars -> ~10 tokens at 4 chars/token.
        let messages = vec![
            LlmChatMessage {
                role: "user".into(),
                content: "a".repeat(40),
            },
            LlmChatMessage {
                role: "assistant".into(),
                content: "b".repeat(40),
            },
            LlmChatMessage {
                role: "user".into(),
                content: "c".repeat(40),
            },
            LlmChatMessage {
                role: "assistant".into(),
                content: "d".repeat(40),
            },
        ];
        // Budget only large enough for the last two messages (~20 tokens).
        let budget = ConversationBudget { max_tokens: 20 };
        let kept = bound_messages_by_tokens(messages, budget);

        assert_eq!(kept.len(), 2, "expected only the most recent 2 messages to fit");
        assert_eq!(kept[0].content, "c".repeat(40), "oldest kept must be the 3rd message");
        assert_eq!(kept[1].content, "d".repeat(40), "newest message must be last, in order");
    }

    #[test]
    fn bound_messages_always_keeps_at_least_the_last_message() {
        let messages = vec![LlmChatMessage {
            role: "user".into(),
            content: "x".repeat(10_000),
        }];
        let budget = ConversationBudget { max_tokens: 1 };
        let kept = bound_messages_by_tokens(messages, budget);
        assert_eq!(kept.len(), 1, "a single oversized message must still be kept");
    }

    #[test]
    fn trim_persisted_history_bounds_independently_of_conversation_budget() {
        let mut history: Vec<ChatTranscriptEntry> = (0..150)
            .map(|i| entry("user", &format!("turn {i}"), i as u64))
            .collect();
        trim_persisted_history(&mut history);
        assert_eq!(history.len(), MAX_PERSISTED_HISTORY_ENTRIES);
        // Most recent entries survive.
        assert_eq!(history.last().unwrap().content, "turn 149");
    }

    // --- F25 direct fix verification -----------------------------------------------
    //
    // The whole point of Task 1.1 is that a *second* chat turn in the same session
    // must include the *first* turn's content in what gets sent to the model. This
    // test builds a minimal `ServerState` (no DB — purely the in-RAM orchestrator
    // context store used by `context_history_or_hydrate`), seeds it exactly the way
    // `chat_message` persists a first turn (`ctx.set("chat_history:{session}", ...)`),
    // then calls `load_conversation` — the same function `chat_message` now calls
    // before building its second-turn request — and asserts the first turn's user
    // and assistant content is present in the resulting message array.
    mod f25_second_turn_sees_first_turn {
        use super::*;
        use crate::server_state::ServerState;
        use std::path::PathBuf;
        use std::sync::Arc;
        use tokio::sync::Mutex;
        use vox_orchestrator::{
            AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
        };
        use vox_repository::{RepoCapabilities, RepositoryContext};
        use vox_skills::new_registry_arc;

        fn test_state() -> ServerState {
            let cfg = OrchestratorConfig::for_testing();
            let orch_cfg = cfg.clone();
            let groups = AffinityGroupRegistry::new(vec![]);
            let session_cfg = SessionConfig {
                persist: false,
                sessions_dir: std::env::temp_dir()
                    .join("vox-mcp-conversation-test-sessions"),
                ..SessionConfig::default()
            };
            let session_manager = SessionManager::new(session_cfg).expect("session manager");
            let repository = RepositoryContext {
                root: PathBuf::from("."),
                git_root: None,
                repository_id: "conversation-test".into(),
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

        /// Persist a turn the same way `chat_message` does: append user+assistant
        /// entries to the session's history and write it back to the context store
        /// under `chat_history:{session_id}`.
        fn persist_turn(state: &ServerState, session_id: &str, user_text: &str, asst_text: &str) {
            let history_key = format!("chat_history:{session_id}");
            let ctx_handle = state.orchestrator.context_handle();
            let mut history: Vec<ChatTranscriptEntry> = {
                let guard = ctx_handle.read().expect("context read lock");
                guard
                    .get(&history_key)
                    .and_then(|s: String| serde_json::from_str(&s).ok())
                    .unwrap_or_default()
            };
            history.push(ChatTranscriptEntry {
                id: format!("usr-{}", history.len()),
                role: "user".into(),
                content: user_text.into(),
                timestamp: history.len() as u64,
                context_files: vec![],
                model_used: None,
                tokens: None,
            });
            history.push(ChatTranscriptEntry {
                id: format!("asst-{}", history.len()),
                role: "assistant".into(),
                content: asst_text.into(),
                timestamp: history.len() as u64,
                context_files: vec![],
                model_used: Some("test-model".into()),
                tokens: Some(3),
            });
            let json = serde_json::to_string(&history).expect("serialize history");
            let ctx = ctx_handle.write().expect("context write lock");
            ctx.set(vox_orchestrator::AgentId(0), &history_key, &json, 0);
        }

        #[tokio::test]
        async fn second_turn_conversation_includes_first_turn_content() {
            let state = test_state();
            let session_id = "f25-session";

            // Turn 1: user asks something distinctive, model answers.
            persist_turn(
                &state,
                session_id,
                "My favorite number is 42.",
                "Got it — 42 is your favorite number.",
            );

            // Turn 2: before the model is asked anything new, `chat_message` (via
            // `message.rs`) now calls `load_conversation` to build the request. We
            // call it directly here to assert on the exact message array a second
            // turn would see.
            let messages_for_turn_2 = load_conversation(&state, session_id).await;

            assert!(
                !messages_for_turn_2.is_empty(),
                "second turn must see prior history, not start from a blank slate"
            );
            let joined = messages_for_turn_2
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                joined.contains("My favorite number is 42."),
                "first turn's user message must reach the second turn's request: {joined}"
            );
            assert!(
                joined.contains("Got it — 42 is your favorite number."),
                "first turn's assistant reply must reach the second turn's request: {joined}"
            );

            // Simulate turn 2 completing and persisting, then verify a third turn
            // sees the whole chain (proves this isn't a one-off, off-by-one fluke).
            persist_turn(
                &state,
                session_id,
                "What's my favorite number?",
                "You said it's 42.",
            );
            let messages_for_turn_3 = load_conversation(&state, session_id).await;
            let joined_3 = messages_for_turn_3
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(joined_3.contains("My favorite number is 42."));
            assert!(joined_3.contains("What's my favorite number?"));
            assert!(joined_3.contains("You said it's 42."));
        }
    }
}
