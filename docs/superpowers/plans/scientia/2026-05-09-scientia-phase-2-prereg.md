# SCIENTIA Phase 2 — `vox-prereg` Crate

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the `vox-prereg` L2 crate that signs and verifies `PreregistrationV1` records with Trusty URIs and Ed25519, enforces analysis-plan integrity via a deviation detector, provides symbolic verifiers and a Bayesian stopping rule for sequential testing, and exposes a campaign gate that the orchestrator uses to refuse unsigned campaigns.

**Architecture:** L2 crate (pure domain logic, no async, no DB). Depends on `vox-research-events` (for `PreregistrationV1` and its constituent types) and `vox-crypto` (for `sign`/`verify`/`SigningKey`/`VerifyingKey`); the orchestrator calls the `PreregGate` in its campaign dispatch path. All signing and verification logic is synchronous and allocation-light so it can run in the hot path without a Tokio runtime.

**Tech Stack:** `serde`, `serde_json`, `sha2`, `hex`, `thiserror`, `vox-research-events`, `vox-crypto`, `workspace-hack`.

**Strategic reference:** [SCIENTIA plan §5 (Pre-registration as code)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#5-pre-registration-as-code), [§3.4 (Ground-truth verifier — symbolic where possible)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#34-ground-truth-verifier--symbolic-where-possible-minicheck-where-not)

---

## File Structure

```
crates/vox-prereg/
  Cargo.toml
  src/
    lib.rs          — re-exports + crate doc
    trusty_uri.rs   — Trusty URI computation (SHA-256 over canonical JSON)
    signing.rs      — sign_prereg / verify_prereg + error types
    deviation.rs    — DeviationDetector + DeviationReport
    symbolic.rs     — NumericComparatorVerifier + BayesianStoppingRule
    gate.rs         — PreregGate (orchestrator integration stub)
```

---

### Task 1: Scaffold `vox-prereg`

**Files:**
- Create: `crates/vox-prereg/Cargo.toml`
- Create: `crates/vox-prereg/src/lib.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "vox-prereg"
description = "SCIENTIA pre-registration: Trusty URI signing, analysis-plan deviation detection, symbolic verifiers, Bayesian stopping rule, and campaign gate."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
thiserror = { workspace = true }
vox-research-events = { workspace = true }
vox-crypto = { workspace = true }
workspace-hack = { workspace = true }
```

- [ ] **Step 2: Write `src/lib.rs`**

```rust
//! `vox-prereg` — SCIENTIA Phase 2 pre-registration crate.
//!
//! # Responsibilities
//! - Compute Trusty URIs (content-hash-in-URI) for [`PreregistrationV1`] records.
//! - Sign and verify pre-registrations with Ed25519 via [`vox_crypto`].
//! - Detect analysis-plan deviations between a signed prereg and the actual run.
//! - Provide symbolic verifiers for numeric claims and Bayesian sequential stopping.
//! - Expose [`gate::PreregGate`] — the orchestrator calls this before launching any campaign.
//!
//! # Layer
//! L2 (pure domain logic). No async, no DB, no direct I/O.

pub mod deviation;
pub mod gate;
pub mod signing;
pub mod symbolic;
pub mod trusty_uri;

pub use deviation::{DeviationDetector, DeviationReport};
pub use gate::{GateResult, PreregGate};
pub use signing::{SignError, Signature, VerifyError, sign_prereg, verify_prereg};
pub use symbolic::{
    BayesianStoppingRule, NumericComparatorVerifier, StopDecision, SymbolicVerdict,
};
pub use trusty_uri::compute_trusty_uri;
```

- [ ] **Step 3: Verify scaffold compiles (will fail on missing modules — expected)**

```bash
cargo check -p vox-prereg 2>&1 | head -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/vox-prereg/
git commit -m "feat(scientia): scaffold vox-prereg L2 crate (Phase 2 Task 1)"
```

---

### Task 2: Trusty URI + `PreregistrationV1` signing

**Files:**
- Create: `crates/vox-prereg/src/trusty_uri.rs`
- Create: `crates/vox-prereg/src/signing.rs`

#### `trusty_uri.rs`

The Trusty URI encodes the SHA-256 hash of the canonical JSON serialization of the prereg directly in the URI string, making the identifier self-verifying.

- [ ] **Step 1: Write failing tests**

```rust
// Place inside trusty_uri.rs under #[cfg(test)] mod tests
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
            signing_key: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899".to_string(),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn trusty_uri_starts_with_ra() {
        let prereg = sample_prereg();
        let uri = compute_trusty_uri(&prereg);
        assert!(uri.starts_with("RA"), "Trusty URI must start with 'RA', got: {uri}");
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
```

- [ ] **Step 2: Run (expect failure)**

```bash
cargo test -p vox-prereg trusty_uri 2>&1 | head -10
```

- [ ] **Step 3: Implement `trusty_uri.rs`**

```rust
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
    let mut out = String::with_capacity((bytes.len() * 4 + 2) / 3);
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
            signing_key: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899".to_string(),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn trusty_uri_starts_with_ra() {
        let prereg = sample_prereg();
        let uri = compute_trusty_uri(&prereg);
        assert!(uri.starts_with("RA"), "Trusty URI must start with 'RA', got: {uri}");
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vox-prereg trusty_uri 2>&1
```

Expected: 3 tests pass.

#### `signing.rs`

- [ ] **Step 5: Write failing tests**

```rust
// Place inside signing.rs under #[cfg(test)] mod tests
#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::facades::generate_signing_keypair;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn draft_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "tool-call malformation rate increased after provider update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:deadbeef".to_string(),
                eval_set_swhid: "swh:1:dir:cafebabe".to_string(),
                inspect_task_id: Some("task-malform-01".to_string()),
            },
            metric: MetricSpec {
                name: "malformation_rate_pct".to_string(),
                aggregation: "mean".to_string(),
                units: "percent".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule {
                max_n: 500,
                alpha: None,
                threshold: Some(0.95),
            },
            decision_rule: DecisionRule {
                description: "if posterior P(increase) > 0.95, flag provider".to_string(),
            },
            cost_cap_usd: 20.0,
            signed_at: 0,
            signing_key: String::new(),
            supersedes: None,
            analysis_tree_commit: Some("abc1234".to_string()),
        }
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        assert!(!prereg.id.is_empty(), "id must be set after signing");
        assert!(!prereg.signing_key.is_empty(), "signing_key must be set after signing");
        assert!(prereg.signed_at > 0, "signed_at must be set after signing");
        verify_prereg(&prereg, &sig.0).expect("verification must succeed");
    }

    #[test]
    fn tamper_detection_fails_verify() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        // Tamper with the hypothesis after signing
        prereg.hypothesis = "TAMPERED hypothesis".to_string();
        let result = verify_prereg(&prereg, &sig.0);
        assert!(result.is_err(), "verification must fail after tampering");
    }

    #[test]
    fn wrong_signature_hex_fails_verify() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let bad_sig = "ff".repeat(64);
        let result = verify_prereg(&prereg, &bad_sig);
        assert!(result.is_err(), "verification must fail with wrong signature");
    }
}
```

- [ ] **Step 6: Run (expect failure)**

```bash
cargo test -p vox-prereg signing 2>&1 | head -10
```

- [ ] **Step 7: Implement `signing.rs`**

```rust
//! Ed25519 signing and verification for [`PreregistrationV1`].
//!
//! # Signing flow
//! 1. Set `prereg.signed_at` to current Unix timestamp.
//! 2. Set `prereg.signing_key` to the hex-encoded Ed25519 verifying key.
//! 3. Set `prereg.id` to the Trusty URI of the (now fully-populated) canonical JSON.
//! 4. Sign the canonical JSON bytes with [`vox_crypto::facades::sign`].
//! 5. Return the 64-byte signature as hex.
//!
//! # Verification flow
//! 1. Decode `signature_hex` → 64-byte array.
//! 2. Decode `prereg.signing_key` → [`vox_crypto::facades::VerifyingKey`].
//! 3. Compute canonical JSON of the prereg (same as during signing).
//! 4. Call [`vox_crypto::facades::verify`]; return error if it returns false.

use crate::trusty_uri::{canonical_json, compute_trusty_uri};
use thiserror::Error;
use vox_crypto::facades::{
    sign, to_verifying_key, verify, verifying_key_from_bytes, verifying_key_to_bytes, SigningKey,
};
use vox_research_events::preregistration::PreregistrationV1;

/// A hex-encoded Ed25519 signature (128 hex characters = 64 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub String);

/// Errors returned by [`sign_prereg`].
#[derive(Debug, Error)]
pub enum SignError {
    #[error("serialization failed: {0}")]
    Serialization(String),
}

/// Errors returned by [`verify_prereg`].
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("invalid signature hex: {0}")]
    BadSignatureHex(String),
    #[error("signature has wrong length: expected 64 bytes, got {0}")]
    BadSignatureLength(usize),
    #[error("invalid signing key hex in prereg: {0}")]
    BadKeyHex(String),
    #[error("invalid signing key bytes: {0}")]
    BadKeyBytes(String),
    #[error("signature verification failed")]
    InvalidSignature,
}

/// Sign `prereg` in place, returning the hex-encoded signature.
///
/// Sets `prereg.signed_at`, `prereg.signing_key`, and `prereg.id` as side-effects.
pub fn sign_prereg(
    prereg: &mut PreregistrationV1,
    signing_key: &SigningKey,
) -> Result<Signature, SignError> {
    // Capture current Unix timestamp
    prereg.signed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Embed the verifying key (hex-encoded) so verifiers don't need a separate key store
    let vk = to_verifying_key(signing_key);
    prereg.signing_key = hex::encode(verifying_key_to_bytes(&vk));

    // Compute Trusty URI now that all fields are set
    prereg.id = compute_trusty_uri(prereg);

    // Sign the canonical JSON bytes
    let canonical = canonical_json(prereg);
    let sig_bytes = sign(signing_key, canonical.as_bytes());
    Ok(Signature(hex::encode(sig_bytes)))
}

/// Verify that `signature_hex` is a valid Ed25519 signature over the canonical JSON of `prereg`.
pub fn verify_prereg(prereg: &PreregistrationV1, signature_hex: &str) -> Result<(), VerifyError> {
    // Decode signature
    let sig_bytes = hex::decode(signature_hex)
        .map_err(|e| VerifyError::BadSignatureHex(e.to_string()))?;
    if sig_bytes.len() != 64 {
        return Err(VerifyError::BadSignatureLength(sig_bytes.len()));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);

    // Decode verifying key from prereg.signing_key
    let pk_bytes = hex::decode(&prereg.signing_key)
        .map_err(|e| VerifyError::BadKeyHex(e.to_string()))?;
    if pk_bytes.len() != 32 {
        return Err(VerifyError::BadKeyHex(format!(
            "expected 32 bytes, got {}",
            pk_bytes.len()
        )));
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk_bytes);
    let vk = verifying_key_from_bytes(&pk_arr).map_err(|e| VerifyError::BadKeyBytes(e))?;

    // Verify over canonical JSON
    let canonical = canonical_json(prereg);
    if verify(&vk, canonical.as_bytes(), &sig_arr) {
        Ok(())
    } else {
        Err(VerifyError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::facades::generate_signing_keypair;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn draft_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "tool-call malformation rate increased after provider update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:deadbeef".to_string(),
                eval_set_swhid: "swh:1:dir:cafebabe".to_string(),
                inspect_task_id: Some("task-malform-01".to_string()),
            },
            metric: MetricSpec {
                name: "malformation_rate_pct".to_string(),
                aggregation: "mean".to_string(),
                units: "percent".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule {
                max_n: 500,
                alpha: None,
                threshold: Some(0.95),
            },
            decision_rule: DecisionRule {
                description: "if posterior P(increase) > 0.95, flag provider".to_string(),
            },
            cost_cap_usd: 20.0,
            signed_at: 0,
            signing_key: String::new(),
            supersedes: None,
            analysis_tree_commit: Some("abc1234".to_string()),
        }
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        assert!(!prereg.id.is_empty(), "id must be set after signing");
        assert!(!prereg.signing_key.is_empty(), "signing_key must be set after signing");
        assert!(prereg.signed_at > 0, "signed_at must be set after signing");
        verify_prereg(&prereg, &sig.0).expect("verification must succeed");
    }

    #[test]
    fn tamper_detection_fails_verify() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        prereg.hypothesis = "TAMPERED hypothesis".to_string();
        let result = verify_prereg(&prereg, &sig.0);
        assert!(result.is_err(), "verification must fail after tampering");
    }

    #[test]
    fn wrong_signature_hex_fails_verify() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let bad_sig = "ff".repeat(64);
        let result = verify_prereg(&prereg, &bad_sig);
        assert!(result.is_err(), "verification must fail with wrong signature");
    }
}
```

- [ ] **Step 8: Run tests**

```bash
cargo test -p vox-prereg signing 2>&1
```

Expected: 3 tests pass (`sign_and_verify_round_trip`, `tamper_detection_fails_verify`, `wrong_signature_hex_fails_verify`).

- [ ] **Step 9: Commit**

```bash
git add crates/vox-prereg/src/trusty_uri.rs crates/vox-prereg/src/signing.rs
git commit -m "feat(scientia): Trusty URI + Ed25519 sign/verify for PreregistrationV1 (Phase 2 Task 2)"
```

---

### Task 3: Analysis-plan-deviation detector

**Files:**
- Create: `crates/vox-prereg/src/deviation.rs`

The deviation detector compares the metric name and statistical test kind used in an actual run against the values declared in the signed prereg. Any mismatch is surfaced as a `DeviationReport` that the publication pipeline stamps onto the output artifact.

- [ ] **Step 1: Write failing tests**

```rust
// Place inside deviation.rs under #[cfg(test)] mod tests
#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn prereg_with(metric_name: &str, test_kind: StatisticalTest) -> PreregistrationV1 {
        PreregistrationV1 {
            id: "RA_test".to_string(),
            hypothesis: "test hypothesis".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:000".to_string(),
                eval_set_swhid: "swh:1:dir:000".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: metric_name.to_string(),
                aggregation: "mean".to_string(),
                units: "ms".to_string(),
            },
            statistical_test: TestSpec {
                kind: test_kind,
                prior: None,
                threshold: None,
                alpha: Some(0.05),
            },
            stopping_rule: StopRule { max_n: 100, alpha: Some(0.05), threshold: None },
            decision_rule: DecisionRule { description: "reject if p < alpha".to_string() },
            cost_cap_usd: 10.0,
            signed_at: 1715299200,
            signing_key: "aa".repeat(32),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn clean_run_no_deviations() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Frequentist);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p95_latency_ms", &StatisticalTest::Frequentist);
        assert!(report.is_clean, "identical metric and test should be clean");
        assert!(report.deviations.is_empty());
        assert!(report.metric_matches);
        assert!(report.test_matches);
    }

    #[test]
    fn metric_mismatch_detected() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Frequentist);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p99_latency_ms", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(!report.metric_matches);
        assert!(report.test_matches);
        assert!(report.deviations.iter().any(|d| d.contains("metric")));
    }

    #[test]
    fn test_kind_mismatch_detected() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Bayesian);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p95_latency_ms", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(report.metric_matches);
        assert!(!report.test_matches);
        assert!(report.deviations.iter().any(|d| d.contains("test")));
    }

    #[test]
    fn both_mismatches_reported() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Bayesian);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "refusal_rate_pct", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(!report.metric_matches);
        assert!(!report.test_matches);
        assert_eq!(report.deviations.len(), 2);
    }
}
```

- [ ] **Step 2: Run (expect failure)**

```bash
cargo test -p vox-prereg deviation 2>&1 | head -10
```

- [ ] **Step 3: Implement `deviation.rs`**

```rust
//! Analysis-plan-deviation detector.
//!
//! Compares the metric name and statistical test kind declared in a signed
//! [`PreregistrationV1`] against what was actually used in the campaign run.
//! Any mismatch is collected into a [`DeviationReport`], which the publication
//! pipeline stamps onto the output artifact as `analysis_plan_deviation: true`.
//!
//! Per SCIENTIA plan §5.3: "Pre-register the **analysis tree**, not just the
//! hypothesis. The system records prereg signature + analysis-code commit hash;
//! any deviation surfaces as `analysis_plan_deviation: true`."

use vox_research_events::preregistration::{PreregistrationV1, StatisticalTest};

/// Report of deviations between a signed prereg and an actual run.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviationReport {
    /// True if the actual metric name matches `prereg.metric.name`.
    pub metric_matches: bool,
    /// True if the actual test kind matches `prereg.statistical_test.kind`.
    pub test_matches: bool,
    /// True if both `metric_matches` and `test_matches` are true (no deviations).
    pub is_clean: bool,
    /// Human-readable descriptions of each deviation found.
    pub deviations: Vec<String>,
}

/// Detects analysis-plan deviations between a signed prereg and an actual campaign run.
#[derive(Debug, Default, Clone)]
pub struct DeviationDetector;

impl DeviationDetector {
    pub fn new() -> Self {
        Self
    }

    /// Check `actual_metric` and `actual_test` against the values declared in `prereg`.
    ///
    /// Returns a [`DeviationReport`] with `is_clean = true` iff both match exactly.
    pub fn check(
        &self,
        prereg: &PreregistrationV1,
        actual_metric: &str,
        actual_test: &StatisticalTest,
    ) -> DeviationReport {
        let mut deviations = Vec::new();

        let metric_matches = prereg.metric.name == actual_metric;
        if !metric_matches {
            deviations.push(format!(
                "metric deviation: prereg declared '{}', actual run used '{}'",
                prereg.metric.name, actual_metric
            ));
        }

        let test_matches = test_kind_eq(&prereg.statistical_test.kind, actual_test);
        if !test_matches {
            deviations.push(format!(
                "test kind deviation: prereg declared '{:?}', actual run used '{:?}'",
                prereg.statistical_test.kind, actual_test
            ));
        }

        let is_clean = metric_matches && test_matches;
        DeviationReport { metric_matches, test_matches, is_clean, deviations }
    }
}

/// Compare two [`StatisticalTest`] variants for equality by discriminant.
fn test_kind_eq(a: &StatisticalTest, b: &StatisticalTest) -> bool {
    matches!(
        (a, b),
        (StatisticalTest::Frequentist, StatisticalTest::Frequentist)
            | (StatisticalTest::Bayesian, StatisticalTest::Bayesian)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn prereg_with(metric_name: &str, test_kind: StatisticalTest) -> PreregistrationV1 {
        PreregistrationV1 {
            id: "RA_test".to_string(),
            hypothesis: "test hypothesis".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:000".to_string(),
                eval_set_swhid: "swh:1:dir:000".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: metric_name.to_string(),
                aggregation: "mean".to_string(),
                units: "ms".to_string(),
            },
            statistical_test: TestSpec {
                kind: test_kind,
                prior: None,
                threshold: None,
                alpha: Some(0.05),
            },
            stopping_rule: StopRule { max_n: 100, alpha: Some(0.05), threshold: None },
            decision_rule: DecisionRule { description: "reject if p < alpha".to_string() },
            cost_cap_usd: 10.0,
            signed_at: 1715299200,
            signing_key: "aa".repeat(32),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn clean_run_no_deviations() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Frequentist);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p95_latency_ms", &StatisticalTest::Frequentist);
        assert!(report.is_clean, "identical metric and test should be clean");
        assert!(report.deviations.is_empty());
        assert!(report.metric_matches);
        assert!(report.test_matches);
    }

    #[test]
    fn metric_mismatch_detected() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Frequentist);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p99_latency_ms", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(!report.metric_matches);
        assert!(report.test_matches);
        assert!(report.deviations.iter().any(|d| d.contains("metric")));
    }

    #[test]
    fn test_kind_mismatch_detected() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Bayesian);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p95_latency_ms", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(report.metric_matches);
        assert!(!report.test_matches);
        assert!(report.deviations.iter().any(|d| d.contains("test")));
    }

    #[test]
    fn both_mismatches_reported() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Bayesian);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "refusal_rate_pct", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(!report.metric_matches);
        assert!(!report.test_matches);
        assert_eq!(report.deviations.len(), 2);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vox-prereg deviation 2>&1
```

Expected: 4 tests pass (`clean_run_no_deviations`, `metric_mismatch_detected`, `test_kind_mismatch_detected`, `both_mismatches_reported`).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-prereg/src/deviation.rs
git commit -m "feat(scientia): analysis-plan deviation detector (Phase 2 Task 3)"
```

---

### Task 4: Symbolic verifiers + Bayesian stopping rule

**Files:**
- Create: `crates/vox-prereg/src/symbolic.rs`

Symbolic verifiers check numeric directional claims (e.g., "p95 latency increased by 15ms") without calling any LLM, by comparing signs and magnitudes directly against measured values. The Bayesian stopping rule implements sequential testing: it tells the orchestrator when to stop collecting samples based on the posterior probability crossing a pre-declared threshold.

- [ ] **Step 1: Write failing tests**

```rust
// Place inside symbolic.rs under #[cfg(test)] mod tests
#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::StopRule;

    // --- NumericComparatorVerifier tests ---

    #[test]
    fn increased_claim_confirmed_when_measured_higher() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency increased by 15ms", 215.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Confirmed);
    }

    #[test]
    fn increased_claim_refuted_when_measured_lower() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency increased by 15ms", 185.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Refuted);
    }

    #[test]
    fn decreased_claim_confirmed_when_measured_lower() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("refusal rate decreased after update", 1.5, 3.0);
        assert_eq!(verdict, SymbolicVerdict::Confirmed);
    }

    #[test]
    fn decreased_claim_refuted_when_measured_higher() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("refusal rate decreased after update", 4.0, 3.0);
        assert_eq!(verdict, SymbolicVerdict::Refuted);
    }

    #[test]
    fn no_direction_keyword_is_inconclusive() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency changed significantly", 210.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Inconclusive);
    }

    #[test]
    fn equal_values_are_inconclusive_even_with_direction() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("latency rose after update", 200.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Inconclusive);
    }

    // --- BayesianStoppingRule tests ---

    fn stop_rule(threshold: f64) -> StopRule {
        StopRule { max_n: 1000, alpha: None, threshold: Some(threshold) }
    }

    #[test]
    fn high_posterior_stops_accept() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.97, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopAccept);
    }

    #[test]
    fn low_posterior_stops_reject() {
        let rule = BayesianStoppingRule::new();
        // posterior = 0.02 → below (1 - 0.95) = 0.05 → StopReject
        let decision = rule.should_stop(0.02, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopReject);
    }

    #[test]
    fn mid_posterior_continues() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.50, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::Continue);
    }

    #[test]
    fn boundary_at_exactly_threshold_stops_accept() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.95, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopAccept);
    }

    #[test]
    fn no_threshold_uses_default_095() {
        let rule = BayesianStoppingRule::new();
        let no_threshold_rule = StopRule { max_n: 500, alpha: None, threshold: None };
        // Default threshold = 0.95; posterior 0.96 should stop-accept
        assert_eq!(rule.should_stop(0.96, &no_threshold_rule), StopDecision::StopAccept);
        // posterior 0.50 should continue
        assert_eq!(rule.should_stop(0.50, &no_threshold_rule), StopDecision::Continue);
    }
}
```

- [ ] **Step 2: Run (expect failure)**

```bash
cargo test -p vox-prereg symbolic 2>&1 | head -10
```

- [ ] **Step 3: Implement `symbolic.rs`**

```rust
//! Symbolic verifiers for numeric directional claims and Bayesian sequential stopping.
//!
//! # [`NumericComparatorVerifier`]
//! Verifies claims of the form "X increased/decreased/rose/fell" by comparing
//! the sign of `(measured_value - baseline_value)` against the direction keyword.
//! No LLM is involved — this is the AlphaEvolve lesson applied: if the ground truth
//! is arithmetic, verify arithmetically.
//!
//! # [`BayesianStoppingRule`]
//! Implements pre-declared sequential stopping per SCIENTIA plan §5.2.
//! The stopping threshold is read from [`StopRule::threshold`]; campaigns stop
//! as soon as the posterior crosses the threshold (accept) or its complement (reject).

