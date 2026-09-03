//! Shared MCP chat model resolution (registry + token-budget hint).
//!
//! Callers pass a [`McpChatModelResolution`](crate::llm_bridge::McpChatModelResolution); when
//! `context_fill_ratio` is unset, it is filled from the global MCP LLM budget agent (`AgentId(0)`).
//!
//! ## Why the budget guard lives in three places, not one
//!
//! There is **no single universal chokepoint** for LLM dispatch in this crate — two
//! genuinely separate HTTP-issuing mechanisms exist, with no shared ancestor function:
//!
//! 1. `crate::llm_bridge::infer::mcp_infer_tool_completion` -> `infer_via_provider_adapter`
//!    (raw `reqwest` calls in `llm_bridge::providers`). This is the sole funnel for
//!    `call_llm`, `mcp_infer_completion` (used directly by the cognitive-profile chat
//!    path), `ghost_text`, `inline_edit`, `plan`, `plan_loop`, `compiler_tools`,
//!    `db_tools`, `oratio_tools`, `scientia_tools::assist`, and `browser_tools.rs`'s
//!    three direct `call_llm` call sites.
//! 2. `chat_tools::chat::agent_loop::run_agent_turn` -> `vox_actor_runtime::llm::llm_chat`
//!    -> `vox_llm_egress::chat_once` (the "sanctioned egress core"). This is used
//!    *only* by `try_run_agent_turn` (`chat_tools::chat::message`) — the tool-calling
//!    loop for the **default** `vox_chat_message` path (no `cognitive_profile` set),
//!    which is the primary real-world path for that tool.
//!
//! `resolve_chat_llm_model` (below) is *not* on the critical path for either: callers
//! that use it (see list above) still separately call into funnel (1) afterward, and
//! `try_run_agent_turn` (funnel 2) resolves its model via `resolve_mcp_chat_model`
//! directly, bypassing this function entirely. So [`enforce_budget_guard`] is called
//! from three places: here (fail-fast, before any registry work — intentionally
//! redundant with (2) below for callers that go through both), inside
//! `mcp_infer_tool_completion` itself (the actual universal point for funnel 1 —
//! covers every caller listed above, including the ones that bypass this function),
//! and inside `try_run_agent_turn` (the actual universal point for funnel 2).

use crate::llm_bridge::{
    McpChatModelResolution, mcp_global_llm_context_fill_ratio, resolve_mcp_chat_model,
};
use crate::server_state::ServerState;
use vox_orchestrator::models::ModelSpec;

/// Fetch recorded LLM spend (daily + this session) and check it against `VoxConfig`'s
/// budget caps via `budget_guard::check`. Shared by every genuine pre-dispatch
/// convergence point in this crate — see the module docs above for why there are
/// three call sites instead of one.
///
/// Reuses the pooled `VoxDb` handle already attached to `ServerState` (via
/// `with_db_initialized`) rather than opening a fresh connection per call. No DB
/// attached yet (fresh install / not connected) falls back to a zero spend summary,
/// matching the `get_llm_spend` GUI command's existing behavior for the same
/// not-yet-connected case. A failed spend query (locked file, I/O error) also falls
/// back to zero spend rather than blocking dispatch on an infra hiccup, but — unlike
/// the "no DB attached" case, which is an expected/benign state — is logged via
/// `tracing::warn!` so a degraded guard is at least observable instead of silently
/// swallowed.
///
/// `session_id` is `user_id` at some call sites — misleadingly named there, but it is
/// the real session id at every actual caller (see the daily/per-session cap tests
/// below for why passing `None` here would permanently disable the session-cap
/// branch of `budget_guard::check`).
pub(crate) async fn enforce_budget_guard(
    state: &ServerState,
    session_id: Option<&str>,
) -> Result<(), String> {
    let cfg = vox_config::VoxConfig::load();
    let spend = match &state.db {
        Some(db) => match db.llm_spend_summary(session_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "llm_spend_summary query failed; budget guard degraded to zero spend for this call"
                );
                Default::default()
            }
        },
        None => Default::default(),
    };
    let warning = crate::llm_bridge::budget_guard::check(
        &spend,
        cfg.daily_budget_usd,
        cfg.per_session_budget_usd,
        cfg.budget_warn_threshold_pct,
    )
    .map_err(|e| e.to_string())?;
    // `check` returns `Ok(Some(warning))` at `budget_warn_threshold_pct`, before the
    // hard block. There is no GUI-facing "warn" channel yet (unlike the "Err(Exceeded)"
    // case, which is a normal dispatch failure the GUI already surfaces as a toast) — a
    // non-blocking notice would need a new success-path signal from every dispatch
    // chokepoint to the GUI, which is out of scope for a guard fix. At minimum, don't
    // silently drop it: log it so it's observable (e.g. via `RUST_LOG=vox_orchestrator_mcp=info`
    // or a future telemetry consumer) instead of vanishing entirely.
    if let Some(w) = warning {
        tracing::info!(
            scope = ?w.scope,
            cap_usd = w.cap_usd,
            spent_usd = w.spent_usd,
            "budget warn threshold reached (dispatch still allowed)"
        );
    }
    Ok(())
}

/// Resolve model from sticky override + registry; fills `context_fill_ratio` when omitted.
pub async fn resolve_chat_llm_model(
    state: &ServerState,
    user_prompt: &str,
    mut resolution: McpChatModelResolution,
    user_id: Option<&str>,
) -> Result<(ModelSpec, bool), String> {
    enforce_budget_guard(state, user_id).await?;

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
            ttft_ms: None,
            tpot_ms: None,
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
    #[serial]
    async fn resolve_refuses_when_session_budget_exceeded() {
        // No `VoxConfig` env override exists for `per_session_budget_usd` (only
        // `daily_budget_usd` has one, via `VOX_BUDGET_USD`/`VoxBudgetUsd`) — it's
        // TOML-only. Rather than hardcoding the assumption that the ambient
        // `per_session_budget_usd` equals `VoxConfig::default()`'s `1.0` (fragile: a
        // stray `~/.vox/config.toml`/workspace `Vox.toml` with a different value on
        // some other machine would silently break this test's premise), read the
        // real loaded value and size the recorded spend relative to it. The daily
        // side of that same ambient-config risk *does* have a real seam
        // (`VOX_BUDGET_USD`), so pin it absurdly high here — this closes the "could
        // the daily branch trip first and hide a session-cap regression" gap
        // completely, rather than just hoping the on-disk default stays low.
        let prior = std::env::var("VOX_BUDGET_USD").ok();
        // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
        unsafe { std::env::set_var("VOX_BUDGET_USD", "1000000") };
        vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

        let per_session_cap = vox_config::VoxConfig::load().per_session_budget_usd;
        assert!(
            per_session_cap.is_finite() && per_session_cap >= 0.0,
            "ambient per_session_budget_usd is not a sane cap ({per_session_cap}); \
             cannot construct a spend value that reliably exceeds it"
        );
        // Comfortably over the cap regardless of what it ambiently is, and nowhere
        // close to the 1_000_000 daily cap pinned above.
        let session_spend = per_session_cap + 10.0;

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
            cost_usd: Some(session_spend),
            quality_score: Some(1.0),
            ttft_ms: None,
            tpot_ms: None,
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
        assert!(
            err.to_lowercase().contains("session"),
            "expected the SESSION cap (not daily, pinned to 1_000_000 above) to be the \
             one that tripped, got: {err}"
        );
    }
}
