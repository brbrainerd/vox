use crate::nanopub::signing::SignedNanopub;

pub struct NanopubNetworkConfig {
    pub endpoint: String, // e.g., "https://np.knowledgepixels.com/"
}

pub struct PublishResult {
    pub success: bool,
    pub nanopub_uri: Option<String>,
    pub error: Option<String>,
}

/// Phase 8: replace with actual HTTP POST to the Nanopub Network.
/// For now returns a stub error result.
pub fn publish_stub(_signed: &SignedNanopub, _config: &NanopubNetworkConfig) -> PublishResult {
    PublishResult {
        success: false,
        nanopub_uri: None,
        error: Some("Phase 8 stub".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nanopub::signing::sign_nanopub;
    use crate::nanopub::trig::build_nanopub;
    use vox_crypto::facades::generate_signing_keypair;

    #[test]
    fn publish_stub_returns_error_result() {
        let (sk, _vk) = generate_signing_keypair();
        let doc = build_nanopub("stub claim", "provider:test", 0);
        let signed = sign_nanopub(doc, &sk);
        let config = NanopubNetworkConfig {
            endpoint: "https://np.knowledgepixels.com/".to_string(),
        };
        let result = publish_stub(&signed, &config);
        assert!(!result.success);
        assert!(result.nanopub_uri.is_none());
        assert_eq!(result.error.as_deref(), Some("Phase 8 stub"));
    }
}
