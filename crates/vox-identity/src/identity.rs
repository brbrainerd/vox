use std::fmt;

use vox_crypto::{SigningKey, VerifyingKey, generate_signing_keypair, secure_hash, sign};

pub struct NodeIdentity {
    node_id: String,
    signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl fmt::Debug for NodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeIdentity")
            .field("node_id", &self.node_id)
            .finish_non_exhaustive()
    }
}

impl NodeIdentity {
    pub fn generate() -> Self {
        let (signing_key, verifying_key) = generate_signing_keypair();
        let pubkey_bytes = vox_crypto::verifying_key_to_bytes(&verifying_key);
        let hash = secure_hash(&pubkey_bytes);
        let node_id = hex::encode(&hash[0..16]);

        Self {
            node_id,
            signing_key,
            verifying_key,
        }
    }

    pub fn from_keys(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        let pubkey_bytes = vox_crypto::verifying_key_to_bytes(&verifying_key);
        let hash = secure_hash(&pubkey_bytes);
        let node_id = hex::encode(&hash[0..16]);

        Self {
            node_id,
            signing_key,
            verifying_key,
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn sign_challenge(&self, nonce: &[u8]) -> [u8; 64] {
        sign(&self.signing_key, nonce)
    }

    /// Full hex-encoded Ed25519 public key (32 bytes → 64 hex chars).
    pub fn pubkey_hex(&self) -> String {
        hex::encode(vox_crypto::verifying_key_to_bytes(&self.verifying_key))
    }

    /// Short, human-readable fingerprint of the public key, e.g.
    /// `ed25519:7f:42:9b…2a:11` — head 3 + tail 2 bytes of the hex pubkey.
    pub fn fingerprint(&self) -> String {
        let h = self.pubkey_hex();
        let pairs: Vec<String> = h
            .as_bytes()
            .chunks(2)
            .map(|c| String::from_utf8_lossy(c).to_string())
            .collect();
        let head = pairs.iter().take(3).cloned().collect::<Vec<_>>().join(":");
        let tail = pairs
            .iter()
            .rev()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(":");
        format!("ed25519:{head}…{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pubkey_hex_is_64_chars() {
        let id = NodeIdentity::generate();
        assert_eq!(id.pubkey_hex().len(), 64, "ed25519 pubkey is 32 bytes hex");
    }

    #[test]
    fn fingerprint_shape() {
        let id = NodeIdentity::generate();
        let fp = id.fingerprint();
        assert!(fp.starts_with("ed25519:"), "fp prefixed: {fp}");
        assert!(fp.contains('…'), "fp elided: {fp}");
    }
}
