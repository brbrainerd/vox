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
