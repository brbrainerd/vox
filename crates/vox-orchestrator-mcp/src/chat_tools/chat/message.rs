use serde_json::Value;

use super::super::params::{ANTI_LAZINESS_RIDER, ChatMessageParams, ChatTranscriptEntry};
use super::super::{build_system_prompt_with_skill, now_ts, ts_to_date_str};
use super::conversation::{load_conversation, trim_persisted_history};
use super::hydrate::context_history_or_hydrate;
use super::mentions::{chat_grounding_score, resolve_mentions};
use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::chat_socrates_meta::{
    LlmSurfaceTelemetry, clarification_turn_for_session, mcp_questioning_session_key,
    socrates_surface_tags, socrates_tool_meta, spawn_questioning_trace_from_socrates,
    spawn_socrates_telemetry_with_llm,
};
use crate::journey_envelope;
use crate::llm_bridge::{McpChatModelResolution, McpInferRouting, call_llm, call_llm_with_pref};
use crate::memory::{
    RetrievalTriggerMode, run_retrieval_bundle, should_trigger_autonomous_research,
};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use crate::session_identity::normalize_chat_session_id;
use vox_actor_runtime::prompt_canonical;
use vox_orchestrator::session_context_envelope_key;

const REM_CHAT_CANONICAL: &str = "Rewrite the prompt to remove disallowed content / injection patterns; simplify objectives and retry.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";

/// Honest classification of *why* the resolved model is what it is, derived
/// from the resolved model id (never the requested one) — see Phase B of
/// `docs/superpowers/plans/2026-08-28-chat-harness-unification.md`. Classifying
/// from the request meant a pinned model the resolver silently ignored still
/// read "Your pick" in `ModelBadge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionSource {
    /// `requested == resolved`: the user's pin was honored.
    UserOverride,
    /// No per-request pref, but the process-global sticky override matches
    /// the resolved model.
    Global,
    /// No pref at all; the auto-selector chose.
    AutoRouted,
    /// A pref was requested but the resolved model differs from it (removed
    /// from the registry, capability mismatch, etc.).
    Fallback,
}

impl SelectionSource {
    /// Classify from the RESOLVED model, not the requested one — that is the
    /// whole point of this type. `requested` is the per-request pin (if any),
    /// `resolved` is the model id the turn actually used, `global` is the
    /// process-global sticky override (if any).
    #[must_use]
    pub fn classify(requested: Option<&str>, resolved: Option<&str>, global: Option<&str>) -> Self {
        match (requested, resolved) {
            (Some(r), Some(res)) if r == res => SelectionSource::UserOverride,
            (Some(_), Some(_)) => SelectionSource::Fallback,
            (None, Some(res)) if global == Some(res) => SelectionSource::Global,
            _ => SelectionSource::AutoRouted,
        }
    }
}

/// Envelope payload for `ModelBadge`: the resolved model, an honest
/// [`SelectionSource`], and — for anything other than a plain user pin — the
/// rationale explaining why.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SelectionDto {
    pub model: String,
    pub source: SelectionSource,
    pub rationale: Option<String>,
}

impl SelectionDto {
    fn new(
        requested: Option<&str>,
        resolved: &str,
        global: Option<&str>,
        rationale: Option<String>,
    ) -> Self {
        Self {
            model: resolved.to_string(),
            source: SelectionSource::classify(requested, Some(resolved), global),
            rationale,
        }
    }

    /// A fallback selection always carries a rationale — never a silent `None`.
    #[cfg(test)]
    fn fallback(model: impl Into<String>, rationale: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            source: SelectionSource::Fallback,
            rationale: Some(rationale.into()),
        }
    }
}

/// [`try_run_agent_turn`]'s success payload. A named struct (rather than a
/// 4-tuple) now that the model-selection rationale rides along too — see
/// Fix Task 7.
#[derive(Debug)]
struct AgentTurnResult {
    text: String,
    model_used: String,
    tokens: u64,
    /// Human-readable reason the model was chosen (`SelectionReason::to_string()`),
    /// when the rationale-carrying resolver produced one. Surfaced to the GUI as
    /// `data.selection_reason` for `ModelBadge`'s tooltip.
    selection_reason: Option<String>,
    /// Chat-turn-visible events derived from tool results this turn (Phase E
    /// Task E1) — empty on every path except the real agent loop
    /// ([`super::agent_loop::run_agent_turn`]), which is the only path that
    /// dispatches tool calls at all.
    events: Vec<Value>,
}

/// Model preference for this turn: per-request pick, then the process-global
/// override, then nothing (the auto-selector runs). Blanks are not picks — the
/// resolver treats `""` as an explicit, unresolvable model id, so an empty
/// composer field must neither shadow the global nor reach it.
pub fn effective_model_pref(request: Option<&str>, global: Option<&str>) -> Option<String> {
    request
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| global.map(str::trim).filter(|s| !s.is_empty()))
        .map(str::to_string)
}

/// Composer tier -> resolution constraints. Deliberately NOT `cognitive_profile`:
/// that field's values are `fast|reasoning|creative` and setting it switches the
/// turn off the agent loop onto `mcp_infer_completion`, losing tool calls and
/// the selection rationale.
pub fn resolution_for_tier(
    tier: Option<&str>,
    base: McpChatModelResolution,
) -> McpChatModelResolution {
    match tier.map(str::trim) {
        Some("local") => McpChatModelResolution {
            enforce_free_tier_only: true,
            // Restore the pre-tier-routing degradation path (previously
            // unconditional for every call site). This is safe to keep on for
            // `local`: `enforce_free_tier_if_needed` (model_route_policy/policy.rs)
            // gates every returned model through free-tier enforcement, so
            // `allow_cheapest_fallback` here can only ever widen which *free*
            // model gets picked when the primary `decide()` selection comes up
            // empty -- it can never leak a paid model back out.
            allow_cheapest_fallback: true,
            ..base
        },
        Some("mesh") => McpChatModelResolution {
            free_tier_latency_critical: true,
            // Same reasoning as `local` above -- restore the fallback rather
            // than silently dropping it just because a tier was named.
            allow_cheapest_fallback: true,
            ..base
        },
        Some("cloud") => McpChatModelResolution {
            // Explicitly paid-preferring and observably different from both
            // "auto" (`_` arm, which always allows degrading to a cheap/free
            // fallback) and from `McpChatModelResolution::default()`: never let
            // a failed/absent primary pick quietly degrade to the cheapest free
            // model, and bias `decide()`'s `SelectionRequest.intent.complexity`
            // (resolve.rs `build_selection_request`) toward stronger models so a
            // "cloud" pick doesn't land on a weak model by the scorer's default.
            allow_cheapest_fallback: false,
            enforce_free_tier_only: false,
            complexity: base.complexity.max(8),
            ..base
        },
        Some(other) => {
            // Unknown/typo'd tier (e.g. "Cloud", "remote"): `tier` is a
            // free-text `Option<String>` at the Rust type level even though the
            // composer only ever sends "local"/"mesh"/"cloud"/omitted, so a typo
            // is representable. Explicit decision (not silent aliasing): treat
            // it exactly like "no tier" (auto/default), and log it so the typo
            // is debug-visible instead of just quietly "working" with no trace.
            // Known gap: this function has no channel back to the caller's
            // `selection_reason`/rationale to surface the typo in the GUI (see
            // `try_run_agent_turn`'s `selection_reason` plumbing) -- wiring
            // that through is out of scope for this fix.
            tracing::warn!(
                target: "vox_mcp::chat_tier",
                tier = other,
                "unrecognized chat tier; falling back to auto/default resolution"
            );
            McpChatModelResolution {
                allow_cheapest_fallback: true,
                ..base
            }
        }
        None => McpChatModelResolution {
            allow_cheapest_fallback: true,
            ..base
        },
    }
}

