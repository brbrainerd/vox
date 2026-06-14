//! Content-addressed **model** bundles (SafeTensors + tokenizer + config) for mesh CAS (Mn-T3).
//!
//! Workflow / activity code bundles live in [`crate::bundle::Bundle`]. Model bundles use the same
//! 64-byte SHA3-512 digest shape but carry weight/tokenizer/config hashes plus an aggregate
//! `bundle_hash` used as the CAS key.

use serde::{Deserialize, Serialize};
use sha3::Digest;

/// Raw SHA3-512 digest (64 bytes), hex-serialised for JSON.
pub type Sha3_512 = [u8; 64];

mod digest_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::Sha3_512;

    pub fn serialize<S: Serializer>(bytes: &Sha3_512, s: S) -> Result<S::Ok, S::Error> {
        let mut hex = String::with_capacity(128);
        for b in bytes {
            hex.push_str(&format!("{b:02x}"));
        }
        s.serialize_str(&hex)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Sha3_512, D::Error> {
        let text = String::deserialize(d)?;
        parse_hex_512(&text).map_err(serde::de::Error::custom)
    }

    pub(super) fn parse_hex_512(s: &str) -> Result<Sha3_512, String> {
        if s.len() != 128 {
            return Err(format!("expected 128 hex chars, got {}", s.len()));
        }
        let mut out = [0u8; 64];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let hex = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
            out[i] = u8::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
        }
        Ok(out)
    }
}

/// Serde for optional Merkle leaf lists — each leaf is a 128-char hex SHA3-512.
mod merkle_leaves_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::Sha3_512;

    pub fn serialize<S: Serializer>(v: &Option<Vec<Sha3_512>>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            None => s.serialize_none(),
            Some(leaves) => {
                let hex_vec: Vec<String> = leaves
                    .iter()
                    .map(|d| d.iter().map(|b| format!("{b:02x}")).collect())
                    .collect();
                s.serialize_some(&hex_vec)
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<Sha3_512>>, D::Error> {
        let opt: Option<Vec<String>> = Option::deserialize(d)?;
        match opt {
            None => Ok(None),
            Some(rows) => {
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    out.push(
                        super::digest_hex::parse_hex_512(&row).map_err(serde::de::Error::custom)?,
                    );
                }
                Ok(Some(out))
            }
        }
    }
}

/// On-disk SafeTensors layout for this bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WeightFormat {
    SafeTensorsSingle,
    SafeTensorsSharded {
        #[serde(with = "digest_hex")]
        index_hash: Sha3_512,
    },
}

/// Human / registry-facing provenance — not trusted for integrity (hashes are).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleProvenance {
    pub source_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hf_repo: Option<String>,
}

/// Immutable description of a SafeTensors model suitable for mesh CAS routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBundle {
    #[serde(with = "digest_hex")]
    pub weights_hash: Sha3_512,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "merkle_leaves_hex"
    )]
    pub weights_merkle_leaves: Option<Vec<Sha3_512>>,
    #[serde(with = "digest_hex")]
    pub tokenizer_hash: Sha3_512,
    #[serde(with = "digest_hex")]
    pub config_hash: Sha3_512,
    /// Aggregate CAS key — must equal [`compute_model_bundle_content_hash`].
    #[serde(with = "digest_hex")]
    pub bundle_hash: Sha3_512,
    pub format: WeightFormat,
    pub provenance: BundleProvenance,
}

/// Deterministic SHA3-512 over semantic bundle fields **excluding** `bundle_hash`.
#[must_use]
pub fn compute_model_bundle_content_hash(bundle: &ModelBundle) -> Sha3_512 {
    #[derive(Serialize)]
    struct Payload<'a> {
        #[serde(with = "digest_hex")]
        weights_hash: Sha3_512,
        #[serde(with = "merkle_leaves_hex")]
        weights_merkle_leaves: &'a Option<Vec<Sha3_512>>,
        #[serde(with = "digest_hex")]
        tokenizer_hash: Sha3_512,
        #[serde(with = "digest_hex")]
        config_hash: Sha3_512,
        format: &'a WeightFormat,
        provenance: &'a BundleProvenance,
    }
    let payload = Payload {
        weights_hash: bundle.weights_hash,
        weights_merkle_leaves: &bundle.weights_merkle_leaves,
        tokenizer_hash: bundle.tokenizer_hash,
        config_hash: bundle.config_hash,
        format: &bundle.format,
        provenance: &bundle.provenance,
    };
    let canonical = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
    sha3::Sha3_512::digest(&canonical).into()
}

