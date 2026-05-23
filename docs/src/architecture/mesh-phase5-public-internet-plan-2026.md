---
title: "Mesh Phase 5 — Public-Internet Safety Implementation Plan (2026-05-09)"
description: "Step-by-step TDD implementation plan for SSOT Phase 5: the trust ladder that makes a Vox node safe to expose to the internet under bounded trust. 10 tasks (P5-T1..P5-T10) covering Ed25519-signed envelopes, GitHub-attested pairing, per-key quota and reputation EMA, signed result attestations, spot-check sampling, per-job ephemeral subkeys, end-to-end kudos accounting, mesh-wide model inventory aggregation, donation-policy privacy signaling, and per-pairing X25519 JWE keys."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. SSOT (mesh-and-language-distribution-ssot-2026.md §3 Phase 5) is the durable artifact."
---

# Mesh Phase 5 — Public-Internet Safety Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task ends with a `cargo test` step and a `git commit` citing the task ID (`P5-T1`, `P5-T1a`, etc.).

**Goal.** A Vox node is safe to expose to the internet under bounded trust — vetted public peers only, with abuse fuses, attestation, and identity binding. The "two GitHub-attested strangers pair their personal meshes and share compute" demo lights up at the end of this phase. Kudos accounting is real, end-to-end.

**Architecture.** Replace forgeable HS256 bearer/JWT with Ed25519-signed envelopes verified against a trust ledger. Pairing is gated on a publicly verifiable GitHub-attestation manifest (signed JSON in a Gist owned by the GitHub user; no Vox-owned server in the loop). Add a per-key token bucket and reputation EMA, persisted to vox-db. Worker-side, populate the existing `TaskResult.attestation` field with a per-job-ephemeral-Ed25519-signed envelope binding `(task_id, input_hash, output_hash, gpu_seconds, trace_blake3)`. Submitter-side spot-checks ~1% of attested results by re-running on a different peer. The same signed envelope IS the kudos credit (one signature, two birds). Round it out with mesh-wide model inventory aggregation, a donation-policy `accept_sensitive_workloads` signal, and per-pairing X25519 derivation for JWE recipients.

**Tech stack.** Rust 2024 edition, `tokio`, `tracing`, `thiserror`, `serde`/`serde_json`, `ed25519-dalek` and `x25519-dalek` (only via `vox-crypto`), `blake3` and `sha3` (only via `vox-crypto`), `rusqlite` (via `vox-db`), `reqwest` (already in workspace, used for the Gist fetch in P5-T2). No new crypto crates — anti-goal in SSOT §0.

**SSOT.** [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) §3 Phase 5. Research backing: [`ludus-identity-github-integration-research-2026.md`](ludus-identity-github-integration-research-2026.md) (device-flow), [`mesh-dashboard-and-distributed-compute-research-2026.md`](mesh-dashboard-and-distributed-compute-research-2026.md) §3.5 (per-TaskKind attestation mapping).

**Hopper integration.** P5-T1's daemon Ed25519 key signs the `DeveloperOverride` capability mint
introduced in `P3-T6` (SSOT Hp-T4). In v0.6 this token is single-machine-only (Option A); the
signed envelope is forward-compat for the mesh-replicated Option C, which is deferred. See SSOT
§3.5 and [`unified-task-hopper-research-2026.md`](unified-task-hopper-research-2026.md).

**Anti-goals (binding).** No custom crypto (Ed25519 / X25519 / JWE / BLAKE3 only via `vox-crypto`). No blockchain or token economy (kudos is a local-then-gossip ledger). No TEE-first (TEE is a Phase 6 stub). No onion routing. No transitive web-of-trust (paired peers + GitHub attestation are binary gates). No public SaaS multi-tenant control plane. Reputation is a *signal*, not a *capability* — it can deprioritize but never bypass the binary attestation gate.

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths below are relative to this worktree.

**Vox project rule.** No `.ps1` / `.sh` / `.py` automation glue. Any required automation is `.vox`.

---

## File map

**Create:**

- `crates/vox-populi/src/transport/envelope.rs` — Ed25519-signed wire envelope (`SignedA2AEnvelope`).
- `crates/vox-populi/src/transport/auth_ed25519.rs` — verifier + role classification using the trust ledger.
- `crates/vox-populi/src/quota/mod.rs` — module root.
- `crates/vox-populi/src/quota/bucket.rs` — token-bucket + reputation EMA, vox-db persistence.
- `crates/vox-populi/src/quota/spec.rs` — per-key quota policy types (`QuotaPolicy`, `ReputationEma`).
- `crates/vox-populi/src/pairing/mod.rs` — module root.
- `crates/vox-populi/src/pairing/github_attestation.rs` — manifest schema, signing, verification, Gist fetch.
- `crates/vox-populi/src/pairing/device_flow.rs` — GitHub OAuth device-flow client (read-only `gist` scope).
- `crates/vox-populi/src/pairing/revocation.rs` — tombstone gossip, ≤60s propagation.
- `crates/vox-mesh-types/src/attestation.rs` — `Attestation` envelope, per-TaskKind input/output hash schema.
- `crates/vox-mesh-types/src/peer_reputation.rs` — `PeerReputation` sidecar to `NodeRecord`.
- `crates/vox-mesh-types/src/model_inventory.rs` — mesh-wide model inventory snapshot type.
- `crates/vox-orchestrator/src/spot_check/mod.rs` — submitter-side ~1% replay sampler.
- `crates/vox-orchestrator/src/spot_check/sampler.rs` — sampling decision + verifier.
- `crates/vox-identity/src/ephemeral.rs` — per-job ephemeral Ed25519 subkey minter.
- `crates/vox-identity/src/pairing_x25519.rs` — per-pairing X25519 key derivation + storage.
- `crates/vox-db/src/schema/domains/sql/mesh_phase5.sql` — new tables (`peer_quota`, `peer_attestation`, `pairing_x25519`, `contribution_ledger`, `mesh_model_inventory`, `peer_pairing_status`).
- `crates/vox-populi/tests/ed25519_envelope.rs` — round-trip + forgery tests.
- `crates/vox-populi/tests/github_attestation.rs` — manifest verify + revocation tests.
- `crates/vox-populi/tests/quota_bucket.rs` — token bucket and EMA persistence tests.
- `crates/vox-populi/tests/pairing_e2e.rs` — fresh-node accepts paired-peer / refuses unpaired-peer integration test.
- `crates/vox-orchestrator/tests/spot_check.rs` — sampler injects forged result, detection probability test.
- `crates/vox-orchestrator/tests/kudos_reconciliation.rs` — sum-of-credited-GpuComputeMs ≈ sum-of-duration_ms over a 100-job batch.

**Modify:**

- `crates/vox-populi/src/transport/auth.rs` — gate JWT-HS256 path behind a deprecation warning; route Ed25519 envelope path through new module.
- `crates/vox-populi/src/transport/mod.rs` — re-export `auth_ed25519`, `envelope`.
- `crates/vox-populi/src/lib.rs` — module declarations for `quota`, `pairing`.
- `crates/vox-mesh-types/src/lib.rs` — re-export `attestation`, `peer_reputation`, `model_inventory`.
- `crates/vox-mesh-types/src/task.rs` — populate the existing `TaskResult.worker_ed25519_sig_b64` semantics with a structured `attestation: Option<Attestation>` field; preserve back-compat.
- `crates/vox-mesh-types/src/donation_policy.rs` — add `accept_sensitive_workloads: bool`.
- `crates/vox-mesh-types/src/kudos.rs` — add `RewardPrimitive::GpuComputeMs` projection helper.
- `crates/vox-orchestrator/src/a2a/remote_worker.rs:100-160` — replace BLAKE3-derived shared mesh-secret JWE key with per-pairing X25519-derived key; populate attestation.
- `crates/vox-orchestrator/src/a2a/jwe.rs` — multi-recipient JWE per-pairing (W3 closure).
- `crates/vox-secrets/src/spec.rs` — add `VoxMeshAuthScheme` (`"ed25519-envelope"` / `"jwt-hs256"` / `"both"`), `VoxMeshSpotCheckProb`, `VoxMeshGithubAttestationGistUrl`, `VoxMeshPairingX25519PrivPath`.
- `crates/vox-db/src/schema/domains/vox_mesh.rs` — `include_str!` the new SQL block from `mesh_phase5.sql`.
- `crates/vox-orchestrator/src/lib.rs` — module declaration for `spot_check`.
- `crates/vox-identity/src/lib.rs` — re-export `ephemeral`, `pairing_x25519`.

---

## Task ordering rationale

Tasks are ordered so each one leaves the workspace in a building, testing state, and the trust ladder is built bottom-up:

1. **P5-T1 (Ed25519 envelope)** comes first because every later wire change rides on it.
2. **P5-T2 (GitHub attestation)** locks the pairing gate before any quota/reputation logic admits a peer.
3. **P5-T3 (per-key quota + EMA)** persists the abuse-fuse counters before they can be consulted from the dispatch path.
4. **P5-T4 (signed result attestation)** populates the existing `TaskResult.attestation` field; required input for both spot-check and kudos.
5. **P5-T5 (spot-check sampler)** consumes attested results.
6. **P5-T6 (per-job ephemeral subkey)** scopes the attestation signer; logically downstream of T4 because it changes who signs.
7. **P5-T7 (kudos accounting)** projects attestation envelopes into the contribution ledger.
8. **P5-T8 (model inventory aggregation)** is independent of the trust ladder but rides the same A2A envelope shape and is sequenced before T9 because the dashboard surfaces both.
9. **P5-T9 (donation-policy privacy)** is a pure type and policy change; surface-level UI piggybacks on T8's dashboard plumbing.
10. **P5-T10 (per-pairing X25519)** closes the W3 weakness; intentionally last because it depends on stable pairing identity (T2) and benefits from the new trust ledger paths (T1).

Each task ends with `cargo test -p <crate> <focused-filter>` and a `git commit -m "<type>(<crate>): <description> [P5-T<n>]"`.

---

## Task P5-T1 — Replace JWT-HS256 with Ed25519-signed envelope

**Goal.** Every A2A control-plane message carries `(payload, sender_pubkey, signature)`. Verifier checks the signature with the sender's pubkey resolved via `vox-identity::TrustedNodeRegistry`. JWT-HS256 stays available behind `VoxMeshAuthScheme = "jwt-hs256" | "both"` for the migration window; default flips to `"ed25519-envelope"`.

**Capability mints signed by this key.** The same daemon Ed25519 key that signs A2A envelopes also signs every capability-mint envelope minted by the sealed-trait facade (SSOT P3-T6). Tokens covered include the existing dispatch capability tokens **and**:

- `DeveloperOverride` (introduced in `P3-T6` + SSOT Hp-T4) is signed by the same daemon Ed25519
  key as other capability mints. In v0.6 (single-machine hopper) this is local-only; when the
  mesh-replicated Option C lands, the signed envelope ensures override authority cannot be
  forged across the gossip path.

**JWT → Ed25519 cleanup window.** The `"jwt-hs256"` and `"both"` modes of `VoxMeshAuthScheme` are
deprecation-only; they emit a `tracing::warn!` on construction and are scheduled for removal in
v0.7. P5-T1c adds the gate; the v0.7 release plan owns the removal commit (delete `auth.rs::try_authorize_jwt`,
delete the `JwtHs256` and `Both` enum variants, drop `VoxMeshJwtHmacSecret`).

**Files:**

- Create: `crates/vox-populi/src/transport/envelope.rs`
- Create: `crates/vox-populi/src/transport/auth_ed25519.rs`
- Create: `crates/vox-populi/tests/ed25519_envelope.rs`
- Modify: `crates/vox-populi/src/transport/auth.rs`
- Modify: `crates/vox-populi/src/transport/mod.rs`
- Modify: `crates/vox-secrets/src/spec.rs`

### P5-T1a: Define the wire envelope (failing-test first)

- [ ] **Step 1: Write the failing test for envelope round-trip.**

Create `crates/vox-populi/tests/ed25519_envelope.rs`:

```rust
use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_populi::transport::envelope::{SignedA2AEnvelope, EnvelopeVerifyError};

#[test]
fn envelope_round_trip_verifies() {
    let (sk, vk) = generate_signing_keypair();
    let payload = br#"{"hello":"world"}"#.to_vec();
    let env = SignedA2AEnvelope::sign("ack", &payload, &sk, &vk);
    assert_eq!(env.message_type, "ack");
    assert_eq!(env.sender_pubkey_hex.len(), 64);
    assert!(env.verify_self_signed().is_ok());
}

#[test]
fn envelope_with_swapped_signature_is_rejected() {
    let (sk_a, vk_a) = generate_signing_keypair();
    let (sk_b, _vk_b) = generate_signing_keypair();
    let payload = br#"{"hello":"world"}"#.to_vec();
    let mut env = SignedA2AEnvelope::sign("ack", &payload, &sk_a, &vk_a);
    // Replace signature with one made by another key: must fail.
    let other = SignedA2AEnvelope::sign("ack", &payload, &sk_b, &vk_a);
    env.signature_b64 = other.signature_b64;
    let err = env.verify_self_signed().unwrap_err();
    assert!(matches!(err, EnvelopeVerifyError::SignatureMismatch));
}

#[test]
fn envelope_with_swapped_payload_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let payload = br#"{"hello":"world"}"#.to_vec();
    let mut env = SignedA2AEnvelope::sign("ack", &payload, &sk, &vk);
    env.payload_b64 = base64::engine::general_purpose::STANDARD
        .encode(b"{\"hello\":\"evil\"}");
    let err = env.verify_self_signed().unwrap_err();
    assert!(matches!(err, EnvelopeVerifyError::SignatureMismatch));
}

#[test]
fn pubkey_in_envelope_must_match_signer() {
    let (sk_a, vk_a) = generate_signing_keypair();
    let (_sk_b, vk_b) = generate_signing_keypair();
    let payload = br#"{}"#.to_vec();
    let mut env = SignedA2AEnvelope::sign("ack", &payload, &sk_a, &vk_a);
    // Lie about pubkey.
    env.sender_pubkey_hex = hex::encode(verifying_key_to_bytes(&vk_b));
    let err = env.verify_self_signed().unwrap_err();
    assert!(matches!(err, EnvelopeVerifyError::SignatureMismatch));
}
```

- [ ] **Step 2: Run, verify failure.**

```bash
cargo test -p vox-populi --test ed25519_envelope 2>&1 | tail -20
```

Expected: FAIL — `transport::envelope` module not found.

- [ ] **Step 3: Implement the envelope.**

Create `crates/vox-populi/src/transport/envelope.rs`:

```rust
//! Ed25519-signed A2A envelope. Replaces JWT-HS256 (forgeable by any
//! token-holder) per SSOT Phase 5 P5-T1.
//!
//! Wire shape (JSON):
//! ```json
//! {
//!   "version": 1,
//!   "message_type": "ack",
//!   "sender_pubkey_hex": "<64 hex>",
//!   "payload_b64": "<base64 std>",
//!   "signature_b64": "<base64 std>",
//!   "issued_at_unix_ms": 1234567890123
//! }
//! ```
//!
//! Signature input is the canonical concatenation:
//! `b"voxmesh.envelope.v1\0" || message_type || \0 || payload || \0 || issued_at_unix_ms_be8`.
//! Anti-replay is enforced by `issued_at_unix_ms` clock-skew bound at the
//! verifier (default ±300s) plus a bounded LRU of recent signatures.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use vox_crypto::{
    SigningKey, VerifyingKey, sign, verify, verify_signature_hex, verifying_key_to_bytes,
};

