//! Utilities to check whether provider API keys are present.

use super::ProviderType;
use vox_secrets::SecretId;

/// Every inference provider Vox can pay for *right now*, by checking each
/// provider's Clavis key via `provider_secret_is_available`. Local providers
/// (no key needed) are always included. This is the credential-aware SSOT the
/// selector and the `vox_credentials_status` surface consult — OpenRouter is
/// one of many.
pub fn available_inference_providers() -> Vec<ProviderType> {
    CANDIDATE_PROVIDERS
        .iter()
        .filter(|p| provider_secret_is_available(p))
        .cloned()
        .collect()
}

/// Every provider the selector will consider, in display order.
pub const CANDIDATE_PROVIDERS: &[ProviderType] = &[
    ProviderType::GoogleDirect,
    ProviderType::OpenRouter,
    ProviderType::Groq,
    ProviderType::Mistral,
    ProviderType::DeepSeek,
    ProviderType::SambaNova,
    ProviderType::Cerebras,
    ProviderType::Anthropic,
    ProviderType::HuggingFaceRouter,
    ProviderType::Ollama,
    ProviderType::PopuliMesh,
    ProviderType::VoxLocal,
];

/// Per-provider credential presence for the full candidate list — the
/// GUI availability panel's SSOT (B9).
pub fn inference_provider_statuses() -> Vec<(ProviderType, bool)> {
    CANDIDATE_PROVIDERS
        .iter()
        .map(|p| (p.clone(), provider_secret_is_available(p)))
        .collect()
}

/// Checks if the primary required secret for a given provider type is currently available.
#[must_use]
pub fn provider_secret_is_available(ptype: &ProviderType) -> bool {
    let secret_id = match ptype {
        ProviderType::GoogleDirect => SecretId::GeminiApiKey,
        ProviderType::OpenRouter => SecretId::OpenRouterApiKey,
        ProviderType::Groq => SecretId::GroqApiKey,
        ProviderType::Cerebras => SecretId::CerebrasApiKey,
        ProviderType::Mistral => SecretId::MistralApiKey,
        ProviderType::DeepSeek => SecretId::DeepSeekApiKey,
        ProviderType::SambaNova => SecretId::SambaNovaApiKey,
        ProviderType::Anthropic => SecretId::AnthropicApiKey,
        ProviderType::HuggingFaceRouter => SecretId::HuggingFaceToken,
        ProviderType::Custom(_) => SecretId::CustomOpenaiApiKey,
        ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal => {
            // Local endpoints don't strictly require a resolved secret in the same way,
            // or use environment variables instead.
            return true;
        }
    };

    vox_secrets::resolve_secret(secret_id).expose().is_some()
}

#[cfg(test)]
mod avail_tests {
    use super::*;

    #[test]
    fn local_providers_always_available_and_listed() {
        let avail = available_inference_providers();
        // Local inference needs no Clavis key, so it must always be present.
        assert!(avail.contains(&ProviderType::Ollama));
        assert!(avail.contains(&ProviderType::PopuliMesh));
        assert!(avail.contains(&ProviderType::VoxLocal));
        // The function must be total (returns the providers it checked, not empty).
        assert!(avail.len() >= 3);
    }

    #[test]
    fn statuses_cover_every_candidate_and_mark_locals_present() {
        let statuses = inference_provider_statuses();
        assert_eq!(statuses.len(), CANDIDATE_PROVIDERS.len());
        for (p, present) in &statuses {
            if matches!(
                p,
                ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal
            ) {
                assert!(*present, "local provider {p:?} must always report present");
            }
        }
    }
}
