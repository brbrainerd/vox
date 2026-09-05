//! Model registry resolution and telemetry labels.

use vox_actor_runtime::model_resolution::{ChatRouteBackend, backend_telemetry_labels};
use vox_orchestrator::Orchestrator;
use vox_orchestrator::models::{
    CandidateScope, ModelSelectionRequest, ModelSpec, ProviderType, SelectionAxes, SelectionIntent,
    decide,
};
use vox_orchestrator::route_policy::route_policy_allows_model;
use vox_orchestrator::types::TaskCategory;

use super::super::MCP_GLOBAL_LLM_AGENT;
use super::policy::{apply_gemini_policy, enforce_free_tier_if_needed, mcp_local_model_allowed};
use super::types::McpChatModelResolution;
use crate::server_state::ServerState;

fn provider_allowed_by_route_policy(model: &ModelSpec) -> bool {
    route_policy_allows_model(model)
}

#[allow(clippy::too_many_arguments)]
fn build_selection_request(
    task: TaskCategory,
    complexity: u8,
    prefer_local: bool,
    cacheable_workload: bool,
    preference: vox_orchestrator::config::CostPreference,
    required_capabilities: Vec<vox_orchestrator::models::Capability>,
    clutch: Option<vox_orchestrator::mode::ClutchProfile>,
    risk: Option<vox_orchestrator::mode::RiskPosture>,
) -> ModelSelectionRequest {
    let mut intent = SelectionIntent::for_task(task);
    intent.complexity = complexity;
    intent.prefer_local = prefer_local;
    intent.cacheable_workload = cacheable_workload;
    intent.axes = if let Some(clutch) = clutch {
        // Task-driven axes: clutch/risk actually steer the scorer. `effective_axes`
        // returns the (cost, responsiveness, intelligence) triple AFTER risk's
        // Intelligence override — the same field order/semantics as `SelectionAxes`,
        // so this is a by-meaning identity map (cost→cost, responsiveness→
        // responsiveness, intelligence→intelligence).
        let (cost, responsiveness, intelligence) = vox_orchestrator::mode::effective_axes(
            clutch,
            risk.unwrap_or(vox_orchestrator::mode::RiskPosture::Moderate),
        );
        SelectionAxes {
            cost,
            responsiveness,
            intelligence,
        }
    } else if preference == vox_orchestrator::config::CostPreference::Economy {
        SelectionAxes::COST_FIRST
    } else {
        SelectionAxes::BALANCED
    };

    ModelSelectionRequest {
        intent,
        required_capabilities,
        candidate_scope: CandidateScope::AllProviders,
    }
}

