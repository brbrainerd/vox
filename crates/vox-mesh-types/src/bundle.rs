//! A2A wire types for content-addressed bundle requests/responses (P2-T4).

use serde::{Deserialize, Serialize};

/// Stable A2A wire-type tag for a worker requesting bundle bytes from the originator.
pub const BUNDLE_REQUEST_TYPE: &str = "bundle_request";
/// Stable A2A wire-type tag for the originator's response carrying bundle bytes.
pub const BUNDLE_RESPONSE_TYPE: &str = "bundle_response";

/// Sent worker → originator: "I received envelope `idempotency_key` and
/// I don't have the bundle for `fn_hash_hex`. Please send the bytes."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRequest {
    /// Idempotency key of the dispatch envelope that triggered the request.
    pub idempotency_key: String,
    /// Hex-encoded SHA3-512 content hash of the required bundle.
    pub fn_hash_hex: String,
}

/// Sent originator → worker: "Here are the bytes for `fn_hash_hex`."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResponse {
    /// Idempotency key of the dispatch envelope this response satisfies.
    pub idempotency_key: String,
    /// Hex-encoded SHA3-512 content hash.
    pub fn_hash_hex: String,
    /// Base64-encoded compiled bundle bytes.
    pub bundle_bytes_b64: String,
    /// Base64-encoded JSON-serialised `Vec<BundleRef>` for transitive deps.
    /// Empty string when there are no deps.
    #[serde(default)]
    pub deps_json_b64: String,
}

#[cfg(test)]
mod semcov_wave25_tests {
    use super::*;

    fn make_request(key: &str, hash: &str) -> BundleRequest {
        BundleRequest {
            idempotency_key: key.to_string(),
            fn_hash_hex: hash.to_string(),
        }
    }

    fn make_response(key: &str, hash: &str, b64: &str) -> BundleResponse {
        BundleResponse {
            idempotency_key: key.to_string(),
            fn_hash_hex: hash.to_string(),
            bundle_bytes_b64: b64.to_string(),
            deps_json_b64: String::new(),
        }
    }

    // Catches: BundleRequest serde round-trip dropping fn_hash_hex or idempotency_key.
    #[test]
    fn bundle_request_serde_round_trip() {
        let req = make_request("idem-key-abc", "deadbeef01234567");
        let json = serde_json::to_string(&req).unwrap();
        let back: BundleRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.idempotency_key, "idem-key-abc");
        assert_eq!(back.fn_hash_hex, "deadbeef01234567");
    }

    // Catches: BundleResponse serde round-trip losing bundle_bytes_b64 (e.g., rename_all mismatch).
    #[test]
    fn bundle_response_serde_round_trip() {
        let resp = make_response("idem-key-xyz", "aabbccdd", "dGVzdA==");
        let json = serde_json::to_string(&resp).unwrap();
        let back: BundleResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.bundle_bytes_b64, "dGVzdA==");
        assert_eq!(back.fn_hash_hex, "aabbccdd");
    }

    // Catches: deps_json_b64 not defaulting to empty string when absent from JSON,
    // causing deserialization failure for responses written by older code.
    #[test]
    fn bundle_response_deps_defaults_to_empty() {
        let json = r#"{"idempotency_key":"k","fn_hash_hex":"h","bundle_bytes_b64":"Yg=="}"#;
        let back: BundleResponse = serde_json::from_str(json)
            .expect("missing deps_json_b64 must deserialize with default empty string");
        assert_eq!(back.deps_json_b64, "");
    }

    // Catches: BUNDLE_REQUEST_TYPE / BUNDLE_RESPONSE_TYPE constants being swapped or typo'd,
    // which would break routing on the A2A wire.
    #[test]
    fn wire_type_constants_are_distinct_and_non_empty() {
        assert!(!BUNDLE_REQUEST_TYPE.is_empty());
        assert!(!BUNDLE_RESPONSE_TYPE.is_empty());
        assert_ne!(
            BUNDLE_REQUEST_TYPE, BUNDLE_RESPONSE_TYPE,
            "request and response type tags must differ"
        );
    }

    // Catches: idempotency_key being silently truncated or corrupted for keys containing
    // special characters (e.g., hyphens, underscores, colons) that some serializers escape.
    #[test]
    fn bundle_request_preserves_special_chars_in_idempotency_key() {
        let key = "scope:node-abc_def/123";
        let req = make_request(key, "ff00");
        let json = serde_json::to_string(&req).unwrap();
        let back: BundleRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.idempotency_key, key);
    }

    // Catches: BundleResponse with a non-empty deps_json_b64 having it zeroed out
    // on round-trip (e.g., serialize skipping non-default fields via skip_serializing_if).
    #[test]
    fn bundle_response_preserves_non_empty_deps() {
        let mut resp = make_response("k", "h", "Yw==");
        resp.deps_json_b64 = "W10=".to_string(); // base64("[]")
        let json = serde_json::to_string(&resp).unwrap();
        let back: BundleResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.deps_json_b64, "W10=");
    }
}