use vox_research_events::preregistration::StopRule;

/// Verdict from the [`NumericComparatorVerifier`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicVerdict {
    /// The measured direction matches the claimed direction.
    Confirmed,
    /// The measured direction contradicts the claimed direction.
    Refuted,
    /// Direction cannot be determined from the claim text, or measured == baseline.
    Inconclusive,
}

/// Decision from the [`BayesianStoppingRule`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopDecision {
    /// Posterior has not crossed either boundary — collect more samples.
    Continue,
    /// Posterior >= threshold — stop and accept the hypothesis.
    StopAccept,
    /// Posterior <= (1 - threshold) — stop and reject the hypothesis.
    StopReject,
}

/// Verifies numeric directional claims symbolically (no LLM).
#[derive(Debug, Default, Clone)]
pub struct NumericComparatorVerifier;

impl NumericComparatorVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Verify `claim_text` against `(measured_value, baseline_value)`.
    ///
    /// Extracts the direction keyword from the claim text, then checks whether
    /// `(measured_value - baseline_value)` has the correct sign.
    pub fn verify(&self, claim_text: &str, measured_value: f64, baseline_value: f64) -> SymbolicVerdict {
        let lower = claim_text.to_ascii_lowercase();

        let upward_keywords = ["increased", "rose", "risen", "grew", "higher", "up"];
        let downward_keywords = ["decreased", "fell", "fallen", "dropped", "lower", "reduced", "down"];

        let claims_increase = upward_keywords.iter().any(|kw| lower.contains(kw));
        let claims_decrease = downward_keywords.iter().any(|kw| lower.contains(kw));

        // If the claim has no clear direction, it is inconclusive
        if !claims_increase && !claims_decrease {
            return SymbolicVerdict::Inconclusive;
        }

        let diff = measured_value - baseline_value;

        // Zero difference: inconclusive even if direction keyword is present
        if diff == 0.0 {
            return SymbolicVerdict::Inconclusive;
        }

        let measured_increase = diff > 0.0;

        if (claims_increase && measured_increase) || (claims_decrease && !measured_increase) {
            SymbolicVerdict::Confirmed
        } else {
            SymbolicVerdict::Refuted
        }
    }
}