#[inline]
fn secrets_truthy(id: vox_secrets::SecretId) -> bool {
    vox_secrets::resolve_secret(id)
        .expose()
        .map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn secrets_required_capabilities() -> Vec<vox_orchestrator::models::Capability> {
    use vox_orchestrator::models::Capability;
    use vox_secrets::SecretId;
    let mut v = Vec::new();
    if secrets_truthy(SecretId::VoxCapabilityRequireToolUse) {
        v.push(Capability::SupportsToolUse);
    }
    if secrets_truthy(SecretId::VoxCapabilityRequireReasoning) {
        v.push(Capability::SupportsReasoning);
    }
    if secrets_truthy(SecretId::VoxCapabilityRequireWebSearch) {
        v.push(Capability::SupportsWebSearch);
    }
    if secrets_truthy(SecretId::VoxCapabilityRequireImageGeneration) {
        v.push(Capability::SupportsImageGeneration);
    }
    v
}

fn secrets_capability_pin_model_id(
    required: &[vox_orchestrator::models::Capability],
    task: TaskCategory,
    prompt: &str,
) -> Option<String> {
    use vox_orchestrator::models::{Capability, PromptIntent};
    use vox_secrets::SecretId;
    let pick = |id: SecretId| {
        vox_secrets::resolve_secret(id)
            .expose()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
    };
    let intents: Vec<PromptIntent> = vox_orchestrator::models::infer_prompt_intents(prompt);
    if required.contains(&Capability::SupportsImageGeneration)
        || intents.contains(&PromptIntent::ImageGeneration)
    {
        if let Some(id) = pick(SecretId::VoxCapabilityImageGenerationModel) {
            return Some(id);
        }
    }
    if required.contains(&Capability::SupportsVision)
        || intents.contains(&PromptIntent::VisionUnderstanding)
    {
        if let Some(id) = pick(SecretId::VoxCapabilityVisionModel) {
            return Some(id);
        }
    }
    if matches!(
        task,
        TaskCategory::CodeGen
            | TaskCategory::Testing
            | TaskCategory::Parsing
            | TaskCategory::TypeChecking
    ) {
        if let Some(id) = pick(SecretId::VoxCapabilityCodeGenModel) {
            return Some(id);
        }
    }
    None
}

/// Task categories where the Vox-trained local model is preferred when available.
const VOX_LOCAL_PREFERRED_TASKS: &[TaskCategory] = &[
    TaskCategory::CodeGen,
    TaskCategory::Testing,
    TaskCategory::Parsing,
    TaskCategory::TypeChecking,
];

/// Token fill ratio for the global MCP LLM budget agent (`AgentId(0)`), if tracked.
#[must_use]
pub fn mcp_global_llm_context_fill_ratio(orch: &Orchestrator) -> Option<f32> {
    vox_orchestrator::sync_lock::rw_read(&*orch.budget_handle())
        .check_budget(MCP_GLOBAL_LLM_AGENT)
        .map(|b| b.tokens_used as f32 / b.effective_max_tokens().max(1) as f32)
}

/// Accumulated USD cost for MCP global LLM agent (in-process "session"), when budget exists.
#[must_use]
pub fn mcp_global_llm_session_spend_usd(orch: &Orchestrator) -> Option<f64> {
    vox_orchestrator::sync_lock::rw_read(&*orch.budget_handle())
        .check_budget(MCP_GLOBAL_LLM_AGENT)
        .map(|b| b.cost_usd)
}

/// Resolve a concrete [`ModelSpec`] synchronously from sticky override + orchestrator registry.
/// The model whose selection should be recorded, plus the human-readable
/// rationale when one was produced (free-tier router). `rationale` is `None`
/// for the ordinary scorer path.
pub struct McpModelChoice {
    pub model: ModelSpec,
    pub is_free: bool,
    pub rationale: Option<String>,
}

/// Public sync resolver (unchanged signature) — drops any rationale. Existing
/// callers (`resolve_chat_llm_model` + its consumers) are untouched.
pub fn resolve_mcp_chat_model_sync(
    orch: &Orchestrator,
    user_prompt: &str,
    pref: Option<&str>,
    res: McpChatModelResolution,
) -> Result<(ModelSpec, bool), String> {
    let mut _rationale = None;
    resolve_mcp_chat_model_sync_inner(orch, user_prompt, pref, res, &mut _rationale)
}

/// Sync resolver that also surfaces the selection rationale (for telemetry).
pub fn resolve_mcp_chat_model_sync_with_rationale(
    orch: &Orchestrator,
    user_prompt: &str,
    pref: Option<&str>,
    res: McpChatModelResolution,
) -> Result<McpModelChoice, String> {
    let mut rationale = None;
    let (model, is_free) =
        resolve_mcp_chat_model_sync_inner(orch, user_prompt, pref, res, &mut rationale)?;
    Ok(McpModelChoice {
        model,
        is_free,
        rationale,
    })
}

fn resolve_mcp_chat_model_sync_inner(
    orch: &Orchestrator,
    user_prompt: &str,
    pref: Option<&str>,
    res: McpChatModelResolution,
    rationale_out: &mut Option<String>,
) -> Result<(ModelSpec, bool), String> {
    if crate::llm_bridge::infer_test_stub::infer_stub_env_active() {
        return Ok((
            crate::llm_bridge::infer_test_stub::stub_plan_model_spec(),
            true,
        ));
    }

    let mut res = res;
    // Resolve task-type/source clutch+risk policy overrides, but ONLY when one
    // genuinely applies (explicit, category, or source policy resolves to
    // `Some`). Computing `resolve_task_policy` unconditionally and always
    // assigning its result would silently flip every unconfigured call from
    // today's `SelectionAxes::COST_FIRST` (Economy default, no clutch) to
    // `SelectionAxes::BALANCED` (clutch defaults to `ClutchProfile::Balanced`
    // when the compiled default tables are empty) — a real behavior change
    // with zero test failures, since `build_selection_request` only takes the
    // `effective_axes` branch when `res.clutch` is `Some`. See
    // `unconfigured_default_still_prefers_free_model_cost_first` in
    // `tests.rs`, which pins the pre-existing default and would catch a
    // regression here.
    {
        let overrides = {
            let config_handle = orch.config_handle();
            vox_orchestrator::sync_lock::rw_read(&*config_handle)
                .task_policy
                .clone()
        };
        // Category policy is intentionally NOT applied here: `res.task_category`
        // on this struct defaults to `TaskCategory::CodeGen` for a pre-existing,
        // unrelated reason (SelectionIntent/capability-pin heuristics), and most
        // real call sites (ordinary chat, ghost-text, inline-edit, plan/plan-loop,
        // db/compiler/oratio tool assists) never set their own category —
        // applying a category override here would silently misattribute all of
        // them as CodeGen. Source policy is safe to apply: every MCP call site
        // constructs this struct directly from a live interactive feature, so
        // `trigger_source` is accurate by construction (see its doc comment).
        // Category policy correctly applies to the `AgentTask`-based execution
        // path instead (`runtime.rs`/`socrates.rs`/`attention_fields.rs`), where
        // `task_category` is a real, deliberately-assigned field.
        let (source_clutch, source_risk) =
            vox_orchestrator::mode::effective_source_policy(&overrides, res.trigger_source);
        if res.clutch.is_some() || source_clutch.is_some() {
            let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
                res.clutch,
                res.risk,
                None,
                None,
                source_clutch,
                source_risk,
            );
            res.clutch = Some(clutch);
            res.risk = Some(risk);
        }
    }
    // Task clutch with `force_free_pool` (the `Free` detent) must never pick a paid
    // model. Reuse the existing free-tier enforcement seam rather than adding a new
    // filter path: `enforce_free_tier_only` gates every return through
    // `enforce_free_tier_if_needed`, which rejects paid models.
    if let Some(clutch) = res.clutch {
        if clutch.resolve().force_free_pool {
            res.enforce_free_tier_only = true;
        }
    }
    let routing_policy = vox_orchestrator::routing::RoutingPolicy::load();
    if let (Some(cap), Some(spent)) = (
        routing_policy.max_spend_usd_per_session,
        mcp_global_llm_session_spend_usd(orch),
    ) {
        if spent >= cap {
            res.enforce_free_tier_only = true;
        }
    }

    let models_handle = orch.models_handle();
    let registry = vox_orchestrator::sync_lock::rw_read(&*models_handle);
    let routing_allows = |m: &ModelSpec| {
        routing_policy.provider_filter_allows(m)
            && provider_allowed_by_route_policy(m)
            && crate::llm_bridge::local_health::local_candidate_allowed(m)
            // F7: VOX_INFERENCE_PRIVACY hard filter — unrelated to
            // VOX_MESH_EXEC_POLICY (mesh task placement); see
            // `local_health::privacy_allows` doc comment.
            && crate::llm_bridge::local_health::privacy_allows(m)
    };

    let mut required_capabilities: Vec<vox_orchestrator::models::Capability> = {
        let mut caps = Vec::new();
        for intent in vox_orchestrator::models::infer_prompt_intents(user_prompt) {
            for c in vox_orchestrator::models::intent_required_capabilities(intent) {
                if !caps.contains(c) {
                    caps.push(*c);
                }
            }
        }
        caps
    };
    for c in secrets_required_capabilities() {
        if !required_capabilities.contains(&c) {
            required_capabilities.push(c);
        }
    }
    let caps_ok = |m: &ModelSpec| {
        required_capabilities
            .iter()
            .all(|c| m.capabilities.supports(*c))
    };
    let preference = {
        let config_handle = orch.config_handle();
        vox_orchestrator::sync_lock::rw_read(&*config_handle).cost_preference
    };
    let task = res.task_category;
    let vox_local_route_preferred = VOX_LOCAL_PREFERRED_TASKS.contains(&task);

    if let Some(pin) = routing_policy.hard_pin_model_id.as_deref() {
        if let Some(m) = registry.get(pin) {
            if mcp_local_model_allowed(&m) && routing_allows(&m) && caps_ok(&m) {
                let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
                return Ok((m.clone(), m.is_free));
            }
        }
    }
    if let Some(pin) = secrets_capability_pin_model_id(&required_capabilities, task, user_prompt) {
        if let Some(m) = registry.get(&pin) {
            if mcp_local_model_allowed(&m) && routing_allows(&m) && caps_ok(&m) {
                let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
                return Ok((m.clone(), m.is_free));
            }
        }
    }

    if let Some(raw) = pref {
        let id = raw.trim();
        if !id.is_empty() {
            if let Some(m) = registry.get(id) {
                if !mcp_local_model_allowed(&m) {
                    return Err(
                        "Sticky MCP model uses Ollama but vox_populi::inference_PROFILE does not allow local Ollama HTTP; use desktop_ollama or lan_gateway, pick a cloud model, or clear the override (see docs/src/architecture/mobile-edge-ai-ssot.md).".into(),
                    );
                }
                if !caps_ok(&m) {
                    return Err(
                        "Sticky MCP model does not satisfy inferred capability requirements for this prompt."
                            .into(),
                    );
                }
                let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
                return Ok((m.clone(), m.is_free));
            }
            // Requested-but-unresolved: the pref names a model id that isn't in the
            // registry (removed, typo'd, or never existed). Previously this fell
            // through to auto-selection silently, so `ModelBadge` still read "Your
            // pick" over a model the user never got. Record the truthful reason
            // before falling through so the caller can classify this turn as a
            // fallback, not a user override.
            *rationale_out = Some(format!("Fallback: requested `{id}` is not in the registry"));
        }
    }

    if res.free_tier_latency_critical {
        // Route latency-critical free selection through the FreeTierRouter
        // (ModelTier::Fast bonus + vision/JSON/FIM hard-constraints) instead of
        // the registry's max_tokens sort. `accept` re-applies the local-Ollama /
        // routing-profile gating the router does not know about.
        let free = registry.free_models();
        if let Some((m, rationale)) = super::free_tier_adapter::route_free_tier_latency(
            &free,
            &res,
            &required_capabilities,
            |m| caps_ok(m) && mcp_local_model_allowed(m) && routing_allows(m),
        ) {
            tracing::info!(
                model_id = %m.id,
                provider = ?m.provider_type,
                rationale,
                route = "free-tier:latency-critical",
                "MCP free-tier model selected via FreeTierRouter"
            );
            *rationale_out = Some(rationale.to_string());
            let m = enforce_free_tier_if_needed(&registry, &res, m)?;
            return Ok((m.clone(), m.is_free));
        }
        if res.allow_cheapest_fallback {
            if let Some(m) = registry.cheapest_free_with_filter(|m| {
                caps_ok(m) && mcp_local_model_allowed(m) && routing_allows(m)
            }) {
                let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
                return Ok((m.clone(), m.is_free));
            }
        }
    }

    // Prefer the Vox-trained local model for code-oriented tasks when available and permitted.
    if vox_local_route_preferred && !res.enforce_free_tier_only {
        if let Some(m) = registry
            .list_models()
            .into_iter()
            .filter(|m| caps_ok(m))
            .filter(|m| matches!(m.provider_type, ProviderType::VoxLocal))
            .filter(mcp_local_model_allowed)
            .filter(|m| routing_allows(m))
            .max_by(|a, b| a.max_tokens.cmp(&b.max_tokens))
        {
            return Ok((m.clone(), m.is_free));
        }
    }

    let req = build_selection_request(
        task,
        res.complexity,
        vox_local_route_preferred,
        res.free_tier_fill_in_middle,
        preference,
        required_capabilities.clone(),
        res.clutch,
        res.risk,
    );

    if let Some(decision) = decide(&req, &registry) {
        let mut m = decision.outcome.model_spec.clone();
        if caps_ok(&m) && mcp_local_model_allowed(&m) && routing_allows(&m) {
            m = apply_gemini_policy(&registry, m, false);
            let m = enforce_free_tier_if_needed(&registry, &res, m)?;
            *rationale_out = Some(decision.outcome.reason.to_string());
            return Ok((m.clone(), m.is_free));
        }
    }

    if res.allow_cheapest_fallback {
        if let Some(m) = registry.cheapest_free_with_filter(|m| {
            caps_ok(m) && mcp_local_model_allowed(m) && routing_allows(m)
        }) {
            let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
            return Ok((m.clone(), m.is_free));
        }
        if let Some(m) = registry
            .cheapest_with_filter(|m| caps_ok(m) && mcp_local_model_allowed(m) && routing_allows(m))
        {
            let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
            return Ok((m.clone(), m.is_free));
        }
    }

    Err(
        "No LLM model available — set OPENROUTER_API_KEY or GEMINI_API_KEY, install Ollama when \
         vox_populi::inference_PROFILE allows local/LAN Ollama (desktop_ollama or lan_gateway), \
         or add models.toml under the Vox config directory."
            .into(),
    )
}

