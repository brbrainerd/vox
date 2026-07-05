#![allow(missing_docs)]
//! T4.2 acceptance test: context management = recover + wire, not build.
//!
//! Drives a session's conversation history over the `CompactionEngine`
//! threshold, assembles the message list via
//! [`vox_orchestrator::SessionManager::assemble_llm_messages`] (the wiring
//! point this task adds immediately before `llm_chat`/`llm_stream`), and
//! verifies:
//!
//! 1. A real `llm_chat` call over the compacted message list completes
//!    successfully (proves the wiring wasn't just measured, but actually
//!    used to shrink what reaches the wire).
//! 2. A compaction event was emitted (`CompactionResult::compacted == true`,
//!    dropped turn count > 0).
//! 3. The turns dropped by compaction are NOT silently lost — they are
//!    retrievable afterward via `Session::archived_turns()` with their exact
//!    original content, and the message list handed to the LLM shrank
//!    relative to the raw session history (the compaction actually ran
//!    before the wire call, not just resulting in a `None`/no-op status).
//!
//! This is an integration test of the real wiring (`SessionManager` +
//! `CompactionEngine` + `vox_actor_runtime::llm::llm_chat`), not a unit test
//! of `CompactionEngine` in isolation — that coverage already exists in
//! `crates/vox-orchestrator/src/compaction.rs`'s own `#[cfg(test)]` module.

use tempfile::TempDir;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use vox_orchestrator::{
    AgentId, CompactionConfig, CompactionEngine, SessionConfig, SessionManager,
};

fn test_session_config(dir: &TempDir) -> SessionConfig {
    SessionConfig {
        sessions_dir: dir.path().to_path_buf(),
        repository_id: None,
        idle_timeout_secs: 3600,
        archive_timeout_secs: 7200,
        max_sessions: 8,
        persist: false,
    }
}

/// A `CompactionEngine` with a deliberately tiny budget so a handful of
/// pushed turns drives the session over the limit without needing to
/// construct tens of thousands of tokens of fixture text.
fn tiny_engine() -> CompactionEngine {
    CompactionEngine::new(CompactionConfig {
        max_context_tokens: 100,
        reserved_tokens: 10,
        compaction_threshold: 0.5, // trigger at 50 tokens
        min_viable_tokens: 5,
        strategy: vox_orchestrator::CompactionStrategy::Balanced,
        head_preserve_tokens: 10,
        tail_preserve_tokens: 15,
        complexity_token_weight: 32,
    })
}

/// RED-then-GREEN acceptance test: a conversation driven over the context
/// limit completes successfully via the real `llm_chat` wiring, emits a
/// compaction event, and the dropped turns remain retrievable afterward.
#[tokio::test]
async fn over_limit_conversation_compacts_and_archives_losslessly_before_llm_call() {
    let dir = TempDir::new().expect("tempdir");
    let cfg = test_session_config(&dir);
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let session_id = mgr.create(AgentId(1), None).expect("create session");

    // Push enough turns to exceed the tiny engine's 50-token trigger.
    // Each turn is padded to a known token estimate via repeated content so
    // the whitespace-count fallback (feature `token-counting` is off by
    // default in this crate) still produces a nontrivial, predictable count.
    let turn_bodies = [
        ("user", "alpha ".repeat(20)),
        ("assistant", "bravo ".repeat(20)),
        ("user", "charlie ".repeat(20)),
        ("assistant", "delta ".repeat(20)),
        ("user", "echo ".repeat(20)),
        ("assistant", "foxtrot ".repeat(20)),
    ];
    for (role, content) in &turn_bodies {
        let tokens = CompactionEngine::estimate_tokens(content);
        mgr.add_turn(&session_id, *role, content.clone(), tokens)
            .expect("add turn");
    }

    let raw_turn_count = mgr.get(&session_id).expect("get").turns.len();
    let raw_tokens = mgr.get(&session_id).expect("get").current_tokens();
    assert!(
        raw_tokens >= tiny_engine().config().trigger_at(),
        "fixture must actually be over the compaction trigger; got {raw_tokens} tokens"
    );

    // This is the T4.2 wiring point: message assembly runs compaction
    // automatically when over threshold, archiving dropped turns losslessly,
    // before the message list is hand to the LLM call.
    let engine = tiny_engine();
    let (messages, compaction_result) = mgr
        .assemble_llm_messages(&session_id, &engine)
        .expect("assemble messages");

    // 1. A compaction event was actually emitted (not a no-op).
    let result = compaction_result.expect("compaction must have run — fixture is over threshold");
    assert!(result.compacted);
    assert!(
        result.dropped_count > 0,
        "compaction must have dropped at least one turn"
    );
    assert_eq!(result.dropped_turns.len(), result.dropped_count);

    // The assembled message list actually shrank relative to raw history —
    // proving compaction ran before assembly, not just recorded a status.
    assert!(
        messages.len() < raw_turn_count,
        "compacted message list ({}) must be shorter than raw history ({raw_turn_count})",
        messages.len()
    );

    // 2. Dropped turns are retrievable afterward — not silently lost.
    let session = mgr.get(&session_id).expect("get session after compaction");
    let archived = session.archived_turns();
    assert_eq!(
        archived.len(),
        result.dropped_count,
        "every dropped turn must be archived exactly once"
    );
    // Cross-check: every archived turn's content matches one of the original
    // pushed turn bodies exactly (lossless — not a summarized/mutated copy).
    for archived_turn in archived {
        assert!(
            turn_bodies
                .iter()
                .any(|(_, body)| body == &archived_turn.content),
            "archived turn content must exactly match an original turn"
        );
    }

    // 3. The compacted message list completes a real `llm_chat` call
    // successfully — proving the wiring is actually consumable end-to-end,
    // not just internally self-consistent.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-test",
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "ok"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 12, "completion_tokens": 1, "total_tokens": 13}
        })))
        .mount(&server)
        .await;

    let llm_config = vox_actor_runtime::llm::LlmConfig {
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
    let opts = vox_actor_runtime::ActivityOptions::new();
    let outcome = vox_actor_runtime::llm::llm_chat(&opts, messages, llm_config).await;

    match outcome {
        vox_actor_runtime::ActivityResult::Ok(Ok(resp)) => {
            assert_eq!(resp.content, "ok");
        }
        other => {
            panic!("expected the compacted conversation's llm_chat call to succeed, got {other:?}")
        }
    }
}