/// Implements Bayesian sequential stopping per a pre-declared [`StopRule`].
#[derive(Debug, Default, Clone)]
pub struct BayesianStoppingRule;

const DEFAULT_POSTERIOR_THRESHOLD: f64 = 0.95;

impl BayesianStoppingRule {
    pub fn new() -> Self {
        Self
    }

    /// Determine whether to stop based on `posterior` and the stopping rule.
    ///
    /// - `posterior >= threshold` → [`StopDecision::StopAccept`]
    /// - `posterior <= (1.0 - threshold)` → [`StopDecision::StopReject`]
    /// - otherwise → [`StopDecision::Continue`]
    ///
    /// If `stop_rule.threshold` is `None`, the default threshold of 0.95 is used.
    pub fn should_stop(&self, posterior: f64, stop_rule: &StopRule) -> StopDecision {
        let threshold = stop_rule.threshold.unwrap_or(DEFAULT_POSTERIOR_THRESHOLD);
        if posterior >= threshold {
            StopDecision::StopAccept
        } else if posterior <= (1.0 - threshold) {
            StopDecision::StopReject
        } else {
            StopDecision::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::StopRule;

    fn stop_rule(threshold: f64) -> StopRule {
        StopRule { max_n: 1000, alpha: None, threshold: Some(threshold) }
    }

    #[test]
    fn increased_claim_confirmed_when_measured_higher() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency increased by 15ms", 215.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Confirmed);
    }

    #[test]
    fn increased_claim_refuted_when_measured_lower() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency increased by 15ms", 185.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Refuted);
    }