/// Async resolver that includes per-user provider availability when DB is attached.
pub async fn resolve_mcp_chat_model(
    state: &ServerState,
    user_prompt: &str,
    pref: Option<&str>,
    res: McpChatModelResolution,
    _user_id: Option<&str>,
) -> Result<(ModelSpec, bool), String> {
    resolve_mcp_chat_model_sync(&state.orchestrator, user_prompt, pref, res)
}

/// Async resolver that also surfaces the selection rationale (for telemetry).
pub async fn resolve_mcp_chat_model_with_rationale(
    state: &ServerState,
    user_prompt: &str,
    pref: Option<&str>,
    res: McpChatModelResolution,
    _user_id: Option<&str>,
) -> Result<McpModelChoice, String> {
    resolve_mcp_chat_model_sync_with_rationale(&state.orchestrator, user_prompt, pref, res)
}

/// Telemetry `(provider_family, route_choice)` — delegates to [`vox_actor_runtime::model_resolution::backend_telemetry_labels`]
/// so MCP and runtime chat lanes share one string SSOT.
#[must_use]
pub fn mcp_provider_telemetry_labels(provider: &ProviderType) -> (&'static str, &'static str) {
    backend_telemetry_labels(match *provider {
        ProviderType::GoogleDirect => ChatRouteBackend::GeminiDirect,
        ProviderType::OpenRouter => ChatRouteBackend::OpenRouter,
        ProviderType::Ollama => ChatRouteBackend::Ollama,
        ProviderType::PopuliMesh => ChatRouteBackend::PopuliMesh,
        ProviderType::VoxLocal => ChatRouteBackend::VoxLocal,
        ProviderType::Groq
        | ProviderType::Cerebras
        | ProviderType::Mistral
        | ProviderType::DeepSeek
        | ProviderType::SambaNova
        | ProviderType::Anthropic
        | ProviderType::HuggingFaceRouter
        | ProviderType::Custom(_) => ChatRouteBackend::CascadeFallback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::models::Capability;

    /// Fix Task 7: the common `decide()` branch of the sync resolver must populate
    /// `rationale_out` too, not just the free-tier-latency branch — otherwise
    /// ordinary chat turns (which take this branch) never get a `selection_reason`
    /// for `ModelBadge` to show.
    ///
    /// Uses a hermetic single-model registry (mirroring
    /// `vox_orchestrator::models::select`'s `key_gate_spec` test pattern) so the
    /// `decide()` branch's single pick is guaranteed to survive the MCP-specific
    /// `routing_allows`/`caps_ok`/`mcp_local_model_allowed` gates applied after it,
    /// rather than depending on the ambient bootstrap catalog + env state.
    #[test]
    fn resolve_with_rationale_populates_reason_on_decide_branch() {
        use vox_orchestrator::models::spec::PricingSource;
        use vox_orchestrator::models::{ModelRegistry, ModelSpec, ProviderType};

        let cfg = vox_orchestrator::OrchestratorConfig::for_testing();
        let groups = vox_orchestrator::AffinityGroupRegistry::new(vec![]);
        let orch = vox_orchestrator::Orchestrator::with_groups(cfg, groups);

        let spec = ModelSpec {
            id: "decide-branch-rationale-test".into(),
            canonical_slug: "decide-branch-rationale-test".into(),
            provider: "test".into(),
            provider_type: ProviderType::PopuliMesh,
            max_tokens: 32_000,
            // Not free: `best_for_internal` (the scorer's inner filter) skips free
            // models outright when `CostPreference::Performance` is in effect
            // (the default `build_selection_request` preference here), which would
            // otherwise make `decide()` fall through to `None` for this hermetic
            // single-model registry.
            cost_per_1k: 0.001,
            cost_per_1k_input: 0.001,
            cost_per_1k_output: 0.001,
            is_free: false,
            observed_cost_per_1k: None,
            // `Generalist` matches every task's strength requirement in
            // `ModelRegistry::matches_strength` — an empty vec matches none.
            strengths: vec![vox_orchestrator::models::StrengthTag::Generalist],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            // UserConfig ⇒ ModelConfidence::Confirmed ⇒ routing-eligible, and skips
            // the provider-key gate the way a locally-configured free route would.
            pricing_source: PricingSource::UserConfig,
            supported_parameters: vec![],
        };
        {
            let handle = orch.models_handle();
            let mut registry: std::sync::RwLockWriteGuard<'_, ModelRegistry> =
                vox_orchestrator::sync_lock::rw_write(&*handle);
            registry.register(spec);
        }

        let res = McpChatModelResolution {
            // Deliberately false: the cheapest-fallback branches (which don't set
            // `rationale_out`) must not be able to mask a decide()-branch failure —
            // if `decide()` doesn't resolve a model here, the test should error out
            // loudly instead of silently falling through to an unrelated branch.
            allow_cheapest_fallback: false,
            // Research is not in `VOX_LOCAL_PREFERRED_TASKS`, so this exercises the
            // general `decide()` branch rather than the vox-local-preferred shortcut.
            task_category: TaskCategory::Research,
            ..Default::default()
        };
        let mut rationale = None;
        let result =
            resolve_mcp_chat_model_sync_inner(&orch, "hello there", None, res, &mut rationale);
        let (model, _is_free) =
            result.expect("decide() branch should resolve the sole hermetic candidate model");
        assert_eq!(model.id, "decide-branch-rationale-test");
        assert!(
            rationale.is_some(),
            "decide() branch of the sync resolver must populate rationale_out"
        );
    }

    #[test]
    fn build_selection_request_maps_economy_to_cost_first() {
        let req = build_selection_request(
            TaskCategory::CodeGen,
            7,
            true,
            true,
            vox_orchestrator::config::CostPreference::Economy,
            vec![Capability::SupportsToolUse],
            None,
            None,
        );
        assert_eq!(req.intent.task, TaskCategory::CodeGen);
        assert_eq!(req.intent.complexity, 7);
        assert_eq!(req.intent.axes, SelectionAxes::COST_FIRST);
        assert!(req.intent.prefer_local);
        assert!(req.intent.cacheable_workload);
        assert_eq!(req.required_capabilities, vec![Capability::SupportsToolUse]);
    }

    #[test]
    fn clutch_genius_maps_to_quality_first_axes() {
        // Genius clutch (15/15/70) should produce QUALITY_FIRST-like axes and
        // override the binary Performance/Economy fallback.
        let req = build_selection_request(
            TaskCategory::CodeGen,
            7,
            false,
            false,
            vox_orchestrator::config::CostPreference::Economy, // would be COST_FIRST without clutch
            vec![],
            Some(vox_orchestrator::mode::ClutchProfile::Genius),
            Some(vox_orchestrator::mode::RiskPosture::Moderate),
        );
        assert_eq!(req.intent.axes, SelectionAxes::QUALITY_FIRST);
    }

    #[test]
    fn clutch_free_maps_to_cost_first_axes() {
        // Free clutch (70/15/15) should produce COST_FIRST-like axes even when the
        // binary preference would otherwise pick BALANCED.
        let req = build_selection_request(
            TaskCategory::CodeGen,
            3,
            false,
            false,
            vox_orchestrator::config::CostPreference::Performance, // would be BALANCED without clutch
            vec![],
            Some(vox_orchestrator::mode::ClutchProfile::Free),
            Some(vox_orchestrator::mode::RiskPosture::Moderate),
        );
        assert_eq!(req.intent.axes, SelectionAxes::COST_FIRST);
    }

    #[test]
    fn low_risk_overrides_cheap_clutch_toward_intelligence() {
        // Free clutch is cost-first, but Low risk's ModelLean::Intelligence must
        // override toward intelligence-weighted axes.
        let req = build_selection_request(
            TaskCategory::CodeGen,
            3,
            false,
            false,
            vox_orchestrator::config::CostPreference::Economy,
            vec![],
            Some(vox_orchestrator::mode::ClutchProfile::Free),
            Some(vox_orchestrator::mode::RiskPosture::Low),
        );
        assert_eq!(req.intent.axes, SelectionAxes::QUALITY_FIRST);
    }
}
