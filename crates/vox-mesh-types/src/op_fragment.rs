//! Federation envelope and op-fragment types (P6-T1).
//!
//! `OpFragmentEnvelope` is the signed wire frame for federation messages. It
//! adopts the ActivityPub *shape* (context / id / type / actor / object /
//! signature) without the transport — federation is gist/git-based (see SSOT
//! §3 anti-goals).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Op-fragment envelope
// ---------------------------------------------------------------------------

/// Signed envelope wrapping a single federation operation fragment.
///
/// The `object` field carries the serialised payload; `signature` covers the
/// canonical JSON of all fields except `signature` itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpFragmentEnvelope {
    /// JSON-LD context (e.g. `"https://www.w3.org/ns/activitystreams"`).
    #[serde(rename = "@context")]
    pub context: String,
    /// Stable UUID (urn:uuid:…) or URL identifying this envelope instance.
    pub id: String,
    /// Envelope type discriminant (mirrors ActivityPub convention).
    #[serde(rename = "type")]
    pub kind: OpFragmentKind,
    /// DID or scope-URI of the originating node.
    pub actor: String,
    /// Serialised payload (may be JSON string or any JSON value).
    pub object: serde_json::Value,
    /// Ed25519 signature metadata.
    pub signature: FederationSignature,
}

impl OpFragmentEnvelope {
    /// Return the canonical bytes to sign: everything except `signature.signature_b64`.
    ///
    /// The canonical form is the JSON of a copy of this envelope with
    /// `signature.signature_b64` replaced by `""`, serialised with sorted keys via
    /// `serde_json`. This is deterministic across platforms and Rust versions.
    pub fn canonical_signing_bytes(&self) -> Vec<u8> {
        let mut v = serde_json::to_value(self).expect("OpFragmentEnvelope is always serialisable");
        if let Some(sig) = v.get_mut("signature")
            && let Some(obj) = sig.as_object_mut()
        {
            obj.insert(
                "signature_b64".to_string(),
                serde_json::Value::String(String::new()),
            );
        }
        // Sort keys for canonical form.
        let canonical = sort_json_keys(v);
        serde_json::to_vec(&canonical).expect("canonical JSON is always serialisable")
    }
}

/// Recursively sort JSON object keys for a canonical representation.
fn sort_json_keys(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut sorted: serde_json::Map<String, serde_json::Value> =
                serde_json::Map::with_capacity(map.len());
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), sort_json_keys(map[&k].clone()));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_json_keys).collect())
        }
        other => other,
    }
}

/// Discriminant for the op-fragment envelope type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OpFragmentKind {
    /// A mesh task has been dispatched.
    TaskDispatched,
    /// A mesh task result is being federated.
    TaskResult,
    /// A trust announcement (manifest digest, epoch).
    TrustAnnouncement,
    /// A kudos award being propagated.
    KudosAward,
    /// A hopper-sync operation (P6-T9).
    HopperSync,
    /// An extension point for future op kinds.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// Federation envelope (generic)
// ---------------------------------------------------------------------------

/// Generic signed federation envelope parameterised by the object type `O`.
///
/// Use this when the object type is known at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationEnvelope<O> {
    /// JSON-LD context.
    #[serde(rename = "@context")]
    pub context: String,
    /// Stable UUID / URL for this envelope instance.
    pub id: String,
    /// Envelope kind discriminant.
    #[serde(rename = "type")]
    pub kind: FederationEnvelopeKind,
    /// Originating node DID / scope-URI.
    pub actor: String,
    /// Typed payload.
    pub object: O,
    /// Ed25519 signature metadata.
    pub signature: FederationSignature,
}

/// Discriminant for a [`FederationEnvelope`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FederationEnvelopeKind {
    /// Carries an `AtlasObservation` (P6-T6).
    AtlasObservation,
    /// Carries a `TrustGraphSnapshot` (P6-T8).
    TrustGraphSnapshot,
    /// Carries a `PublicAttestationManifest` (P6-T2).
    AttestationManifest,
    /// Carries a `HopperOpSync` (P6-T9).
    HopperOpSync,
    /// Extension point.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// Signature block
// ---------------------------------------------------------------------------

/// Ed25519 signature block embedded in federation envelopes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationSignature {
    /// Signature type identifier.
    #[serde(rename = "type")]
    pub sig_type: String,
    /// ISO-8601 creation timestamp.
    pub created: String,
    /// DID or URL of the signing key.
    pub creator: String,
    /// Base64-encoded raw Ed25519 signature over the canonical signing bytes.
    pub signature_b64: String,
}

impl FederationSignature {
    /// Construct an unsigned placeholder (used for testing before signing).
    pub fn placeholder(creator: impl Into<String>) -> Self {
        Self {
            sig_type: "Ed25519Signature2020".to_string(),
            created: "1970-01-01T00:00:00Z".to_string(),
            creator: creator.into(),
            signature_b64: String::new(),
        }
    }
}