    #[test]
    fn decreased_claim_confirmed_when_measured_lower() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("refusal rate decreased after update", 1.5, 3.0);
        assert_eq!(verdict, SymbolicVerdict::Confirmed);
    }

    #[test]
    fn decreased_claim_refuted_when_measured_higher() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("refusal rate decreased after update", 4.0, 3.0);
        assert_eq!(verdict, SymbolicVerdict::Refuted);
    }

    #[test]
    fn no_direction_keyword_is_inconclusive() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency changed significantly", 210.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Inconclusive);
    }

    #[test]
    fn equal_values_are_inconclusive_even_with_direction() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("latency rose after update", 200.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Inconclusive);
    }

    #[test]
    fn high_posterior_stops_accept() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.97, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopAccept);
    }

    #[test]
    fn low_posterior_stops_reject() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.02, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopReject);
    }

    #[test]
    fn mid_posterior_continues() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.50, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::Continue);
    }

    #[test]
    fn boundary_at_exactly_threshold_stops_accept() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.95, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopAccept);
    }

    #[test]
    fn no_threshold_uses_default_095() {
        let rule = BayesianStoppingRule::new();
        let no_threshold_rule = StopRule { max_n: 500, alpha: None, threshold: None };
        assert_eq!(rule.should_stop(0.96, &no_threshold_rule), StopDecision::StopAccept);
        assert_eq!(rule.should_stop(0.50, &no_threshold_rule), StopDecision::Continue);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vox-prereg symbolic 2>&1
```

Expected: 11 tests pass (6 verifier + 5 stopping rule).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-prereg/src/symbolic.rs
git commit -m "feat(scientia): NumericComparatorVerifier + BayesianStoppingRule (Phase 2 Task 4)"
```

---

### Task 5: Campaign gate (orchestrator integration stub)

**Files:**
- Create: `crates/vox-prereg/src/gate.rs`

The `PreregGate` is the single enforcement point that the orchestrator calls before launching any measurement campaign. It refuses campaigns with missing or invalid pre-registrations.

- [ ] **Step 1: Write failing tests**

```rust
// Place inside gate.rs under #[cfg(test)] mod tests
#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::facades::generate_signing_keypair;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };
    use crate::signing::sign_prereg;

    fn draft_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "JSON-mode violation rate rose after provider update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:aabbcc".to_string(),
                eval_set_swhid: "swh:1:dir:ddeeff".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: "json_violation_rate_pct".to_string(),
                aggregation: "mean".to_string(),
                units: "percent".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule { max_n: 300, alpha: None, threshold: Some(0.95) },
            decision_rule: DecisionRule {
                description: "if posterior P(increase) > 0.95, flag provider".to_string(),
            },
            cost_cap_usd: 15.0,
            signed_at: 0,
            signing_key: String::new(),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn approved_with_valid_signed_prereg() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let result = gate.check_campaign(Some(&prereg), Some(&sig.0));
        assert_eq!(result, GateResult::Approved, "valid signed prereg must be approved");
    }

    #[test]
    fn refused_without_prereg() {
        let gate = PreregGate::new();
        let result = gate.check_campaign(None, None);
        assert!(matches!(result, GateResult::Refused { .. }), "missing prereg must be refused");
        if let GateResult::Refused { reason } = result {
            assert!(reason.contains("preregistration"), "reason must mention preregistration");
        }
    }

    #[test]
    fn refused_without_signature() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        // Pass prereg but no signature
        let result = gate.check_campaign(Some(&prereg), None);
        assert!(matches!(result, GateResult::Refused { .. }), "missing signature must be refused");
        if let GateResult::Refused { reason } = result {
            assert!(reason.contains("signature"), "reason must mention signature");
        }
    }

    #[test]
    fn refused_with_bad_signature() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let bad_sig = "00".repeat(64);
        let result = gate.check_campaign(Some(&prereg), Some(&bad_sig));
        assert!(matches!(result, GateResult::Refused { .. }), "bad signature must be refused");
        if let GateResult::Refused { reason } = result {
            assert!(reason.contains("signature"), "reason must mention signature");
        }
    }
}
```

- [ ] **Step 2: Run (expect failure)**

```bash
cargo test -p vox-prereg gate 2>&1 | head -10
```

- [ ] **Step 3: Implement `gate.rs`**

```rust
//! Campaign gate — the orchestrator enforcement point for pre-registration.
//!
//! Per SCIENTIA plan §5.1: "The orchestrator **refuses to run a campaign without
//! a signed prereg**."
//!
//! [`PreregGate::check_campaign`] is a synchronous call the orchestrator makes
//! in its campaign-dispatch path before allocating any compute budget.

use crate::signing::verify_prereg;
use vox_research_events::preregistration::PreregistrationV1;

/// Result of [`PreregGate::check_campaign`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    /// The campaign is approved to proceed.
    Approved,
    /// The campaign is refused; `reason` is a human-readable explanation.
    Refused { reason: String },
}

/// Enforces pre-registration requirements before a campaign may start.
#[derive(Debug, Default, Clone)]
pub struct PreregGate;

impl PreregGate {
    pub fn new() -> Self {
        Self
    }

    /// Check whether a campaign may proceed.
    ///
    /// # Refusal conditions
    /// - `prereg` is `None` → refused with "no preregistration provided"
    /// - `signature_hex` is `None` → refused with "no signature provided"
    /// - signature verification fails → refused with the verification error
    pub fn check_campaign(
        &self,
        prereg: Option<&PreregistrationV1>,
        signature_hex: Option<&str>,
    ) -> GateResult {
        let prereg = match prereg {
            Some(p) => p,
            None => {
                return GateResult::Refused {
                    reason: "no preregistration provided; campaigns require a signed prereg before data collection".to_string(),
                }
            }
        };

        let sig = match signature_hex {
            Some(s) => s,
            None => {
                return GateResult::Refused {
                    reason: "no signature provided; the preregistration must be signed with an Ed25519 key".to_string(),
                }
            }
        };

        match verify_prereg(prereg, sig) {
            Ok(()) => GateResult::Approved,
            Err(e) => GateResult::Refused {
                reason: format!("invalid signature: {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::sign_prereg;
    use vox_crypto::facades::generate_signing_keypair;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn draft_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "JSON-mode violation rate rose after provider update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:aabbcc".to_string(),
                eval_set_swhid: "swh:1:dir:ddeeff".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: "json_violation_rate_pct".to_string(),
                aggregation: "mean".to_string(),
                units: "percent".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule { max_n: 300, alpha: None, threshold: Some(0.95) },
            decision_rule: DecisionRule {
                description: "if posterior P(increase) > 0.95, flag provider".to_string(),
            },
            cost_cap_usd: 15.0,
            signed_at: 0,
            signing_key: String::new(),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn approved_with_valid_signed_prereg() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let result = gate.check_campaign(Some(&prereg), Some(&sig.0));
        assert_eq!(result, GateResult::Approved, "valid signed prereg must be approved");
    }

    #[test]
    fn refused_without_prereg() {
        let gate = PreregGate::new();
        let result = gate.check_campaign(None, None);
        assert!(matches!(result, GateResult::Refused { .. }), "missing prereg must be refused");
        if let GateResult::Refused { reason } = result {
            assert!(reason.contains("preregistration"), "reason must mention preregistration");
        }
    }

    #[test]
    fn refused_without_signature() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let result = gate.check_campaign(Some(&prereg), None);
        assert!(matches!(result, GateResult::Refused { .. }), "missing signature must be refused");
        if let GateResult::Refused { reason } = result {
            assert!(reason.contains("signature"), "reason must mention signature");
        }
    }

    #[test]
    fn refused_with_bad_signature() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let bad_sig = "00".repeat(64);
        let result = gate.check_campaign(Some(&prereg), Some(&bad_sig));
        assert!(matches!(result, GateResult::Refused { .. }), "bad signature must be refused");
        if let GateResult::Refused { reason } = result {
            assert!(reason.contains("signature"), "reason must mention signature");
        }
    }
}
```

- [ ] **Step 4: Run all tests**

```bash
cargo test -p vox-prereg 2>&1 | tail -20
```

Expected: all tests pass (3 trusty_uri + 3 signing + 4 deviation + 11 symbolic + 4 gate = 25 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-prereg/src/gate.rs
git commit -m "feat(scientia): PreregGate campaign enforcement stub (Phase 2 Task 5)"
```

---

### Task 6: Wire into workspace + mark Phase 2 Complete

**Files:**
- Modify: root `Cargo.toml` — add `vox-prereg` to workspace.dependencies
- Modify: `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md` — mark Phase 2 Complete

- [ ] **Step 1: Add `vox-prereg` to root `Cargo.toml` workspace.dependencies**

In the `[workspace.dependencies]` section (alphabetically near other `vox-p*` entries):

```toml
vox-prereg = { path = "crates/vox-prereg" }
```

The crate is already included in the workspace `members` glob (`crates/*`). This line allows downstream crates to write `vox-prereg = { workspace = true }` without specifying the path.

- [ ] **Step 2: Verify workspace compiles cleanly**

```bash
cargo check -p vox-prereg 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Run full test suite one final time**

```bash
cargo test -p vox-prereg 2>&1
```

Expected: all 25 tests pass.

- [ ] **Step 4: Mark Phase 2 Complete in strategic plan**

In `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`, find the Phase 2 heading:

```markdown
### Phase 2 — Pre-registration + symbolic verifiers (2 wk)
```

Replace the opening line with:

```markdown
### Phase 2 — Pre-registration + symbolic verifiers (2 wk) — **Complete**

> Status: **Complete** 2026-05-09. `vox-prereg` shipped; Trusty URI signing, deviation detector, symbolic verifiers, Bayesian stopping rule, and campaign gate all implemented and tested.
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml docs/src/architecture/scientia-self-publication-finalization-plan-2026.md
git commit -m "feat(scientia): wire vox-prereg into workspace + mark Phase 2 Complete"
```

---

## Self-review checklist

- [ ] All types used in Task 5 (`GateResult`, `PreregGate`) and Task 4 (`SymbolicVerdict`, `StopDecision`, `BayesianStoppingRule`, `NumericComparatorVerifier`) are defined before first use
- [ ] `vox_crypto::facades::sign`, `verify`, `SigningKey`, `VerifyingKey`, `generate_signing_keypair`, `to_verifying_key`, `verifying_key_to_bytes`, `verifying_key_from_bytes` — all used exactly as they appear in `crates/vox-crypto/src/facades.rs`
- [ ] `PreregistrationV1`, `StatisticalTest`, `StopRule`, `MetricSpec`, `TestSpec`, `DecisionRule`, `SubstrateRef` — all field names match `crates/vox-research-events/src/preregistration.rs` (using `max_n`, `threshold`, `prior`, `alpha` — not `max_sample`, `bayesian_prior`, etc.)
- [ ] No placeholders: every function body is complete and every test has assertions
- [ ] `cargo test -p vox-prereg` references a real package name (matches `name = "vox-prereg"` in Cargo.toml)
- [ ] Trusty URI excludes `id` field from hash input (prevents circular dependency)
- [ ] `canonical_json` is `pub(crate)` so `signing.rs` can reuse it without duplicating serialization
- [ ] Workspace dep line added; arch-check will enforce L2 layer via existing `layers.toml` entry
- [ ] Phase 2 marked Complete in strategic plan with date and summary