/// Task 1.3d (F24 wiring): attempt the tool-calling agent loop
/// ([`super::agent_loop::run_agent_turn`]) for the default (non-`cognitive_profile`)
/// `vox_chat_message` path.
///
/// Returns:
/// - `None` when this turn should fall back to the existing `call_llm` pipeline
///   unchanged — either because `has_attachment` is `true` (the mapper does not
///   handle vision/attachment content) or because the resolved model's
///   [`vox_orchestrator::models::ProviderType`] isn't one of the simple shapes
///   [`super::agent_loop::model_spec_to_llm_config`] covers (OpenRouter, Ollama).
/// - `Some(Ok(..))` / `Some(Err(..))` when the agent loop actually ran.
///
/// Model *selection*: this calls
/// `crate::llm_bridge::resolve_mcp_chat_model_with_rationale` (the rationale-carrying
/// sibling of `resolve_mcp_chat_model`, which `call_llm` also uses internally via
/// `mcp_infer_completion` — both delegate to the same
/// `resolve_mcp_chat_model_sync_inner`) so this hot path's `selection_reason` reaches
/// the `vox_chat_message` envelope for `ModelBadge`'s tooltip. Everything else about
/// *after* a model is chosen is unchanged. `temperature`/`top_p` are
/// `params.temperature`/`params.top_p` straight from the request, applied to the
/// mapped `LlmConfig` exactly as the `call_llm` fallback applies them via
/// `temperature_override`/`top_p_override`.
async fn try_run_agent_turn(
    state: &ServerState,
    system_prompt: &str,
    user_prompt: &str,
    session_id: &str,
    active_skill_id: Option<String>,
    has_attachment: bool,
    temperature: Option<f32>,
    top_p: Option<f32>,
    request_model_override: Option<&str>,
    tier: Option<&str>,
    clutch: Option<&str>,
    risk: Option<&str>,
) -> Option<Result<AgentTurnResult, String>> {
    if has_attachment {
        return None;
    }

    // Budget gate, before any model resolution work. This function (not
    // `resolve_chat_llm_model`) is the real entry point for the DEFAULT
    // `vox_chat_message` path: it resolves via `resolve_mcp_chat_model` directly and,
    // when the model maps to a simple provider shape, dispatches through
    // `run_agent_turn` -> `vox_actor_runtime::llm::llm_chat` ->
    // `vox_llm_egress::chat_once` — a completely separate HTTP-issuing mechanism from
    // `mcp_infer_tool_completion`'s `infer_via_provider_adapter`, with no shared
    // ancestor and no budget check of its own. See `chat_model_resolve`'s module docs
    // for the full map of why the guard lives in three places, not one.
    if let Err(e) = crate::chat_model_resolve::enforce_budget_guard(state, Some(session_id)).await {
        return Some(Err(e));
    }

    let global_pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => return Some(Err(e.to_string())),
    };
    let pref = effective_model_pref(request_model_override, global_pref.as_deref());
    let context_fill_ratio =
        crate::llm_bridge::mcp_global_llm_context_fill_ratio(&state.orchestrator);
    let resolution_template = resolution_for_tier(
        tier,
        McpChatModelResolution {
            context_fill_ratio,
            clutch: clutch.and_then(vox_orchestrator::mode::ClutchProfile::from_label),
            risk: risk.and_then(vox_orchestrator::mode::RiskPosture::from_label),
            ..Default::default()
        },
    );
    let choice = match crate::llm_bridge::resolve_mcp_chat_model_with_rationale(
        state,
        user_prompt,
        pref.as_deref(),
        resolution_template,
        Some(session_id),
    )
    .await
    {
        Ok(c) => c,
        // Model resolution failing here is not this function's problem to report —
        // let the existing `call_llm` path attempt (and correctly surface) it. But
        // when the caller pinned a model, don't swallow the reason it couldn't be
        // honored entirely: log it so it's debug-visible instead of vanishing
        // silently into an unrelated auto-selected fallback.
        Err(e) => {
            if request_model_override.is_some() {
                tracing::warn!(
                    target: "vox_mcp::model_selection",
                    pref = ?pref,
                    error = %e,
                    "user-supplied model pref failed to resolve; falling back to call_llm pipeline"
                );
            }
            return None;
        }
    };
    let model = choice.model;
    let is_free = choice.is_free;
    let selection_reason = choice.rationale;

    let mut llm_config = super::agent_loop::model_spec_to_llm_config(&model)?;
    // Thread sampling overrides through on the mapped path exactly as the
    // `call_llm` fallback already does (via `temperature_override`/`top_p_override`
    // -> `mcp_infer_completion`) — without this, a caller's temperature/top_p would
    // be silently dropped for every provider this task newly routes through
    // `run_agent_turn` (OpenRouter/Ollama).
    llm_config.temperature = temperature;
    llm_config.top_p = top_p;

    // `Box::pin`: `run_agent_turn` dispatches tool calls through
    // `handle_tool_call_with_mode`, which (for the `vox_chat_message` tool
    // specifically) can call back into `chat_message` -> `try_run_agent_turn` ->
    // `run_agent_turn` — a real mutual-recursion cycle the compiler must be able
    // to size, hence the heap indirection here rather than a plain `.await`.
    match Box::pin(super::agent_loop::run_agent_turn(
        state,
        Some(session_id),
        vec![], // history is already folded into `user_prompt` as text (see Task 1.1
        // context_parts above) — passing it again here would duplicate it.
        system_prompt.to_string(),
        user_prompt.to_string(),
        None, // permission_mode: `ChatMessageParams` carries no transport-authenticated
        // permission mode; `handle_tool_call_with_mode`'s fail-safe default (Ask)
        // applies, matching every other MCP call path that doesn't have one.
        active_skill_id,
        llm_config,
        super::agent_loop::DEFAULT_MAX_ITERATIONS,
        // Task G1: stream tokens over the existing agent-events bus as they
        // arrive, carrying this turn's session_id so the GUI's
        // `resolveSessionForEvent` can route the frame straight to the
        // quick-chat bubble instead of only updating once the full reply is
        // back.
        true,
    ))
    .await
    {
        Ok(outcome) => {
            tracing::info!(
                target: "vox_mcp::agent_loop",
                tool_calls_made = outcome.tool_calls_made,
                hit_iteration_limit = outcome.hit_iteration_limit,
                model = %outcome.model_used,
                "vox_chat_message agent-loop turn completed"
            );
            let final_text = if outcome.hit_iteration_limit {
                tracing::warn!(
                    target: "vox_mcp::agent_loop",
                    tool_calls_made = outcome.tool_calls_made,
                    max_iterations = super::agent_loop::DEFAULT_MAX_ITERATIONS,
                    "vox_chat_message agent-loop turn hit the iteration bound before the \
                     model returned a final answer"
                );
                format!(
                    "{}\n\n[Note: this response was cut off after {} tool-use round-trips \
                     without reaching a final answer.]",
                    outcome.final_text, outcome.tool_calls_made
                )
            } else {
                outcome.final_text
            };

            // Task M1/M2: fire-and-forget the three-axis join (harness_eval_run +
            // harness_eval_task_result + model_selection_event) for this turn, tagged
            // `triggered_by = "live_chat"` so `list_harness_eval_runs` keeps excluding it
            // from real `vox harness eval --live` reporting.
            //
            // `completeness_ok` here is `!hit_iteration_limit && !has_unbalanced_fence` — the
            // two `completeness_ok` components knowable synchronously at turn-completion time.
            // `hit_iteration_limit` stands in for the plan's `finish_reason == 'length'` check
            // (no wire finish_reason is captured anywhere between the egress response and
            // `AgentTurnOutcome` today — a real gap, left for a follow-up rather than a blind
            // cross-crate wire-parsing change); a turn that was cut off before a final answer
            // is the same failure shape either way. The third component, "user re-asked within
            // 2 turns", genuinely can't be known yet — it's queued via
            // `queue_live_chat_reask_check` and resolved retroactively below, by the NEXT
            // turn's own write (each new turn rescores its session's still-pending prior
            // turns before recording itself).
            if let Some(db) = state.db.clone() {
                let task_id = format!(
                    "{session_id}-{}",
                    vox_telemetry::current_trace_ctx().trace_id
                );
                let model_id = model.id.clone();
                let blended = vox_orchestrator::models::blended_cost_per_1k(&model);
                let cost_tier = vox_orchestrator::models::cost_tier_for_blended(is_free, blended)
                    .as_str()
                    .to_string();
                let selection_reason_for_event = selection_reason.clone().unwrap_or_default();
                let completeness_ok = !outcome.hit_iteration_limit
                    && !vox_orchestrator::grounding::has_unbalanced_fence(&final_text);
                let session_run_prefix = session_id.to_string();
                let user_prompt_owned = user_prompt.to_string();
                // `outcome.latency_ms` used to have no path to this call at all -- this
                // record_live_chat_turn call passed a hardcoded `None` for both cost_usd and
                // latency_ms since Task M1. cost_usd genuinely isn't computed anywhere on
                // this path yet (a separate gap); latency_ms is now real (Task M3).
                let turn_latency_ms = outcome.latency_ms.map(|v| v as i64);
                tokio::spawn(async move {
                    // Rescore this session's still-pending prior turns against this turn's
                    // prompt first — cheap (at most 2 rows), and keeps each pending row's
                    // window tight to "the next 2 actual user turns" rather than drifting if a
                    // later write races ahead of an earlier one.
                    if let Ok(pending) =
                        db.pending_live_chat_reask_checks(&session_run_prefix).await
                    {
                        for check in pending {
                            let matched = vox_orchestrator::grounding::looks_like_reask(
                                &check.user_prompt,
                                &user_prompt_owned,
                            );
                            let _ = db
                                .resolve_live_chat_reask_check(&check.run_id, matched)
                                .await;
                        }
                    }

                    let now_ms = chrono::Utc::now().timestamp_millis();
                    let Ok(run_id) = db
                        .record_live_chat_turn(
                            &task_id,
                            &model_id,
                            &cost_tier,
                            &selection_reason_for_event,
                            None,
                            turn_latency_ms,
                            completeness_ok,
                            now_ms,
                        )
                        .await
                    else {
                        return;
                    };
                    if completeness_ok {
                        let _ = db
                            .queue_live_chat_reask_check(
                                &run_id,
                                &task_id,
                                &session_run_prefix,
                                &user_prompt_owned,
                                now_ms,
                            )
                            .await;
                    }
                });
            }

            Some(Ok(AgentTurnResult {
                text: final_text,
                model_used: outcome.model_used,
                tokens: outcome.total_tokens,
                selection_reason,
                events: outcome.events,
            }))
        }
        Err(e) => Some(Err(e)),
    }
}