impl ModelBundle {
    /// Returns `true` when `bundle_hash` matches the semantic digest.
    #[must_use]
    pub fn verify_bundle_hash(&self) -> bool {
        self.bundle_hash == compute_model_bundle_content_hash(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_digest() -> Sha3_512 {
        [0u8; 64]
    }

    #[test]
    fn bundle_hash_round_trip_stable() {
        let mut b = ModelBundle {
            weights_hash: zero_digest(),
            weights_merkle_leaves: None,
            tokenizer_hash: zero_digest(),
            config_hash: zero_digest(),
            bundle_hash: zero_digest(),
            format: WeightFormat::SafeTensorsSingle,
            provenance: BundleProvenance {
                source_label: "unit".into(),
                hf_repo: None,
            },
        };
        b.bundle_hash = compute_model_bundle_content_hash(&b);
        assert!(b.verify_bundle_hash());

        let json = serde_json::to_string(&b).unwrap();
        let back: ModelBundle = serde_json::from_str(&json).unwrap();
        assert!(back.verify_bundle_hash());
        assert_eq!(back.bundle_hash, b.bundle_hash);
    }
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use digest_hex::parse_hex_512;

    fn make_bundle(weights: [u8; 64]) -> ModelBundle {
        ModelBundle {
            weights_hash: weights,
            weights_merkle_leaves: None,
            tokenizer_hash: [0u8; 64],
            config_hash: [0u8; 64],
            bundle_hash: [0u8; 64],
            format: WeightFormat::SafeTensorsSingle,
            provenance: BundleProvenance {
                source_label: "test".into(),
                hf_repo: None,
            },
        }
    }

    // Catches: parse_hex_512 accepting strings shorter than 128 chars without error.
    #[test]
    fn parse_hex_512_rejects_short_string() {
        assert!(parse_hex_512("abcd").is_err());
    }

    // Catches: parse_hex_512 accepting a 129-char string (one extra) instead of erroring.
    #[test]
    fn parse_hex_512_rejects_130_char_string() {
        let s: String = "a".repeat(130);
        assert!(parse_hex_512(&s).is_err());
    }

    // Catches: parse_hex_512 accepting non-hex characters (e.g., 'g') without error.
    #[test]
    fn parse_hex_512_rejects_non_hex_chars() {
        let s: String = "g".repeat(128);
        assert!(parse_hex_512(&s).is_err());
    }

    // Catches: parse_hex_512 accepting an empty string (0 chars, not 128) without error.
    #[test]
    fn parse_hex_512_rejects_empty() {
        assert!(parse_hex_512("").is_err());
    }

    // Catches: compute_model_bundle_content_hash being sensitive to weights_hash changes
    // (i.e., not ignoring fields in the payload — a tampered weight must invalidate the hash).
    #[test]
    fn content_hash_changes_when_weights_hash_changes() {
        let b1 = make_bundle([0u8; 64]);
        let mut b2 = make_bundle([0u8; 64]);
        b2.weights_hash[0] = 0xff;

        let h1 = compute_model_bundle_content_hash(&b1);
        let h2 = compute_model_bundle_content_hash(&b2);
        assert_ne!(
            h1, h2,
            "different weights_hash must produce different content hash"
        );
    }

    // Catches: verify_bundle_hash returning true for a bundle whose bundle_hash is all-zeros
    // (the zero hash is unlikely to be the real SHA3-512 of the payload).
    #[test]
    fn zero_bundle_hash_fails_verify() {
        let b = make_bundle([0u8; 64]); // bundle_hash is also [0u8;64] — wrong
        assert!(
            !b.verify_bundle_hash(),
            "a zero bundle_hash should not verify (SHA3-512 of any real payload ≠ all-zeros)"
        );
    }

    // Catches: compute_model_bundle_content_hash not being deterministic (two calls differ).
    #[test]
    fn content_hash_is_deterministic() {
        let b = make_bundle([0xab; 64]);
        assert_eq!(
            compute_model_bundle_content_hash(&b),
            compute_model_bundle_content_hash(&b)
        );
    }

    // Catches: WeightFormat::SafeTensorsSharded serde losing the index_hash field.
    #[test]
    fn sharded_format_round_trips_index_hash() {
        let mut b = make_bundle([0u8; 64]);
        b.bundle_hash = compute_model_bundle_content_hash(&b);
        b.format = WeightFormat::SafeTensorsSharded {
            index_hash: [0xde; 64],
        };
        // Recompute with new format.
        b.bundle_hash = compute_model_bundle_content_hash(&b);
        let json = serde_json::to_string(&b).unwrap();
        let back: ModelBundle = serde_json::from_str(&json).unwrap();
        if let WeightFormat::SafeTensorsSharded { index_hash } = back.format {
            assert_eq!(index_hash, [0xde; 64]);
        } else {
            panic!("format variant lost on round-trip");
        }
    }
}
