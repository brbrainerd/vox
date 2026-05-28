use std::sync::Mutex;
use vox_orchestrator::Orchestrator;
use vox_orchestrator::config::{CostPreference, OrchestratorConfig};
use vox_orchestrator::models::{
    ModelRegistry, ModelRouteBackend, ModelSpec, ProviderType, route_backend_for_model,
};

use super::{
    McpChatModelResolution, mcp_global_llm_context_fill_ratio, mcp_provider_telemetry_labels,
    resolve_mcp_chat_model_sync,
};

static INFERENCE_PROFILE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn tiny_registry_with_free_and_paid() -> ModelRegistry {
    let mut r = ModelRegistry::default();
    r.register(ModelSpec {
        id: "free-model".into(),
        canonical_slug: "test/free-model".into(),
        provider: "test".into(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 1000,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: true,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    });
    r.register(ModelSpec {
        id: "paid-model".into(),
        canonical_slug: "test/paid-model".into(),
        provider: "test".into(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 1000,
        cost_per_1k: 0.01,
        cost_per_1k_input: 0.01,
        cost_per_1k_output: 0.01,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    });
    r
}

#[test]
fn mcp_global_llm_context_fill_ratio_none_without_budget() {
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Performance;
    let orch = Orchestrator::new(config);
    assert!(mcp_global_llm_context_fill_ratio(&orch).is_none());
}

#[test]
fn enforce_free_tier_only_swaps_paid_best_for() {
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Performance;
    let orch = Orchestrator::new(config);
    *vox_orchestrator::sync_lock::rw_write(&*orch.models_handle()) =
        tiny_registry_with_free_and_paid();

    let resolved = resolve_mcp_chat_model_sync(
        &orch,
        "",
        None,
        McpChatModelResolution {
            complexity: 8,
            allow_cheapest_fallback: true,
            enforce_free_tier_only: true,
            ..Default::default()
        },
        None,
    )
    .expect("resolve");
    assert!(resolved.0.is_free);
    assert_eq!(resolved.0.id, "free-model");
}

fn registry_ollama_only() -> ModelRegistry {
    let mut r = ModelRegistry::default();
    r.register(ModelSpec {
        id: "llama-local".into(),
        canonical_slug: "local/llama".into(),
        provider: "ollama".into(),
        provider_type: ProviderType::Ollama,
        max_tokens: 8192,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: true,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    });
    r
}

fn registry_paid_plus_ollama_free() -> ModelRegistry {
    let mut r = registry_ollama_only();
    r.register(ModelSpec {
        id: "paid-model".into(),
        canonical_slug: "test/paid-model".into(),
        provider: "test".into(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 1000,
        cost_per_1k: 0.01,
        cost_per_1k_input: 0.01,
        cost_per_1k_output: 0.01,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    });
    r
}

#[test]
fn sticky_ollama_rejected_when_inference_profile_disallows() {
    let _g = INFERENCE_PROFILE_TEST_LOCK.lock().expect("lock");
    // SAFETY: serialized with `INFERENCE_PROFILE_TEST_LOCK`; no concurrent env access in tests.
    unsafe { std::env::set_var("vox_populi::inference_PROFILE", "cloud_openai_compatible") };
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Performance;
    let orch = Orchestrator::new(config);
    *vox_orchestrator::sync_lock::rw_write(&*orch.models_handle()) = registry_ollama_only();

    let err = resolve_mcp_chat_model_sync(
        &orch,
        "",
        Some("llama-local"),
        McpChatModelResolution {
            complexity: 5,
            allow_cheapest_fallback: true,
            ..Default::default()
        },
        None,
    )
    .expect_err("sticky ollama must fail");
    assert!(
        err.contains("vox_populi::inference_PROFILE"),
        "expected profile hint: {err}"
    );
    unsafe {
        std::env::remove_var("vox_populi::inference_PROFILE");
    }
}

#[test]
fn mcp_openrouter_label_matches_runtime_route_telemetry() {
    use vox_actor_runtime::model_resolution::{ChatProviderRouteKind, route_telemetry_labels};
    let route = ChatProviderRouteKind::OpenRouter {
        model: "openai/gpt-4o".into(),
    };
    assert_eq!(
        route_telemetry_labels(&route),
        mcp_provider_telemetry_labels(&ProviderType::OpenRouter)
    );
}

#[test]
fn mcp_ollama_label_matches_runtime_populi_local_telemetry() {
    use vox_actor_runtime::model_resolution::{ChatProviderRouteKind, route_telemetry_labels};
    let route = ChatProviderRouteKind::PopuliLocal {
        base_url: "http://127.0.0.1:11434".into(),
        model: "llama3.2".into(),
    };
    assert_eq!(
        route_telemetry_labels(&route),
        mcp_provider_telemetry_labels(&ProviderType::Ollama)
    );
}

#[test]
fn mcp_google_direct_label_matches_runtime_manual_gemini_route_telemetry() {
    use vox_actor_runtime::model_resolution::{ChatProviderRouteKind, route_telemetry_labels};
    let route = ChatProviderRouteKind::ManualOpenAiCompatible {
        base_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
            .into(),
        model: "gemini-2.0-flash".into(),
        bearer: None,
    };
    assert_eq!(
        route_telemetry_labels(&route),
        mcp_provider_telemetry_labels(&ProviderType::GoogleDirect)
    );
}

#[test]
fn mcp_cascade_label_matches_runtime_hf_and_manual_byok_telemetry() {
    use vox_actor_runtime::model_resolution::{ChatProviderRouteKind, route_telemetry_labels};
    let manual = ChatProviderRouteKind::ManualOpenAiCompatible {
        base_url: "https://api.custom/v1/chat/completions".into(),
        model: "m".into(),
        bearer: None,
    };
    assert_eq!(
        route_telemetry_labels(&manual),
        mcp_provider_telemetry_labels(&ProviderType::Groq)
    );
    let ep = vox_actor_runtime::inference_env::resolve_huggingface_router("org/hf-model");
    let hf = ChatProviderRouteKind::HuggingFaceRouter(ep);
    assert_eq!(
        route_telemetry_labels(&hf),
        mcp_provider_telemetry_labels(&ProviderType::Mistral)
    );
}

#[test]
fn orchestrator_route_backend_matches_runtime_chat_backend_for_four_lanes() {
    use vox_actor_runtime::model_resolution::{
        ChatProviderRouteKind, ChatRouteBackend, route_backend_for_chat_route,
    };

    fn chat_lane_for_orchestrator_backend(b: ModelRouteBackend) -> ChatRouteBackend {
        match b {
            ModelRouteBackend::GeminiDirect => ChatRouteBackend::GeminiDirect,
            ModelRouteBackend::OpenRouter => ChatRouteBackend::OpenRouter,
            ModelRouteBackend::Ollama => ChatRouteBackend::Ollama,
            ModelRouteBackend::PopuliMesh => ChatRouteBackend::PopuliMesh,
            ModelRouteBackend::CascadeFallback => ChatRouteBackend::CascadeFallback,
            ModelRouteBackend::VoxLocal => ChatRouteBackend::VoxLocal,
        }
    }

    let gemini_spec = ModelSpec {
        id: "gemini-2.0-flash".into(),
        canonical_slug: "google/gemini-2.0-flash".into(),
        provider: "google".into(),
        provider_type: ProviderType::GoogleDirect,
        max_tokens: 1000,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    };
    let gemini_route = ChatProviderRouteKind::ManualOpenAiCompatible {
        base_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
            .into(),
        model: gemini_spec.id.clone(),
        bearer: None,
    };
    let gb = route_backend_for_model(&gemini_spec);
    assert_eq!(
        route_backend_for_chat_route(&gemini_route),
        chat_lane_for_orchestrator_backend(gb)
    );

    let or_spec = ModelSpec {
        id: "openai/gpt-4o".into(),
        canonical_slug: "openai/gpt-4o".into(),
        provider: "openrouter".into(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 1000,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    };
    let or_route = ChatProviderRouteKind::OpenRouter {
        model: or_spec.id.clone(),
    };
    assert_eq!(
        route_backend_for_chat_route(&or_route),
        chat_lane_for_orchestrator_backend(route_backend_for_model(&or_spec))
    );

    let ollama_spec = ModelSpec {
        id: "llama-local".into(),
        canonical_slug: "local/llama".into(),
        provider: "ollama".into(),
        provider_type: ProviderType::Ollama,
        max_tokens: 1000,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: true,
        observed_cost_per_1k: None,
        strengths: vec![],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    };
    let ollama_route = ChatProviderRouteKind::PopuliLocal {
        base_url: "http://127.0.0.1:11434".into(),
        model: ollama_spec.id.clone(),
    };
    assert_eq!(
        route_backend_for_chat_route(&ollama_route),
        chat_lane_for_orchestrator_backend(route_backend_for_model(&ollama_spec))
    );

    // Groq id without `/` → orchestrator CascadeFallback; manual BYOK chat route matches.
    let groq_spec = ModelSpec {
        id: "llama-3.1-70b".into(),
        canonical_slug: "groq/llama-3.1-70b".into(),
        provider: "groq".into(),
        provider_type: ProviderType::Groq,
        max_tokens: 1000,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    };
    let cascade_route = ChatProviderRouteKind::ManualOpenAiCompatible {
        base_url: "https://api.groq.com/openai/v1/chat/completions".into(),
        model: groq_spec.id.clone(),
        bearer: None,
    };
    assert_eq!(
        route_backend_for_chat_route(&cascade_route),
        ChatRouteBackend::CascadeFallback
    );
}

#[test]
fn enforce_free_tier_only_fails_when_only_ollama_free_under_cloud_profile() {
    let _g = INFERENCE_PROFILE_TEST_LOCK.lock().expect("lock");
    unsafe { std::env::set_var("vox_populi::inference_PROFILE", "cloud_openai_compatible") };
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Performance;
    let orch = Orchestrator::new(config);
    *vox_orchestrator::sync_lock::rw_write(&*orch.models_handle()) =
        registry_paid_plus_ollama_free();

    let err = resolve_mcp_chat_model_sync(
        &orch,
        "",
        Some("paid-model"),
        McpChatModelResolution {
            complexity: 8,
            allow_cheapest_fallback: true,
            enforce_free_tier_only: true,
            ..Default::default()
        },
        None,
    )
    .expect_err("no allowed free model");
    assert!(
        err.contains("vox_populi::inference_PROFILE") || err.contains("enforce_free_tier_only"),
        "expected profile or enforce hint: {err}"
    );
    unsafe {
        std::env::remove_var("vox_populi::inference_PROFILE");
    }
}

fn registry_with_vox_local_and_openrouter() -> ModelRegistry {
    let mut r = ModelRegistry::default();
    r.register(ModelSpec {
        id: "vox-ml-cli-v1".into(),
        canonical_slug: "local/vox-ml-cli-v1".into(),
        provider: "vox".into(),
        provider_type: ProviderType::VoxLocal,
        max_tokens: 8192,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: true,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    });
    r.register(ModelSpec {
        id: "cloud-model".into(),
        canonical_slug: "openrouter/cloud-model".into(),
        provider: "openrouter".into(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 8192,
        cost_per_1k: 0.001,
        cost_per_1k_input: 0.001,
        cost_per_1k_output: 0.001,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    });
    r
}

#[test]
fn vox_local_preferred_for_codegen_when_desktop_ollama_profile() {
    let _g = INFERENCE_PROFILE_TEST_LOCK.lock().expect("lock");
    unsafe { std::env::set_var("vox_populi::inference_PROFILE", "desktop_ollama") };
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Performance;
    let orch = Orchestrator::new(config);
    *vox_orchestrator::sync_lock::rw_write(&*orch.models_handle()) =
        registry_with_vox_local_and_openrouter();

    let (model, _is_free) = resolve_mcp_chat_model_sync(
        &orch,
        "generate a parser",
        None,
        McpChatModelResolution {
            complexity: 5,
            task_category: vox_orchestrator::types::TaskCategory::CodeGen,
            ..Default::default()
        },
        None,
    )
    .expect("should resolve");

    assert_eq!(
        model.provider_type,
        ProviderType::VoxLocal,
        "CodeGen should prefer VoxLocal; got model '{}' ({})",
        model.id,
        model.provider
    );
    unsafe {
        std::env::remove_var("vox_populi::inference_PROFILE");
    }
}

#[test]
fn vox_local_not_preferred_for_non_code_tasks() {
    let _g = INFERENCE_PROFILE_TEST_LOCK.lock().expect("lock");
    unsafe { std::env::set_var("vox_populi::inference_PROFILE", "desktop_ollama") };
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Performance;
    let orch = Orchestrator::new(config);
    *vox_orchestrator::sync_lock::rw_write(&*orch.models_handle()) =
        registry_with_vox_local_and_openrouter();

    let (model, _is_free) = resolve_mcp_chat_model_sync(
        &orch,
        "summarize this text",
        None,
        McpChatModelResolution {
            complexity: 5,
            task_category: vox_orchestrator::types::TaskCategory::Research,
            ..Default::default()
        },
        None,
    )
    .expect("should resolve");

    assert_ne!(
        model.provider_type,
        ProviderType::VoxLocal,
        "Research tasks should not prefer VoxLocal; got model '{}'",
        model.id,
    );
    unsafe {
        std::env::remove_var("vox_populi::inference_PROFILE");
    }
}

#[test]
fn restricted_route_overrides_allow_cloud_not_local_http_until_local_enabled() {
    let _g = INFERENCE_PROFILE_TEST_LOCK.lock().expect("lock");
    let cloud = ModelSpec {
        id: "or-route".into(),
        canonical_slug: "test/or-route".into(),
        provider: "test".into(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 1000,
        cost_per_1k: 0.01,
        cost_per_1k_input: 0.01,
        cost_per_1k_output: 0.01,
        is_free: false,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    };
    let local = ModelSpec {
        id: "ollama-route".into(),
        canonical_slug: "local/route".into(),
        provider: "ollama".into(),
        provider_type: ProviderType::Ollama,
        max_tokens: 8192,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: true,
        observed_cost_per_1k: None,
        strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
        capabilities: Default::default(),
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
        supported_parameters: vec![],
    };

    unsafe {
        std::env::set_var("VOX_ROUTE_POLICY_PROFILE", "restricted");
        std::env::set_var("VOX_ROUTE_ALLOW_NET", "1");
        std::env::set_var("VOX_ROUTE_ALLOW_PROVIDER_NETWORK", "1");
        std::env::remove_var("VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP");
    }
    assert!(
        vox_orchestrator::route_policy::route_policy_allows_model(&cloud),
        "OpenRouter should be allowed when net + provider_network overrides are set"
    );
    assert!(
        !vox_orchestrator::route_policy::route_policy_allows_model(&local),
        "local HTTP lanes should stay blocked until explicitly allowed"
    );

    unsafe {
        std::env::set_var("VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP", "1");
    }
    assert!(
        vox_orchestrator::route_policy::route_policy_allows_model(&local),
        "local HTTP should be permitted when VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP is truthy"
    );

    unsafe {
        std::env::remove_var("VOX_ROUTE_POLICY_PROFILE");
        std::env::remove_var("VOX_ROUTE_ALLOW_NET");
        std::env::remove_var("VOX_ROUTE_ALLOW_PROVIDER_NETWORK");
        std::env::remove_var("VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP");
    }
}
