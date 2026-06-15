//! Opt-in **diagnostics / integration-test** hook: when both `VOX_MCP_TEST_INFER_STUB_BODY` (JSON
//! body) and `VOX_MCP_INFER_STUB_ACK` (`1` / `true`) are set, model resolution returns a dummy spec
//! and [`super::infer::mcp_infer_completion`] returns that body for tool `vox_plan` without HTTP.
//! Do **not** set in production MCP servers.

use vox_orchestrator::models::{ModelCapabilities, ModelSpec, ProviderType};

/// JSON body for a fake `vox_plan` completion (see module docs).
pub const INFER_STUB_BODY_ENV: &str = "VOX_MCP_TEST_INFER_STUB_BODY";
/// Must be `1` or `true` together with [`INFER_STUB_BODY_ENV`] to activate the stub.
pub const INFER_STUB_ACK_ENV: &str = "VOX_MCP_INFER_STUB_ACK";

#[must_use]
pub fn infer_stub_env_active() -> bool {
    let Ok(body) = std::env::var(INFER_STUB_BODY_ENV) else {
        return false;
    };
    if body.trim().is_empty() {
        return false;
    }
    std::env::var(INFER_STUB_ACK_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[must_use]
pub fn stub_completion_body() -> Option<String> {
    let s = std::env::var(INFER_STUB_BODY_ENV).ok()?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[must_use]
pub fn stub_plan_model_spec() -> ModelSpec {
    ModelSpec {
        id: "vox_mcp_infer_test_stub".into(),
        canonical_slug: "vox_mcp_infer_test_stub".into(),
        provider: "test".into(),
        provider_type: ProviderType::Ollama,
        max_tokens: 8192,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        is_free: true,
        strengths: Vec::new(),
        capabilities: ModelCapabilities::default(),
        supported_parameters: Vec::new(),
        observed_cost_per_1k: None,
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
    }
}
