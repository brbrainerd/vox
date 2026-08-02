//! Shared MCP chat model resolution (registry + token-budget hint).
//!
//! Callers pass a [`McpChatModelResolution`](crate::llm_bridge::McpChatModelResolution); when
//! `context_fill_ratio` is unset, it is filled from the global MCP LLM budget agent (`AgentId(0)`).

use crate::llm_bridge::{
    McpChatModelResolution, mcp_global_llm_context_fill_ratio, resolve_mcp_chat_model,
};
use crate::server_state::ServerState;
use vox_orchestrator::models::ModelSpec;

/// Resolve model from sticky override + registry; fills `context_fill_ratio` when omitted.
pub async fn resolve_chat_llm_model(
    state: &ServerState,
    user_prompt: &str,
    mut resolution: McpChatModelResolution,
    user_id: Option<&str>,
) -> Result<(ModelSpec, bool), String> {
    // Budget gate, before any model resolution work: reuse the pooled `VoxDb` handle
    // already attached to `ServerState` (via `with_db_initialized`) rather than opening
    // a fresh connection on every chat dispatch. No DB attached yet (fresh install /
    // not connected) falls back to a zero spend summary, matching the `get_llm_spend`
    // GUI command's existing behavior for the same not-yet-connected case.
    //
    // `user_id` is misleadingly named at this call boundary: every real caller
    // (chat/message.rs, ghost_text.rs, inline_edit.rs, plan.rs, plan_loop.rs,
    // compiler_tools.rs, db_tools.rs, oratio_tools.rs) actually passes the current
    // session id here, not an end-user identity. Pass it straight through to
    // `llm_spend_summary` so the per-session cap is evaluated against real
    // session spend — `llm_spend_summary(None)` would report `session_usd: 0.0`
    // unconditionally, permanently disabling the session-cap branch of
    // `budget_guard::check` for every caller.
    let cfg = vox_config::VoxConfig::load();
    let spend = match &state.db {
        Some(db) => db.llm_spend_summary(user_id).await.unwrap_or_default(),
        None => Default::default(),
    };
    if let Err(e) = crate::llm_bridge::budget_guard::check(
        &spend,
        cfg.daily_budget_usd,
        cfg.per_session_budget_usd,
        cfg.budget_warn_threshold_pct,
    ) {
        return Err(e.to_string());
    }

    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => return Err(e.to_string()),
    };
    let orch = &state.orchestrator;
    if resolution.context_fill_ratio.is_none() {
        resolution.context_fill_ratio = mcp_global_llm_context_fill_ratio(orch);
    }
    resolve_mcp_chat_model(state, user_prompt, pref.as_deref(), resolution, user_id).await
}

#[cfg(test)]
#[allow(unsafe_code)] // test-only std::env::set_var (unsafe on edition 2024); serialized via #[serial]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;
    use vox_db::store::types::ModelOutcome;
    use vox_db::{DbConfig, VoxDb};

    #[tokio::test]
    #[serial]
    async fn resolve_refuses_when_daily_budget_exceeded() {
        let prior = std::env::var("VOX_BUDGET_USD").ok();
        // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
        unsafe { std::env::set_var("VOX_BUDGET_USD", "0.01") };
        vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("open in-memory db");
        db.record_llm_outcome(ModelOutcome {
            session_id: "resolve-test",
            user_id: None,
            tenant_id: None,
            prompt: "p",
            response: "r",
            model_id: "m",
            provider: "openrouter",
            task_category: "general",
            strength_tag: "generalist",
            latency_ms: Some(10),
            input_tokens: Some(5),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            trace_id: None,
            context_utilization_pct: None,
            success: true,
            cost_usd: Some(0.02),
            quality_score: Some(1.0),
        })
        .await
        .expect("record spend");

        let state = ServerState::new_test()
            .await
            .with_db_initialized(Arc::new(db))
            .await;

        let result =
            resolve_chat_llm_model(&state, "hello", McpChatModelResolution::default(), None).await;

        // SAFETY: `#[serial]` — restore prior env state before asserting/panicking.
        unsafe {
            match &prior {
                Some(v) => std::env::set_var("VOX_BUDGET_USD", v),
                None => std::env::remove_var("VOX_BUDGET_USD"),
            }
        }
        vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

        let err = result.expect_err("expected budget guard to refuse dispatch");
        assert!(
            err.to_lowercase().contains("budget"),
            "expected error to mention budget, got: {err}"
        );
    }

    #[tokio::test]
    #[serial] // shares the crate-wide `#[serial]` lock with the daily-cap test above,
    // so this can't observe the other test's mid-flight `VOX_BUDGET_USD` override.
    async fn resolve_refuses_when_session_budget_exceeded() {
        // No `VoxConfig` env override exists for `per_session_budget_usd` (only
        // `daily_budget_usd` has one, via `VOX_BUDGET_USD`/`VoxBudgetUsd`) — it's
        // TOML-only. So this test relies on `VoxConfig::default()`'s
        // `per_session_budget_usd: 1.0` (confirmed no `budget` key exists in this
        // repo's root `Vox.toml` or in any global `~/.vox/config.toml` that would
        // override it) and records spend comfortably under the 5.0 daily default,
        // so the *session* branch of `budget_guard::check` is what trips — not
        // the daily branch already covered above.
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("open in-memory db");
        // Spend recorded against the exact session id `resolve_chat_llm_model` is
        // called with below (passed through as its `user_id` parameter) — proves
        // the per-session cap is evaluated against real session spend, not the
        // permanently-zero `session_usd` that `llm_spend_summary(None)` returns.
        db.record_llm_outcome(ModelOutcome {
            session_id: "session-under-test",
            user_id: None,
            tenant_id: None,
            prompt: "p",
            response: "r",
            model_id: "m",
            provider: "openrouter",
            task_category: "general",
            strength_tag: "generalist",
            latency_ms: Some(10),
            input_tokens: Some(5),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            trace_id: None,
            context_utilization_pct: None,
            success: true,
            cost_usd: Some(1.5), // over the 1.0 session default, under the 5.0 daily default
            quality_score: Some(1.0),
        })
        .await
        .expect("record spend");

        let state = ServerState::new_test()
            .await
            .with_db_initialized(Arc::new(db))
            .await;

        let result = resolve_chat_llm_model(
            &state,
            "hello",
            McpChatModelResolution::default(),
            Some("session-under-test"),
        )
        .await;

        let err = result.expect_err("expected budget guard to refuse dispatch");
        assert!(
            err.to_lowercase().contains("budget"),
            "expected error to mention budget, got: {err}"
        );
        assert!(
            err.to_lowercase().contains("session"),
            "expected the SESSION cap (not daily) to be the one that tripped, got: {err}"
        );
    }
}
