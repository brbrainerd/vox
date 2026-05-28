//! GitHub attestation manifest: sign, verify, and Gist-fetch (P5-T2a, P5-T2c).

#[cfg(feature = "transport")]
use base64::Engine as _;
use serde::{Deserialize, Serialize};
#[cfg(feature = "transport")]
use vox_crypto::{SigningKey, VerifyingKey, sign, verify_signature_hex, verifying_key_to_bytes};

/// Stable canonical-input prefix for the attestation manifest signature.
pub const MANIFEST_DOMAIN: &[u8] = b"voxmesh.attestation.v1\0";

/// GitHub attestation manifest. Hosted in a Gist owned by `github_login`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttestationManifest {
    pub version: u8,
    pub node_pubkey_hex: String,
    pub github_user_id: String,
    pub github_login: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub signer_pubkey_hex: String,
    pub signature_b64: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestVerifyError {
    #[error("unsupported manifest version: {0}")]
    UnsupportedVersion(u8),
    #[error("signature does not verify")]
    SignatureMismatch,
    #[error("invalid signature base64")]
    InvalidSignatureB64,
    #[error("invalid pubkey hex")]
    InvalidPubkey,
    #[error("manifest expired (expires_at_unix_ms={expires_at_unix_ms}, now={now_unix_ms})")]
    Expired {
        expires_at_unix_ms: u64,
        now_unix_ms: u64,
    },
}

#[cfg(feature = "transport")]
fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(feature = "transport")]
fn canonical_manifest_input(
    node_pubkey_hex: &str,
    github_user_id: &str,
    github_login: &str,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(MANIFEST_DOMAIN);
    buf.extend_from_slice(node_pubkey_hex.as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(github_user_id.as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(github_login.as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(&issued_at_unix_ms.to_be_bytes());
    buf.extend_from_slice(&expires_at_unix_ms.to_be_bytes());
    buf
}

impl AttestationManifest {
    #[cfg(feature = "transport")]
    pub fn new_signed(
        node_pubkey_hex: &str,
        github_user_id: &str,
        github_login: &str,
        expires_at_unix_ms: u64,
        sk: &SigningKey,
        vk: &VerifyingKey,
    ) -> Self {
        let issued_at_unix_ms = now_unix_ms();
        let input = canonical_manifest_input(
            node_pubkey_hex,
            github_user_id,
            github_login,
            issued_at_unix_ms,
            expires_at_unix_ms,
        );
        let sig = sign(sk, &input);
        Self {
            version: 1,
            node_pubkey_hex: node_pubkey_hex.to_string(),
            github_user_id: github_user_id.to_string(),
            github_login: github_login.to_string(),
            issued_at_unix_ms,
            expires_at_unix_ms,
            signer_pubkey_hex: hex::encode(verifying_key_to_bytes(vk)),
            signature_b64: base64::engine::general_purpose::STANDARD.encode(sig),
        }
    }

    #[cfg(feature = "transport")]
    pub fn verify(&self) -> Result<(), ManifestVerifyError> {
        if self.version != 1 {
            return Err(ManifestVerifyError::UnsupportedVersion(self.version));
        }
        let now = now_unix_ms();
        if now > self.expires_at_unix_ms {
            return Err(ManifestVerifyError::Expired {
                expires_at_unix_ms: self.expires_at_unix_ms,
                now_unix_ms: now,
            });
        }
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature_b64)
            .map_err(|_| ManifestVerifyError::InvalidSignatureB64)?;
        if sig_bytes.len() != 64 {
            return Err(ManifestVerifyError::InvalidSignatureB64);
        }
        let input = canonical_manifest_input(
            &self.node_pubkey_hex,
            &self.github_user_id,
            &self.github_login,
            self.issued_at_unix_ms,
            self.expires_at_unix_ms,
        );
        let ok = verify_signature_hex(&self.signer_pubkey_hex, &input, &hex::encode(&sig_bytes))
            .map_err(|_| ManifestVerifyError::InvalidPubkey)?;
        if !ok {
            return Err(ManifestVerifyError::SignatureMismatch);
        }
        // The signer pubkey MUST equal the node pubkey — a node attests its own GitHub identity.
        if self.signer_pubkey_hex != self.node_pubkey_hex {
            return Err(ManifestVerifyError::SignatureMismatch);
        }
        Ok(())
    }
}

// ── Fetch + verify (P5-T2c) ───────────────────────────────────────────────────

#[cfg(feature = "transport")]
#[derive(Debug, thiserror::Error)]
pub enum FetchAndVerifyError {
    #[error("http: {0}")]
    Http(String),
    #[error("manifest verify: {0}")]
    Verify(#[from] ManifestVerifyError),
    #[error("invalid json: {0}")]
    Json(String),
}

#[cfg(feature = "transport")]
pub async fn fetch_and_verify(url: &str) -> Result<AttestationManifest, FetchAndVerifyError> {
    let body = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| FetchAndVerifyError::Http(e.to_string()))?
        .error_for_status()
        .map_err(|e| FetchAndVerifyError::Http(e.to_string()))?
        .text()
        .await
        .map_err(|e| FetchAndVerifyError::Http(e.to_string()))?;
    let manifest: AttestationManifest =
        serde_json::from_str(&body).map_err(|e| FetchAndVerifyError::Json(e.to_string()))?;
    manifest.verify()?;
    Ok(manifest)
}
