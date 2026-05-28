//! Trusty URI computation for [`PreregistrationV1`].
//!
//! A Trusty URI embeds a base64url-encoded content hash in the URI so the identifier
//! is self-verifying. Per the nanopublication spec, the hash covers the canonical
//! serialization of the artifact.
//!
//! Algorithm:
//! 1. Serialize the prereg to canonical JSON (BTreeMap-ordered keys via serde_json).
//! 2. SHA-256 hash the UTF-8 bytes.
//! 3. Base64url-encode (no padding).
//! 4. Prepend "RA" (the Trusty URI type code for nanopublication artifacts).

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use vox_research_events::preregistration::PreregistrationV1;

/// Compute the Trusty URI for `prereg`.
///
/// The `id` field of the prereg is excluded from the hash (it is the output of this function).
/// All other fields are included in canonical key-sorted JSON order.
pub fn compute_trusty_uri(prereg: &PreregistrationV1) -> String {
    let canonical = canonical_json(prereg);
    let hash = Sha256::digest(canonical.as_bytes());
    let b64 = base64url_encode_no_pad(&hash);
    format!("RA{b64}")
}

/// Serialize `prereg` to canonical JSON with BTreeMap-ordered keys.
///
/// The `id` field is set to an empty string in the canonical form so that the
/// Trusty URI can be computed before the field is written back.
pub(crate) fn canonical_json(prereg: &PreregistrationV1) -> String {
    // Deserialize into a BTreeMap to guarantee key sort order, then re-serialize.
    // We temporarily zero out `id` so the hash does not depend on itself.
    let mut with_empty_id = prereg.clone();
    with_empty_id.id = String::new();

    let value: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&serde_json::to_string(&with_empty_id).expect("serialization failed"))
            .expect("round-trip failed");
    serde_json::to_string(&value).expect("canonical serialization failed")
}

/// Base64url encoding without padding characters.
fn base64url_encode_no_pad(bytes: &[u8]) -> String {
    // Manual base64url — avoids adding a `base64` dep not in workspace.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((combined >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((combined >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((combined >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(combined & 0x3F) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn sample_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "p95 latency rises by >10ms after model update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:abc123".to_string(),
                eval_set_swhid: "swh:1:dir:def456".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: "p95_latency_ms".to_string(),
                aggregation: "percentile_95".to_string(),
                units: "milliseconds".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule {
                max_n: 1000,
                alpha: None,
                threshold: Some(0.95),
            },
            decision_rule: DecisionRule {
                description: "if posterior P(direction) > 0.95, conclude hypothesis".to_string(),
            },
            cost_cap_usd: 50.0,
            signed_at: 1715299200,
            signing_key: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
                .to_string(),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn trusty_uri_starts_with_ra() {
        let prereg = sample_prereg();
        let uri = compute_trusty_uri(&prereg);
        assert!(
            uri.starts_with("RA"),
            "Trusty URI must start with 'RA', got: {uri}"
        );
    }

    #[test]
    fn trusty_uri_is_deterministic() {
        let prereg = sample_prereg();
        let uri1 = compute_trusty_uri(&prereg);
        let uri2 = compute_trusty_uri(&prereg);
        assert_eq!(uri1, uri2, "Trusty URI must be deterministic");
    }

    #[test]
    fn trusty_uri_changes_on_field_change() {
        let mut prereg = sample_prereg();
        let uri1 = compute_trusty_uri(&prereg);
        prereg.hypothesis = "p95 latency rises by >20ms after model update".to_string();
        let uri2 = compute_trusty_uri(&prereg);
        assert_ne!(uri1, uri2, "Trusty URI must change when content changes");
    }
}