/// Handle a user chat message. Resolves @mentions, injects context from the editor,
/// calls the best available LLM, persists to session history, and returns the updated history.
///
/// **Session Isolation**: History is keyed by `params.session_id` (defaulting to `"default"`).
/// Each unique session_id maintains a completely independent chat transcript in the
/// orchestrator `ContextStore`. Pass a stable UUID/slug per-window to prevent context bleeding.
///
/// **Autonomous Research**: Before invoking the LLM, this function silently queries the
/// `MemoryManager` and knowledge graph for facts related to the prompt. High-relevance hits
/// are injected as `[AUTONOMOUS RESEARCH]` preamble blocks so the model has evidence without
/// the user needing to explicitly invoke search tools.
///
/// **Cognitive Profile Routing**: Pass `"fast"`, `"reasoning"`, or `"creative"` to influence
/// model selection and temperature without changing the MCP tool contract.
pub async fn chat_message(state: &ServerState, params: ChatMessageParams) -> String {
    // 1. Resolve @mentions in the prompt
    let workspace_root = state
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let (expanded_prompt, mention_files) =
        resolve_mentions(&params.prompt, &workspace_root, &state.mention_path_cache);
    let (expanded_prompt, canonical_meta) = match prompt_canonical::canonicalize_prompt(
        &expanded_prompt,
        true, // order_invariant
        true, // run_safety_pass
    ) {
        Ok(c) => {
            let hash = c.original_hash;
            let conflict_count = c.conflict_warnings.len();
            let objective_count = c.objectives.len();
            (c.text, Some((hash, conflict_count, objective_count)))
        }
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("Prompt rejected by safety canonicalizer: {e}"),
                REM_CHAT_CANONICAL,
            )
            .to_json();
        }
    };
    let mention_count = mention_files.len();
    if let Some((original_hash, conflict_count, objective_count)) = canonical_meta {
        tracing::debug!(
            target: "vox_mcp::prompt_canonical",
            original_hash = %original_hash,
            conflict_warning_count = conflict_count,
            objective_count = objective_count,
            "chat prompt canonicalized"
        );
    }

    // Resolve session id early (before the rest of the function needs it) so we can
    // load this session's prior conversation turns and thread them into the request —
    // see Task 1.1 / Finding F25: history was persisted and returned for display but
    // never actually sent to the model.
    let (session_id, implicit_session_default) =
        normalize_chat_session_id(params.session_id.as_deref());
    let prior_conversation = load_conversation(state, session_id.as_str()).await;

    // 2a. Build context preamble from editor state
    let mut context_parts = Vec::new();

    if !prior_conversation.is_empty() {
        let transcript = prior_conversation
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        context_parts.push(format!("[CONVERSATION HISTORY]:\n{transcript}"));
    }

    if let Some(active_file) = &params.active_file {
        let line_info = params
            .active_line
            .map(|l| format!(" (line {l})"))
            .unwrap_or_default();
        context_parts.push(format!("[ACTIVE FILE]: {active_file}{line_info}"));
    }

    if let Some(selected) = &params.selected_text
        && !selected.is_empty()
    {
        context_parts.push(format!("[SELECTED TEXT]:\n{selected}"));
    }

    if !params.diagnostics.is_empty() {
        let diag_str: Vec<String> = params
            .diagnostics
            .iter()
            .filter_map(|d| {
                let msg = d["message"].as_str()?;
                let line = d["line"].as_u64().unwrap_or(0);
                let sev = d["severity"].as_str().unwrap_or("error");
                Some(format!("  Line {line} [{sev}]: {msg}"))
            })
            .collect();
        if !diag_str.is_empty() {
            context_parts.push(format!(
                "[ACTIVE ERRORS/WARNINGS]:\n{}",
                diag_str.join("\n")
            ));
        }
    }

    if !params.open_files.is_empty() {
        context_parts.push(format!("[OPEN FILES]: {}", params.open_files.join(", ")));
    }

    // 2b/2c. Unified autonomous retrieval injection:
    // Use the same retrieval pipeline as `vox_memory_search` with deterministic fallback
    // (hybrid -> BM25 -> lexical fallback), then append memory + knowledge snippets.
    let mut retrieval_evidence = None;
    let retrieval_trace = params
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            params
                .correlation_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        });
    match run_retrieval_bundle(
        state,
        &expanded_prompt,
        RetrievalTriggerMode::AutoChatPreamble,
        3,
        retrieval_trace,
    )
    .await
    {
        Ok(bundle) => {
            if !bundle.rrf_fused_lines.is_empty() {
                let snippets = bundle
                    .rrf_fused_lines
                    .iter()
                    .map(|h| format!("- {h}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — RRF FUSION (tier: {})]:\n{snippets}",
                    bundle.evidence.retrieval_tier
                ));
            }
            if !bundle.memory_lines.is_empty() {
                let snippets = bundle
                    .memory_lines
                    .iter()
                    .map(|h| format!("- {h}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — MEMORY (tier: {})]:\n{snippets}",
                    bundle.evidence.retrieval_tier
                ));
            }
            if !bundle.knowledge_lines.is_empty() {
                let formatted = bundle
                    .knowledge_lines
                    .iter()
                    .map(|n| format!("- {n}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — KNOWLEDGE GRAPH]:\n{formatted}"
                ));
            }
            if !bundle.chunk_lines.is_empty() {
                let formatted = bundle
                    .chunk_lines
                    .iter()
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — DOCUMENT CHUNKS]:\n{formatted}"
                ));
            }
            if !bundle.repo_lines.is_empty() {
                let formatted = bundle
                    .repo_lines
                    .iter()
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!("[AUTONOMOUS RESEARCH — REPOSITORY]:\n{formatted}"));
            }
            retrieval_evidence = Some(bundle.evidence.clone());

            // Check if autonomous deep research should be triggered
            if should_trigger_autonomous_research(&expanded_prompt, &bundle, params.force_research)
            {
                tracing::info!("Triggering autonomous research for additional context");
                let scope = params.research_scope.as_deref().unwrap_or("both");

                // Spawn autonomous research execution
                let queries = vec![expanded_prompt.clone()];
                let trigger_reason = format!(
                    "Chat context injection (forced: {:?}, scope: {})",
                    params.force_research, scope
                );

                match state
                    .orchestrator
                    .perform_autonomous_research(None, None, queries, &trigger_reason)
                    .await
                {
                    Ok(results) => {
                        if !results.is_empty() {
                            let formatted = results.join("\n");
                            context_parts.push(format!(
                                "[AUTONOMOUS RESEARCH — SYNTHESIS SUMMARY]:\n{formatted}"
                            ));
                            tracing::info!(
                                count = results.len(),
                                "Autonomous research results injected successfully"
                            );
                        }
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "Autonomous research execution failed");
                    }
                }
            }
            if !bundle.kb_lines.is_empty() {
                let formatted = bundle
                    .kb_lines
                    .iter()
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — KNOWLEDGE BASE]:\n{formatted}"
                ));
            }
        }
        Err(e) => {
            tracing::debug!(
                target: "vox_mcp::autonomous_research",
                error = %e,
                "autonomous retrieval injection failed — continuing without injected context"
            );
        }
    }

    let kb_mention_lines = if let Some(db) = state.db.clone() {
        use vox_orchestrator::knowledge_base::store::KbStore;
        let mentioned_names = crate::memory::parse_kb_mentions(&expanded_prompt);
        if mentioned_names.is_empty() {
            Vec::new()
        } else {
            let store = KbStore::new(db);
            let kbs = store.list().await.unwrap_or_default();
            let mut lines = Vec::new();
            for name in &mentioned_names {
                if let Some(kb) = kbs.iter().find(|k| k.name.to_ascii_lowercase() == *name) {
                    let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
                    for e in entries {
                        lines.push(format!("[KB:{}] {}", kb.name, e.content));
                    }
                }
            }
            lines
        }
    } else {
        Vec::new()
    };

    if !kb_mention_lines.is_empty() {
        let formatted = kb_mention_lines.join("\n");
        context_parts.push(format!("[MENTIONED KNOWLEDGE BASES]:\n{formatted}"));
    }

    let all_context_files: Vec<String> = {
        let mut v = params.context_files.clone();
        v.extend(mention_files);
        v.dedup();
        v
    };

    let user_prompt = if context_parts.is_empty() {
        expanded_prompt.clone()
    } else {
        format!("{}\n\n{}", context_parts.join("\n"), expanded_prompt)
    };

    // 3. Call LLM with cognitive-profile aware routing.
    // When cognitive_profile is set we use mcp_infer_completion() with an explicit
    // resolution template — the same pattern already used by inline_edit() and ghost_text().
    let thread_id_for_envelope = params.thread_id.clone();
    let journey_id = params
        .journey_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "journey_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            )
        });
    if implicit_session_default {
        tracing::debug!(
            target: "vox_mcp::session",
            tool = "vox_chat_message",
            "session_id omitted; using default chat session bucket"
        );
    }
    let ctx_handle = state.orchestrator.context_handle();
    let session_ts =
        match crate::sync_poison::poison_rw_read(ctx_handle.read(), "orchestrator context") {
            Ok(guard) => guard
                .age_secs(&format!("chat_history:{session_id}"))
                .map(|a: u64| format!(" Session last active: {a}s ago."))
                .unwrap_or_default(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    tool = "vox_chat_message",
                    "context lock poisoned; skipping session age hint"
                );
                String::new()
            }
        };
    // F3: resolve sticky model override to pass as model_key for profile injection.
    let sticky_model_key: Option<String> = effective_model_pref(
        params.model_override.as_deref(),
        match crate::sync_poison::poison_rw_read(
            state.mcp_chat_model_override.read(),
            "mcp_chat_model_override",
        ) {
            Ok(g) => g.clone(),
            Err(_) => None,
        }
        .as_deref(),
    );
    let system_prompt = format!(
        "{}{}\n\n{}",
        build_system_prompt_with_skill(
            state,
            None,
            params.skill.as_deref(),
            sticky_model_key.as_deref(),
            &params.skill_exclusions,
        )
        .await,
        session_ts,
        ANTI_LAZINESS_RIDER
    );
    let llm_started = std::time::Instant::now();

    let (response_text, model_used, tokens, selection_reason, events) = match params
        .cognitive_profile
        .as_deref()
    {
        Some(profile) => {
            let resolution_template = McpChatModelResolution {
                allow_cheapest_fallback: profile == "fast",
                complexity: match profile {
                    "reasoning" => 9,
                    "creative" => 7,
                    _ => 5,
                },
                ..Default::default()
            };
            let profile_complexity = resolution_template.complexity;
            let base_temperature = if profile == "creative" {
                0.8_f32
            } else {
                0.3_f32
            };
            match resolve_chat_llm_model(
                state,
                &user_prompt,
                resolution_template.clone(),
                Some(session_id.as_str()),
            )
            .await
            {
                Ok((model, free_only)) => {
                    let pref = effective_model_pref(
                        params.model_override.as_deref(),
                        match crate::sync_poison::poison_rw_read(
                            state.mcp_chat_model_override.read(),
                            "mcp_chat_model_override",
                        ) {
                            Ok(g) => g.clone(),
                            Err(e) => {
                                tracing::warn!(error = %e, "mcp_chat_model_override poisoned");
                                None
                            }
                        }
                        .as_deref(),
                    );
                    let max_tokens =
                        crate::llm_bridge::clamp_http_max_output_tokens(model.max_tokens);
                    let routing = McpInferRouting {
                        user_prompt: &user_prompt,
                        sticky_model_pref: pref.as_deref(),
                        resolution_template,
                        free_only,
                        allow_cloud_ollama_fallback: true,
                        selection_rationale: None,
                        user_id: Some(session_id.as_str()),
                    };
                    match crate::llm_bridge::mcp_infer_completion(
                        state,
                        model,
                        "vox_chat_message",
                        &system_prompt,
                        &routing,
                        max_tokens,
                        base_temperature,
                        params.temperature,
                        params.top_p,
                        params.json_mode,
                        params.attachment_manifest.clone(),
                    )
                    .await
                    {
                        // `mcp_infer_completion` doesn't surface a selection rationale
                        // (its `McpInferRouting.selection_rationale` above is `None`
                        // on the cognitive-profile path); record a truthful reason for
                        // *why* there's no rationale rather than leaving it a silent
                        // `None` — see Phase B Task B1.
                        Ok(r) => (
                            r.0,
                            r.1,
                            r.2,
                            Some(format!(
                                "cognitive profile '{profile}' \u{2192} complexity {profile_complexity}; rationale not surfaced"
                            )),
                            vec![],
                        ),
                        Err(e) => {
                            return ToolResult::<String>::err_with_remediation(
                                format!("LLM error: {e}"),
                                REM_LLM_COMPLETION,
                            )
                            .to_json();
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target: "vox_mcp::cognitive_routing",
                        profile,
                        error = %e,
                        "cognitive profile model resolution failed — using standard routing"
                    );
                    match call_llm(
                        state,
                        &system_prompt,
                        &user_prompt,
                        Some(session_id.as_str()),
                        params.temperature,
                        params.top_p,
                        params.attachment_manifest.clone(),
                    )
                    .await
                    {
                        Ok(r) => (
                            r.0,
                            r.1,
                            r.2,
                            Some(format!(
                                "Fallback: cognitive-profile resolution failed ({e})"
                            )),
                            vec![],
                        ),
                        Err(e2) => {
                            return ToolResult::<String>::err_with_remediation(
                                format!("LLM error: {e2}"),
                                REM_LLM_COMPLETION,
                            )
                            .to_json();
                        }
                    }
                }
            }
        }
        // Default (no cognitive_profile) chat path: Task 1.3d (F24 wiring). Attempt
        // the tool-calling agent loop (Task 1.3c) whenever the resolved model maps
        // to a simple provider shape and no multimodal attachment is present (the
        // mapper does not handle vision/attachment content — see
        // `super::agent_loop::model_spec_to_llm_config`). Otherwise fall back to
        // the existing `call_llm` -> `mcp_infer_completion` pipeline unchanged,
        // which still handles every other provider plus vision/budget/fallback.
        None => match try_run_agent_turn(
            state,
            &system_prompt,
            &user_prompt,
            session_id.as_str(),
            params.skill.clone(),
            params.attachment_manifest.is_some(),
            params.temperature,
            params.top_p,
            params.model_override.as_deref(),
            params.tier.as_deref(),
            params.clutch.as_deref(),
            params.risk.as_deref(),
        )
        .await
        {
            Some(Ok(r)) => (r.text, r.model_used, r.tokens, r.selection_reason, r.events),
            Some(Err(e)) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("LLM error: {e}"),
                    REM_LLM_COMPLETION,
                )
                .to_json();
            }
            // Bug 2 fix: `try_run_agent_turn` already resolved (or tried to
            // resolve) this turn's model using `params.model_override` /
            // `params.tier`. Falling through to plain `call_llm` here used to
            // silently discard both and re-resolve from only the
            // process-global override with `allow_cheapest_fallback: true`
            // unconditionally and no free-tier enforcement -- a different
            // model could be picked with no error surfaced, and a `tier:
            // "local"` request could silently escape free-tier enforcement.
            // `call_llm_with_pref` threads the same per-request pref/tier
            // through so this fallback resolves consistently with the path
            // that just failed, instead of picking an unrelated model.
            None => match call_llm_with_pref(
                state,
                &system_prompt,
                &user_prompt,
                Some(session_id.as_str()),
                params.temperature,
                params.top_p,
                params.attachment_manifest.clone(),
                params.model_override.as_deref(),
                params.tier.as_deref(),
            )
            .await
            {
                // `call_llm_with_pref` resolves via the rationale-carrying resolver
                // internally but doesn't return the rationale through its
                // `(String, String, u64)` return type — out of scope for this task
                // (see `try_run_agent_turn`'s doc comment). Record a truthful reason
                // for the missing detail rather than a silent `None`.
                Ok(r) => (
                    r.0,
                    r.1,
                    r.2,
                    Some("Fallback: attachment present, or provider shape unmapped".to_string()),
                    vec![],
                ),
                Err(e) => {
                    return ToolResult::<String>::err_with_remediation(
                        format!("LLM error: {e}"),
                        REM_LLM_COMPLETION,
                    )
                    .to_json();
                }
            },
        },
    };

    // KB signal adapter: fire-and-forget after response is assembled
    if let Some(db) = state.db.clone() {
        let content_for_kb = response_text.clone();
        let session_ref = session_id.clone();
        tokio::spawn(async move {
            crate::kb::signal_chat::ingest_chat_turn(
                db,
                &content_for_kb,
                Some(session_ref.as_str()),
            )
            .await;
        });
    }

    let chat_q_key =
        mcp_questioning_session_key(state, "vox_chat_message", Some(session_id.as_str()));
    state.record_questioning_attention_spend(&chat_q_key, llm_started.elapsed().as_millis() as u64);

    // Debit the pilot AttentionBudget for this completed chat turn. This mirrors
    // what the now-deleted `ChatTaskProcessor::process` (commit bd2ade59ae) used to
    // do via `Orchestrator::record_chat_attention` after every successful chat
    // turn — real chat turns moved to this synchronous `chat_message` path, but the
    // debit call was never carried over, so the GUI's `AttentionBudgetMeter`
    // (driven purely by `AttentionBudget.spent_ms`, see `record_chat_attention`'s
    // doc comment on `Orchestrator`) silently stopped moving for normal chat
    // activity. `state.record_questioning_attention_spend` above debits a
    // different, unrelated structure (`ServerState`'s in-memory
    // `questioning_attention_bounds` map) and does not substitute for this.
    // Token counts are estimated from the actual prompt/response text via
    // `CompactionEngine::estimate_tokens`, matching `ChatTaskProcessor`'s original
    // input/output token estimation (it also estimated from `task.description`/
    // `reply_text` rather than using provider-reported usage numbers).
    let est_input_tokens =
        vox_orchestrator::compaction::CompactionEngine::estimate_tokens(&user_prompt) as u32;
    let est_output_tokens =
        vox_orchestrator::compaction::CompactionEngine::estimate_tokens(&response_text) as u32;
    state
        .orchestrator
        .record_chat_attention(est_input_tokens, est_output_tokens);

    tracing::info!(
        target: "vox_mcp::populi_kpi",
        tool = "vox_chat_message",
        model_id = %model_used,
        tokens,
        elapsed_ms = llm_started.elapsed().as_millis() as u64,
        cognitive_profile = params.cognitive_profile.as_deref().unwrap_or("standard"),
        "mcp chat LLM round-trip"
    );

    // 4. Persist to session-scoped history.
    //
    // The history key is derived from `params.session_id` (defaulting to `"default"`).
    // Each distinct value yields an independent key, preventing context bleeding
    // across concurrent VS Code windows, agent threads, or other logical sessions.
    let history_key = format!("chat_history:{session_id}");
    let context_key = session_context_envelope_key(session_id.as_str());

    let user_msg = ChatTranscriptEntry {
        id: format!("usr-{}", now_ts()),
        role: "user".to_string(),
        content: params.prompt.clone(),
        timestamp: now_ts(),
        context_files: all_context_files,
        model_used: None,
        tokens: None,
    };
    let asst_msg = ChatTranscriptEntry {
        id: format!("asst-{}", now_ts() + 1),
        role: "assistant".to_string(),
        content: response_text.clone(),
        timestamp: now_ts() + 1,
        context_files: vec![],
        model_used: Some(model_used.clone()),
        tokens: Some(tokens),
    };

    let mut history =
        context_history_or_hydrate(state, history_key.as_str(), session_id.as_str()).await;
    history.push(user_msg.clone());
    history.push(asst_msg.clone());
    // Bound the *persisted/display* transcript independently of the token-aware
    // budget applied to what gets sent to the model (see `conversation.rs` /
    // Task 1.1): this cap exists only to bound storage/GUI-transcript size.
    trim_persisted_history(&mut history);

    match serde_json::to_string(&history) {
        Ok(history_json) => {
            let ctx_handle = state.orchestrator.context_handle();
            match crate::sync_poison::poison_rw_write(ctx_handle.write(), "orchestrator context") {
                Ok(ctx) => {
                    ctx.set(vox_orchestrator::AgentId(0), &history_key, &history_json, 0);
                    if let Some(ev) = &retrieval_evidence {
                        let context_envelope = ev.to_context_envelope(
                            state.repository.repository_id.as_str(),
                            Some(session_id.as_str()),
                        );
                        if let Ok(context_json) = serde_json::to_string(&context_envelope) {
                            ctx.set(
                                vox_orchestrator::AgentId(0),
                                &context_key,
                                &context_json,
                                3600,
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "chat_message: context poisoned persisting history");
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                session_id,
                "chat_message: failed to serialize chat history — \
                 history will not persist for this turn"
            );
        }
    }

    if let Some(db) = &state.db {
        let repo_id = &state.repository.repository_id;
        let q_session = session_id.clone();
        let q_repo = repo_id.to_string();

        let route_reason = serde_json::json!({
            "cognitive_profile": params.cognitive_profile,
        });
        let route_reason_s = route_reason.to_string();
        let _ = db
            .record_routing_decision(
                Some(journey_id.as_str()),
                q_repo.as_str(),
                Some(q_session.as_str()),
                "vox_chat_message",
                Some(model_used.as_str()),
                Some(route_reason_s.as_str()),
            )
            .await;

        // Insert user turn
        let user_ctx_files = serde_json::to_string(&user_msg.context_files).unwrap_or_default();
        let _ = db
            .insert_chat_transcript_turn(
                user_msg.id.as_str(),
                q_session.as_str(),
                user_msg.role.as_str(),
                user_msg.content.as_str(),
                user_msg.model_used.as_deref(),
                user_msg.tokens.map(|t| t as i64),
                user_ctx_files.as_str(),
                q_repo.as_str(),
            )
            .await;

        // Insert assistant turn into chat_transcripts (V17 legacy / VS Code history API)
        let asst_ctx_files = serde_json::to_string(&asst_msg.context_files).unwrap_or_default();
        let _ = db
            .insert_chat_transcript_turn(
                asst_msg.id.as_str(),
                q_session.as_str(),
                asst_msg.role.as_str(),
                asst_msg.content.as_str(),
                asst_msg.model_used.as_deref(),
                asst_msg.tokens.map(|t| t as i64),
                asst_ctx_files.as_str(),
                q_repo.as_str(),
            )
            .await;

        let journey_payload = journey_envelope::build_journey_envelope_v1(
            journey_id.as_str(),
            q_session.as_str(),
            thread_id_for_envelope.as_deref(),
            params.trace_id.as_deref(),
            params.correlation_id.as_deref(),
            q_repo.as_str(),
            "mcp",
            params.cognitive_profile.as_deref(),
        );
        let journey_json = journey_payload.to_string();
        if let Ok(conv_id) = db
            .chat_ensure_workspace_conversation(
                q_repo.as_str(),
                q_session.as_str(),
                thread_id_for_envelope.as_deref(),
                "mcp",
            )
            .await
        {
            let _ = db
                .chat_append_workspace_message(
                    conv_id,
                    user_msg.id.as_str(),
                    user_msg.role.as_str(),
                    user_msg.content.as_str(),
                    user_msg.model_used.as_deref(),
                    user_msg.tokens.map(|t| t as i64),
                    Some(user_ctx_files.as_str()),
                    Some(journey_json.as_str()),
                )
                .await;
            let _ = db
                .chat_append_workspace_message(
                    conv_id,
                    asst_msg.id.as_str(),
                    asst_msg.role.as_str(),
                    asst_msg.content.as_str(),
                    asst_msg.model_used.as_deref(),
                    asst_msg.tokens.map(|t| t as i64),
                    Some(asst_ctx_files.as_str()),
                    Some(journey_json.as_str()),
                )
                .await;
        }

        let now_s = now_ts();
        let date_str = ts_to_date_str(now_s);
        let server_idle_secs = now_s.saturating_sub(state.orchestrator.last_activity_ms() / 1000);
        let ctx_handle = state.orchestrator.context_handle();
        let session_age_secs = match crate::sync_poison::poison_rw_read(
            ctx_handle.read(),
            "orchestrator context",
        ) {
            Ok(g) => g
                .age_secs(&format!("chat_history:{session_id}"))
                .unwrap_or(0),
            Err(e) => {
                tracing::warn!(error = %e, "chat_message: context poisoned for session_age_secs");
                0
            }
        };

        // Record high-quality LLM turn in agent_events for Mens replay/SFT
        let mut payload = serde_json::json!({
            "type": "llm_turn",
            "agent_id": 0u64,
            "prompt": user_prompt,
            "response": response_text,
            "model": model_used,
            "tokens": tokens,
            "session_id": q_session,
            "repository_id": state.repository.repository_id,
            "temporal_context": {
                "date": date_str,
                "server_idle_secs": server_idle_secs,
                "session_age_secs": session_age_secs,
            }
        });
        if let Some(ev) = &retrieval_evidence {
            payload["retrieval"] = serde_json::to_value(ev).unwrap_or(Value::Null);
        }
        if vox_gamify::config_gate::is_enabled() {
            let _ = vox_gamify::event_router::route_event_auto_user(db, &payload).await;
        } else {
            let _ =
                vox_gamify::db::insert_event(db, "0", "llm_turn", Some(&payload.to_string())).await;
        }
    }

    // 5. Return updated history + the new assistant message

    let retrieval_contradiction = retrieval_evidence
        .as_ref()
        .map(|e| e.contradiction_count > 0)
        .unwrap_or(false);
    let retrieval_boost = retrieval_evidence
        .as_ref()
        .map(|e| match e.retrieval_tier.as_str() {
            "hybrid" => 0.08_f64,
            "bm25" => 0.04_f64,
            _ => 0.0_f64,
        })
        .unwrap_or(0.0_f64);
    let grounding =
        (chat_grounding_score(&params, mention_count) + retrieval_boost).clamp(0.0, 1.0);
    let pol = state.orchestrator_config.effective_socrates_policy();
    let session_key =
        mcp_questioning_session_key(state, "vox_chat_message", Some(session_id.as_str()));
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(
        &pol,
        grounding,
        retrieval_contradiction,
        turn,
        spent_att,
        max_att,
        retrieval_evidence.as_ref(),
    );
    let mut retrieval_meta = retrieval_evidence
        .as_ref()
        .and_then(|ev| serde_json::to_value(ev).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(meta_obj) = retrieval_meta.as_object_mut() {
        meta_obj.insert(
            "task_class".to_string(),
            serde_json::Value::String("chat_turn".to_string()),
        );
        meta_obj.insert(
            "domain_tags".to_string(),
            serde_json::json!(["interactive", "general_coding"]),
        );
    } else {
        retrieval_meta = socrates_surface_tags("chat_turn", &["interactive", "general_coding"]);
    }
    let llm_turn = state.db.as_ref().map(|_| {
        let provider_slug = model_used
            .split_once('/')
            .map(|(p, _)| p)
            .unwrap_or("openrouter")
            .to_string();
        let strength_tag = params
            .cognitive_profile
            .clone()
            .unwrap_or_else(|| "generalist".to_string());
        LlmSurfaceTelemetry {
            session_id: session_id.clone(),
            user_id: None,
            tenant_id: None,
            prompt: user_prompt.clone(),
            response: response_text.clone(),
            model_id: model_used.clone(),
            provider: provider_slug,
            task_category: "General".to_string(),
            strength_tag,
            latency_ms: llm_started.elapsed().as_millis() as i64,
            input_tokens: None,
            output_tokens: Some(tokens as i64),
            cache_read_tokens: None,
            trace_id: Some(journey_id.clone()),
            success: true,
            cost_usd: None,
            quality_score: Some(1.0),
        }
    });
    spawn_socrates_telemetry_with_llm(
        state,
        "vox_chat_message",
        soc.clone(),
        Some(model_used.clone()),
        Some(retrieval_meta),
        llm_turn,
    );
    spawn_questioning_trace_from_socrates(
        state,
        "vox_chat_message",
        soc.clone(),
        Some(session_key.clone()),
        Some(user_prompt.clone()),
    );
    // Honest selection classification (Phase B Task B1): derive `source` from the
    // RESOLVED model (`model_used`), never the requested one — see
    // `SelectionSource::classify`'s doc comment for why.
    let requested_model_for_selection = params
        .model_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let global_model_for_selection = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(_) => None,
    };
    let selection = SelectionDto::new(
        requested_model_for_selection,
        &model_used,
        global_model_for_selection.as_deref(),
        selection_reason.clone(),
    );

    let result = serde_json::json!({
        "message": asst_msg,
        "history": history,
        "model_used": model_used,
        "tokens": tokens,
        "latency_ms": llm_started.elapsed().as_millis() as u64,
        "selection_reason": selection_reason,
        "selection": selection,
        "session_id": session_id,
        "socrates": soc,
        "retrieval": retrieval_evidence,
        "events": events,
    });

    ToolResult::ok(result).to_json()
}

#[cfg(test)]
#[allow(unsafe_code)] // test-only std::env::set_var (unsafe on edition 2024); serialized via #[serial]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_db::store::types::ModelOutcome;
    use vox_db::{DbConfig, VoxDb};
    use vox_orchestrator::models::spec::PricingSource;
    use vox_orchestrator::models::{ModelCapabilities, ModelSpec, ProviderType};
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::chat_tools::params::ChatMessageParams;

    // Shared with `agent_loop::tests` — see that lock's doc comment: a private
    // per-module lock does not serialize against a sibling module's private
    // lock, so both modules that mutate `OPENROUTER_BASE_URL`/`OPENROUTER_API_KEY`
    // must hold the same crate-wide lock.
    use super::super::agent_loop::CHAT_MESSAGE_ENV_LOCK;

    fn test_state() -> ServerState {
        let cfg = OrchestratorConfig::for_testing();
        let orch_cfg = cfg.clone();
        let groups = AffinityGroupRegistry::new(vec![]);
        let session_cfg = SessionConfig {
            persist: false,
            sessions_dir: std::env::temp_dir().join("vox-mcp-message-test-sessions"),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg).expect("session manager");
        let repository = RepositoryContext {
            root: PathBuf::from("."),
            git_root: None,
            repository_id: "message-test".into(),
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

    fn model_spec(provider_type: ProviderType, id: &str) -> ModelSpec {
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
            pricing_source: PricingSource::Bootstrap,
            is_free: false,
            strengths: Vec::new(),
            capabilities: ModelCapabilities::default(),
            supported_parameters: Vec::new(),
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

    /// Task 4 spec review finding: `try_run_agent_turn` is the *actual* entry point
    /// for the DEFAULT `vox_chat_message` path (no `cognitive_profile` set) — it
    /// resolves a model and dispatches via `run_agent_turn` ->
    /// `vox_actor_runtime::llm::llm_chat` -> `vox_llm_egress::chat_once`, entirely
    /// bypassing `resolve_chat_llm_model` (whose own budget gate therefore never runs
    /// for this path) and the pre-existing `vox_orchestrator::BudgetGate` inside
    /// `mcp_infer_tool_completion` (which this path also never reaches). This test
    /// exercises that exact function — the real default-path entry point, not
    /// `resolve_chat_llm_model` — end to end, and confirms it is now budget-gated via
    /// the `enforce_budget_guard` call added at its top.
    #[tokio::test]
    #[serial]
    async fn default_chat_path_refuses_when_daily_budget_exceeded() {
        let prior = std::env::var("VOX_BUDGET_USD").ok();
        // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
        unsafe { std::env::set_var("VOX_BUDGET_USD", "0.01") };
        vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("open in-memory db");
        db.record_llm_outcome(ModelOutcome {
            session_id: "default-path-test",
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

        // No `cognitive_profile` involved at all — this calls the exact function
        // `chat_message`'s `None =>` arm (the default path) calls, with the same
        // shape of arguments (no attachment, real session id).
        let result = try_run_agent_turn(
            &state,
            "system prompt",
            "hello",
            "default-path-test",
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
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

        let err = result
            .expect("budget-exceeded must refuse via Some(Err(..)), not silently fall through to call_llm via None")
            .expect_err("expected budget guard to refuse dispatch");
        assert!(
            err.to_lowercase().contains("budget"),
            "expected error to mention budget, got: {err}"
        );
    }

    /// Fix Task 6: the turn's elapsed time is already computed (`llm_started.elapsed()`)
    /// but was never threaded into the envelope `data` object the GUI's `ModelBadge`
    /// reads. Mirrors `agent_loop::tests::chat_message_default_path_sends_tools_bearing_request`'s
    /// wiremock harness to drive a real, successful `chat_message` call and assert the
    /// returned envelope's `data.latency_ms` field is present.
    #[tokio::test]
    #[allow(unsafe_code)] // env var mutation under a process-wide lock, like agent_loop's test
    #[allow(clippy::await_holding_lock)]
    async fn chat_message_envelope_includes_latency_ms() {
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
        let model_id = "test-openrouter-model-latency";
        {
            let handle = state.orchestrator.models_handle();
            let mut registry = handle.write().expect("models registry lock");
            registry.register(model_spec(ProviderType::OpenRouter, model_id));
        }
        *state.mcp_chat_model_override.write() = Some(model_id.to_string());

        let params: ChatMessageParams =
            serde_json::from_value(serde_json::json!({ "prompt": "hello there" }))
                .expect("chat message params");

        let response_json = chat_message(&state, params).await;

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

        let parsed: serde_json::Value =
            serde_json::from_str(&response_json).expect("chat_message must return valid JSON");
        assert_eq!(
            parsed["success"], true,
            "chat_message should succeed via the mapped agent-loop path: {response_json}"
        );
        let data = &parsed["data"];
        assert!(
            data["model_used"].as_str().is_some(),
            "sanity check: envelope should carry a model_used string: {response_json}"
        );
        assert!(
            data.get("latency_ms").and_then(|v| v.as_u64()).is_some(),
            "chat_message envelope `data` must carry a `latency_ms` field for ModelBadge: {response_json}"
        );
    }

    /// Regression test for the "attention budget meter never increments during
    /// chat activity" bug: `ChatTaskProcessor` (the only prior caller of
    /// `Orchestrator::record_chat_attention`) was deleted in commit `bd2ade59ae`
    /// once real chat turns moved to this synchronous `chat_message` path, but
    /// nothing replaced the debit call here. Drives a real, successful
    /// `chat_message` call through the same wiremock harness as
    /// `chat_message_envelope_includes_latency_ms` and asserts the
    /// orchestrator's `BudgetManager` attention `spent_ms` actually increased —
    /// proving the GUI's `AttentionBudgetMeter` (driven purely by `spent_ms`)
    /// moves for normal chat activity again.
    #[tokio::test]
    #[allow(unsafe_code)] // env var mutation under a process-wide lock, like agent_loop's test
    #[allow(clippy::await_holding_lock)]
    async fn chat_message_debits_attention_budget_on_success() {
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
        let model_id = "test-openrouter-model-attention";
        {
            let handle = state.orchestrator.models_handle();
            let mut registry = handle.write().expect("models registry lock");
            registry.register(model_spec(ProviderType::OpenRouter, model_id));
        }
        *state.mcp_chat_model_override.write() = Some(model_id.to_string());

        let spent_before =
            vox_orchestrator::sync_lock::rw_read(&*state.orchestrator.budget_manager_handle())
                .attention_snapshot()
                .spent_ms;

        let params: ChatMessageParams =
            serde_json::from_value(serde_json::json!({ "prompt": "hello there" }))
                .expect("chat message params");

        let response_json = chat_message(&state, params).await;

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

        let parsed: serde_json::Value =
            serde_json::from_str(&response_json).expect("chat_message must return valid JSON");
        assert_eq!(
            parsed["success"], true,
            "chat_message should succeed via the mapped agent-loop path: {response_json}"
        );

        let spent_after =
            vox_orchestrator::sync_lock::rw_read(&*state.orchestrator.budget_manager_handle())
                .attention_snapshot()
                .spent_ms;
        assert!(
            spent_after > spent_before,
            "a successful chat turn must debit AttentionBudget.spent_ms via \
             Orchestrator::record_chat_attention so the GUI meter moves; \
             before={spent_before} after={spent_after}"
        );
    }

    // ── Task A2: per-request model override + composer tier ──────────────

    #[test]
    fn request_override_beats_process_global() {
        assert_eq!(
            effective_model_pref(Some("req"), Some("glob")),
            Some("req".into())
        );
    }

    #[test]
    fn falls_back_to_process_global() {
        assert_eq!(
            effective_model_pref(None, Some("glob")),
            Some("glob".into())
        );
    }

    #[test]
    fn blank_request_override_is_not_a_pick() {
        // A blank must not shadow the global, and must never reach the
        // resolver -- which treats "" as an explicit, unresolvable model id.
        assert_eq!(
            effective_model_pref(Some("  "), Some("glob")),
            Some("glob".into())
        );
        assert_eq!(effective_model_pref(Some(""), None), None);
    }

    #[test]
    fn tier_maps_to_resolution_not_cognitive_profile() {
        let r = resolution_for_tier(Some("local"), McpChatModelResolution::default());
        assert!(r.enforce_free_tier_only || r.allow_cheapest_fallback);
        let auto = resolution_for_tier(None, McpChatModelResolution::default());
        assert!(auto.allow_cheapest_fallback);
    }

    // ── Phase B / Task B1: honest selection classification ───────────────
    // Nested so `cargo test -p vox-orchestrator-mcp selection` (the plan's
    // documented command) matches these by module path.
    mod selection {
        use super::*;

        #[test]
        fn source_is_classified_from_the_resolved_model_not_the_request() {
            // The honesty bug: classifying from the request means a pinned model
            // that the resolver silently ignored still reads "Your pick".
            assert_eq!(
                SelectionSource::classify(Some("m"), Some("m"), None),
                SelectionSource::UserOverride
            );
            assert_eq!(
                SelectionSource::classify(Some("gone"), Some("other"), None),
                SelectionSource::Fallback
            );
            assert_eq!(
                SelectionSource::classify(None, Some("auto"), None),
                SelectionSource::AutoRouted
            );
            assert_eq!(
                SelectionSource::classify(None, Some("g"), Some("g")),
                SelectionSource::Global
            );
        }

        #[test]
        fn fallback_always_carries_a_rationale() {
            let d = SelectionDto::fallback("free-x", "requested `gone` is not in the registry");
            assert_eq!(d.source, SelectionSource::Fallback);
            assert!(
                d.rationale
                    .expect("rationale")
                    .contains("not in the registry")
            );
        }
    }

    /// Regression test for the bug the assertion above was too weak to catch:
    /// `r.enforce_free_tier_only || r.allow_cheapest_fallback` can never fail for
    /// `"local"` even when `allow_cheapest_fallback` silently regressed to
    /// `false`, because `enforce_free_tier_only` alone satisfies the OR. Pin
    /// both fields explicitly, and require the whole struct to differ from
    /// `McpChatModelResolution::default()` in at least one field -- the
    /// independent-review finding was that `"cloud"` produced a byte-identical
    /// no-op vs default.
    #[test]
    fn tier_local_sets_free_tier_and_keeps_cheapest_fallback() {
        let base = McpChatModelResolution::default();
        let r = resolution_for_tier(Some("local"), McpChatModelResolution::default());
        assert!(r.enforce_free_tier_only, "local must force free-tier-only");
        assert!(
            r.allow_cheapest_fallback,
            "local must not silently drop the pre-existing cheapest-fallback \
             degradation path just because a tier was named -- \
             enforce_free_tier_if_needed always re-swaps to a free model anyway, \
             so this can never leak a paid pick"
        );
        assert_ne!(
            r.enforce_free_tier_only, base.enforce_free_tier_only,
            "local's resolution must be observably different from default()"
        );
    }

    #[test]
    fn tier_mesh_sets_latency_critical_and_keeps_cheapest_fallback() {
        let base = McpChatModelResolution::default();
        let r = resolution_for_tier(Some("mesh"), McpChatModelResolution::default());
        assert!(r.free_tier_latency_critical);
        assert!(
            r.allow_cheapest_fallback,
            "mesh must not silently drop the cheapest-fallback degradation path"
        );
        assert_ne!(
            r.free_tier_latency_critical,
            base.free_tier_latency_critical
        );
    }

    /// Regression test for the specific "cloud is a no-op" finding:
    /// `McpChatModelResolution::default()` already has `allow_cheapest_fallback:
    /// false`, so the old `cloud` arm (which only ever set that field to
    /// `false`) produced a struct indistinguishable from doing nothing. The
    /// fixed arm must differ from both `default()` and from "auto" (`None`,
    /// which always allows cheapest-fallback).
    #[test]
    fn tier_cloud_is_not_a_no_op() {
        let base = McpChatModelResolution::default();
        let auto = resolution_for_tier(None, McpChatModelResolution::default());
        let cloud = resolution_for_tier(Some("cloud"), McpChatModelResolution::default());

        assert!(
            !cloud.allow_cheapest_fallback,
            "cloud must not degrade to the cheapest free/paid fallback"
        );
        assert_ne!(
            cloud.allow_cheapest_fallback, auto.allow_cheapest_fallback,
            "cloud must be observably different from auto/unset tier"
        );
        assert_ne!(
            cloud.complexity, base.complexity,
            "cloud must be observably different from McpChatModelResolution::default() \
             in at least one field -- the reviewed bug was a byte-identical no-op"
        );
    }

    /// An unrecognized/typo'd tier string (`tier` is free-text at the Rust type
    /// level) must be an explicit, documented alias for auto/default -- not an
    /// accidental match of a *different* recognized tier's behavior. This pins
    /// the documented decision in `resolution_for_tier`'s `Some(other) =>` arm.
    #[test]
    fn unknown_tier_is_documented_alias_for_auto_not_a_recognized_tier() {
        let auto = resolution_for_tier(None, McpChatModelResolution::default());
        let typo = resolution_for_tier(Some("Cloud"), McpChatModelResolution::default());

        assert_eq!(typo.allow_cheapest_fallback, auto.allow_cheapest_fallback);
        assert_eq!(typo.enforce_free_tier_only, auto.enforce_free_tier_only);
        assert_eq!(
            typo.free_tier_latency_critical,
            auto.free_tier_latency_critical
        );

        let local = resolution_for_tier(Some("local"), McpChatModelResolution::default());
        assert_ne!(
            typo.enforce_free_tier_only, local.enforce_free_tier_only,
            "a typo'd tier must never silently behave like a *different* \
             recognized tier"
        );
    }

    /// The trap: `cognitive_profile` takes `fast|reasoning|creative` and a
    /// non-`None` value switches the whole turn off the agent loop onto
    /// `mcp_infer_completion` (losing tool calls and `selection_reason`). A
    /// composer tier must therefore never land in it. Pinned at the only place
    /// a tier can enter the daemon: `ChatMessageParams` deserialization.
    #[test]
    fn tier_never_populates_cognitive_profile() {
        for tier in ["local", "mesh", "cloud", "auto"] {
            let params: ChatMessageParams = serde_json::from_value(serde_json::json!({
                "prompt": "hi",
                "tier": tier,
                "clutch": "genius",
                "risk": "low",
                "model_override": "some/model",
            }))
            .expect("composer payload must deserialize");
            assert_eq!(params.tier.as_deref(), Some(tier));
            assert_eq!(
                params.cognitive_profile, None,
                "tier {tier:?} must not set cognitive_profile -- doing so would                  route the turn to mcp_infer_completion and kill tool calls"
            );
        }
    }
}
