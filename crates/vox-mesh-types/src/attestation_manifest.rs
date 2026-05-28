//! Public attestation manifest for grand-network opt-in (P6-T2).
//!
//! A `PublicAttestationManifest` is a signed JSON document that a node
//! publishes to a GitHub Gist or `.well-known/vox-manifest.json` path.
//! Any peer can fetch and verify it without contacting a Vox-owned server.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// A task kind + capability declaration inside a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedTask {
    /// Task kind string (e.g. `"text_infer"`, `"image_gen"`).
    pub kind: String,
    /// Whether this node currently accepts this task kind.
    pub supported: bool,
    /// Optional minimum VRAM in MB required.
    pub min_vram_mb: Option<u32>,
    /// Optional maximum concurrent executions.
    pub max_concurrent: Option<u8>,
}

/// Signed public attestation manifest published by a node to announce
/// its identity, capabilities, and trust relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicAttestationManifest {
    /// Manifest schema version (currently `"1"`).
    pub version: String,
    /// Stable node identifier (typically hex of the pubkey hash).
    pub node_id: String,
    /// Hex-encoded Ed25519 verifying key of this node.
    pub pubkey_hex: String,
    /// ISO-8601 timestamp when this manifest was last published.
    pub published_at: String,
    /// Task kinds this node supports.
    pub supported_tasks: Vec<SupportedTask>,
    /// Additional key-value metadata (optional; for future extensibility).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    /// Base64-encoded Ed25519 signature over the canonical manifest bytes.
    /// The canonical bytes are the JSON of this struct with `signature_b64` set to `""`.
    pub signature_b64: String,
}

impl PublicAttestationManifest {
    /// Return the canonical bytes to sign (all fields, `signature_b64` zeroed).
    pub fn canonical_signing_bytes(&self) -> serde_json::Result<Vec<u8>> {
        let mut v = serde_json::to_value(self)?;
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                "signature_b64".to_string(),
                serde_json::Value::String(String::new()),
            );
        }
        serde_json::to_vec(&sort_json_value(v))
    }
}

fn sort_json_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut sorted = serde_json::Map::with_capacity(map.len());
            for k in keys {
                sorted.insert(k.clone(), sort_json_value(map[&k].clone()));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_json_value).collect())
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

/// Errors that can occur when verifying a `PublicAttestationManifest`.
#[derive(Debug, thiserror::Error)]
pub enum ManifestVerifyError {
    #[error("pubkey_hex is not valid hex: {0}")]
    InvalidPubkeyHex(String),
    #[error("pubkey has wrong length (expected 32 bytes)")]
    PubkeyLengthMismatch,
    #[error("signature_b64 is not valid base64: {0}")]
    InvalidSignatureBase64(String),
    #[error("signature has wrong length (expected 64 bytes)")]
    SignatureLengthMismatch,
    #[error("manifest signature verification failed")]
    SignatureInvalid,
    #[error("serialisation error: {0}")]
    Serialise(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Attestation cache
// ---------------------------------------------------------------------------

/// In-memory cache of fetched and verified attestation manifests, keyed by
/// `node_id`. Callers should re-fetch after the `ttl_secs` window.
#[derive(Debug, Default)]
pub struct AttestationCache {
    inner: HashMap<String, CacheEntry>,
    /// Time-to-live in seconds before a cached manifest is considered stale.
    pub ttl_secs: u64,
}

#[derive(Debug)]
struct CacheEntry {
    pub manifest: PublicAttestationManifest,
    /// Unix epoch seconds when this entry was inserted.
    pub inserted_at_unix_s: u64,
}

impl AttestationCache {
    /// Create a new cache with the given TTL.
    pub fn with_ttl(ttl_secs: u64) -> Self {
        Self {
            inner: HashMap::new(),
            ttl_secs,
        }
    }

    /// Insert or replace a manifest for the given node_id.
    pub fn insert(&mut self, manifest: PublicAttestationManifest, now_unix_s: u64) {
        self.inner.insert(
            manifest.node_id.clone(),
            CacheEntry {
                manifest,
                inserted_at_unix_s: now_unix_s,
            },
        );
    }

    /// Retrieve a non-expired manifest by node_id.
    pub fn get(&self, node_id: &str, now_unix_s: u64) -> Option<&PublicAttestationManifest> {
        self.inner.get(node_id).and_then(|e| {
            let age = now_unix_s.saturating_sub(e.inserted_at_unix_s);
            if age <= self.ttl_secs {
                Some(&e.manifest)
            } else {
                None
            }
        })
    }

    /// Evict all entries older than `ttl_secs`.
    pub fn evict_stale(&mut self, now_unix_s: u64) {
        let ttl = self.ttl_secs;
        self.inner.retain(|_, e| {
            let age = now_unix_s.saturating_sub(e.inserted_at_unix_s);
            age <= ttl
        });
    }

    /// Return the number of live (non-expired) entries.
    pub fn len(&self, now_unix_s: u64) -> usize {
        self.inner
            .values()
            .filter(|e| {
                let age = now_unix_s.saturating_sub(e.inserted_at_unix_s);
                age <= self.ttl_secs
            })
            .count()
    }

    /// Returns `true` when there are no live entries.
    pub fn is_empty(&self, now_unix_s: u64) -> bool {
        self.len(now_unix_s) == 0
    }
}
