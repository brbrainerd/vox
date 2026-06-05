use super::super::model_select::ResolvedResearchModels;

/// Sanitize a string for ChatML formatting by replacing control tokens that could
/// trigger prompt injection (e.g., `<|im_start|>`, `<|im_end|>`).
pub(super) fn sanitize_chatml(input: &str) -> String {
    input
        .replace("<|im_start|>", "[im_start]")
        .replace("<|im_end|>", "[im_end]")
}

/// Sanitize evidence snippets from search results to prevent ChatML injection.
pub(super) fn sanitize_evidence(text: &str) -> String {
    sanitize_chatml(text)
}

/// When the verifier still uses the default NLI sentinel ([`super::super::model_select::FALLBACK_NLI_MODEL_ID`]),
/// align NLI with the registry-resolved claim model for consistent routing.
pub(super) fn verifier_config_for_research_run(
    base: &super::super::config::VerifierConfig,
    resolved: &ResolvedResearchModels,
) -> super::super::config::VerifierConfig {
    let mut v = base.clone();
    if v.nli_model_id == super::super::model_select::FALLBACK_NLI_MODEL_ID {
        v.nli_model_id = resolved.claim_model.clone();
    }
    v
}

/// FNV-1a 64-bit hash used to generate stable `claim_id` values from claim text.
///
/// No external dependency — uses the FNV-1a algorithm (public domain).
pub(super) fn fnv1a_hash(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
