//! Model registry resolution and telemetry labels.

use vox_actor_runtime::model_resolution::{ChatRouteBackend, backend_telemetry_labels};
use vox_orchestrator::Orchestrator;
use vox_orchestrator::models::{ModelSpec, ProviderType};
use vox_orchestrator::route_policy::route_policy_allows_model;
use vox_orchestrator::types::TaskCategory;
use vox_orchestrator::usage::{RemainingBudget, UsageTracker};

use super::super::MCP_GLOBAL_LLM_AGENT;
use super::policy::{apply_gemini_policy, enforce_free_tier_if_needed, mcp_local_model_allowed};
use super::types::McpChatModelResolution;
use crate::server_state::ServerState;
use vox_orchestrator::routing::ModelSelectionEngine;

fn provider_allowed_by_route_policy(model: &ModelSpec) -> bool {
    route_policy_allows_model(model)
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
pub fn resolve_mcp_chat_model_sync(
    orch: &Orchestrator,
    user_prompt: &str,
    pref: Option<&str>,
    res: McpChatModelResolution,
    availability_hint: Option<&[RemainingBudget]>,
) -> Result<(ModelSpec, bool), String> {
    if crate::llm_bridge::infer_test_stub::infer_stub_env_active() {
        return Ok((
            crate::llm_bridge::infer_test_stub::stub_plan_model_spec(),
            true,
        ));
    }

    let mut res = res;
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
        routing_policy.provider_filter_allows(m) && provider_allowed_by_route_policy(m)
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
        }
    }

    if res.free_tier_latency_critical {
        if let Some(m) = registry.best_free_for_with_filter(task, |m| {
            caps_ok(m) && mcp_local_model_allowed(m) && routing_allows(m)
        }) {
            let m = enforce_free_tier_if_needed(&registry, &res, m.clone())?;
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

    let candidates: Vec<ModelSpec> = registry
        .list_models()
        .into_iter()
        .filter(caps_ok)
        .filter(mcp_local_model_allowed)
        .filter(|m| routing_allows(m))
        .filter(|m| vox_local_route_preferred || !matches!(m.provider_type, ProviderType::VoxLocal))
        .collect();
    let arm_stats = registry.arm_stats_snapshot().clone();
    let novel_trials = vox_orchestrator::sync_lock::rw_read(&*orch.budget_handle())
        .novel_routing_explores(MCP_GLOBAL_LLM_AGENT);
    let mut engine = ModelSelectionEngine::new(None);
    if let Some(m) = engine.pick_with_auto_score_thompson(
        &candidates,
        task,
        res.complexity,
        res.free_tier_latency_critical,
        res.context_fill_ratio,
        preference,
        availability_hint,
        &arm_stats,
        novel_trials,
    ) {
        let (s, f) = arm_stats.get(&m.id).copied().unwrap_or((0, 0));
        let max_ne = routing_policy.exploration.max_concurrent_explorations;
        if routing_policy.routing_objective.kind == "quality_first"
            && s + f == 0
            && novel_trials < max_ne
        {
            vox_orchestrator::sync_lock::rw_write(&*orch.budget_handle())
                .record_novel_routing_explore(MCP_GLOBAL_LLM_AGENT);
        }
        let m = apply_gemini_policy(&registry, m, false);
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
    user_id: Option<&str>,
) -> Result<(ModelSpec, bool), String> {
    let availability = if let Some(db) = state.db.as_ref() {
        let tracker = if let Some(uid) = user_id {
            UsageTracker::with_user(db.as_ref(), uid)
        } else {
            UsageTracker::new_ref(db.as_ref())
        };
        tracker.remaining_all().await.ok()
    } else {
        None
    };
    resolve_mcp_chat_model_sync(
        &state.orchestrator,
        user_prompt,
        pref,
        res,
        availability.as_deref(),
    )
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
