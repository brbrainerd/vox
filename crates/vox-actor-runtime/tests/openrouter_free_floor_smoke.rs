//! Live smoke test: a concrete OpenRouter `:free` slug actually dispatches and
//! returns a completion. This proves the research free-tier FLOOR is real — i.e.
//! the slugs in `vox_config::OPENROUTER_FREE_FALLBACK_MODELS` are dispatchable,
//! not a non-dispatchable virtual id (the AGH-0006 defect). Verifies EFFECT, not
//! shape.
//!
//! Network + `OPENROUTER_API_KEY` required → `#[ignore]`. Run on demand:
//!   cargo test -p vox-actor-runtime --test openrouter_free_floor_smoke -- --ignored --nocapture

use vox_actor_runtime::activity::ActivityOptions;
use vox_actor_runtime::llm::cascade::chat_with_cascade;
use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig};

#[tokio::test]
#[ignore = "requires OPENROUTER_API_KEY and network; run with --ignored"]
async fn free_floor_slug_dispatches_and_returns_content() {
    if vox_config::inference::openrouter_api_key().is_none() {
        eprintln!("SKIP: OPENROUTER_API_KEY not set — cannot run live dispatch smoke test");
        return;
    }

    let slug = vox_config::OPENROUTER_FREE_FALLBACK_MODELS[0];
    assert!(
        slug.ends_with(":free"),
        "floor slug must be a free model: {slug}"
    );

    let candidate = LlmConfig::openrouter(slug);
    let messages = vec![LlmChatMessage {
        role: "user".to_string(),
        content: "Reply with exactly the single word: pong".to_string(),
        ..Default::default()
    }];
    let opts = ActivityOptions::new().with_timeout_secs(60);

    let response = chat_with_cascade(&opts, messages, vec![candidate], None)
        .await
        .unwrap_or_else(|e| panic!("free slug `{slug}` failed to dispatch: {e}"));

    assert!(
        !response.content.trim().is_empty(),
        "free slug `{slug}` dispatched but returned empty content"
    );
    eprintln!("OK: `{slug}` dispatched -> {:?}", response.content);
}
