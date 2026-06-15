use vox_orchestrator::models::{ModelSpec, ProviderType};

use super::error::HttpInferError;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const CEREBRAS_URL: &str = "https://api.cerebras.ai/v1/chat/completions";
const MISTRAL_URL: &str = "https://api.mistral.ai/v1/chat/completions";
const DEEPSEEK_URL: &str = "https://api.deepseek.com/chat/completions";
const SAMBANOVA_URL: &str = "https://api.sambanova.ai/v1/chat/completions";
const ANTHROPIC_PROXY_URL: &str = "http://127.0.0.1:4000/v1/chat/completions";
const ANTHROPIC_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";

fn env_or_default(id: vox_secrets::SecretId, default_value: &str) -> String {
    vox_secrets::resolve_secret(id)
        .expose()
        .filter(|v| !v.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_value.to_string())
}

pub(crate) fn endpoint_for(model: &ModelSpec) -> Result<String, HttpInferError> {
    match &model.provider_type {
        ProviderType::OpenRouter => Ok(vox_config::openrouter_chat_completions_url()),
        ProviderType::Groq => Ok(env_or_default(
            vox_secrets::SecretId::VoxGroqChatCompletionsUrl,
            GROQ_URL,
        )),
        ProviderType::Cerebras => Ok(env_or_default(
            vox_secrets::SecretId::VoxCerebrasChatCompletionsUrl,
            CEREBRAS_URL,
        )),
        ProviderType::Mistral => Ok(env_or_default(
            vox_secrets::SecretId::VoxMistralChatCompletionsUrl,
            MISTRAL_URL,
        )),
        ProviderType::DeepSeek => Ok(env_or_default(
            vox_secrets::SecretId::VoxDeepseekChatCompletionsUrl,
            DEEPSEEK_URL,
        )),
        ProviderType::SambaNova => Ok(env_or_default(
            vox_secrets::SecretId::VoxSambanovaChatCompletionsUrl,
            SAMBANOVA_URL,
        )),
        ProviderType::Anthropic => Ok(env_or_default(
            vox_secrets::SecretId::VoxAnthropicChatCompletionsUrl,
            if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxAnthropicDirect)
                .expose()
                .unwrap_or("")
                == "1"
            {
                ANTHROPIC_MESSAGES_URL
            } else {
                ANTHROPIC_PROXY_URL
            },
        )),
        ProviderType::Custom(base) => {
            let trimmed = base.trim();
            if trimmed.is_empty() {
                return Err(HttpInferError {
                    status: 0,
                    message: format!("Custom provider base URL is empty for model '{}'", model.id),
                    is_capability_gap: false,
                });
            }
            let suffix = "/chat/completions";
            if trimmed.ends_with(suffix) {
                Ok(trimmed.to_string())
            } else if trimmed.ends_with('/') {
                Ok(format!("{}v1{}", trimmed, suffix))
            } else {
                Ok(format!("{trimmed}/v1{suffix}"))
            }
        }
        ProviderType::HuggingFaceRouter => {
            Ok(vox_config::inference::hf_router_chat_completions_url())
        }
        ProviderType::VoxLocal => {
            let url = std::env::var("VOX_LOCAL_ENDPOINT")
                .unwrap_or_else(|_| "http://127.0.0.1:7863".to_string());
            Ok(format!("{}/generate", url.trim_end_matches('/')))
        }
        ProviderType::GoogleDirect | ProviderType::Ollama | ProviderType::PopuliMesh => {
            Err(HttpInferError {
                status: 0,
                message: format!(
                    "endpoint_for is not applicable to provider {:?}",
                    model.provider_type
                ),
                is_capability_gap: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::models::{ModelCapabilities, ModelSpec, ProviderType};

    #[test]
    fn test_endpoint_for_populi_mesh_rejected() {
        let model = ModelSpec {
            id: "mesh-model".into(),
            canonical_slug: "mesh/model".into(),
            provider: "mesh".into(),
            provider_type: ProviderType::PopuliMesh,
            max_tokens: 8000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        };
        let err = endpoint_for(&model).expect_err("should reject mesh");
        assert!(
            err.message
                .contains("not applicable to provider PopuliMesh")
        );
    }
}