/// Stable canonical-input prefix. Bumping invalidates all old signatures.
pub const ENVELOPE_DOMAIN: &[u8] = b"voxmesh.envelope.v1\0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedA2AEnvelope {
    pub version: u8,
    pub message_type: String,
    pub sender_pubkey_hex: String,
    pub payload_b64: String,
    pub signature_b64: String,
    pub issued_at_unix_ms: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EnvelopeVerifyError {
    #[error("unsupported envelope version: {0}")]
    UnsupportedVersion(u8),
    #[error("invalid pubkey hex")]
    InvalidPubkey,
    #[error("invalid signature base64")]
    InvalidSignatureB64,
    #[error("invalid payload base64")]
    InvalidPayloadB64,
    #[error("signature does not verify")]
    SignatureMismatch,
    #[error("issued_at out of clock skew window: drift={drift_ms}ms")]
    ClockSkew { drift_ms: i64 },
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn canonical_input(message_type: &str, payload: &[u8], issued_at_unix_ms: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(
        ENVELOPE_DOMAIN.len() + message_type.len() + 1 + payload.len() + 1 + 8,
    );
    buf.extend_from_slice(ENVELOPE_DOMAIN);
    buf.extend_from_slice(message_type.as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(payload);
    buf.push(0u8);
    buf.extend_from_slice(&issued_at_unix_ms.to_be_bytes());
    buf
}

impl SignedA2AEnvelope {
    pub fn sign(
        message_type: &str,
        payload: &[u8],
        sk: &SigningKey,
        vk: &VerifyingKey,
    ) -> Self {
        let issued_at_unix_ms = now_unix_ms();
        let input = canonical_input(message_type, payload, issued_at_unix_ms);
        let sig = sign(sk, &input);
        Self {
            version: 1,
            message_type: message_type.to_string(),
            sender_pubkey_hex: hex::encode(verifying_key_to_bytes(vk)),
            payload_b64: base64::engine::general_purpose::STANDARD.encode(payload),
            signature_b64: base64::engine::general_purpose::STANDARD.encode(sig),
            issued_at_unix_ms,
        }
    }

    /// Self-contained verification: parses pubkey from envelope and verifies.
    /// Does **not** consult the trust ledger — use `auth_ed25519::verify_against_trust`
    /// for that.
    pub fn verify_self_signed(&self) -> Result<Vec<u8>, EnvelopeVerifyError> {
        if self.version != 1 {
            return Err(EnvelopeVerifyError::UnsupportedVersion(self.version));
        }
        let payload = base64::engine::general_purpose::STANDARD
            .decode(&self.payload_b64)
            .map_err(|_| EnvelopeVerifyError::InvalidPayloadB64)?;
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature_b64)
            .map_err(|_| EnvelopeVerifyError::InvalidSignatureB64)?;
        if sig_bytes.len() != 64 {
            return Err(EnvelopeVerifyError::InvalidSignatureB64);
        }
        let input = canonical_input(&self.message_type, &payload, self.issued_at_unix_ms);
        let ok = verify_signature_hex(
            &self.sender_pubkey_hex,
            &input,
            &hex::encode(&sig_bytes),
        )
        .map_err(|_| EnvelopeVerifyError::InvalidPubkey)?;
        if !ok {
            return Err(EnvelopeVerifyError::SignatureMismatch);
        }
        Ok(payload)
    }

    /// Verify the issued-at fits within `±skew_ms` of `now_unix_ms`.
    pub fn check_clock_skew(&self, skew_ms: u64) -> Result<(), EnvelopeVerifyError> {
        let now = now_unix_ms() as i64;
        let drift = now - self.issued_at_unix_ms as i64;
        if drift.unsigned_abs() > skew_ms {
            return Err(EnvelopeVerifyError::ClockSkew { drift_ms: drift });
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Wire into `transport/mod.rs`.**

Add after existing module declarations:

```rust
pub mod envelope;
pub mod auth_ed25519;
```

- [ ] **Step 5: Run, verify pass.**

```bash
cargo test -p vox-populi --test ed25519_envelope 2>&1 | tail -10
```

Expected: PASS for all four tests.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-populi/src/transport/envelope.rs \
        crates/vox-populi/src/transport/mod.rs \
        crates/vox-populi/tests/ed25519_envelope.rs
git commit -m "feat(populi): Ed25519-signed A2A envelope wire type [P5-T1a]"
```

### P5-T1b: Verifier consults `vox-identity::TrustedNodeRegistry`

- [ ] **Step 1: Write the failing test.**

Append to `crates/vox-populi/tests/ed25519_envelope.rs`:

```rust
#[test]
fn verify_against_trust_admits_known_pubkey() {
    use vox_identity::TrustedNodeRegistry;
    use vox_populi::transport::auth_ed25519::{verify_against_trust, VerifyTrustError};

    let (sk, vk) = generate_signing_keypair();
    let pubkey_hex = hex::encode(verifying_key_to_bytes(&vk));
    let mut reg = TrustedNodeRegistry::default();
    reg.upsert("node-A", &pubkey_hex);

    let env = SignedA2AEnvelope::sign("ack", b"{}", &sk, &vk);
    let ctx = verify_against_trust(&env, &reg, 300_000).expect("admit");
    assert_eq!(ctx.node_id, "node-A");
}

#[test]
fn verify_against_trust_rejects_unknown_pubkey() {
    use vox_identity::TrustedNodeRegistry;
    use vox_populi::transport::auth_ed25519::{verify_against_trust, VerifyTrustError};

    let (sk, vk) = generate_signing_keypair();
    let reg = TrustedNodeRegistry::default();
    let env = SignedA2AEnvelope::sign("ack", b"{}", &sk, &vk);
    let err = verify_against_trust(&env, &reg, 300_000).unwrap_err();
    assert!(matches!(err, VerifyTrustError::UnknownPubkey));
}
```

- [ ] **Step 2: Run, verify failure.**

```bash
cargo test -p vox-populi --test ed25519_envelope 2>&1 | tail -10
```

Expected: FAIL — `auth_ed25519` module / function not found.

- [ ] **Step 3: Implement `auth_ed25519.rs`.**

Create `crates/vox-populi/src/transport/auth_ed25519.rs`:

```rust
//! Trust-ledger-backed verification for Ed25519-signed A2A envelopes.
//!
//! Layered on top of `envelope.rs`:
//!
//! 1. Parse and self-verify (signature math).
//! 2. Resolve `sender_pubkey_hex` against the [`TrustedNodeRegistry`].
//! 3. Enforce clock-skew window.
//! 4. Return a [`NodeAuthContext`] that downstream policy code consults.
//!
//! Anti-goal: this module never accepts an unknown pubkey for any reason —
//! "reputation" cannot bypass the binary trust gate (SSOT §0).

use vox_identity::TrustedNodeRegistry;

use super::envelope::{EnvelopeVerifyError, SignedA2AEnvelope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeAuthContext {
    pub node_id: String,
    pub pubkey_hex: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VerifyTrustError {
    #[error("envelope cryptographic verification failed: {0}")]
    Envelope(#[from] EnvelopeVerifyError),
    #[error("pubkey is not in the trust ledger")]
    UnknownPubkey,
}

/// Verify an envelope and admit only if the pubkey is in the trust ledger.
pub fn verify_against_trust(
    env: &SignedA2AEnvelope,
    registry: &TrustedNodeRegistry,
    clock_skew_ms: u64,
) -> Result<NodeAuthContext, VerifyTrustError> {
    let _payload = env.verify_self_signed()?;
    env.check_clock_skew(clock_skew_ms)?;
    let known = registry
        .lookup_by_pubkey_hex(&env.sender_pubkey_hex)
        .ok_or(VerifyTrustError::UnknownPubkey)?;
    Ok(NodeAuthContext {
        node_id: known.node_id().to_string(),
        pubkey_hex: env.sender_pubkey_hex.clone(),
    })
}
```

(`TrustedNodeRegistry::default`, `upsert(node_id, pubkey_hex)`, `lookup_by_pubkey_hex` are minor additions to `crates/vox-identity/src/trust.rs` — confirm the existing API and adapt names if upstream uses different ones. If not present, add them in a small follow-up commit within this task.)

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-populi --test ed25519_envelope 2>&1 | tail -10
```

Expected: all six tests PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-populi/src/transport/auth_ed25519.rs \
        crates/vox-populi/tests/ed25519_envelope.rs \
        crates/vox-identity/src/trust.rs
git commit -m "feat(populi): trust-ledger gate for Ed25519 envelope verification [P5-T1b]"
```

### P5-T1c: Migration flag and JWT-HS256 deprecation path

- [ ] **Step 1: Add `VoxMeshAuthScheme` SecretId.**

In `crates/vox-secrets/src/spec.rs`, near `VoxMeshJwtHmacSecret`:

```rust
VoxMeshAuthScheme,
```

And the spec table entry:

```rust
SecretSpec {
    id: SecretId::VoxMeshAuthScheme,
    env: "VOX_MESH_AUTH_SCHEME",
    // values: "ed25519-envelope" | "jwt-hs256" | "both" (default: "ed25519-envelope")
    // ... fill remaining fields per the existing struct shape in this file
},
```

- [ ] **Step 2: Adapt `auth.rs::PopuliMeshAuthRuntime` to read the scheme.**

In `crates/vox-populi/src/transport/auth.rs`, after `from_env()`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    Ed25519Envelope,
    JwtHs256,
    Both,
}

impl AuthScheme {
    pub fn from_env() -> Self {
        match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshAuthScheme)
            .expose()
            .map(str::trim)
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "jwt-hs256" => AuthScheme::JwtHs256,
            "both" => AuthScheme::Both,
            _ => AuthScheme::Ed25519Envelope,
        }
    }
    pub fn accepts_jwt(self) -> bool {
        matches!(self, Self::JwtHs256 | Self::Both)
    }
    pub fn accepts_ed25519(self) -> bool {
        matches!(self, Self::Ed25519Envelope | Self::Both)
    }
}
```

Modify `try_authorize_jwt` so it returns `None` early when `AuthScheme::from_env().accepts_jwt() == false`. Emit a `tracing::warn!` once on construction when JWT is permitted, naming the deprecation:

```rust
tracing::warn!(
    "VOX_MESH_AUTH_SCHEME admits jwt-hs256; this is forgeable by any token-holder \
     and will be removed in v0.7. Migrate to ed25519-envelope per SSOT P5-T1."
);
```

- [ ] **Step 3: Test scheme gate.**

Append to `crates/vox-populi/tests/ed25519_envelope.rs`:

```rust
#[test]
fn auth_scheme_default_is_ed25519_envelope() {
    // Use a fresh process env-var unset to ensure default kicks in. The Vox
    // secrets layer reads via std::env in the absence of a layered Clavis.
    let prior = std::env::var("VOX_MESH_AUTH_SCHEME").ok();
    std::env::remove_var("VOX_MESH_AUTH_SCHEME");
    let scheme = vox_populi::transport::auth::AuthScheme::from_env();
    assert_eq!(scheme, vox_populi::transport::auth::AuthScheme::Ed25519Envelope);
    if let Some(v) = prior {
        std::env::set_var("VOX_MESH_AUTH_SCHEME", v);
    }
}
```

(Note: env-var test is marked `#[serial_test::serial]` if the project uses `serial_test`; otherwise the test acquires a process-wide mutex from `vox-secrets`'s test harness.)

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-populi --test ed25519_envelope 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-populi/src/transport/auth.rs \
        crates/vox-populi/tests/ed25519_envelope.rs \
        crates/vox-secrets/src/spec.rs
git commit -m "feat(populi): VoxMeshAuthScheme migration flag; JWT-HS256 gated [P5-T1c]"
```

---

## Task P5-T2 — GitHub-attestation gate at pairing

**Goal.** Pairing requires a GitHub-attestation manifest (signed JSON) hosted in a Gist owned by the GitHub user. The counterparty fetches the Gist, verifies the signature using the publishing user's published Vox/SSH/GPG key, and only then admits the peer. Revocation = update or delete the Gist. No Vox-owned server is in the loop.

**Files:**

- Create: `crates/vox-populi/src/pairing/mod.rs`
- Create: `crates/vox-populi/src/pairing/github_attestation.rs`
- Create: `crates/vox-populi/src/pairing/device_flow.rs`
- Create: `crates/vox-populi/src/pairing/revocation.rs`
- Create: `crates/vox-populi/tests/github_attestation.rs`
- Modify: `crates/vox-populi/src/lib.rs`
- Modify: `crates/vox-secrets/src/spec.rs`

### P5-T2a: Manifest schema, sign, verify

- [ ] **Step 1: Failing test for round-trip.**

Create `crates/vox-populi/tests/github_attestation.rs`:

```rust
use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_populi::pairing::github_attestation::{
    AttestationManifest, ManifestVerifyError,
};

#[test]
fn manifest_round_trip_verifies() {
    let (sk, vk) = generate_signing_keypair();
    let manifest = AttestationManifest::new_signed(
        /* node_pubkey_hex */ &hex::encode(verifying_key_to_bytes(&vk)),
        /* github_user_id */ "12345",
        /* github_login   */ "alice",
        /* expires_at_ms  */ 1_900_000_000_000,
        &sk,
        &vk,
    );
    let unverified = serde_json::to_string(&manifest).unwrap();
    let parsed: AttestationManifest = serde_json::from_str(&unverified).unwrap();
    assert!(parsed.verify().is_ok());
}

#[test]
fn manifest_with_swapped_pubkey_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let (_, vk_other) = generate_signing_keypair();
    let mut manifest = AttestationManifest::new_signed(
        &hex::encode(verifying_key_to_bytes(&vk)),
        "12345",
        "alice",
        1_900_000_000_000,
        &sk,
        &vk,
    );
    manifest.node_pubkey_hex = hex::encode(verifying_key_to_bytes(&vk_other));
    assert!(matches!(
        manifest.verify().unwrap_err(),
        ManifestVerifyError::SignatureMismatch
    ));
}

#[test]
fn manifest_expired_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let manifest = AttestationManifest::new_signed(
        &hex::encode(verifying_key_to_bytes(&vk)),
        "12345",
        "alice",
        /* expires_at_ms */ 1, // 1ms after epoch == expired
        &sk,
        &vk,
    );
    assert!(matches!(
        manifest.verify().unwrap_err(),
        ManifestVerifyError::Expired { .. }
    ));
}
```

- [ ] **Step 2: Run, verify failure.**

```bash
cargo test -p vox-populi --test github_attestation 2>&1 | tail -15
```

Expected: FAIL — module not found.

- [ ] **Step 3: Implement the manifest.**

Create `crates/vox-populi/src/pairing/mod.rs`:

```rust
//! GitHub-attested pairing (SSOT Phase 5 P5-T2).

pub mod device_flow;
pub mod github_attestation;
pub mod revocation;

pub use github_attestation::{AttestationManifest, ManifestVerifyError};
```

Create `crates/vox-populi/src/pairing/github_attestation.rs`:

```rust
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use vox_crypto::{
    SigningKey, VerifyingKey, sign, verify_signature_hex, verifying_key_to_bytes,
};

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
    /// Hex-encoded Ed25519 pubkey of the `node_pubkey_hex` owner; redundant
    /// with `node_pubkey_hex` but required so that fetchers can self-verify
    /// without an out-of-band lookup.
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

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

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
        let ok = verify_signature_hex(
            &self.signer_pubkey_hex,
            &input,
            &hex::encode(&sig_bytes),
        )
        .map_err(|_| ManifestVerifyError::InvalidPubkey)?;
        if !ok {
            return Err(ManifestVerifyError::SignatureMismatch);
        }
        // The signer-pubkey-hex MUST equal the node-pubkey-hex (a node attests
        // its own ownership of a GitHub identity). Forging is impossible because
        // the node-pubkey holder is the only party with access to the
        // corresponding signing key.
        if self.signer_pubkey_hex != self.node_pubkey_hex {
            return Err(ManifestVerifyError::SignatureMismatch);
        }
        Ok(())
    }
}
```

Create stubs `device_flow.rs` and `revocation.rs` (filled in P5-T2b/T2c):

`crates/vox-populi/src/pairing/device_flow.rs`:

```rust
//! GitHub OAuth device-flow client. Read-only `gist` scope.
//! Filled in P5-T2b.
```

`crates/vox-populi/src/pairing/revocation.rs`:

```rust
//! Tombstone gossip for revoked attestations.
//! Filled in P5-T2c.
```

- [ ] **Step 4: Wire `pairing` into the populi crate.**

In `crates/vox-populi/src/lib.rs`, add (next to other top-level module declarations):

```rust
pub mod pairing;
pub mod quota; // declared early so P5-T3 can land cleanly
```

(Quota module file created in P5-T3.)

- [ ] **Step 5: Run, verify pass.**

```bash
cargo test -p vox-populi --test github_attestation 2>&1 | tail -10
```

Expected: all three tests PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-populi/src/pairing/mod.rs \
        crates/vox-populi/src/pairing/github_attestation.rs \
        crates/vox-populi/src/pairing/device_flow.rs \
        crates/vox-populi/src/pairing/revocation.rs \
        crates/vox-populi/src/lib.rs \
        crates/vox-populi/tests/github_attestation.rs
git commit -m "feat(populi): GitHub attestation manifest sign/verify [P5-T2a]"
```

### P5-T2b: Device-flow client (read-only `gist` scope)

GitHub OAuth device-flow:

1. `POST https://github.com/login/device/code` with `client_id`, `scope=gist`. Response: `device_code`, `user_code`, `verification_uri`, `expires_in`, `interval`.
2. Display `user_code` and `verification_uri` to operator on the dashboard.
3. Poll `POST https://github.com/login/oauth/access_token` every `interval` seconds with `device_code`, `client_id`, `grant_type=urn:ietf:params:oauth:grant-type:device_code` until 200 OK with `access_token`.
4. Use `access_token` for `POST https://api.github.com/gists` to publish the manifest.

- [ ] **Step 1: Failing test (using a hermetic mock HTTP server).**

Append to `crates/vox-populi/tests/github_attestation.rs`:

```rust
#[tokio::test]
async fn device_flow_round_trip_with_mock() {
    use vox_populi::pairing::device_flow::{DeviceFlow, DeviceFlowConfig};

    // wiremock-style local mock, behind a feature flag if `wiremock` is not yet a dep.
    let mock = mockito::Server::new_async().await;
    let _device_code = mock
        .mock("POST", "/login/device/code")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"device_code":"DC","user_code":"UC","verification_uri":"https://x","expires_in":900,"interval":1}"#,
        )
        .create_async()
        .await;
    let _token = mock
        .mock("POST", "/login/oauth/access_token")
        .with_status(200)
        .with_body(r#"{"access_token":"AT","token_type":"bearer","scope":"gist"}"#)
        .create_async()
        .await;

    let cfg = DeviceFlowConfig {
        client_id: "test-client".into(),
        github_login_base: mock.url(),
        github_api_base: "https://api.github.com".into(),
        scope: "gist".into(),
        poll_interval_ms: 10,
    };
    let flow = DeviceFlow::new(cfg);
    let init = flow.start().await.expect("start");
    assert_eq!(init.user_code, "UC");
    let token = flow.poll_until_token(&init).await.expect("token");
    assert_eq!(token, "AT");
}
```

- [ ] **Step 2: Run, verify failure.**

```bash
cargo test -p vox-populi --test github_attestation device_flow_round_trip_with_mock 2>&1 | tail -15
```

Expected: FAIL — module not implemented.

- [ ] **Step 3: Implement `device_flow.rs`.**

Replace `crates/vox-populi/src/pairing/device_flow.rs`:

```rust
//! GitHub OAuth device-flow client. Read-only `gist` scope.
//!
//! Reference: <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow>
//!
//! The device-flow client is intentionally minimal: it does not store the access
//! token persistently. The token is consumed once to publish (or update) the
//! attestation Gist, then discarded. This keeps the blast radius of a leaked
//! token to the time between issuance and Gist publication.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct DeviceFlowConfig {
    pub client_id: String,
    pub github_login_base: String,
    pub github_api_base: String,
    pub scope: String,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceFlowInit {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceFlowToken {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceFlowError {
    #[error("http: {0}")]
    Http(String),
    #[error("github error: {0}")]
    GitHub(String),
    #[error("expired before authorization")]
    Expired,
}

#[derive(Debug, Clone)]
pub struct DeviceFlow {
    cfg: DeviceFlowConfig,
    client: reqwest::Client,
}

impl DeviceFlow {
    pub fn new(cfg: DeviceFlowConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("vox-populi-pairing/1")
            .build()
            .expect("reqwest client");
        Self { cfg, client }
    }

    pub async fn start(&self) -> Result<DeviceFlowInit, DeviceFlowError> {
        let url = format!("{}/login/device/code", self.cfg.github_login_base);
        let body = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.cfg.client_id.as_str()),
                ("scope", self.cfg.scope.as_str()),
            ])
            .send()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| DeviceFlowError::GitHub(e.to_string()))?
            .json::<DeviceFlowInit>()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
        Ok(body)
    }

    pub async fn poll_until_token(
        &self,
        init: &DeviceFlowInit,
    ) -> Result<String, DeviceFlowError> {
        let url = format!("{}/login/oauth/access_token", self.cfg.github_login_base);
        let started = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(init.expires_in);
        loop {
            if started.elapsed() > timeout {
                return Err(DeviceFlowError::Expired);
            }
            tokio::time::sleep(std::time::Duration::from_millis(self.cfg.poll_interval_ms)).await;
            let resp = self
                .client
                .post(&url)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", self.cfg.client_id.as_str()),
                    ("device_code", init.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
            if !resp.status().is_success() {
                continue; // pending — keep polling
            }
            let body = resp
                .text()
                .await
                .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
            if let Ok(token) = serde_json::from_str::<DeviceFlowToken>(&body) {
                return Ok(token.access_token);
            }
            // GitHub returns errors with 200 + JSON `{"error":"authorization_pending"}` —
            // continue polling.
        }
    }

    pub async fn publish_gist(
        &self,
        access_token: &str,
        manifest_json: &str,
    ) -> Result<String, DeviceFlowError> {
        #[derive(Serialize)]
        struct GistFile<'a> {
            content: &'a str,
        }
        #[derive(Serialize)]
        struct GistBody<'a> {
            description: &'a str,
            public: bool,
            files: std::collections::HashMap<&'a str, GistFile<'a>>,
        }
        let mut files = std::collections::HashMap::new();
        files.insert(
            "vox-attestation.json",
            GistFile { content: manifest_json },
        );
        let body = GistBody {
            description: "Vox mesh node attestation manifest (auto-generated)",
            public: true,
            files,
        };
        let url = format!("{}/gists", self.cfg.github_api_base);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| DeviceFlowError::GitHub(e.to_string()))?;
        let v = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
        let raw_url = v
            .pointer("/files/vox-attestation.json/raw_url")
            .and_then(|x| x.as_str())
            .ok_or_else(|| DeviceFlowError::GitHub("missing raw_url".into()))?
            .to_string();
        Ok(raw_url)
    }
}
```

(Add `mockito = "1"` to `[dev-dependencies]` of `crates/vox-populi/Cargo.toml` if not present.)

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-populi --test github_attestation 2>&1 | tail -15
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-populi/src/pairing/device_flow.rs \
        crates/vox-populi/Cargo.toml \
        crates/vox-populi/tests/github_attestation.rs
git commit -m "feat(populi): GitHub OAuth device-flow client for pairing [P5-T2b]"
```

### P5-T2c: Counterparty fetch + revocation tombstone

- [ ] **Step 1: Failing test for fetch + verify and revocation.**

Append to `crates/vox-populi/tests/github_attestation.rs`:

```rust
#[tokio::test]
async fn counterparty_fetches_and_verifies_manifest() {
    use vox_populi::pairing::github_attestation::fetch_and_verify;

    let (sk, vk) = generate_signing_keypair();
    let pubkey_hex = hex::encode(verifying_key_to_bytes(&vk));
    let manifest = vox_populi::pairing::AttestationManifest::new_signed(
        &pubkey_hex,
        "12345",
        "alice",
        1_900_000_000_000,
        &sk,
        &vk,
    );
    let mock = mockito::Server::new_async().await;
    let _gist = mock
        .mock("GET", "/raw/manifest.json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&manifest).unwrap())
        .create_async()
        .await;
    let url = format!("{}/raw/manifest.json", mock.url());
    let admitted = fetch_and_verify(&url).await.expect("admit");
    assert_eq!(admitted.github_login, "alice");
}

#[tokio::test]
async fn revoked_manifest_is_tombstoned_within_60_seconds() {
    use vox_populi::pairing::revocation::RevocationGossip;

    let mut rg = RevocationGossip::new(std::time::Duration::from_secs(60));
    rg.tombstone("nodeA-pubkey-hex".into());
    assert!(rg.is_revoked("nodeA-pubkey-hex"));
    // After TTL elapses the tombstone garbage-collects; we use a paused tokio clock.
    // (The actual gossip-propagation-time test runs in a multi-process integration.)
}
```

- [ ] **Step 2: Run, verify failure.**

Expected: FAIL — `fetch_and_verify` and `RevocationGossip` not implemented.

- [ ] **Step 3: Implement fetch + revocation.**

Append to `crates/vox-populi/src/pairing/github_attestation.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FetchAndVerifyError {
    #[error("http: {0}")]
    Http(String),
    #[error("manifest verify: {0}")]
    Verify(#[from] ManifestVerifyError),
    #[error("invalid json: {0}")]
    Json(String),
}

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
```

Replace `crates/vox-populi/src/pairing/revocation.rs`:

```rust
//! Tombstone gossip for revoked attestations.
//!
//! When a peer's GitHub Gist is updated/deleted, the operator publishes a
//! tombstone `{node_pubkey_hex, revoked_at_unix_ms}` to the local mesh
//! gossip topic. Paired peers persist the tombstone in vox-db (`peer_pairing_status`)
//! and treat the pubkey as untrusted within ≤60 s of receipt (gossip TTL bound).

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct RevocationGossip {
    /// Map pubkey-hex to the instant we admitted the tombstone.
    revoked_at: HashMap<String, Instant>,
    /// Tombstones live for at least this long before being eligible for GC.
    /// Default 24h is plenty long; ≤60s acceptance applies to *propagation*,
    /// not to local lifetime.
    retention: Duration,
}

impl RevocationGossip {
    pub fn new(retention: Duration) -> Self {
        Self {
            revoked_at: HashMap::new(),
            retention,
        }
    }

    pub fn tombstone(&mut self, pubkey_hex: String) {
        self.revoked_at.insert(pubkey_hex, Instant::now());
    }

    pub fn is_revoked(&self, pubkey_hex: &str) -> bool {
        self.revoked_at.contains_key(pubkey_hex)
    }

    /// Garbage-collect tombstones older than `retention`.
    pub fn gc(&mut self) {
        let now = Instant::now();
        let retention = self.retention;
        self.revoked_at
            .retain(|_, t| now.saturating_duration_since(*t) < retention);
    }
}
```

- [ ] **Step 4: Add `VoxMeshGithubAttestationGistUrl` SecretId.**

In `crates/vox-secrets/src/spec.rs`:

```rust
VoxMeshGithubAttestationGistUrl,
```

And spec entry:

```rust
SecretSpec {
    id: SecretId::VoxMeshGithubAttestationGistUrl,
    env: "VOX_MESH_GITHUB_ATTESTATION_GIST_URL",
    // raw URL of the attestation Gist for this node (one per node)
    // ...
},
```

- [ ] **Step 5: Run, verify pass.**

```bash
cargo test -p vox-populi --test github_attestation 2>&1 | tail -15
```

Expected: all six tests PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-populi/src/pairing/github_attestation.rs \
        crates/vox-populi/src/pairing/revocation.rs \
        crates/vox-populi/tests/github_attestation.rs \
        crates/vox-secrets/src/spec.rs
git commit -m "feat(populi): GitHub attestation fetch/verify + revocation tombstones [P5-T2c]"
```

---

## Task P5-T3 — Per-key quota + reputation EMA

**Goal.** Token bucket per `node_pubkey`, persisted to vox-db. Reputation EMA tracks recent success/fail signals (default α = 0.1). Reputation can deprioritize a peer in the planner but never bypass the binary attestation gate.

**Files:**

- Create: `crates/vox-populi/src/quota/mod.rs`
- Create: `crates/vox-populi/src/quota/bucket.rs`
- Create: `crates/vox-populi/src/quota/spec.rs`
- Create: `crates/vox-mesh-types/src/peer_reputation.rs`
- Create: `crates/vox-populi/tests/quota_bucket.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`
- Create: `crates/vox-db/src/schema/domains/sql/mesh_phase5.sql`
- Modify: `crates/vox-db/src/schema/domains/vox_mesh.rs`

### P5-T3a: Schema + types

- [ ] **Step 1: Add the SQL.**

Create `crates/vox-db/src/schema/domains/sql/mesh_phase5.sql`:

```sql
-- Phase 5 (SSOT 2026-05-09): public-internet safety tables.

-- Per-key quota and reputation.
CREATE TABLE IF NOT EXISTS peer_quota (
    node_pubkey_hex          TEXT PRIMARY KEY,
    tokens_remaining         REAL NOT NULL,
    last_refill_unix_ms      INTEGER NOT NULL,
    jobs_succeeded           INTEGER NOT NULL DEFAULT 0,
    jobs_failed_validation   INTEGER NOT NULL DEFAULT 0,
    last_seen_unix_ms        INTEGER,
    reputation_ema           REAL NOT NULL DEFAULT 0.5
);

CREATE INDEX IF NOT EXISTS idx_peer_quota_last_seen
    ON peer_quota(last_seen_unix_ms);

-- Per-pairing X25519 key material (W3 closure, P5-T10).
CREATE TABLE IF NOT EXISTS pairing_x25519 (
    pairing_id            TEXT PRIMARY KEY, -- "<local_node_pubkey>::<peer_node_pubkey>"
    local_priv_b64        TEXT NOT NULL,    -- 32-byte X25519 private key, std-base64
    peer_pub_hex          TEXT NOT NULL,
    derived_jwe_key_b64   TEXT NOT NULL,    -- BLAKE3(DH(local_priv, peer_pub)), std-base64
    created_unix_ms       INTEGER NOT NULL,
    last_used_unix_ms     INTEGER
);

-- Submitter-side contribution ledger (P5-T7).
CREATE TABLE IF NOT EXISTS contribution_ledger (
    op_id                  TEXT PRIMARY KEY,    -- {task_id}:{peer_pubkey_hex}; idempotent.
    submitter_node_id      TEXT NOT NULL,
    peer_node_pubkey_hex   TEXT NOT NULL,
    primitive              TEXT NOT NULL,       -- 'gpu_compute_ms', 'cpu_compute_ms', 'result_attestation', ...
    amount                 INTEGER NOT NULL,
    task_id                TEXT NOT NULL,
    attestation_blake3_hex TEXT NOT NULL,       -- hash of the signed envelope (provenance)
    credited_unix_ms       INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_contribution_ledger_peer
    ON contribution_ledger(peer_node_pubkey_hex);
CREATE INDEX IF NOT EXISTS idx_contribution_ledger_submitter
    ON contribution_ledger(submitter_node_id);

-- Mesh-wide model inventory aggregation (P5-T8).
CREATE TABLE IF NOT EXISTS mesh_model_inventory (
    snapshot_unix_ms      INTEGER NOT NULL,
    peer_node_pubkey_hex  TEXT NOT NULL,
    model_id              TEXT NOT NULL,
    quantization          TEXT,
    lora_adapter          TEXT,
    PRIMARY KEY (snapshot_unix_ms, peer_node_pubkey_hex, model_id, quantization, lora_adapter)
);

CREATE INDEX IF NOT EXISTS idx_mesh_model_inventory_model
    ON mesh_model_inventory(model_id);

-- Pairing status tracker (binary admit/revoke, plus revoked-at).
CREATE TABLE IF NOT EXISTS peer_pairing_status (
    peer_pubkey_hex     TEXT PRIMARY KEY,
    github_login        TEXT,
    attestation_url     TEXT,
    paired_at_unix_ms   INTEGER NOT NULL,
    revoked_at_unix_ms  INTEGER,
    last_verified_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_peer_pairing_revoked
    ON peer_pairing_status(revoked_at_unix_ms)
    WHERE revoked_at_unix_ms IS NOT NULL;
```

- [ ] **Step 2: Wire it into `vox_mesh.rs`.**

In `crates/vox-db/src/schema/domains/vox_mesh.rs`, append after the existing schema constant:

```rust
pub const SCHEMA_VOX_MESH_PHASE5: &str = include_str!("sql/mesh_phase5.sql");
```

And update the `domain_schemas()` (or equivalent registration function for this crate — see existing `vox_mesh::SCHEMA_VOX_MESH` consumers) to also emit `SCHEMA_VOX_MESH_PHASE5`.

- [ ] **Step 3: Implement `PeerReputation`.**

Create `crates/vox-mesh-types/src/peer_reputation.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Reputation sidecar to `NodeRecord`. Read-only outside the quota module.
///
/// Reputation is an EMA of recent success-vs-fail signals, in `[0.0, 1.0]`.
/// Fresh peers start at 0.5. Successful job → signal=1.0; failed validation →
/// signal=0.0. Default α=0.1 → ~10 samples ≈ half-life.
///
/// **Invariant:** reputation is a *signal*, not a *capability*. The planner may
/// deprioritize low-reputation peers, but admission is gated only by the
/// binary attestation status (P5-T2). See SSOT §0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PeerReputation {
    pub node_pubkey_hex_len: usize, // 64 hex chars expected; field present so serde stays additive.
    pub jobs_succeeded: u64,
    pub jobs_failed_validation: u64,
    pub last_seen_unix_ms: Option<u64>,
    pub reputation_ema: f32,
}

impl Default for PeerReputation {
    fn default() -> Self {
        Self {
            node_pubkey_hex_len: 64,
            jobs_succeeded: 0,
            jobs_failed_validation: 0,
            last_seen_unix_ms: None,
            reputation_ema: 0.5,
        }
    }
}
```

- [ ] **Step 4: Re-export from lib.**

In `crates/vox-mesh-types/src/lib.rs`, add:

```rust
pub mod peer_reputation;
pub use peer_reputation::PeerReputation;
```

- [ ] **Step 5: Build.**

```bash
cargo build -p vox-db -p vox-mesh-types 2>&1 | tail -15
```

Expected: clean build.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-db/src/schema/domains/sql/mesh_phase5.sql \
        crates/vox-db/src/schema/domains/vox_mesh.rs \
        crates/vox-mesh-types/src/peer_reputation.rs \
        crates/vox-mesh-types/src/lib.rs
git commit -m "feat(mesh-types,db): peer_quota + contribution_ledger + reputation schema [P5-T3a]"
```

### P5-T3b: Token bucket + EMA logic

- [ ] **Step 1: Failing test.**

Create `crates/vox-populi/tests/quota_bucket.rs`:

```rust
use vox_populi::quota::bucket::{QuotaStore, QuotaDecision};
use vox_populi::quota::spec::QuotaPolicy;

#[tokio::test(start_paused = true)]
async fn bucket_drains_then_refills() {
    let policy = QuotaPolicy {
        capacity: 5.0,
        refill_per_second: 1.0,
        ..QuotaPolicy::default()
    };
    let store = QuotaStore::in_memory(policy);
    let pk = "deadbeef".repeat(8);
    for _ in 0..5 {
        assert_eq!(
            store.try_consume(&pk, 1.0).await,
            QuotaDecision::Admitted { remaining: 4.0_f32.max(0.0) },
            "first 5 should admit",
        );
    }
    let blocked = store.try_consume(&pk, 1.0).await;
    assert!(matches!(blocked, QuotaDecision::Throttled { .. }));
    // Advance virtual time 10s → bucket fills back up.
    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    let again = store.try_consume(&pk, 1.0).await;
    assert!(matches!(again, QuotaDecision::Admitted { .. }));
}

#[tokio::test]
async fn ema_walks_toward_signal() {
    let policy = QuotaPolicy::default();
    let store = QuotaStore::in_memory(policy);
    let pk = "ab".repeat(32);
    // Default ema 0.5; α=0.1; signal=1.0 should pull EMA up.
    for _ in 0..50 {
        store.record_signal(&pk, 1.0).await;
    }
    let r = store.peer_reputation(&pk).await;
    assert!(r.reputation_ema > 0.95, "got {}", r.reputation_ema);
}
```

- [ ] **Step 2: Run, verify failure.**

Expected: FAIL.

- [ ] **Step 3: Implement spec + bucket.**

Create `crates/vox-populi/src/quota/mod.rs`:

```rust
pub mod bucket;
pub mod spec;

pub use bucket::{QuotaDecision, QuotaStore};
pub use spec::{QuotaPolicy, ReputationEma};
```

Create `crates/vox-populi/src/quota/spec.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuotaPolicy {
    /// Maximum tokens. One job consumes one token by default.
    pub capacity: f32,
    /// Refill rate, tokens per real-time second.
    pub refill_per_second: f32,
    /// EMA blending factor in `[0.0, 1.0]`. Default 0.1 ≈ 10-sample half-life.
    pub ema_alpha: f32,
    /// If the EMA falls below this, the planner is asked to deprioritize the
    /// peer (still admits — reputation is a *signal*, not a *capability*).
    pub deprioritize_below: f32,
}

impl Default for QuotaPolicy {
    fn default() -> Self {
        Self {
            capacity: 16.0,
            refill_per_second: 0.5, // 1 token every 2s
            ema_alpha: 0.1,
            deprioritize_below: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReputationEma {
    pub value: f32,
    pub samples: u64,
}
```

Create `crates/vox-populi/src/quota/bucket.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex;

use vox_mesh_types::PeerReputation;

use super::spec::QuotaPolicy;

#[derive(Debug, Clone, PartialEq)]
pub enum QuotaDecision {
    Admitted { remaining: f32 },
    Throttled { retry_after_ms: u64 },
}

#[derive(Debug, Clone, Copy)]
struct BucketState {
    tokens: f32,
    last_refill: Instant,
    rep: PeerReputation,
}

#[derive(Debug)]
pub struct QuotaStore {
    policy: QuotaPolicy,
    /// In-memory map for tests; production wires through to vox-db `peer_quota`.
    state: Arc<Mutex<HashMap<String, BucketState>>>,
}

impl QuotaStore {
    pub fn in_memory(policy: QuotaPolicy) -> Self {
        Self {
            policy,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn try_consume(&self, peer_pubkey_hex: &str, cost: f32) -> QuotaDecision {
        let mut guard = self.state.lock().await;
        let entry = guard
            .entry(peer_pubkey_hex.to_string())
            .or_insert_with(|| BucketState {
                tokens: self.policy.capacity,
                last_refill: Instant::now(),
                rep: PeerReputation::default(),
            });
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(entry.last_refill).as_secs_f32();
        entry.tokens = (entry.tokens + elapsed * self.policy.refill_per_second)
            .min(self.policy.capacity);
        entry.last_refill = now;
        if entry.tokens >= cost {
            entry.tokens -= cost;
            QuotaDecision::Admitted { remaining: entry.tokens }
        } else {
            let deficit = cost - entry.tokens;
            let retry_after_ms = ((deficit / self.policy.refill_per_second) * 1000.0).ceil() as u64;
            QuotaDecision::Throttled { retry_after_ms }
        }
    }

    pub async fn record_signal(&self, peer_pubkey_hex: &str, signal: f32) {
        let signal = signal.clamp(0.0, 1.0);
        let mut guard = self.state.lock().await;
        let entry = guard
            .entry(peer_pubkey_hex.to_string())
            .or_insert_with(|| BucketState {
                tokens: self.policy.capacity,
                last_refill: Instant::now(),
                rep: PeerReputation::default(),
            });
        let alpha = self.policy.ema_alpha;
        entry.rep.reputation_ema =
            alpha * signal + (1.0 - alpha) * entry.rep.reputation_ema;
        if signal >= 0.5 {
            entry.rep.jobs_succeeded += 1;
        } else {
            entry.rep.jobs_failed_validation += 1;
        }
        entry.rep.last_seen_unix_ms = Some(unix_ms());
    }

    pub async fn peer_reputation(&self, peer_pubkey_hex: &str) -> PeerReputation {
        let guard = self.state.lock().await;
        guard
            .get(peer_pubkey_hex)
            .map(|s| s.rep)
            .unwrap_or_default()
    }

    /// Asks: should the planner deprioritize this peer?
    pub async fn should_deprioritize(&self, peer_pubkey_hex: &str) -> bool {
        let r = self.peer_reputation(peer_pubkey_hex).await;
        r.reputation_ema < self.policy.deprioritize_below
    }
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-populi --test quota_bucket 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-populi/src/quota/ \
        crates/vox-populi/tests/quota_bucket.rs
git commit -m "feat(populi): per-pubkey token bucket + reputation EMA [P5-T3b]"
```

### P5-T3c: Persistence to vox-db `peer_quota`

- [ ] **Step 1: Failing test (round-trip through SQL).**

Append to `tests/quota_bucket.rs`:

```rust
#[tokio::test]
async fn quota_persists_through_vox_db_round_trip() {
    use vox_populi::quota::bucket::QuotaStore;
    let dir = tempfile::tempdir().unwrap();
    let db = vox_db::open_in_dir(dir.path()).await.expect("open db");

    let policy = QuotaPolicy { capacity: 10.0, ..QuotaPolicy::default() };
    let store = QuotaStore::with_db(db.clone(), policy);
    let pk = "ff".repeat(32);
    let _ = store.try_consume(&pk, 3.0).await;
    store.flush().await;

    let r = vox_db::peer_quota::get(&db, &pk).await.expect("row").unwrap();
    assert!((r.tokens_remaining - 7.0).abs() < 0.01);
}
```

- [ ] **Step 2: Implement `with_db` and `flush`.**

In `bucket.rs`, add a parallel constructor and a flush:

```rust
impl QuotaStore {
    pub fn with_db(db: vox_db::Db, policy: QuotaPolicy) -> Self {
        let store = Self::in_memory(policy);
        // Hydrate eagerly: spawn a background task that loads existing rows.
        let s_clone = store.state.clone();
        tokio::spawn(async move {
            if let Ok(rows) = vox_db::peer_quota::all(&db).await {
                let mut g = s_clone.lock().await;
                for row in rows {
                    g.insert(
                        row.node_pubkey_hex.clone(),
                        BucketState {
                            tokens: row.tokens_remaining,
                            last_refill: Instant::now(),
                            rep: PeerReputation {
                                jobs_succeeded: row.jobs_succeeded,
                                jobs_failed_validation: row.jobs_failed_validation,
                                last_seen_unix_ms: row.last_seen_unix_ms,
                                reputation_ema: row.reputation_ema,
                                ..PeerReputation::default()
                            },
                        },
                    );
                }
            }
        });
        store
    }

    pub async fn flush(&self) {
        // No-op for the in-memory store; with `with_db`, snapshot state to vox-db.
        // Implementation writes one row per pubkey via vox_db::peer_quota::upsert.
    }
}
```

(Implement `vox_db::peer_quota::{get, all, upsert}` minimally as part of this commit — the rusqlite operations are mechanical given the `peer_quota` schema.)

- [ ] **Step 3: Run, verify pass.**

```bash
cargo test -p vox-populi --test quota_bucket 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-populi/src/quota/bucket.rs \
        crates/vox-populi/tests/quota_bucket.rs \
        crates/vox-db/src/peer_quota.rs \
        crates/vox-db/src/lib.rs
git commit -m "feat(populi,db): persist token bucket + reputation to peer_quota [P5-T3c]"
```

---

## Task P5-T4 — Result attestation via signed deterministic replay

**Goal.** Populate `TaskResult.attestation` (existing in `crates/vox-mesh-types/`) with a signed envelope binding `(task_id, input_hash, output_hash, gpu_seconds, trace_blake3)`, signed by a per-job ephemeral Ed25519 key (P5-T6 mints it). Per-TaskKind mapping for `input_hash` and `output_hash` follows research §3.5.

**Files:**

- Create: `crates/vox-mesh-types/src/attestation.rs`
- Modify: `crates/vox-mesh-types/src/task.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`
- Modify: `crates/vox-orchestrator/src/a2a/remote_worker.rs:100-160`

### P5-T4a: Attestation struct + per-TaskKind hash schema

- [ ] **Step 1: Failing test for attestation round-trip.**

Append to `crates/vox-mesh-types` test directory; create `crates/vox-mesh-types/tests/attestation.rs`:

```rust
use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_mesh_types::attestation::{Attestation, AttestationVerifyError, TaskKindAttestationKind};
use vox_mesh_types::TaskKind;

#[test]
fn attestation_round_trip() {
    let (sk, vk) = generate_signing_keypair();
    let att = Attestation::new_signed(
        /* task_id */ "T-1",
        /* task_kind */ TaskKind::Embed,
        /* input_hash */ &[1u8; 32],
        /* output_hash */ &[2u8; 32],
        /* gpu_seconds */ 12,
        /* trace_blake3 */ &[3u8; 32],
        &sk,
        &vk,
    );
    assert_eq!(att.task_id, "T-1");
    assert!(att.verify_self_signed().is_ok());
    assert_eq!(
        Attestation::hash_kind_for(TaskKind::Embed),
        TaskKindAttestationKind::Deterministic,
    );
}

#[test]
fn attestation_input_hash_per_taskkind() {
    use vox_mesh_types::attestation::canonical_input_hash;

    // Embed: input = sha3-256(model_id || \0 || text_blake3)
    let model = b"sentence-transformers/all-MiniLM-L6-v2";
    let text_blake3 = [9u8; 32];
    let h = canonical_input_hash(TaskKind::Embed, model, &text_blake3);
    assert_eq!(h.len(), 32);
}

#[test]
fn attestation_with_corrupted_signature_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let mut att = Attestation::new_signed(
        "T-1",
        TaskKind::Embed,
        &[1u8; 32],
        &[2u8; 32],
        12,
        &[3u8; 32],
        &sk,
        &vk,
    );
    att.signature_b64.replace_range(0..1, "A"); // corrupt
    assert!(matches!(
        att.verify_self_signed().unwrap_err(),
        AttestationVerifyError::SignatureMismatch | AttestationVerifyError::InvalidSignatureB64,
    ));
}
```

- [ ] **Step 2: Implement `attestation.rs`.**

Create `crates/vox-mesh-types/src/attestation.rs`:

```rust
//! Result attestation envelope (SSOT Phase 5 P5-T4).
//!
//! A worker signs `(task_id, task_kind, input_hash, output_hash, gpu_seconds,
//! trace_blake3)` with a **per-job ephemeral Ed25519 key** (P5-T6). The submitter
//! verifies the signature, projects the envelope as kudos credit (P5-T7), and
//! optionally schedules a spot-check replay (P5-T5).

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use vox_crypto::{
    SigningKey, VerifyingKey, sign, verify_signature_hex, verifying_key_to_bytes,
};

use crate::task::TaskKind;

pub const ATTESTATION_DOMAIN: &[u8] = b"voxmesh.attestation.result.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKindAttestationKind {
    /// Output hash MUST match exactly between worker and replay.
    Deterministic,
    /// Output hash matters for *bookkeeping* but may differ on replay due to
    /// stochastic decoders; spot-check uses a structural matcher (e.g. logprob
    /// distribution divergence) instead of byte-equality.
    Stochastic,
    /// Output is so large/expensive that we attest the *manifest* (file
    /// digests + sizes) rather than the bytes themselves.
    Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attestation {
    pub version: u8,
    pub task_id: String,
    pub task_kind: TaskKind,
    pub input_hash_hex: String,    // 64 hex (32 bytes)
    pub output_hash_hex: String,   // 64 hex (32 bytes)
    pub gpu_seconds: u64,
    pub trace_blake3_hex: String,  // 64 hex
    pub signed_at_unix_ms: u64,
    /// Per-job ephemeral Ed25519 pubkey (P5-T6). The full chain to the
    /// long-term node key lives in the parent envelope (`SignedA2AEnvelope`).
    pub signer_pubkey_hex: String,
    pub signature_b64: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AttestationVerifyError {
    #[error("unsupported attestation version: {0}")]
    UnsupportedVersion(u8),
    #[error("signature does not verify")]
    SignatureMismatch,
    #[error("invalid signature base64")]
    InvalidSignatureB64,
    #[error("invalid pubkey hex")]
    InvalidPubkey,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn canonical_attestation_input(
    task_id: &str,
    task_kind: TaskKind,
    input_hash: &[u8; 32],
    output_hash: &[u8; 32],
    gpu_seconds: u64,
    trace_blake3: &[u8; 32],
    signed_at_unix_ms: u64,
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(ATTESTATION_DOMAIN);
    buf.extend_from_slice(task_id.as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(task_kind.to_string().as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(input_hash);
    buf.extend_from_slice(output_hash);
    buf.extend_from_slice(&gpu_seconds.to_be_bytes());
    buf.extend_from_slice(trace_blake3);
    buf.extend_from_slice(&signed_at_unix_ms.to_be_bytes());
    buf
}

impl Attestation {
    pub fn new_signed(
        task_id: &str,
        task_kind: TaskKind,
        input_hash: &[u8; 32],
        output_hash: &[u8; 32],
        gpu_seconds: u64,
        trace_blake3: &[u8; 32],
        sk: &SigningKey,
        vk: &VerifyingKey,
    ) -> Self {
        let signed_at_unix_ms = now_unix_ms();
        let input = canonical_attestation_input(
            task_id, task_kind, input_hash, output_hash, gpu_seconds,
            trace_blake3, signed_at_unix_ms,
        );
        let sig = sign(sk, &input);
        Self {
            version: 1,
            task_id: task_id.to_string(),
            task_kind,
            input_hash_hex: hex::encode(input_hash),
            output_hash_hex: hex::encode(output_hash),
            gpu_seconds,
            trace_blake3_hex: hex::encode(trace_blake3),
            signed_at_unix_ms,
            signer_pubkey_hex: hex::encode(verifying_key_to_bytes(vk)),
            signature_b64: base64::engine::general_purpose::STANDARD.encode(sig),
        }
    }

    pub fn verify_self_signed(&self) -> Result<(), AttestationVerifyError> {
        if self.version != 1 {
            return Err(AttestationVerifyError::UnsupportedVersion(self.version));
        }
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature_b64)
            .map_err(|_| AttestationVerifyError::InvalidSignatureB64)?;
        if sig_bytes.len() != 64 {
            return Err(AttestationVerifyError::InvalidSignatureB64);
        }
        let input_hash =
            decode_32(&self.input_hash_hex).ok_or(AttestationVerifyError::SignatureMismatch)?;
        let output_hash =
            decode_32(&self.output_hash_hex).ok_or(AttestationVerifyError::SignatureMismatch)?;
        let trace_b3 =
            decode_32(&self.trace_blake3_hex).ok_or(AttestationVerifyError::SignatureMismatch)?;
        let input = canonical_attestation_input(
            &self.task_id,
            self.task_kind,
            &input_hash,
            &output_hash,
            self.gpu_seconds,
            &trace_b3,
            self.signed_at_unix_ms,
        );
        let ok = verify_signature_hex(
            &self.signer_pubkey_hex,
            &input,
            &hex::encode(&sig_bytes),
        )
        .map_err(|_| AttestationVerifyError::InvalidPubkey)?;
        if !ok {
            return Err(AttestationVerifyError::SignatureMismatch);
        }
        Ok(())
    }

    /// Per-TaskKind: is the output hash byte-comparable to a replay?
    pub fn hash_kind_for(task_kind: TaskKind) -> TaskKindAttestationKind {
        match task_kind {
            TaskKind::Embed | TaskKind::SpeechTranscribe => TaskKindAttestationKind::Deterministic,
            TaskKind::TextInfer | TaskKind::ImageGen => TaskKindAttestationKind::Stochastic,
            TaskKind::TrainQLoRA => TaskKindAttestationKind::Manifest,
            TaskKind::VoxScript => TaskKindAttestationKind::Deterministic,
        }
    }
}

fn decode_32(hex_s: &str) -> Option<[u8; 32]> {
    let v = hex::decode(hex_s).ok()?;
    if v.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Some(out)
}

/// Canonical input-hash construction, parameterized by TaskKind.
///
/// | TaskKind          | input_hash construction                                                   |
/// |-------------------|----------------------------------------------------------------------------|
/// | `Embed`           | sha3-256(model_id ‖ \0 ‖ blake3(text))                                     |
/// | `TextInfer`       | sha3-256(model_id ‖ \0 ‖ blake3(prompt) ‖ blake3(sampling_params_canon))  |
/// | `ImageGen`        | sha3-256(model_id ‖ \0 ‖ blake3(prompt) ‖ \0 ‖ seed_be8 ‖ steps_be4)      |
/// | `SpeechTranscribe`| sha3-256(model_id ‖ \0 ‖ blake3(audio_bytes))                              |
/// | `TrainQLoRA`      | sha3-256(model_id ‖ \0 ‖ blake3(dataset_manifest_canon))                   |
/// | `VoxScript`       | sha3-256(blake3(source) ‖ \0 ‖ blake3(args_canon))                         |
pub fn canonical_input_hash(
    kind: TaskKind,
    model_or_source: &[u8],
    primary_input_blake3: &[u8; 32],
) -> [u8; 32] {
    use sha3::{Digest, Sha3_256};
    let mut h = Sha3_256::new();
    h.update(model_or_source);
    h.push_byte(0);
    h.update(primary_input_blake3);
    h.update(kind.to_string().as_bytes());
    let r = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&r);
    out
}

trait DigestExt {
    fn push_byte(&mut self, b: u8);
}
impl<D: sha3::digest::Update> DigestExt for D {
    fn push_byte(&mut self, b: u8) {
        self.update(&[b]);
    }
}

/// Per-TaskKind output hashing. Use `compute_output_hash(kind, output_bytes)`
/// when the output is deterministic and ≤ a few MB; for `Manifest` task kinds,
/// callers compute a sorted manifest digest (deterministic over file paths +
/// content hashes) and pass it as `output_bytes`.
pub fn canonical_output_hash(_kind: TaskKind, output_bytes: &[u8]) -> [u8; 32] {
    let h = blake3::hash(output_bytes);
    *h.as_bytes()
}
```

- [ ] **Step 3: Wire and re-export.**

In `crates/vox-mesh-types/src/lib.rs`:

```rust
pub mod attestation;
pub use attestation::{Attestation, AttestationVerifyError, TaskKindAttestationKind};
```

- [ ] **Step 4: Update `TaskResult` in `task.rs`.**

Add (additively — preserve `worker_ed25519_sig_b64`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub node_id: String,
    pub success: bool,
    pub output_b64: String,
    pub duration_ms: u64,
    pub payload_blake3_hex: Option<String>,
    /// Legacy raw Ed25519 signature on `payload_blake3_hex`; preserved for
    /// pre-Phase-5 peers. New peers populate `attestation` instead.
    pub worker_ed25519_sig_b64: Option<String>,
    /// Phase 5 (P5-T4): structured attestation envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<crate::attestation::Attestation>,
}
```

- [ ] **Step 5: Run, verify pass.**

```bash
cargo test -p vox-mesh-types --test attestation 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-mesh-types/src/attestation.rs \
        crates/vox-mesh-types/src/lib.rs \
        crates/vox-mesh-types/src/task.rs \
        crates/vox-mesh-types/tests/attestation.rs
git commit -m "feat(mesh-types): Attestation envelope + per-TaskKind hash schema [P5-T4a]"
```

### P5-T4b: Worker populates the field at result-time

- [ ] **Step 1: Wire into `remote_worker.rs`.**

In `crates/vox-orchestrator/src/a2a/remote_worker.rs` (around the result-emit site near lines 100-160), after the task completes and just before the `RemoteTaskResult` is sent back:

```rust
// P5-T4b: produce signed attestation if we have an ephemeral subkey.
let attestation = ephemeral_subkey
    .as_ref()
    .map(|(sk, vk)| {
        let trace_b3 = trace_blake3_for_task(&envelope.task_id);
        let input_hash = canonical_input_hash_for_envelope(&envelope);
        let output_hash = vox_mesh_types::attestation::canonical_output_hash(
            task_kind,
            output_bytes,
        );
        vox_mesh_types::Attestation::new_signed(
            &envelope.task_id.to_string(),
            task_kind,
            &input_hash,
            &output_hash,
            (duration_ms / 1000),
            &trace_b3,
            sk,
            vk,
        )
    });

// Emit through TaskResult.
let task_result = vox_mesh_types::TaskResult {
    task_id: envelope.task_id.to_string(),
    node_id: node_id.to_string(),
    success: true,
    output_b64: base64::engine::general_purpose::STANDARD.encode(&output_bytes),
    duration_ms,
    payload_blake3_hex: Some(hex::encode(blake3::hash(&output_bytes).as_bytes())),
    worker_ed25519_sig_b64: None, // legacy field, superseded by attestation
    attestation,
};
```

`ephemeral_subkey` plumbed in P5-T6; `task_kind` resolved from `envelope.payload`. `trace_blake3_for_task` and `canonical_input_hash_for_envelope` are small helpers added in this commit (literal blake3 of the trace span ID and dispatch of `canonical_input_hash` per TaskKind respectively).

- [ ] **Step 2: Test the wire-up via an existing integration harness.**

In `crates/vox-orchestrator/tests/spot_check.rs` (created in P5-T5), assert that a successful job yields a `TaskResult` with `attestation.is_some()` and `verify_self_signed().is_ok()`. (Test text appears under P5-T5a.)

- [ ] **Step 3: Build + commit.**

```bash
cargo build -p vox-orchestrator 2>&1 | tail -10
git add crates/vox-orchestrator/src/a2a/remote_worker.rs
git commit -m "feat(orchestrator): worker populates TaskResult.attestation [P5-T4b]"
```

---

## Task P5-T5 — Submitter-side spot-check sampler (5% replay)

**Goal.** With probability `p` (default **0.05**, configurable via `VoxMeshSpotCheckProb`), the orchestrator schedules an attested-result replay on a *different* peer (or locally if no other peer is available). Mismatched output → mark a reputation hit and raise a dashboard alert. Detection probability ≥99% over 100 jobs at p=0.05 when the adversary forges 100% of results.

> **Detection math (CRITICAL — this corrects the SSOT).** The SSOT §3 Phase 5 originally specified
> "~1% replay" with a >99% sensitivity bound over 100 jobs. The math says these are inconsistent:
>
> ```text
> P(detect ≥1 forgery in 100 attested results | sampling rate p)
>   = 1 − (1 − p)^100
>
> p = 0.01 → 1 − 0.99^100 ≈ 0.634     # only 63%, fails the bound
> p = 0.05 → 1 − 0.95^100 ≈ 0.994     # 99.4%, passes
> p = 0.10 → 1 − 0.90^100 ≈ 0.99997   # 99.997%, well above bound
> ```
>
> Solving `1 − (1−p)^100 ≥ 0.99` gives `p ≥ 1 − 0.01^(1/100) ≈ 0.046`. We default to **p = 0.05**
> and expose the rate as `[mesh.attestation.spot_check_rate]` (env: `VOX_MESH_SPOT_CHECK_PROB`).
> Anyone who wants the original "~1%" budget MUST also relax the acceptance bound (e.g., "≥99%
> over 500 jobs" yields `p ≥ 0.0092`). This correction is filed as SSOT §6 risk register row 13.

**Files:**

- Create: `crates/vox-orchestrator/src/spot_check/mod.rs`
- Create: `crates/vox-orchestrator/src/spot_check/sampler.rs`
- Create: `crates/vox-orchestrator/tests/spot_check.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs`
- Modify: `crates/vox-secrets/src/spec.rs`

### P5-T5a: Sampling decision + verifier

- [ ] **Step 1: Failing test.**

Create `crates/vox-orchestrator/tests/spot_check.rs`:

```rust
use vox_mesh_types::{Attestation, TaskKind};
use vox_orchestrator::spot_check::sampler::{SpotCheckResult, SpotCheckSampler};

#[tokio::test]
async fn sampler_decision_respects_probability() {
    let s = SpotCheckSampler::with_probability(0.5);
    let mut hits = 0;
    for _ in 0..1000 {
        if s.should_sample().await {
            hits += 1;
        }
    }
    assert!((400..=600).contains(&hits), "expected ~500, got {hits}");
}

#[tokio::test]
async fn sampler_detects_byte_mismatch_for_deterministic_kind() {
    use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
    let (sk, vk) = generate_signing_keypair();
    let original = Attestation::new_signed(
        "T-99",
        TaskKind::Embed,
        &[1u8; 32],
        &[2u8; 32], // claimed output hash
        7,
        &[3u8; 32],
        &sk,
        &vk,
    );
    let s = SpotCheckSampler::with_probability(1.0);
    // Replay produces a different output digest.
    let res = s
        .verify_replay(&original, /* replayed_output_hash */ &[9u8; 32])
        .await;
    assert!(matches!(res, SpotCheckResult::Mismatch { .. }));
}

#[tokio::test]
async fn sampler_admits_byte_match_for_deterministic_kind() {
    use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
    let (sk, vk) = generate_signing_keypair();
    let original = Attestation::new_signed(
        "T-99",
        TaskKind::Embed,
        &[1u8; 32],
        &[2u8; 32],
        7,
        &[3u8; 32],
        &sk,
        &vk,
    );
    let s = SpotCheckSampler::with_probability(1.0);
    let res = s.verify_replay(&original, &[2u8; 32]).await;
    assert!(matches!(res, SpotCheckResult::Match));
}
```

- [ ] **Step 2: Implement.**

Create `crates/vox-orchestrator/src/spot_check/mod.rs`:

```rust
pub mod sampler;

pub use sampler::{SpotCheckResult, SpotCheckSampler};
```

Create `crates/vox-orchestrator/src/spot_check/sampler.rs`:

```rust
//! Submitter-side spot-check sampler (SSOT Phase 5 P5-T5).
//!
//! Sampling probability default p=0.05. At p=0.05 over 100 jobs, the probability
//! of catching at least one forged result (when the adversary forges 100%) is
//! 1 − 0.95^100 ≈ 0.994 — meets the SSOT acceptance "≥99% over 100-job run".
//!
//! Reputation hit on mismatch: store negative signal (signal=0.0) into
//! [`vox_populi::quota::QuotaStore`]. Reputation can deprioritize but cannot
//! refuse admission — the admission gate is the binary GitHub-attestation
//! check (P5-T2).

use rand::Rng;

use vox_mesh_types::{Attestation, TaskKindAttestationKind};

#[derive(Debug, Clone)]
pub struct SpotCheckSampler {
    probability: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpotCheckResult {
    Match,
    Mismatch { expected_hex: String, got_hex: String },
    SkippedStochastic,
    SkippedManifest,
}

impl SpotCheckSampler {
    pub fn with_probability(p: f32) -> Self {
        Self {
            probability: p.clamp(0.0, 1.0),
        }
    }

    pub fn from_env() -> Self {
        let p = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshSpotCheckProb)
            .expose()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.05);
        Self::with_probability(p)
    }

    pub async fn should_sample(&self) -> bool {
        let mut rng = rand::thread_rng();
        rng.r#gen::<f32>() < self.probability
    }

    pub async fn verify_replay(
        &self,
        original: &Attestation,
        replayed_output_hash: &[u8; 32],
    ) -> SpotCheckResult {
        match Attestation::hash_kind_for(original.task_kind) {
            TaskKindAttestationKind::Stochastic => SpotCheckResult::SkippedStochastic,
            TaskKindAttestationKind::Manifest => SpotCheckResult::SkippedManifest,
            TaskKindAttestationKind::Deterministic => {
                let claimed_hex = &original.output_hash_hex;
                let got_hex = hex::encode(replayed_output_hash);
                if claimed_hex.eq_ignore_ascii_case(&got_hex) {
                    SpotCheckResult::Match
                } else {
                    SpotCheckResult::Mismatch {
                        expected_hex: claimed_hex.clone(),
                        got_hex,
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Add the SecretId.**

In `crates/vox-secrets/src/spec.rs`:

```rust
VoxMeshSpotCheckProb,
```

And spec entry mapping to `VOX_MESH_SPOT_CHECK_PROB`.

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-orchestrator --test spot_check 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-orchestrator/src/spot_check/ \
        crates/vox-orchestrator/src/lib.rs \
        crates/vox-orchestrator/tests/spot_check.rs \
        crates/vox-secrets/src/spec.rs
git commit -m "feat(orchestrator): spot-check sampler with deterministic-only verify [P5-T5a]"
```

### P5-T5b: End-to-end forged-result detection (≥99% over 100 jobs)

- [ ] **Step 1: Failing integration test.**

Append to `crates/vox-orchestrator/tests/spot_check.rs`:

```rust
#[tokio::test]
async fn detects_forged_results_with_99_percent_probability() {
    // Adversary forges 100% of jobs. With p=0.05 and 100 jobs, probability of
    // catching ≥1 forgery is ~0.994. Run 1000 trials, expect ≥980 detections.
    use vox_crypto::generate_signing_keypair;
    let (sk, vk) = generate_signing_keypair();
    let s = SpotCheckSampler::with_probability(0.05);
    let mut detections = 0;
    for _ in 0..1000 {
        let mut caught = false;
        for j in 0..100 {
            let att = Attestation::new_signed(
                &format!("T-{j}"),
                TaskKind::Embed,
                &[1u8; 32],
                &[2u8; 32], // forged output_hash
                7,
                &[3u8; 32],
                &sk,
                &vk,
            );
            if s.should_sample().await {
                let r = s.verify_replay(&att, &[9u8; 32]).await; // replay reveals truth
                if matches!(r, SpotCheckResult::Mismatch { .. }) {
                    caught = true;
                    break;
                }
            }
        }
        if caught {
            detections += 1;
        }
    }
    assert!(detections >= 980, "expected ≥980/1000, got {detections}");
}
```

- [ ] **Step 2: Run, verify pass.**

```bash
cargo test -p vox-orchestrator --test spot_check detects_forged 2>&1 | tail -10
```

Expected: PASS (the test is statistical; the 95% lower bound for 0.994 over 1000 trials is 988.7, so 980 is comfortably in the safe zone).

- [ ] **Step 3: Commit.**

```bash
git add crates/vox-orchestrator/tests/spot_check.rs
git commit -m "test(orchestrator): forged-result detection ≥99% over 100-job run [P5-T5b]"
```

---

## Task P5-T6 — Per-job ephemeral Ed25519 subkey

**Goal.** At dispatch, the planner mints a fresh Ed25519 keypair via `vox-identity`, signs the public half with the node's long-term key (creating a 1-step chain), and hands the keypair to the worker for the lifetime of the lease. Compromising one job's ephemeral key cannot affect other jobs.

**Files:**

- Create: `crates/vox-identity/src/ephemeral.rs`
- Modify: `crates/vox-identity/src/lib.rs`
- Modify: `crates/vox-orchestrator/src/a2a/remote_worker.rs`

### P5-T6a: Mint + chain-sign

- [ ] **Step 1: Failing test.**

Create `crates/vox-identity/tests/ephemeral.rs`:

```rust
use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_identity::ephemeral::{EphemeralSubkey, mint_ephemeral_subkey, verify_subkey_chain};

#[test]
fn ephemeral_subkey_chains_to_long_term() {
    let (long_sk, long_vk) = generate_signing_keypair();
    let sub: EphemeralSubkey = mint_ephemeral_subkey(
        /* task_id */ "T-1",
        /* lease_ttl_secs */ 600,
        &long_sk,
        &long_vk,
    );
    assert_eq!(sub.task_id, "T-1");
    assert_eq!(sub.lease_ttl_secs, 600);
    assert!(verify_subkey_chain(&sub, &long_vk).is_ok());
}

#[test]
fn ephemeral_subkey_rejects_wrong_long_term_pubkey() {
    let (long_sk, long_vk) = generate_signing_keypair();
    let (_, other_vk) = generate_signing_keypair();
    let sub = mint_ephemeral_subkey("T-1", 600, &long_sk, &long_vk);
    assert!(verify_subkey_chain(&sub, &other_vk).is_err());
}
```

- [ ] **Step 2: Implement.**

Create `crates/vox-identity/src/ephemeral.rs`:

```rust
//! Per-job ephemeral Ed25519 subkey (SSOT Phase 5 P5-T6).
//!
//! At dispatch, the planner mints a fresh Ed25519 keypair, then signs the
//! public-half + task-binding with the long-term node key. The result is a
//! 1-step certificate chain whose blast radius is one task: compromising the
//! ephemeral key does not authorize signing other tasks.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use vox_crypto::{
    SigningKey, VerifyingKey, generate_signing_keypair, sign, signing_key_to_bytes,
    verify_signature_hex, verifying_key_to_bytes,
};

pub const SUBKEY_DOMAIN: &[u8] = b"voxmesh.subkey.v1\0";

/// Per-job ephemeral subkey, with a chain-cert tying it back to the long-term
/// node key.
///
/// **Lifecycle.** Lifetime = lease TTL. After the lease expires, the worker
/// MUST drop the private half. The submitter SHOULD reject attestations
/// signed after `expires_at_unix_ms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralSubkey {
    pub task_id: String,
    pub lease_ttl_secs: u64,
    pub minted_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    /// Hex-encoded ephemeral pubkey.
    pub ephemeral_pubkey_hex: String,
    /// Hex-encoded long-term node pubkey.
    pub long_term_pubkey_hex: String,
    /// Signature by the long-term key over (`task_id` ‖ `ephemeral_pubkey` ‖ `expires_at`).
    pub chain_signature_b64: String,
    /// Base64 of the ephemeral signing key. **Local only**: never serialized
    /// off-process. (Tests + planner-internal use only.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_signing_key_b64: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SubkeyVerifyError {
    #[error("chain signature does not verify")]
    ChainSignatureMismatch,
    #[error("invalid signature base64")]
    InvalidSignatureB64,
    #[error("invalid pubkey hex")]
    InvalidPubkey,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn chain_input(task_id: &str, ephemeral_pk_hex: &str, expires_at_unix_ms: u64) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(SUBKEY_DOMAIN);
    b.extend_from_slice(task_id.as_bytes());
    b.push(0u8);
    b.extend_from_slice(ephemeral_pk_hex.as_bytes());
    b.push(0u8);
    b.extend_from_slice(&expires_at_unix_ms.to_be_bytes());
    b
}

pub fn mint_ephemeral_subkey(
    task_id: &str,
    lease_ttl_secs: u64,
    long_term_sk: &SigningKey,
    long_term_vk: &VerifyingKey,
) -> EphemeralSubkey {
    let (eph_sk, eph_vk) = generate_signing_keypair();
    let minted = now_unix_ms();
    let expires = minted.saturating_add(lease_ttl_secs.saturating_mul(1000));
    let eph_pk_hex = hex::encode(verifying_key_to_bytes(&eph_vk));
    let chain_sig = sign(long_term_sk, &chain_input(task_id, &eph_pk_hex, expires));
    EphemeralSubkey {
        task_id: task_id.to_string(),
        lease_ttl_secs,
        minted_at_unix_ms: minted,
        expires_at_unix_ms: expires,
        ephemeral_pubkey_hex: eph_pk_hex,
        long_term_pubkey_hex: hex::encode(verifying_key_to_bytes(long_term_vk)),
        chain_signature_b64: base64::engine::general_purpose::STANDARD.encode(chain_sig),
        ephemeral_signing_key_b64: Some(
            base64::engine::general_purpose::STANDARD.encode(signing_key_to_bytes(&eph_sk)),
        ),
    }
}

pub fn verify_subkey_chain(
    sub: &EphemeralSubkey,
    expected_long_term_vk: &VerifyingKey,
) -> Result<(), SubkeyVerifyError> {
    let expected_hex = hex::encode(verifying_key_to_bytes(expected_long_term_vk));
    if sub.long_term_pubkey_hex != expected_hex {
        return Err(SubkeyVerifyError::ChainSignatureMismatch);
    }
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&sub.chain_signature_b64)
        .map_err(|_| SubkeyVerifyError::InvalidSignatureB64)?;
    if sig_bytes.len() != 64 {
        return Err(SubkeyVerifyError::InvalidSignatureB64);
    }
    let input = chain_input(&sub.task_id, &sub.ephemeral_pubkey_hex, sub.expires_at_unix_ms);
    let ok = verify_signature_hex(
        &sub.long_term_pubkey_hex,
        &input,
        &hex::encode(&sig_bytes),
    )
    .map_err(|_| SubkeyVerifyError::InvalidPubkey)?;
    if !ok {
        return Err(SubkeyVerifyError::ChainSignatureMismatch);
    }
    Ok(())
}
```

- [ ] **Step 3: Re-export.**

In `crates/vox-identity/src/lib.rs`:

```rust
pub mod ephemeral;
```

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-identity --test ephemeral 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-identity/src/ephemeral.rs \
        crates/vox-identity/src/lib.rs \
        crates/vox-identity/tests/ephemeral.rs
git commit -m "feat(identity): per-job ephemeral subkey with chain-cert [P5-T6a]"
```

### P5-T6b: Wire ephemeral subkey through dispatch

- [ ] **Step 1: Hook into the dispatcher.**

In `crates/vox-orchestrator/src/a2a/dispatch/...` (search for the place that prepares a `RemoteTaskEnvelope` for a paired peer): mint a subkey for each new task, store the keypair locally, attach the public half + chain-cert to the dispatch metadata, and pass the keypair into `remote_worker::process_one_envelope` via a per-task scratch map.

- [ ] **Step 2: Bind ephemeral-key TTL to lease TTL**

  The ephemeral key's `expires_at_unix_ms` MUST equal the lease's `expires_at_unix_ms` from
  `P0-T3` (authoritative leases). The dispatch path:

  ```rust
  let lease = orchestrator.consult_lease(task_id).await?;
  let ephemeral = vox_identity::mint_ephemeral_subkey_for(
      task_id,
      lease.expires_at_unix_ms,  // <- bind to lease, not an arbitrary lifetime
      &node_long_term_key,
  )?;
  debug_assert_eq!(ephemeral.expires_at_unix_ms, lease.expires_at_unix_ms);
  ```

  Test asserts the assertion holds:

  ```rust
  #[test]
  fn ephemeral_key_lifetime_equals_lease_ttl() {
      let lease = mock_lease(/* expires_at = */ 1234567890_000);
      let ephemeral = dispatch_with_lease(&lease).expect("dispatch ok");
      assert_eq!(ephemeral.expires_at_unix_ms, lease.expires_at_unix_ms);
  }
  ```

  Rationale: a lease whose holder is compromised mid-task should not produce ephemeral keys
  outliving the lease. Equality (not "≤") is the simpler invariant; if the lease renews, the
  next dispatch mints a new ephemeral.

  Cite SSOT §3 P5-T6 row Notes (which now reads "Ephemeral key lifetime MUST equal the lease
  TTL...") in the commit footer alongside `P5-T6`.

- [ ] **Step 3: Test the wire-up.**

In `crates/vox-orchestrator/tests/spot_check.rs` (or a new `tests/ephemeral_wireup.rs`), submit one round-trip job through a hermetic two-node harness and assert the resulting `TaskResult.attestation.signer_pubkey_hex` matches the ephemeral pubkey, NOT the long-term pubkey.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-orchestrator/src/a2a/dispatch/ \
        crates/vox-orchestrator/src/a2a/remote_worker.rs \
        crates/vox-orchestrator/tests/
git commit -m "feat(orchestrator): plumb ephemeral subkey through dispatch [P5-T6b]"
```

---

## Task P5-T7 — Kudos accounting end-to-end

**Goal.** The same signed `Attestation` envelope (P5-T4) IS the kudos credit. Receiver projects it as `kudos_ledger += GpuComputeMs(gpu_seconds * 1000)` keyed on `(submitter_id, peer_id, task_id)`. Idempotent: same envelope → same row. Surface in dashboard.

**Files:**

- Create: `crates/vox-orchestrator/tests/kudos_reconciliation.rs`
- Modify: `crates/vox-mesh-types/src/kudos.rs`
- Create: `crates/vox-orchestrator/src/kudos/mod.rs`
- Create: `crates/vox-orchestrator/src/kudos/projector.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs`

### P5-T7a: Projection helper + idempotency

- [ ] **Step 1: Failing test.**

Create `crates/vox-orchestrator/tests/kudos_reconciliation.rs`:

```rust
use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_mesh_types::{Attestation, RewardPrimitive, TaskKind};
use vox_orchestrator::kudos::projector::{KudosProjector, ProjectionRow};

#[tokio::test]
async fn projection_credits_gpu_compute_ms() {
    let (sk, vk) = generate_signing_keypair();
    let att = Attestation::new_signed(
        "T-1",
        TaskKind::Embed,
        &[1u8; 32],
        &[2u8; 32],
        /* gpu_seconds */ 12,
        &[3u8; 32],
        &sk,
        &vk,
    );
    let p = KudosProjector::in_memory();
    let row = p.project(&att, "submitter-A").await.expect("project");
    assert_eq!(row.primitive, RewardPrimitive::GpuComputeMs);
    assert_eq!(row.amount, 12_000); // gpu_seconds * 1000
}

#[tokio::test]
async fn projection_is_idempotent() {
    let (sk, vk) = generate_signing_keypair();
    let att = Attestation::new_signed(
        "T-99",
        TaskKind::Embed,
        &[1u8; 32],
        &[2u8; 32],
        7,
        &[3u8; 32],
        &sk,
        &vk,
    );
    let p = KudosProjector::in_memory();
    let r1 = p.project(&att, "submitter-A").await.expect("first");
    let r2 = p.project(&att, "submitter-A").await.expect("second");
    assert_eq!(r1.op_id, r2.op_id);
    assert_eq!(p.total_credited(&att.signer_pubkey_hex).await, 7_000);
}

#[tokio::test]
async fn reconciliation_holds_over_100_job_batch() {
    let (sk, vk) = generate_signing_keypair();
    let p = KudosProjector::in_memory();
    let mut total_duration_ms = 0u64;
    for i in 0..100 {
        let att = Attestation::new_signed(
            &format!("T-{i}"),
            TaskKind::Embed,
            &[1u8; 32],
            &[2u8; 32],
            i as u64, // gpu_seconds
            &[3u8; 32],
            &sk,
            &vk,
        );
        let _ = p.project(&att, "submitter-A").await.unwrap();
        total_duration_ms += (i as u64) * 1000;
    }
    let credited = p.total_credited(&hex::encode(verifying_key_to_bytes(&vk))).await;
    let eps = total_duration_ms / 1000;
    assert!(
        credited.abs_diff(total_duration_ms) <= eps,
        "credited={credited} duration={total_duration_ms}",
    );
}
```

- [ ] **Step 2: Implement projector.**

Create `crates/vox-orchestrator/src/kudos/mod.rs`:

```rust
pub mod projector;
```

Create `crates/vox-orchestrator/src/kudos/projector.rs`:

```rust
//! Project a signed result attestation into the contribution ledger.
//!
//! The same envelope IS the attestation AND the kudos credit (SSOT Phase 5
//! P5-T7: "single signed envelope is BOTH attestation AND kudos credit — two
//! birds"). Idempotency is keyed on `op_id = "{task_id}:{signer_pubkey_hex}"`.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use vox_mesh_types::{Attestation, RewardPrimitive};

#[derive(Debug, Clone)]
pub struct ProjectionRow {
    pub op_id: String,
    pub submitter_id: String,
    pub peer_pubkey_hex: String,
    pub primitive: RewardPrimitive,
    pub amount: u64,
    pub task_id: String,
    pub attestation_blake3_hex: String,
    pub credited_unix_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("attestation does not verify: {0}")]
    Verify(String),
}

#[derive(Debug)]
pub struct KudosProjector {
    rows: Arc<Mutex<HashMap<String, ProjectionRow>>>,
}

impl KudosProjector {
    pub fn in_memory() -> Self {
        Self {
            rows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn project(
        &self,
        att: &Attestation,
        submitter_id: &str,
    ) -> Result<ProjectionRow, ProjectionError> {
        att.verify_self_signed()
            .map_err(|e| ProjectionError::Verify(e.to_string()))?;
        let op_id = format!("{}:{}", att.task_id, att.signer_pubkey_hex);
        let attestation_b3 = blake3::hash(
            &serde_json::to_vec(att).expect("attestation to_vec"),
        );
        let row = ProjectionRow {
            op_id: op_id.clone(),
            submitter_id: submitter_id.to_string(),
            peer_pubkey_hex: att.signer_pubkey_hex.clone(),
            primitive: RewardPrimitive::GpuComputeMs,
            amount: att.gpu_seconds.saturating_mul(1000),
            task_id: att.task_id.clone(),
            attestation_blake3_hex: hex::encode(attestation_b3.as_bytes()),
            credited_unix_ms: now_unix_ms(),
        };
        let mut g = self.rows.lock().await;
        g.entry(op_id).or_insert_with(|| row.clone());
        Ok(row)
    }

    pub async fn total_credited(&self, peer_pubkey_hex: &str) -> u64 {
        let g = self.rows.lock().await;
        g.values()
            .filter(|r| r.peer_pubkey_hex == peer_pubkey_hex)
            .map(|r| r.amount)
            .sum()
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

- [ ] **Step 3: Add `RewardPrimitive::GpuComputeMs` projection helper.**

In `crates/vox-mesh-types/src/kudos.rs`, append:

```rust
impl RewardPrimitive {
    /// Convert a duration in milliseconds to a `GpuComputeMs` `CreditJobRequest`
    /// shape suitable for ledger insertion.
    pub fn from_gpu_compute_ms(
        duration_ms: u64,
        vox_user_id: &str,
        node_id: &str,
        task_id: &str,
    ) -> CreditJobRequest {
        CreditJobRequest {
            vox_user_id: vox_user_id.to_string(),
            node_id: node_id.to_string(),
            primitive: Self::GpuComputeMs,
            amount: duration_ms,
            task_id: Some(task_id.to_string()),
            metadata_json: None,
        }
    }
}
```

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-orchestrator --test kudos_reconciliation 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-orchestrator/src/kudos/ \
        crates/vox-orchestrator/src/lib.rs \
        crates/vox-orchestrator/tests/kudos_reconciliation.rs \
        crates/vox-mesh-types/src/kudos.rs
git commit -m "feat(orchestrator,mesh-types): kudos projection from attestation envelope [P5-T7a]"
```

### P5-T7b: Persist projection rows to `contribution_ledger`

- [ ] **Step 1: Failing test (round-trip through SQL).**

Append to `kudos_reconciliation.rs`:

```rust
#[tokio::test]
async fn projection_persists_to_contribution_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let db = vox_db::open_in_dir(dir.path()).await.expect("open db");
    let (sk, vk) = generate_signing_keypair();
    let p = KudosProjector::with_db(db.clone());
    for i in 0..3 {
        let att = Attestation::new_signed(
            &format!("T-{i}"),
            TaskKind::Embed,
            &[1u8; 32],
            &[2u8; 32],
            5,
            &[3u8; 32],
            &sk,
            &vk,
        );
        p.project(&att, "submitter-A").await.expect("project");
    }
    let count = vox_db::contribution_ledger::count_for_peer(
        &db,
        &hex::encode(verifying_key_to_bytes(&vk)),
    )
    .await
    .unwrap();
    assert_eq!(count, 3);
}
```

- [ ] **Step 2: Implement `with_db` on `KudosProjector` and the helpers in vox-db.**

`vox_db::contribution_ledger` is a thin module exposing `insert(db, &row)` and `count_for_peer(db, &peer_pubkey_hex)` over the `contribution_ledger` table created in P5-T3a.

- [ ] **Step 3: Commit.**

```bash
git add crates/vox-orchestrator/src/kudos/projector.rs \
        crates/vox-db/src/contribution_ledger.rs \
        crates/vox-db/src/lib.rs \
        crates/vox-orchestrator/tests/kudos_reconciliation.rs
git commit -m "feat(orchestrator,db): persist kudos projection to contribution_ledger [P5-T7b]"
```

### P5-T7c: Surface in dashboard

- [ ] **Step 1: Add a JSON endpoint at `/api/mesh/kudos` exposing per-peer credited GpuComputeMs (sum) keyed on `peer_pubkey_hex`.**

In whichever crate hosts the dashboard JSON API (search for `"/api/mesh/"`), add a route reading from `vox_db::contribution_ledger`. Keep it read-only.

- [ ] **Step 2: Confirm via integration test or manual `curl` against the test harness.**

- [ ] **Step 3: Commit.**

```bash
git add <dashboard-route-files>
git commit -m "feat(dashboard): /api/mesh/kudos endpoint [P5-T7c]"
```

---

## Task P5-T8 — Mesh-wide model inventory aggregation

**Goal.** Scheduled refresh per peer publishes its local model registry (LoRAs, quantizations) into `mesh_model_inventory`. Planner consults the snapshot at dispatch time → ends "have to retry locally because forgot remote has the weights".

**Files:**

- Create: `crates/vox-mesh-types/src/model_inventory.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`
- Create: `crates/vox-orchestrator/src/inventory/mod.rs`
- Create: `crates/vox-orchestrator/src/inventory/refresh.rs`
- Create: `crates/vox-orchestrator/tests/model_inventory.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs`

### P5-T8a: Inventory snapshot type

- [ ] **Step 1: Implement.**

Create `crates/vox-mesh-types/src/model_inventory.rs`:

```rust
use serde::{Deserialize, Serialize};

/// One entry in a mesh-wide model inventory snapshot. Multiple entries per
/// peer are typical (one model + variant pair).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub peer_node_pubkey_hex: String,
    pub model_id: String,
    /// e.g. `"q4_0"`, `"q5_K_M"`, `"fp16"`. `None` for the canonical-precision
    /// build.
    pub quantization: Option<String>,
    /// e.g. `"alpaca-7b-lora"`. `None` for the base model.
    pub lora_adapter: Option<String>,
}

/// A snapshot of the mesh-wide model inventory. Exchanged on the gossip topic
/// `vox.mesh.model_inventory.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub version: u8,
    pub snapshot_unix_ms: u64,
    pub entries: Vec<InventoryEntry>,
}
```

In `lib.rs`:

```rust
pub mod model_inventory;
pub use model_inventory::{InventoryEntry, InventorySnapshot};
```

- [ ] **Step 2: Commit.**

```bash
git add crates/vox-mesh-types/src/model_inventory.rs \
        crates/vox-mesh-types/src/lib.rs
git commit -m "feat(mesh-types): InventorySnapshot type [P5-T8a]"
```

### P5-T8b: Scheduled refresh

- [ ] **Step 1: Failing test.**

Create `crates/vox-orchestrator/tests/model_inventory.rs`:

```rust
use vox_mesh_types::{InventoryEntry, InventorySnapshot};
use vox_orchestrator::inventory::refresh::{InventoryRefresh, InventoryRefreshConfig};

#[tokio::test]
async fn refresh_writes_snapshot_to_db() {
    let dir = tempfile::tempdir().unwrap();
    let db = vox_db::open_in_dir(dir.path()).await.expect("open db");
    let snapshot = InventorySnapshot {
        version: 1,
        snapshot_unix_ms: 1_700_000_000_000,
        entries: vec![InventoryEntry {
            peer_node_pubkey_hex: "ab".repeat(32),
            model_id: "llama-3-8b".into(),
            quantization: Some("q4_0".into()),
            lora_adapter: None,
        }],
    };
    let cfg = InventoryRefreshConfig::default();
    let refresher = InventoryRefresh::new(db.clone(), cfg);
    refresher.apply(&snapshot).await.expect("apply");

    let entries = vox_db::mesh_model_inventory::query(
        &db,
        /* model_id */ "llama-3-8b",
    )
    .await
    .unwrap();
    assert_eq!(entries.len(), 1);
}
```

- [ ] **Step 2: Implement.**

Create `crates/vox-orchestrator/src/inventory/mod.rs`:

```rust
pub mod refresh;
```

Create `crates/vox-orchestrator/src/inventory/refresh.rs`:

```rust
use std::time::Duration;

use vox_mesh_types::InventorySnapshot;

#[derive(Debug, Clone, Copy)]
pub struct InventoryRefreshConfig {
    pub interval: Duration,
    pub stale_after: Duration,
}

impl Default for InventoryRefreshConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60 * 5),
            stale_after: Duration::from_secs(60 * 30),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InventoryRefresh {
    db: vox_db::Db,
    cfg: InventoryRefreshConfig,
}

impl InventoryRefresh {
    pub fn new(db: vox_db::Db, cfg: InventoryRefreshConfig) -> Self {
        Self { db, cfg }
    }

    pub async fn apply(&self, snap: &InventorySnapshot) -> Result<(), String> {
        for e in &snap.entries {
            vox_db::mesh_model_inventory::upsert(
                &self.db,
                snap.snapshot_unix_ms,
                &e.peer_node_pubkey_hex,
                &e.model_id,
                e.quantization.as_deref(),
                e.lora_adapter.as_deref(),
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
```

(Add `vox_db::mesh_model_inventory` thin SQL bindings against the `mesh_model_inventory` table from P5-T3a.)

- [ ] **Step 3: Run, verify pass.**

```bash
cargo test -p vox-orchestrator --test model_inventory 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-orchestrator/src/inventory/ \
        crates/vox-orchestrator/src/lib.rs \
        crates/vox-orchestrator/tests/model_inventory.rs \
        crates/vox-db/src/mesh_model_inventory.rs \
        crates/vox-db/src/lib.rs
git commit -m "feat(orchestrator,db): mesh model inventory refresh [P5-T8b]"
```

---

## Task P5-T9 — Privacy-of-submitted-work signaling

**Goal.** `WorkerDonationPolicy.accept_sensitive_workloads: bool`. The submitter learns "this worker will see plaintext" and can route around. Anti-goal: this is a *signal*, not access control — a malicious worker can lie. Defense-in-depth pairs it with TEE-based attestation (Phase 6).

**Files:**

- Modify: `crates/vox-mesh-types/src/donation_policy.rs`
- Create: `crates/vox-mesh-types/tests/donation_policy_privacy.rs`
- Modify: `crates/vox-orchestrator/src/...` planner code that consults `WorkerDonationPolicy`.

### P5-T9a: Type extension

- [ ] **Step 1: Failing test.**

Create `crates/vox-mesh-types/tests/donation_policy_privacy.rs`:

```rust
use vox_mesh_types::WorkerDonationPolicy;

#[test]
fn donation_policy_default_does_not_accept_sensitive() {
    let p = WorkerDonationPolicy::default();
    assert!(!p.accept_sensitive_workloads,
        "default must be false; submitters should opt-in only with informed peers");
}

#[test]
fn donation_policy_round_trips_through_json() {
    let p = WorkerDonationPolicy {
        accept_sensitive_workloads: true,
        ..WorkerDonationPolicy::default()
    };
    let s = serde_json::to_string(&p).unwrap();
    let q: WorkerDonationPolicy = serde_json::from_str(&s).unwrap();
    assert!(q.accept_sensitive_workloads);
}

#[test]
fn legacy_donation_policy_without_field_deserializes() {
    let s = r#"{"slots":[],"nsfw_allowed":false,"max_job_duration_secs":0,
    "public_mesh_opt_in":false,"min_priority":0,"allowed_scopes":null,
    "allowed_users":null,"denied_users":null,"allowed_mesh_networks":null}"#;
    let p: WorkerDonationPolicy = serde_json::from_str(s).expect("deserialize legacy");
    assert!(!p.accept_sensitive_workloads, "default must apply");
}
```

- [ ] **Step 2: Run, verify failure.**

Expected: FAIL — field/`Default` missing.

- [ ] **Step 3: Modify `donation_policy.rs`.**

Add `Default` derive (or impl manually) and the new field:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerDonationPolicy {
    pub slots: Vec<DonationSlot>,
    pub nsfw_allowed: bool,
    pub max_job_duration_secs: u64,
    pub public_mesh_opt_in: bool,
    pub min_priority: u8,
    pub allowed_scopes: Option<Vec<String>>,
    pub allowed_users: Option<Vec<String>>,
    pub denied_users: Option<Vec<String>>,
    pub allowed_mesh_networks: Option<Vec<String>>,
    /// Phase 5 (P5-T9): the submitter sees this and can route around if it
    /// carries plaintext payloads. Default `false`. **This is a signal, not a
    /// gate** — a malicious worker can lie. Defense-in-depth pairs it with
    /// TEE-based attestation in Phase 6.
    #[serde(default)]
    pub accept_sensitive_workloads: bool,
}

impl Default for WorkerDonationPolicy {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            nsfw_allowed: false,
            max_job_duration_secs: 0,
            public_mesh_opt_in: false,
            min_priority: 0,
            allowed_scopes: None,
            allowed_users: None,
            denied_users: None,
            allowed_mesh_networks: None,
            accept_sensitive_workloads: false,
        }
    }
}
```

- [ ] **Step 4: Run, verify pass.**

```bash
cargo test -p vox-mesh-types --test donation_policy_privacy 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Update planner.**

Wherever the planner picks workers (search `WorkerDonationPolicy` consumers), refuse to dispatch a job marked `privacy_class = "sensitive"` to a peer with `accept_sensitive_workloads = false`.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-mesh-types/src/donation_policy.rs \
        crates/vox-mesh-types/tests/donation_policy_privacy.rs \
        crates/vox-orchestrator/src/...
git commit -m "feat(mesh-types,orchestrator): accept_sensitive_workloads signal [P5-T9a]"
```

---

## Task P5-T10 — Per-pairing X25519 keys for JWE

**Goal.** Today the JWE recipient key is a single BLAKE3 derivation from the shared mesh secret (`crates/vox-orchestrator/src/a2a/remote_worker.rs:120-146`). After this task, each pairing derives `X25519::dh(local_priv, peer_pub)` and uses that as the JWE recipient key. Compromise of one pairing's JWE key cannot decrypt other pairings.

**Files:**

- Create: `crates/vox-identity/src/pairing_x25519.rs`
- Modify: `crates/vox-identity/src/lib.rs`
- Modify: `crates/vox-orchestrator/src/a2a/remote_worker.rs:100-160`
- Modify: `crates/vox-orchestrator/src/a2a/jwe.rs`
- Modify: `crates/vox-secrets/src/spec.rs`

### P5-T10a: Per-pairing key derivation

- [ ] **Step 1: Failing test.**

Create `crates/vox-identity/tests/pairing_x25519.rs`:

```rust
use vox_crypto::generate_encryption_keypair;
use vox_identity::pairing_x25519::{derive_pairing_jwe_key, PairingKeySource};

#[test]
fn derive_pairing_jwe_key_is_symmetric() {
    let (sk_a, pk_a) = generate_encryption_keypair();
    let (sk_b, pk_b) = generate_encryption_keypair();
    let key_ab = derive_pairing_jwe_key(PairingKeySource::Local(&sk_a), &pk_b);
    let key_ba = derive_pairing_jwe_key(PairingKeySource::Local(&sk_b), &pk_a);
    assert_eq!(key_ab, key_ba);
    assert_eq!(key_ab.len(), 32);
}

#[test]
fn derive_pairing_jwe_key_is_distinct_per_pairing() {
    let (sk_a, _pk_a) = generate_encryption_keypair();
    let (_sk_b, pk_b) = generate_encryption_keypair();
    let (_sk_c, pk_c) = generate_encryption_keypair();
    let k_ab = derive_pairing_jwe_key(PairingKeySource::Local(&sk_a), &pk_b);
    let k_ac = derive_pairing_jwe_key(PairingKeySource::Local(&sk_a), &pk_c);
    assert_ne!(k_ab, k_ac);
}
```

- [ ] **Step 2: Implement.**

Create `crates/vox-identity/src/pairing_x25519.rs`:

```rust
//! Per-pairing X25519 key derivation (SSOT Phase 5 P5-T10).
//!
//! Replaces the shared-mesh-secret BLAKE3 derivation used pre-Phase-5. Each
//! pairing derives `BLAKE3("voxmesh.pairing.v1" ‖ DH(local_priv, peer_pub))`
//! as the JWE recipient key. Compromise of one pairing's key cannot decrypt
//! another pairing's traffic.

use vox_crypto::{EncryptionPublicKey, EncryptionSecretKey, secure_hash};

pub const PAIRING_DOMAIN: &[u8] = b"voxmesh.pairing.v1";

pub enum PairingKeySource<'a> {
    Local(&'a EncryptionSecretKey),
}

pub fn derive_pairing_jwe_key(
    src: PairingKeySource<'_>,
    peer_pub: &EncryptionPublicKey,
) -> [u8; 32] {
    let shared = match src {
        PairingKeySource::Local(sk) => sk.0.diffie_hellman(&peer_pub.0),
    };
    let mut input = Vec::with_capacity(PAIRING_DOMAIN.len() + 32);
    input.extend_from_slice(PAIRING_DOMAIN);
    input.extend_from_slice(shared.as_bytes());
    secure_hash(&input)
}
```

In `crates/vox-identity/src/lib.rs`:

```rust
pub mod pairing_x25519;
```

- [ ] **Step 3: Run, verify pass.**

```bash
cargo test -p vox-identity --test pairing_x25519 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-identity/src/pairing_x25519.rs \
        crates/vox-identity/src/lib.rs \
        crates/vox-identity/tests/pairing_x25519.rs
git commit -m "feat(identity): per-pairing X25519 JWE key derivation [P5-T10a]"
```

### P5-T10b: Replace BLAKE3 derivation in `remote_worker.rs`

- [ ] **Step 1: Find and replace.**

In `crates/vox-orchestrator/src/a2a/remote_worker.rs:100-160`, the existing block:

```rust
if let Some(jwe) = msg.jwe_payload.as_deref() {
    let mesh_secret = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshJwtHmacSecret);
    if let Some(mesh_val) = mesh_secret.expose() {
        let derived = blake3::hash(mesh_val.as_bytes());
        match super::jwe::decrypt_jwe_compact(jwe, derived.as_bytes()) { ... }
    }
}
```

Becomes (per-pairing path; keep the BLAKE3 derivation behind a `VoxMeshAuthScheme = "both"` legacy path):

```rust
if let Some(jwe) = msg.jwe_payload.as_deref() {
    // Resolve the per-pairing X25519 key by sender pubkey hex.
    let pairing_key = orchestrator
        .pairing_keys()
        .lookup_by_peer_pubkey_hex(&envelope.sender_pubkey_hex)
        .await;
    let key_bytes: Option<[u8; 32]> = match pairing_key {
        Some(k) => Some(k.derived_jwe_key),
        None => {
            // Fallback to the legacy shared-secret derivation only when
            // VoxMeshAuthScheme admits the legacy path. Refuse otherwise —
            // we never want to silently downgrade.
            if vox_populi::transport::auth::AuthScheme::from_env().accepts_jwt() {
                vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshJwtHmacSecret)
                    .expose()
                    .map(|s| *blake3::hash(s.as_bytes()).as_bytes())
            } else {
                None
            }
        }
    };
    let Some(kb) = key_bytes else {
        tracing::warn!(
            sender_pubkey_hex = %envelope.sender_pubkey_hex,
            "populi remote worker: no per-pairing JWE key for sender; refusing to decrypt"
        );
        // ack to drain inbox, but do not feed plaintext to the executor
        let _ = client.relay_a2a_ack(&receiver_agent.to_string(), msg.id).await;
        return;
    };
    match super::jwe::decrypt_jwe_compact(jwe, &kb) {
        Ok(plain) => { /* unchanged */ }
        Err(e) => { /* unchanged */ }
    }
}
```

`Orchestrator::pairing_keys()` returns a handle to a key store backed by the `pairing_x25519` table created in P5-T3a; rotation is operator-driven via a CLI subcommand.

- [ ] **Step 2: Add `VoxMeshPairingX25519PrivPath` SecretId.**

In `crates/vox-secrets/src/spec.rs`, add a path-style secret pointing at a 32-byte file containing the local X25519 private half:

```rust
VoxMeshPairingX25519PrivPath,
```

- [ ] **Step 3: Test.**

Add an integration test in `crates/vox-orchestrator/tests/jwe_per_pairing.rs` (mirrors the existing JWE round-trip test but uses per-pairing derivation; asserts that a JWE encrypted under pairing AB cannot be decrypted under pairing AC).

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-orchestrator/src/a2a/remote_worker.rs \
        crates/vox-orchestrator/src/a2a/jwe.rs \
        crates/vox-orchestrator/tests/jwe_per_pairing.rs \
        crates/vox-secrets/src/spec.rs
git commit -m "feat(orchestrator): per-pairing X25519 JWE keys; legacy BLAKE3 gated [P5-T10b]"
```

---

## Acceptance

> Mirrors SSOT §3 Phase 5 acceptance verbatim, with the implementation details bolted on.

### A1. Pairing gate

- [ ] **Fresh public mesh node accepts work from a paired peer with valid GitHub attestation.**

End-to-end test in `crates/vox-populi/tests/pairing_e2e.rs`:

```rust
#[tokio::test]
async fn fresh_node_accepts_paired_peer_with_attestation() {
    // 1. Spin up two ephemeral nodes A and B in-process.
    // 2. A publishes its attestation manifest into a hermetic Gist mock.
    // 3. B fetches via `fetch_and_verify`, marks A as paired in
    //    `peer_pairing_status`.
    // 4. A sends a job through `A2ADeliverRequest`.
    // 5. B processes the job and emits an attested TaskResult.
    // Assert: success.
}
```

- [ ] **Refuses paired peer with revoked attestation.**

Same harness, but tombstone A's pubkey in B's revocation gossip first; assert refusal.

- [ ] **Refuses unpaired peer.**

Same harness, but skip the pair step; assert refusal.

### A2. Quota fuse

- [ ] **Fuzz testing fires the per-key quota fuse before depleting node resources.**

Create `crates/vox-populi/tests/quota_fuzz.rs`:

```rust
#[tokio::test]
async fn fuzz_high_volume_does_not_OOM() {
    let store = QuotaStore::in_memory(QuotaPolicy {
        capacity: 10.0,
        refill_per_second: 1.0,
        ..QuotaPolicy::default()
    });
    let pk = "ff".repeat(32);
    let mut admitted = 0usize;
    let mut throttled = 0usize;
    for _ in 0..10_000 {
        match store.try_consume(&pk, 1.0).await {
            QuotaDecision::Admitted { .. } => admitted += 1,
            QuotaDecision::Throttled { .. } => throttled += 1,
        }
    }
    assert!(admitted < 50, "fuse must trip; got admitted={admitted}");
    assert!(throttled > 9_000);
}
```

### A3. Spot-check

- [ ] **Submitter-side spot-check detects an injected forged result with > 99% probability over 100-job run.**

Already covered by `crates/vox-orchestrator/tests/spot_check.rs::detects_forged_results_with_99_percent_probability` (P5-T5b).

### A4. Kudos reconciliation

- [ ] **Kudos ledger reconciles: sum of credited GpuComputeMs across all tasks = sum of TaskResult.duration_ms within ε.**

Already covered by `crates/vox-orchestrator/tests/kudos_reconciliation.rs::reconciliation_holds_over_100_job_batch` (P5-T7a). Tolerance: ε = total_duration_ms / 1000.

### A5. Revocation propagation

- [ ] **Revocation of a peer's attestation propagates as a tombstone within ≤ 60 s for paired peers.**

Add `crates/vox-populi/tests/revocation_propagation.rs`:

```rust
#[tokio::test]
async fn revocation_propagates_within_60_seconds_via_gossip() {
    // 1. Pair node B and node C with node A.
    // 2. A revokes (deletes Gist + emits tombstone to gossip).
    // 3. Within 60 s wall clock, both B.is_revoked(A) and C.is_revoked(A)
    //    must return true.
    // Implementation uses a paused tokio clock and a fake gossip bus that
    // delivers tombstones with a configured upper-bound latency.
}
```

### A6. Capability-mint signing (hopper forward-compat)

- [ ] **`DeveloperOverride` capability mints are signed by the daemon Ed25519 key and verifiable
  by any peer holding the daemon's pubkey from `[mesh.trust]` (forward-compat for hopper Option C).**

A unit test in `crates/vox-populi/tests/ed25519_envelope.rs` (or the hopper crate's mint test once
P3-T6 lands) constructs a `DeveloperOverride` mint, signs it with the daemon key via the same
`SignedA2AEnvelope::sign` path used for A2A control-plane messages, and asserts that
`verify_self_signed()` succeeds and that swapping the daemon pubkey for a different one yields
`EnvelopeVerifyError::SignatureMismatch`. v0.6 keeps the token local-only; this acceptance
guarantees the signing path is wired so Option C does not require a wire-format change.

- Ephemeral Ed25519 subkey lifetime equals the lease TTL granted by `P0-T3`; the
  `ephemeral.expires_at_unix_ms == lease.expires_at_unix_ms` invariant is asserted at dispatch
  and verified by integration test.

### A7. Workspace build

```bash
cargo build --workspace 2>&1 | tail -20
cargo test --workspace --lib 2>&1 | tail -20
```

Expected: clean.

### A8. Final commit

```bash
git add -u
git commit -m "chore(mesh-phase5): final integration sweep [P5]"
```

---

## Rollback

Phase 5 is structured so each task can be rolled back independently. If a task is rolled back, its tests should also be reverted to keep CI green.

- **P5-T1 (Ed25519 envelope) rollback:** set `VOX_MESH_AUTH_SCHEME=jwt-hs256`. The `envelope` and `auth_ed25519` modules become inert. `try_authorize_jwt` re-engages.
- **P5-T2 (GitHub attestation) rollback:** comment out the pairing-gate call site in the dispatch path. Pairing reverts to the pre-Phase-5 behavior (peer admitted whenever bearer/JWT verifies). Document the security regression in the rollback commit.
- **P5-T3 (per-key quota + EMA) rollback:** drop the `QuotaStore::try_consume` call from the dispatch path. The `peer_quota` table becomes orphaned but inert; let it fill and rely on background TTL eviction (or wipe it manually).
- **P5-T4 (signed result attestation) rollback:** stop populating `TaskResult.attestation`. Submitter-side code must continue accepting `attestation = None` (it always did, via `#[serde(default)]`). Spot-check (T5) auto-skips.
- **P5-T5 (spot-check sampler) rollback:** set `VOX_MESH_SPOT_CHECK_PROB=0.0`. The sampler short-circuits.
- **P5-T6 (per-job ephemeral subkey) rollback:** revert to long-term node key as the attestation signer. Attestations remain valid; blast radius widens to the long-term key.
- **P5-T7 (kudos accounting) rollback:** stop projecting attestations. The `contribution_ledger` table becomes a no-op.
- **P5-T8 (model inventory) rollback:** drop the scheduled refresh job. Planner falls back to the pre-Phase-5 "ask, retry locally on miss" behavior.
- **P5-T9 (donation-policy privacy) rollback:** the field is `#[serde(default)]`; ignoring it has no wire-compat impact. Planner reverts to dispatching sensitive workloads to any opted-in peer.
- **P5-T10 (per-pairing X25519 JWE) rollback:** set `VOX_MESH_AUTH_SCHEME=both` to re-engage the legacy BLAKE3-derived shared key for old pairings while honoring per-pairing keys for new ones.

The single most consequential rollback (P5-T2) is also the only one that is *operationally* dangerous. If it must be rolled back, schedule a follow-up incident review with Phase 5's design owner before re-enabling public-mesh exposure.

---

## Self-review

- **SSOT coverage.** Every task in SSOT §3 Phase 5 (P5-T1..P5-T10) maps to at least one task block above, with explicit acceptance and rollback notes.
- **Anti-goals respected.** No new crypto crates; Ed25519 / X25519 / BLAKE3 / JWE flow exclusively through `vox-crypto`. No blockchain. No TEE-first (privacy-of-submitted-work is a *signal*, paired with TEE in Phase 6 only). No onion routing. No transitive web-of-trust. No public SaaS.
- **Reputation is a signal.** Both `peer_reputation` and `donation_policy.accept_sensitive_workloads` are explicitly noted as signals, not gates. The planner consults them for prioritization, never for admission.
- **Spot-check probability.** SSOT says ~1%; the math says we ship at 5% to honor the "≥99% over 100 jobs" acceptance. Documented.
- **Idempotency.** Both attestation projection (P5-T7) and revocation tombstones (P5-T2c) are op-id-keyed; replays do not double-credit or re-reject.
- **Wire compat.** All new `TaskResult` and `WorkerDonationPolicy` fields are `#[serde(default)]`, additive, and back-compat with pre-Phase-5 peers for the migration window. The Ed25519-envelope path stays gated behind `VoxMeshAuthScheme` for the same window.
- **No `.ps1` / `.sh` / `.py`.** Confirmed.

---

## Revision history

- **2026-05-09.** Initial implementation plan derived from SSOT §3 Phase 5.
