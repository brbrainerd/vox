---
title: "Mesh Phase 6 — Grand Network (Volunteer Compute) Implementation Plan (2026-05-09)"
description: "Step-by-step TDD implementation plan for Phase 6 of the Mesh & Language-Distribution SSOT: opt-in joinable bounded-trust global mesh. Eight tasks (P6-T1..P6-T8) producing a federation envelope, a public attestation registry, a Tier-4 micro-VM sandbox interface, redundant-execution voting, a TEE attestation envelope, the Scientia discovery feedback loop, the `vox populi join` flow, and trust-graph self-publication."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. The SSOT (mesh-and-language-distribution-ssot-2026.md) is the durable artifact."
---

# Mesh Phase 6 — Grand Network (Volunteer Compute) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Cite the task ID (`P6-T1`, …, `P6-T8`, or sub-IDs `P6-T1a`, `P6-T1b`, …) in every commit message.

**Goal.** Deliver an opt-in, joinable, bounded-trust global mesh: strangers with GitHub-attested identities can pair their meshes via signed manifests and contribute compute to (and consume from) each other's projects, *without a central server, a token, or a SaaS dependency*. This plan implements the eight P6 tasks from the SSOT in TDD order. This phase also completes the unified-task hopper Option C as the natural endpoint of the cross-cutting `Hp-T*` track (SSOT §3.5), wiring P3-T9's `HopperInboxProjection` over P6-T1's federation envelope via a single mesh-adapter task (`P6-T9`).

**Architecture.** Phase 6 builds on Phases 0–5 and is pure additive surface area: a richer envelope wrapping the existing `A2ADeliverRequest`/`MeshDirectoryEntry` shape, a `vox populi attest publish` subcommand that writes a signed JSON manifest to a Gist or `.well-known` path, a `Tier::MicroVm` sandbox tier behind the existing `SkillRuntime` trait (mock-only impl in this phase), a `RedundancyPolicy` field on `WorkerDonationPolicy` driving N-redundant dispatch for declared-deterministic tasks, an optional `tee_quote` field on `TaskResult` (interface only), a Scientia feedback loop publishing a `Vox Provider Atlas` Finding via `vox-publisher`, a `vox populi join <invite>` subcommand, and a periodic trust-graph snapshot publisher.

**Tech stack.** Rust 2024 edition, existing workspace deps (`serde`, `serde_json`, `tokio`, `tracing`, `thiserror`, `ed25519-dalek`, `blake3`, `reqwest`, `clap`). One new dev-dep: `wiremock` (already in workspace). No new runtime deps; firecracker/kata bindings are deferred to v1.x (only the trait seam ships now).

**SSOT.** [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) §3 Phase 6 (canonical task table, acceptance criteria, anti-goals).

- Hopper integration: this phase lands `P6-T9` as the mesh-adapter completing hopper Option C.
  See SSOT §3.5 and [unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md).

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths below are relative to this worktree.

**Anti-goals (binding from SSOT §0; restated for reviewers).** No blockchain, no token economy, no public SaaS multi-tenant control plane, no TEE-first (we build the *interface* but stub the verifier), no onion routing, no transitive web-of-trust. Federation is gist/git-based — there is no Vox-owned server in any P6 deliverable. Discovery auto-publishing is opt-in: the default in `[mesh.discovery_publishing]` is `enabled = false` and a fresh node never broadcasts its first observation without explicit operator action.

---

## File map

**Create:**

- `crates/vox-mesh-types/src/op_fragment.rs` — `OpFragmentEnvelope` plus the federation-shape extension (`@context`, `id`, `type`, `actor`, `object`, `signature`).
- `crates/vox-mesh-types/src/redundancy.rs` — `RedundancyPolicy`, `RedundancyMode`, `TrustTier`, voting helpers.
- `crates/vox-mesh-types/src/tee_attestation.rs` — `TeeQuote`, `TeeQuoteKind`, `TeeVerifier` trait (verifier returns `NotImplemented`).
- `crates/vox-mesh-types/src/attestation_manifest.rs` — `PublicAttestationManifest` JSON shape + canonical-bytes signing helper.
- `crates/vox-mesh-types/tests/federation_envelope.rs` — round-trip + signature-verify integration tests.
- `crates/vox-mesh-types/tests/redundancy_voting.rs` — BOINC-style adaptive replication tests.
- `crates/vox-mesh-types/tests/attestation_manifest.rs` — manifest sign/verify round-trip + cache-invalidation behavior.
- `crates/vox-skill-runtime/src/microvm.rs` — `MicroVmRuntime` mock impl returning `Err(NotImplemented)`; documents the firecracker/kata seam.
- `crates/vox-skill-runtime/tests/microvm_tier.rs` — `Tier::MicroVm` planner integration test.
- `crates/vox-ml-cli/src/commands/populi_attest.rs` — `vox populi attest publish` and `vox populi attest fetch` subcommands.
- `crates/vox-ml-cli/src/commands/populi_join.rs` — `vox populi join <invite-url>` subcommand.
- `crates/vox-publisher/src/atlas/provider_atlas.rs` — `ProviderAtlasFinding` + emission helper (extends the existing `atlas/` module).
- `crates/vox-publisher/src/atlas/trust_snapshot.rs` — `TrustGraphSnapshot` finding publisher.
- `crates/vox-populi/src/mens/discovery_publish.rs` — opt-in cron-skill that aggregates `vox.workflow.*`/`vox.mesh.*` telemetry into Atlas Findings.
- `crates/vox-orchestrator/src/hopper/mesh_adapter.rs` — P6-T9 mesh adapter completing hopper Option C (Hp-T1+T5+T8 over the federation envelope).
- `tests/hopper_mesh_replication.vox` — Vox-language acceptance test for two-daemon hopper convergence (per AGENTS.md VoxScript-First).
- `docs/src/how-to/grand-network-quickstart.md` — operator-facing quickstart for the volunteer mesh.

**Modify:**

- `crates/vox-mesh-types/src/lib.rs` — re-export the four new modules.
- `crates/vox-mesh-types/src/task.rs` — add `Attestation`, append optional `attestation: Option<Attestation>` field on `TaskResult` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
- `crates/vox-mesh-types/src/donation_policy.rs` — append optional `redundancy: Option<RedundancyPolicy>`.
- `crates/vox-skill-runtime/src/runtime.rs` — add `Tier` enum (`Wasm`, `Container`, `BareMetal`, `MicroVm`); add `tier(&self) -> Tier` method with `Tier::Container` default.
- `crates/vox-skill-runtime/src/detect.rs` — extend planner to honor `min_tier` requests.
- `crates/vox-skill-runtime/src/lib.rs` — re-export `microvm` module, `Tier`.
- `crates/vox-ml-cli/src/commands/populi_cli.rs` — wire `Attest`, `Join` variants.
- `crates/vox-ml-cli/src/commands/mod.rs` — declare `populi_attest`, `populi_join` modules.
- `crates/vox-publisher/src/atlas/mod.rs` — declare `provider_atlas` and `trust_snapshot` submodules.
- `crates/vox-populi/src/mens/mod.rs` — declare `discovery_publish` module (gated behind `mesh-discovery-publish` feature).
- `crates/vox-populi/Cargo.toml` — add `mesh-discovery-publish` feature.
- `docs/src/reference/populi.md` — append a Phase 6 appendix linking the howto.
- `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` — add `HopperOpSync` message kind for P6-T9.
- `crates/vox-orchestrator/src/hopper/mod.rs` — wire the mesh adapter behind a feature flag for P6-T9.
- `Cargo.lock` — regenerated as a side effect of new deps (no manual edit).

**Auto-generated (do not edit by hand):**

- `docs/src/SUMMARY.md` — regenerated by `vox run scripts/regenerate-summary.vox`.
- `docs/feed.xml` — regenerated by the feed builder.

`research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.

---

## Task ordering rationale

Each task ends in a `cargo test` invocation and a single commit, leaving the workspace green. Tasks are ordered so that types come before the systems that consume them:

- **P6-T1** (federation envelope) is the data spine the rest of the phase signs and exchanges, so it goes first.
- **P6-T2** (public attestation registry) consumes the envelope shape and ships the first user-visible CLI subcommand. Without T1's signature-bytes helper, T2 has no canonical signing surface.
- **P6-T3** (micro-VM tier) is a pure interface change with a mock impl — small, independent, and unblocks the planner work that T4 depends on.
- **P6-T4** (redundant-execution voting) needs `Tier` (T3) to decide whether to skip redundancy for trusted-tier peers, so T4 follows T3.
- **P6-T5** (TEE quote envelope) extends `TaskResult` and is independent of T4 in principle, but landing it after T4 keeps the `TaskResult` field additions in one logical block.
- **P6-T6** (Scientia discovery loop) consumes telemetry produced by the prior tasks; it is the first task that emits findings, so it ships after the data shapes are stable.
- **P6-T7** (`vox populi join`) builds on T2 (manifest fetching) and on T6's opt-in defaults; it lands second-to-last so the operator UX is grounded in a fully working envelope/attestation/discovery substrate.
- **P6-T8** (trust-graph snapshots) is the loop closer: it needs T6's publisher seam and T2's manifests as inputs.

Each task is independently revertible — a failed P6-T6 does not block the partial value already delivered by P6-T1..T5.

---

## Task P6-T1: Federation envelope shape

> SSOT: "op-fragment compatible-in-concept with ATProto/ForgeFed (signed Activity-object). Adopt the *shape*, not the transport; ActivityPub is too verbose."

The Phase 3 SSOT references `OpFragmentEnvelope` as already existing; in this worktree the type lives only in the SSOT prose, so we land the canonical struct in `vox-mesh-types` and immediately wrap it in the federation-shape extension. The wrapper is JSON-LD-shaped (`@context`, `id`, `type`, `actor`, `object`, `signature`) but our parser stays strict-JSON: we do not load an LD context resolver, we do not perform Webfinger lookup, we do not implement ActivityPub HTTP semantics.

**Files:**

- Create: `crates/vox-mesh-types/src/op_fragment.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`
- Create: `crates/vox-mesh-types/tests/federation_envelope.rs`

### P6-T1a: failing test for OpFragmentEnvelope round-trip

- [ ] **Step 1: Write the failing test**

Create `crates/vox-mesh-types/tests/federation_envelope.rs`:

```rust
//! Integration tests for the federation envelope shape (P6-T1).

use vox_mesh_types::op_fragment::{
    FederationEnvelope, FederationEnvelopeKind, OpFragmentEnvelope, OpFragmentKind,
};

fn sample_op_fragment() -> OpFragmentEnvelope {
    OpFragmentEnvelope {
        fragment_id: "frag-0001".to_string(),
        kind: OpFragmentKind::A2ADeliver,
        producer_node_id: "node-aaaa".to_string(),
        produced_at_unix_ms: 1_715_212_345_000,
        causal_parents: vec!["frag-0000".to_string()],
        payload_blake3_hex: "00".repeat(32),
        payload_b64: "ZGV0ZXJtaW5pc3RpYy1mcmFnbWVudC1ib2R5".to_string(),
        signer_pubkey: [0u8; 32],
        signature: vec![0u8; 64],
    }
}

#[test]
fn op_fragment_envelope_round_trips() {
    let frag = sample_op_fragment();
    let json = serde_json::to_string(&frag).unwrap();
    let back: OpFragmentEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(back.fragment_id, "frag-0001");
    assert_eq!(back.kind, OpFragmentKind::A2ADeliver);
    assert_eq!(back.causal_parents.len(), 1);
}

#[test]
fn federation_envelope_has_jsonld_shape_keys() {
    let env = FederationEnvelope::new(
        "https://example.org/vox/op/frag-0001",
        FederationEnvelopeKind::OpFragment,
        "did:vox:node-aaaa",
        sample_op_fragment(),
    );
    let json = serde_json::to_value(&env).unwrap();
    let obj = json.as_object().expect("FederationEnvelope must be a JSON object");
    assert!(obj.contains_key("@context"));
    assert_eq!(obj["type"], "vox.op.fragment");
    assert_eq!(obj["actor"], "did:vox:node-aaaa");
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("object"));
    assert!(obj.contains_key("signature"));
}

#[test]
fn federation_envelope_strict_json_no_lazy_context_resolution() {
    // We accept @context as opaque string(s); we do not resolve it.
    let raw = r#"{
      "@context": ["https://www.w3.org/ns/activitystreams", "https://vox.dev/contexts/op/v1"],
      "id": "https://example.org/vox/op/frag-0001",
      "type": "vox.op.fragment",
      "actor": "did:vox:node-aaaa",
      "object": {
        "fragment_id": "frag-0001",
        "kind": "a2a_deliver",
        "producer_node_id": "node-aaaa",
        "produced_at_unix_ms": 1715212345000,
        "causal_parents": ["frag-0000"],
        "payload_blake3_hex": "0000000000000000000000000000000000000000000000000000000000000000",
        "payload_b64": "ZGV0ZXJtaW5pc3RpYy1mcmFnbWVudC1ib2R5",
        "signer_pubkey": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
        "signature": []
      },
      "signature": {
        "kind": "ed25519",
        "value_b64": "",
        "key_b64": ""
      }
    }"#;
    let env: FederationEnvelope = serde_json::from_str(raw).unwrap();
    assert_eq!(env.actor, "did:vox:node-aaaa");
    assert_eq!(env.kind, FederationEnvelopeKind::OpFragment);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-mesh-types --test federation_envelope 2>&1 | tail -10`
Expected: FAIL — `op_fragment` module not found.

### P6-T1b: implement OpFragmentEnvelope and FederationEnvelope

- [ ] **Step 1: Create the new module file**

`crates/vox-mesh-types/src/op_fragment.rs`:

```rust
//! Op-fragment envelope (Phase 3 spine) and the Phase 6 federation shape that wraps it.
//!
//! `OpFragmentEnvelope` is the inner payload — a content-addressed, causally
//! linked, signer-authenticated fragment of mesh state. It is designed to be
//! embeddable in any transport.
//!
//! `FederationEnvelope` is a JSON-LD-*shaped* wrapper (id / type / actor /
//! object / signature) modeled on ATProto's signed-record shape and ForgeFed's
//! Activity object shape. We adopt the *shape*, not the transport: there is no
//! ActivityPub HTTP delivery, no Webfinger resolution, and no LD-context
//! resolver. Parsers treat `@context` as an opaque string array and verify the
//! signature against the embedded `key_b64` against the canonical bytes of the
//! `object` field.

use serde::{Deserialize, Serialize};

/// Discriminator for the inner op-fragment payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpFragmentKind {
    /// A2A deliver request.
    A2ADeliver,
    /// Mesh directory entry advertisement.
    DirectoryAdvertise,
    /// Task spec proposal.
    TaskPropose,
    /// Task result attestation.
    TaskResult,
    /// Donation-policy update.
    DonationPolicyUpdate,
    /// Trust-tier transition observation.
    TrustObservation,
    /// Public attestation manifest publication notice.
    AttestationPublish,
    /// Provider Atlas finding emission notice.
    AtlasFindingEmit,
}

/// Inner op-fragment envelope. This is the durable, content-addressed unit of
/// mesh state. It is signed by the producing node and chains via
/// `causal_parents` (a Lamport-style DAG, not a linear log).
///
/// `payload_b64` carries the type-specific body (e.g. a serialized
/// `A2ADeliverRequest` or `TaskResult`); `payload_blake3_hex` pins it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpFragmentEnvelope {
    /// Fragment id: 32-byte BLAKE3 of canonical bytes, hex-encoded; or an
    /// operator-friendly slug for tests.
    pub fragment_id: String,
    /// Discriminator for the inner body.
    pub kind: OpFragmentKind,
    /// Producing node's mesh id.
    pub producer_node_id: String,
    /// Producer wall-clock at fragment creation. Advisory; replay determinism
    /// depends on causal_parents, not on this timestamp.
    pub produced_at_unix_ms: u64,
    /// Fragment ids this fragment causally follows (zero or more; zero is
    /// reserved for genesis).
    pub causal_parents: Vec<String>,
    /// 32-byte BLAKE3 of the payload bytes, hex-encoded.
    pub payload_blake3_hex: String,
    /// Base64-encoded payload body. Type per `kind`.
    pub payload_b64: String,
    /// Producer's Ed25519 public key (32 bytes).
    pub signer_pubkey: [u8; 32],
    /// Ed25519 signature over the canonical bytes (64 bytes).
    pub signature: Vec<u8>,
}

impl OpFragmentEnvelope {
    /// Returns the canonical JSON representation for signing. The signature
    /// field is zeroed during canonicalization.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut clone = self.clone();
        clone.signature = Vec::new();
        serde_json::to_vec(&clone).unwrap_or_default()
    }
}

/// Discriminator for the Phase 6 federation envelope's `type` field. The
/// string serializations are stable across the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationEnvelopeKind {
    /// An op-fragment broadcast.
    #[serde(rename = "vox.op.fragment")]
    OpFragment,
    /// A public attestation manifest publish notice.
    #[serde(rename = "vox.attestation.publish")]
    AttestationPublish,
    /// A trust-graph snapshot publish notice.
    #[serde(rename = "vox.trust.snapshot")]
    TrustSnapshot,
    /// A Provider Atlas finding emission notice.
    #[serde(rename = "vox.atlas.finding")]
    AtlasFinding,
}

/// Federation-shape outer envelope. Modeled on ATProto signed-record /
/// ForgeFed Activity-object shape. JSON-LD-*shaped*, not LD-resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationEnvelope<O> {
    /// Opaque LD context array. We do not resolve it; we accept arbitrary
    /// strings here and ignore them.
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    /// Stable URL identity for this envelope (e.g. a gist raw URL).
    pub id: String,
    /// Discriminator (see `FederationEnvelopeKind`).
    #[serde(rename = "type")]
    pub kind: FederationEnvelopeKind,
    /// Actor DID. We use `did:vox:<node-id>` for vox-native nodes; we accept
    /// arbitrary DID method strings (e.g. `did:plc:…` from ATProto) but make
    /// no claim to resolve them.
    pub actor: String,
    /// Inner object. For `OpFragment` this is `OpFragmentEnvelope`.
    pub object: O,
    /// Outer-envelope signature (over canonical bytes of `object`).
    pub signature: FederationSignature,
}

/// Outer-envelope signature block.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FederationSignature {
    /// Signature scheme. Only `ed25519` is recognized in Phase 6.
    pub kind: String,
    /// Standard base64 of the signature bytes.
    pub value_b64: String,
    /// Standard base64 of the signer's public key (32 bytes for Ed25519).
    pub key_b64: String,
}

impl<O: Serialize> FederationEnvelope<O> {
    /// Construct a fresh envelope with the canonical Vox context and an empty
    /// signature (callers fill `signature` after computing it).
    pub fn new(id: impl Into<String>, kind: FederationEnvelopeKind, actor: impl Into<String>, object: O) -> Self {
        Self {
            context: vec![
                "https://www.w3.org/ns/activitystreams".to_string(),
                "https://vox.dev/contexts/op/v1".to_string(),
            ],
            id: id.into(),
            kind,
            actor: actor.into(),
            object,
            signature: FederationSignature {
                kind: "ed25519".to_string(),
                value_b64: String::new(),
                key_b64: String::new(),
            },
        }
    }

    /// Canonical bytes for signing: serialize the `object` field on its own.
    /// The outer envelope fields (`id`, `actor`, etc.) are *not* part of the
    /// signed bytes — they are distribution metadata.
    pub fn canonical_object_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.object).unwrap_or_default()
    }
}
```

- [ ] **Step 2: Wire the module into `lib.rs`**

In `crates/vox-mesh-types/src/lib.rs`, add `pub mod op_fragment;` next to the existing module declarations and add a `pub use op_fragment::*;` re-export.

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-mesh-types --test federation_envelope 2>&1 | tail -10`
Expected: PASS for all three tests.

### P6-T1c: signature-verify round-trip

- [ ] **Step 1: Append a signature-verify test**

Append to `tests/federation_envelope.rs`:

```rust
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey, Signature};
use rand::rngs::OsRng;

#[test]
fn federation_envelope_signs_and_verifies() {
    let mut rng = OsRng;
    let signing = SigningKey::generate(&mut rng);
    let verifying: VerifyingKey = signing.verifying_key();

    let mut env = FederationEnvelope::new(
        "https://example.org/vox/op/frag-0001",
        FederationEnvelopeKind::OpFragment,
        "did:vox:node-aaaa",
        sample_op_fragment(),
    );
    let bytes = env.canonical_object_bytes();
    let sig: Signature = signing.sign(&bytes);

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    env.signature.kind = "ed25519".to_string();
    env.signature.value_b64 = STANDARD.encode(sig.to_bytes());
    env.signature.key_b64 = STANDARD.encode(verifying.to_bytes());

    let key_bytes = STANDARD.decode(&env.signature.key_b64).unwrap();
    let key_arr: [u8; 32] = key_bytes.as_slice().try_into().unwrap();
    let key = VerifyingKey::from_bytes(&key_arr).unwrap();
    let sig_bytes = STANDARD.decode(&env.signature.value_b64).unwrap();
    let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().unwrap();
    let sig = Signature::from_bytes(&sig_arr);
    assert!(key.verify(&env.canonical_object_bytes(), &sig).is_ok());
}
```

- [ ] **Step 2: Confirm `ed25519-dalek` and `rand` are in dev-dependencies**

In `crates/vox-mesh-types/Cargo.toml` `[dev-dependencies]`, ensure:

```toml
ed25519-dalek = { workspace = true, features = ["rand_core"] }
rand = { workspace = true }
base64 = { workspace = true }
```

If a workspace lookup is absent for any of these (the workspace already vendors all three), use the explicit version pinned to the rest of the workspace.

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-mesh-types --test federation_envelope 2>&1 | tail -10`
Expected: PASS for all four tests.

### P6-T1d: commit

- [ ] **Commit**

```bash
git add crates/vox-mesh-types/src/op_fragment.rs \
        crates/vox-mesh-types/src/lib.rs \
        crates/vox-mesh-types/tests/federation_envelope.rs \
        crates/vox-mesh-types/Cargo.toml
git commit -m "feat(mesh-types): P6-T1 federation envelope + OpFragmentEnvelope

Adopts ATProto/ForgeFed signed-Activity shape (JSON-LD-shaped, strict-JSON
parsed) over the existing op-fragment payload. No transport semantics."
```

---

## Task P6-T2: Public attestation registry

> SSOT: "Optional public attestation registry — signed JSON manifest in a known git repo, like ATProto DID-doc. Lets a new node bootstrap discovery without a Vox-owned server."

A node publishes a signed JSON manifest to one of two well-known locations:

1. A GitHub Gist owned by the operator's GitHub user (raw URL is canonical).
2. A path inside a project repo at `.well-known/vox-attestation.json`.

Counterparties fetch the manifest by URL, verify the embedded Ed25519 signature, cache it locally keyed by `(actor_did, published_at)`, and re-fetch when `expiry` elapses or `published_at` advances. There is no Vox-owned discovery server; bootstrap is out-of-band (the inviter shares a URL via Signal/email/etc.).

**Files:**

- Create: `crates/vox-mesh-types/src/attestation_manifest.rs`
- Create: `crates/vox-mesh-types/tests/attestation_manifest.rs`
- Create: `crates/vox-ml-cli/src/commands/populi_attest.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`
- Modify: `crates/vox-ml-cli/src/commands/mod.rs`
- Modify: `crates/vox-ml-cli/src/commands/populi_cli.rs`

### P6-T2a: failing test for manifest sign / verify / cache invalidation

- [ ] **Step 1: Write the failing test**

Create `crates/vox-mesh-types/tests/attestation_manifest.rs`:

```rust
//! Tests for the public attestation manifest (P6-T2).

use vox_mesh_types::attestation_manifest::{
    AttestationCache, ManifestVerifyError, PublicAttestationManifest, SupportedTask,
};

fn sample_manifest() -> PublicAttestationManifest {
    PublicAttestationManifest {
        actor_did: "did:vox:node-aaaa".to_string(),
        node_pubkey: [0u8; 32],
        github_user_id: Some("12345678".to_string()),
        github_login: Some("alice".to_string()),
        supported_tasks: vec![
            SupportedTask {
                kind: "embed".to_string(),
                model_id: Some("bge-m3".to_string()),
            },
            SupportedTask {
                kind: "text_infer".to_string(),
                model_id: Some("ollama:llama3-70b".to_string()),
            },
        ],
        peer_capabilities: vec!["redundant_voting".to_string(), "tier_microvm_mock".to_string()],
        published_at_unix_ms: 1_715_212_345_000,
        expiry_unix_ms: 1_715_212_345_000 + 7 * 86_400 * 1000,
        signature_b64: String::new(),
        signature_alg: "ed25519".to_string(),
    }
}

#[test]
fn manifest_sign_and_verify_round_trip() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    let mut rng = OsRng;
    let signing = SigningKey::generate(&mut rng);
    let mut manifest = sample_manifest();
    manifest.node_pubkey = signing.verifying_key().to_bytes();

    let canonical = manifest.canonical_bytes();
    let sig = signing.sign(&canonical);
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    manifest.signature_b64 = STANDARD.encode(sig.to_bytes());

    manifest.verify().expect("freshly-signed manifest must verify");
}

#[test]
fn manifest_verify_rejects_tampered_supported_tasks() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    let mut rng = OsRng;
    let signing = SigningKey::generate(&mut rng);
    let mut manifest = sample_manifest();
    manifest.node_pubkey = signing.verifying_key().to_bytes();
    let sig = signing.sign(&manifest.canonical_bytes());
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    manifest.signature_b64 = STANDARD.encode(sig.to_bytes());

    // Tamper with supported_tasks after signing.
    manifest.supported_tasks.push(SupportedTask {
        kind: "image_gen".to_string(),
        model_id: Some("sdxl".to_string()),
    });
    assert!(matches!(
        manifest.verify(),
        Err(ManifestVerifyError::SignatureInvalid)
    ));
}

#[test]
fn cache_returns_hit_for_same_published_at() {
    let manifest = sample_manifest();
    let mut cache = AttestationCache::default();
    cache.insert(manifest.clone());
    let hit = cache.get(&manifest.actor_did, manifest.published_at_unix_ms).unwrap();
    assert_eq!(hit.actor_did, "did:vox:node-aaaa");
}

#[test]
fn cache_invalidates_on_newer_published_at() {
    let mut older = sample_manifest();
    older.published_at_unix_ms = 1_715_212_345_000;
    let mut newer = sample_manifest();
    newer.published_at_unix_ms = 1_715_298_745_000;

    let mut cache = AttestationCache::default();
    cache.insert(older.clone());
    cache.insert(newer.clone());
    let latest = cache.latest(&older.actor_did).unwrap();
    assert_eq!(latest.published_at_unix_ms, newer.published_at_unix_ms);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-mesh-types --test attestation_manifest 2>&1 | tail -10`
Expected: FAIL — `attestation_manifest` module not found.

### P6-T2b: implement the manifest type and cache

- [ ] **Step 1: Create the manifest module**

`crates/vox-mesh-types/src/attestation_manifest.rs`:

```rust
//! Public attestation manifest (P6-T2).
//!
//! A signed JSON document a node publishes to a Gist or a `.well-known` path.
//! Counterparties fetch it by URL and verify the signature against the
//! embedded `node_pubkey`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedTask {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicAttestationManifest {
    /// Actor DID. For vox-native nodes: `did:vox:<node-id>`.
    pub actor_did: String,
    /// Ed25519 public key (32 bytes). Used to verify `signature_b64`.
    pub node_pubkey: [u8; 32],
    /// Optional GitHub numeric user id (stable across rename).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_user_id: Option<String>,
    /// Optional GitHub login (advisory, may rename).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_login: Option<String>,
    /// What this node will accept work for.
    pub supported_tasks: Vec<SupportedTask>,
    /// Capability tags (e.g. "redundant_voting", "tier_microvm").
    pub peer_capabilities: Vec<String>,
    /// Wall-clock at signing.
    pub published_at_unix_ms: u64,
    /// When this manifest goes stale (counterparties should refresh).
    pub expiry_unix_ms: u64,
    /// Signature algorithm name. Only `ed25519` is recognized.
    pub signature_alg: String,
    /// Standard base64 of the signature bytes.
    #[serde(default)]
    pub signature_b64: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestVerifyError {
    #[error("unsupported signature algorithm: {0}")]
    UnsupportedAlgorithm(String),
    #[error("signature is empty")]
    SignatureEmpty,
    #[error("signature is malformed")]
    SignatureMalformed,
    #[error("signature did not verify")]
    SignatureInvalid,
    #[error("manifest is expired")]
    Expired,
}

impl PublicAttestationManifest {
    /// Canonical bytes: serialize with `signature_b64` replaced by an empty
    /// string. All other fields are part of the signed payload.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut clone = self.clone();
        clone.signature_b64 = String::new();
        serde_json::to_vec(&clone).unwrap_or_default()
    }

    /// Verify the embedded signature. Returns `Ok(())` on success.
    pub fn verify(&self) -> Result<(), ManifestVerifyError> {
        if self.signature_alg != "ed25519" {
            return Err(ManifestVerifyError::UnsupportedAlgorithm(
                self.signature_alg.clone(),
            ));
        }
        if self.signature_b64.is_empty() {
            return Err(ManifestVerifyError::SignatureEmpty);
        }
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let sig_bytes = STANDARD
            .decode(&self.signature_b64)
            .map_err(|_| ManifestVerifyError::SignatureMalformed)?;
        let sig_arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| ManifestVerifyError::SignatureMalformed)?;
        let key = ed25519_dalek::VerifyingKey::from_bytes(&self.node_pubkey)
            .map_err(|_| ManifestVerifyError::SignatureMalformed)?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
        use ed25519_dalek::Verifier;
        key.verify(&self.canonical_bytes(), &sig)
            .map_err(|_| ManifestVerifyError::SignatureInvalid)
    }

    /// Returns true when wall-clock has passed `expiry_unix_ms`.
    pub fn is_expired_at(&self, now_unix_ms: u64) -> bool {
        now_unix_ms >= self.expiry_unix_ms
    }
}

/// Per-actor manifest cache keyed by `published_at_unix_ms`.
///
/// Cache invalidation rule: a fetch returns the entry whose
/// `published_at_unix_ms` matches the URL's published_at hint (when the
/// caller knows it); otherwise the highest `published_at_unix_ms` for that
/// actor wins.
#[derive(Debug, Default, Clone)]
pub struct AttestationCache {
    by_actor: HashMap<String, Vec<PublicAttestationManifest>>,
}

impl AttestationCache {
    pub fn insert(&mut self, manifest: PublicAttestationManifest) {
        let entry = self.by_actor.entry(manifest.actor_did.clone()).or_default();
        entry.push(manifest);
        entry.sort_by_key(|m| m.published_at_unix_ms);
    }

    pub fn get(&self, actor_did: &str, published_at_unix_ms: u64) -> Option<&PublicAttestationManifest> {
        self.by_actor
            .get(actor_did)?
            .iter()
            .find(|m| m.published_at_unix_ms == published_at_unix_ms)
    }

    pub fn latest(&self, actor_did: &str) -> Option<&PublicAttestationManifest> {
        self.by_actor.get(actor_did)?.last()
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Append in `crates/vox-mesh-types/src/lib.rs`:

```rust
pub mod attestation_manifest;
pub use attestation_manifest::{
    AttestationCache, ManifestVerifyError, PublicAttestationManifest, SupportedTask,
};
```

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-mesh-types --test attestation_manifest 2>&1 | tail -10`
Expected: PASS for all four tests.

### P6-T2c: CLI subcommand `vox populi attest publish` / `attest fetch`

- [ ] **Step 1: Create the CLI module**

`crates/vox-ml-cli/src/commands/populi_attest.rs`:

```rust
//! `vox populi attest …` subcommands (P6-T2).

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;
use vox_mesh_types::attestation_manifest::{PublicAttestationManifest, SupportedTask};

#[derive(Subcommand, Debug)]
pub enum AttestCmd {
    /// Build, sign, and emit a manifest. Prints the canonical JSON to stdout
    /// or, when `--out <path>` is given, writes it there. The operator is
    /// responsible for actually uploading the file (Gist UI, `gh gist
    /// create`, or committing under `.well-known/vox-attestation.json`).
    Publish {
        #[arg(long)]
        node_id: String,
        #[arg(long)]
        github_user_id: Option<String>,
        #[arg(long)]
        github_login: Option<String>,
        /// Repeatable: each `--task <kind>[:<model_id>]` adds one entry.
        #[arg(long = "task")]
        tasks: Vec<String>,
        /// Repeatable capability tag (e.g. `--cap redundant_voting`).
        #[arg(long = "cap")]
        capabilities: Vec<String>,
        /// Days until the manifest expires (default 7).
        #[arg(long, default_value_t = 7)]
        expiry_days: u32,
        /// Optional path to write the manifest to.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Fetch a manifest by URL and verify its signature. Prints a one-line
    /// summary on success; nonzero exit on verify failure.
    Fetch {
        #[arg(long)]
        url: String,
    },
}

pub async fn run(cmd: AttestCmd) -> Result<()> {
    match cmd {
        AttestCmd::Publish {
            node_id,
            github_user_id,
            github_login,
            tasks,
            capabilities,
            expiry_days,
            out,
        } => publish(node_id, github_user_id, github_login, tasks, capabilities, expiry_days, out).await,
        AttestCmd::Fetch { url } => fetch(url).await,
    }
}

async fn publish(
    node_id: String,
    github_user_id: Option<String>,
    github_login: Option<String>,
    tasks: Vec<String>,
    capabilities: Vec<String>,
    expiry_days: u32,
    out: Option<PathBuf>,
) -> Result<()> {
    let now_ms = now_unix_ms();
    let supported_tasks: Vec<SupportedTask> = tasks
        .into_iter()
        .map(|t| {
            let mut split = t.splitn(2, ':');
            SupportedTask {
                kind: split.next().unwrap_or_default().to_string(),
                model_id: split.next().map(|s| s.to_string()),
            }
        })
        .collect();

    let signing_key = load_node_signing_key(&node_id)
        .context("loading node Ed25519 signing key from vox-secrets")?;
    let pubkey = signing_key.verifying_key().to_bytes();

    let mut manifest = PublicAttestationManifest {
        actor_did: format!("did:vox:{node_id}"),
        node_pubkey: pubkey,
        github_user_id,
        github_login,
        supported_tasks,
        peer_capabilities: capabilities,
        published_at_unix_ms: now_ms,
        expiry_unix_ms: now_ms + (expiry_days as u64) * 86_400 * 1000,
        signature_alg: "ed25519".to_string(),
        signature_b64: String::new(),
    };
    use ed25519_dalek::Signer;
    let sig = signing_key.sign(&manifest.canonical_bytes());
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    manifest.signature_b64 = STANDARD.encode(sig.to_bytes());

    let json = serde_json::to_string_pretty(&manifest)?;
    if let Some(path) = out {
        std::fs::write(&path, json.as_bytes())
            .with_context(|| format!("writing manifest to {}", path.display()))?;
        eprintln!("manifest written to {}", path.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

async fn fetch(url: String) -> Result<()> {
    let body = reqwest::get(&url).await?.error_for_status()?.text().await?;
    let manifest: PublicAttestationManifest = serde_json::from_str(&body)
        .with_context(|| format!("parsing manifest at {url}"))?;
    manifest.verify().with_context(|| "manifest signature failed to verify")?;
    println!(
        "ok actor_did={} tasks={} capabilities={} published_at_unix_ms={} expiry_unix_ms={}",
        manifest.actor_did,
        manifest.supported_tasks.len(),
        manifest.peer_capabilities.len(),
        manifest.published_at_unix_ms,
        manifest.expiry_unix_ms,
    );
    Ok(())
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn load_node_signing_key(node_id: &str) -> Result<ed25519_dalek::SigningKey> {
    let raw = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeEd25519Sk)
        .expose()
        .ok_or_else(|| anyhow::anyhow!("VOX_MESH_NODE_ED25519_SK not configured for node {node_id}"))?;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = STANDARD
        .decode(raw.trim())
        .context("decoding VOX_MESH_NODE_ED25519_SK base64")?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("VOX_MESH_NODE_ED25519_SK must be 32 bytes"))?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&arr))
}
```

- [ ] **Step 2: Wire the new module**

In `crates/vox-ml-cli/src/commands/mod.rs`, add `pub mod populi_attest;`.

In `crates/vox-ml-cli/src/commands/populi_cli.rs`, add a new variant to `PopuliCli`:

```rust
    /// Public attestation manifest publish/fetch (P6-T2).
    Attest {
        #[command(subcommand)]
        cmd: crate::commands::populi_attest::AttestCmd,
    },
```

Then dispatch in the matching `run` (or `dispatch`) function:

```rust
    PopuliCli::Attest { cmd } => crate::commands::populi_attest::run(cmd).await,
```

- [ ] **Step 3: Add the secret id**

If `VoxMeshNodeEd25519Sk` is not already defined in `crates/vox-secrets/src/spec/`, add it next to existing mesh secrets, with env name `VOX_MESH_NODE_ED25519_SK`. (The vox-secrets file follows a `SecretSpec { id, env, … }` pattern; copy the shape of an existing entry.)

- [ ] **Step 4: Verify build and CLI smoke**

Run: `cargo build -p vox-populi 2>&1 | tail -10`
Run: `cargo run -p vox-populi -- populi attest --help 2>&1 | tail -10`
Expected: clean build; `--help` shows `publish` and `fetch` subcommands.

### P6-T2d: commit

- [ ] **Commit**

```bash
git add crates/vox-mesh-types/src/attestation_manifest.rs \
        crates/vox-mesh-types/src/lib.rs \
        crates/vox-mesh-types/tests/attestation_manifest.rs \
        crates/vox-ml-cli/src/commands/populi_attest.rs \
        crates/vox-ml-cli/src/commands/mod.rs \
        crates/vox-ml-cli/src/commands/populi_cli.rs \
        crates/vox-secrets/src/spec/
git commit -m "feat(populi): P6-T2 public attestation manifest + vox populi attest CLI

Signed JSON manifest published out-of-band (Gist or .well-known/) with
Ed25519 sign/verify and a per-actor cache invalidating on newer
published_at."
```

---

## Task P6-T3: Tier-4 micro-VM sandbox interface

> SSOT: "extend `vox-skill-runtime/` trait; mock impl ships first. Real impl deferred to v1.x; pre-wire the seam."

This task adds a `Tier` enum to the existing `SkillRuntime` trait, a `MicroVmRuntime` mock implementation that returns `Err(NotImplemented)` from `build` and `run`, and a planner extension so an orchestrator can request `min_tier = MicroVm` and have the planner refuse peers without a MicroVm-tier runtime. We document the firecracker/kata API the real implementation will target so that v1.x has a concrete seam to fill.

Explicitly *not* in this phase: a working firecracker/kata launch, snapshot/restore, vsock plumbing, or vendor-specific TEE attachment. We are pre-wiring the seam, not delivering it.

**Files:**

- Modify: `crates/vox-skill-runtime/src/runtime.rs`
- Create: `crates/vox-skill-runtime/src/microvm.rs`
- Modify: `crates/vox-skill-runtime/src/lib.rs`
- Modify: `crates/vox-skill-runtime/src/detect.rs`
- Create: `crates/vox-skill-runtime/tests/microvm_tier.rs`

### P6-T3a: failing test for `Tier::MicroVm` planner refusal

- [ ] **Step 1: Write the failing test**

Create `crates/vox-skill-runtime/tests/microvm_tier.rs`:

```rust
//! P6-T3 tests: `Tier::MicroVm` planner integration.

use vox_skill_runtime::microvm::MicroVmRuntime;
use vox_skill_runtime::runtime::{BuildOpts, RunOpts, SkillRuntime, Tier};
use std::path::PathBuf;

#[test]
fn microvm_runtime_reports_tier_microvm() {
    let rt = MicroVmRuntime::new();
    assert_eq!(rt.tier(), Tier::MicroVm);
    assert_eq!(rt.name(), "microvm");
}

#[test]
fn microvm_runtime_build_returns_not_implemented() {
    let rt = MicroVmRuntime::new();
    let opts = BuildOpts {
        context_dir: PathBuf::from("."),
        artifact_path: None,
        tag: "test".to_string(),
        build_args: Vec::new(),
    };
    let err = rt.build(&opts).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("not implemented"));
}

#[test]
fn microvm_runtime_run_returns_not_implemented() {
    let rt = MicroVmRuntime::new();
    let opts = RunOpts::default();
    let err = rt.run(&opts).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("not implemented"));
}

#[test]
fn planner_refuses_microvm_when_only_container_runtime_available() {
    use vox_skill_runtime::detect::plan_for_min_tier;
    let runtimes: Vec<Box<dyn SkillRuntime>> = vec![Box::new(StubContainerRuntime)];
    let plan = plan_for_min_tier(&runtimes, Tier::MicroVm);
    assert!(plan.is_none(), "expected no runtime to satisfy min_tier=MicroVm");
}

#[test]
fn planner_picks_microvm_when_available_and_required() {
    use vox_skill_runtime::detect::plan_for_min_tier;
    let runtimes: Vec<Box<dyn SkillRuntime>> = vec![
        Box::new(StubContainerRuntime),
        Box::new(MicroVmRuntime::new()),
    ];
    let plan = plan_for_min_tier(&runtimes, Tier::MicroVm).expect("microvm planner");
    assert_eq!(plan.tier(), Tier::MicroVm);
}

struct StubContainerRuntime;

impl SkillRuntime for StubContainerRuntime {
    fn name(&self) -> &str { "stub-container" }
    fn available(&self) -> bool { true }
    fn build(&self, _opts: &BuildOpts) -> anyhow::Result<()> { Ok(()) }
    fn run(&self, _opts: &RunOpts) -> anyhow::Result<vox_skill_runtime::runtime::RunOutcome> {
        Ok(vox_skill_runtime::runtime::RunOutcome {
            exit_code: 0, stdout: String::new(), stderr: String::new(), wall_ms: 0,
        })
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-skill-runtime --test microvm_tier 2>&1 | tail -15`
Expected: FAIL — `microvm` module / `Tier` enum / `plan_for_min_tier` not found.

### P6-T3b: implement `Tier`, `MicroVmRuntime`, and the planner

- [ ] **Step 1: Add `Tier` to `runtime.rs`**

In `crates/vox-skill-runtime/src/runtime.rs`, append:

```rust
/// Sandbox isolation tier. Tiers are ordered from least to most isolated;
/// `Tier::MicroVm > Tier::Container > Tier::Wasm > Tier::BareMetal`.
///
/// A planner asked to satisfy `min_tier = X` picks any available runtime
/// whose `tier()` is `>= X`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Trusted host process. Lowest isolation.
    BareMetal,
    /// In-process WebAssembly sandbox.
    Wasm,
    /// OS-level container (Docker/Podman).
    Container,
    /// Hardware-virtualized micro-VM (firecracker / kata). Highest isolation
    /// reachable in this phase. Real impl deferred to v1.x.
    MicroVm,
}
```

Add a default-providing trait method on `SkillRuntime`:

```rust
pub trait SkillRuntime: Send + Sync {
    fn name(&self) -> &str;
    fn available(&self) -> bool;
    fn build(&self, opts: &BuildOpts) -> anyhow::Result<()>;
    fn run(&self, opts: &RunOpts) -> anyhow::Result<RunOutcome>;
    /// Sandbox tier this runtime provides. Default is `Tier::Container`
    /// (matches existing Docker/Podman runtimes); WASM and BareMetal impls
    /// override this.
    fn tier(&self) -> Tier {
        Tier::Container
    }
}
```

(Adding a defaulted method is non-breaking for existing implementors.)

- [ ] **Step 2: Create the mock micro-VM runtime**

`crates/vox-skill-runtime/src/microvm.rs`:

```rust
//! Micro-VM sandbox tier (P6-T3).
//!
//! This module ships only the trait seam and a mock implementation. A real
//! impl will land in v1.x targeting either:
//!
//! - **Firecracker** (AWS): kernel + rootfs boot via `firecracker-go-sdk`-
//!   equivalent JSON API on a Unix socket. We will spawn a host-side
//!   `firecracker` process per task, send `PUT /machine-config`, `PUT
//!   /boot-source`, `PUT /drives/rootfs`, `PUT /actions
//!   {action_type:"InstanceStart"}`, then attach over vsock.
//!
//! - **Kata Containers**: existing OCI runtime path (`kata-runtime`) so we
//!   can treat the micro-VM as a `Tier::MicroVm` *container* runtime — the
//!   build path stays Dockerfile-based.
//!
//! Both real impls will land behind a feature flag (`microvm-firecracker`,
//! `microvm-kata`) and surface on Linux only. The mock impl below is
//! cross-platform and lets the planner and dispatch path exercise
//! `Tier::MicroVm` end-to-end without the real launcher.

use crate::runtime::{BuildOpts, RunOpts, RunOutcome, SkillRuntime, Tier};

/// Mock implementation. `available()` always returns `true` so planner tests
/// can pick it up; `build()` and `run()` return `Err(...)` carrying the
/// string `"not implemented"` so callers know to fall back.
pub struct MicroVmRuntime;

impl MicroVmRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MicroVmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRuntime for MicroVmRuntime {
    fn name(&self) -> &str {
        "microvm"
    }
    fn available(&self) -> bool {
        // Mock: always advertise available so the planner seam can be tested.
        true
    }
    fn build(&self, _opts: &BuildOpts) -> anyhow::Result<()> {
        anyhow::bail!("microvm build is not implemented in Phase 6 (mock impl)")
    }
    fn run(&self, _opts: &RunOpts) -> anyhow::Result<RunOutcome> {
        anyhow::bail!("microvm run is not implemented in Phase 6 (mock impl)")
    }
    fn tier(&self) -> Tier {
        Tier::MicroVm
    }
}
```

- [ ] **Step 3: Extend the planner**

In `crates/vox-skill-runtime/src/detect.rs`, append:

```rust
use crate::runtime::{SkillRuntime, Tier};

/// Pick the lowest-tier runtime from `runtimes` whose tier is `>= min_tier`
/// and which reports `available() = true`. Returns `None` when no runtime
/// satisfies the constraint.
///
/// Ordering: lowest-satisfying tier wins so a request for `min_tier =
/// Container` does not unnecessarily promote to `MicroVm`.
pub fn plan_for_min_tier<'a>(
    runtimes: &'a [Box<dyn SkillRuntime>],
    min_tier: Tier,
) -> Option<&'a dyn SkillRuntime> {
    runtimes
        .iter()
        .filter(|r| r.available())
        .filter(|r| r.tier() >= min_tier)
        .min_by_key(|r| r.tier())
        .map(|r| r.as_ref())
}
```

- [ ] **Step 4: Re-export the new surface**

In `crates/vox-skill-runtime/src/lib.rs`, add:

```rust
pub mod microvm;
pub use runtime::Tier;
```

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vox-skill-runtime --test microvm_tier 2>&1 | tail -10`
Expected: PASS for all five tests.

### P6-T3c: commit

- [ ] **Commit**

```bash
git add crates/vox-skill-runtime/src/runtime.rs \
        crates/vox-skill-runtime/src/microvm.rs \
        crates/vox-skill-runtime/src/lib.rs \
        crates/vox-skill-runtime/src/detect.rs \
        crates/vox-skill-runtime/tests/microvm_tier.rs
git commit -m "feat(skill-runtime): P6-T3 Tier::MicroVm seam + mock MicroVmRuntime

Adds Tier enum (BareMetal < Wasm < Container < MicroVm) and a planner that
picks the lowest tier satisfying min_tier. MicroVmRuntime is a mock; real
firecracker/kata impl lands in v1.x."
```

---

## Task P6-T4: Redundant-execution voting

> SSOT: "new `RedundancyPolicy` in `WorkerDonationPolicy`; dispatch path forks N-redundant on declared-deterministic tasks. Adaptive: only re-verify untrusted hosts; skip for trust-tier-3 peers."

We add a `RedundancyPolicy` configuration block, a `TrustTier` enum mirroring the BOINC adaptive-replication tiers, a `vote` helper that decides the canonical winner of N executions, and a dispatch hint surfaced via `WorkerDonationPolicy.redundancy`. We do *not* couple this to the actual orchestrator dispatch path here; the orchestrator integration is a follow-up that consumes the types we land in this task.

### Trust-tier table (canonical)

| Tier | Name | Source of trust | Skip redundancy? |
|------|------|------------------|-------------------|
| `0` | `Untrusted` | First contact, no completed jobs | No (always replicate) |
| `1` | `Probationary` | < 100 completed jobs OR any disagreement in last 30 days | No |
| `2` | `Established` | 100+ completed jobs, no disagreement in 30 days | Replicate at sample rate (5% adaptive) |
| `3` | `Vetted` | Operator-promoted (manual confirm of attestation) | Yes (skip redundancy) |
| `4` | `Federated` | Reciprocal vetted-tier from a peer mesh | Yes (skip redundancy) |

**Files:**

- Create: `crates/vox-mesh-types/src/redundancy.rs`
- Create: `crates/vox-mesh-types/tests/redundancy_voting.rs`
- Modify: `crates/vox-mesh-types/src/donation_policy.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`

### P6-T4a: failing tests for trust-tier skip + voting agreement

- [ ] **Step 1: Write the failing test**

Create `crates/vox-mesh-types/tests/redundancy_voting.rs`:

```rust
//! P6-T4 tests: BOINC-style adaptive replication.

use vox_mesh_types::redundancy::{
    decide_replicas, vote_majority, RedundancyMode, RedundancyPolicy, TrustTier, VoteOutcome,
};

fn policy(mode: RedundancyMode) -> RedundancyPolicy {
    RedundancyPolicy {
        mode,
        min_replicas: 3,
        agreement_threshold: 0.66,
        sample_rate_pct: 5,
        deterministic_only: true,
    }
}

#[test]
fn trio_mode_replicates_three_for_untrusted_peer() {
    let p = policy(RedundancyMode::Trio);
    assert_eq!(decide_replicas(&p, TrustTier::Untrusted, true), 3);
}

#[test]
fn vetted_peer_skips_redundancy() {
    let p = policy(RedundancyMode::Trio);
    assert_eq!(decide_replicas(&p, TrustTier::Vetted, true), 1);
    assert_eq!(decide_replicas(&p, TrustTier::Federated, true), 1);
}

#[test]
fn established_peer_uses_sample_rate() {
    let p = policy(RedundancyMode::Boinc);
    // sample_rate_pct = 5 means 5% of jobs are replicated. Pure decision
    // returns either 1 or 3 depending on the seed; deterministic by job_id.
    let mut replicated = 0;
    for job_id in 0..1000u64 {
        if decide_replicas_seeded(&p, TrustTier::Established, true, job_id) > 1 {
            replicated += 1;
        }
    }
    // 5% of 1000 with tolerance.
    assert!((40..=60).contains(&replicated), "got {replicated}, expected ~50");
}

#[test]
fn non_deterministic_task_never_replicates_when_deterministic_only_set() {
    let p = policy(RedundancyMode::Trio);
    assert_eq!(decide_replicas(&p, TrustTier::Untrusted, false), 1);
}

#[test]
fn vote_majority_with_full_agreement() {
    let p = policy(RedundancyMode::Trio);
    let results = vec![
        ("hash-A".to_string(), "node-1".to_string()),
        ("hash-A".to_string(), "node-2".to_string()),
        ("hash-A".to_string(), "node-3".to_string()),
    ];
    let outcome = vote_majority(&p, &results);
    assert!(matches!(outcome, VoteOutcome::Agreed { .. }));
    if let VoteOutcome::Agreed { winning_hash, support, .. } = outcome {
        assert_eq!(winning_hash, "hash-A");
        assert_eq!(support, 3);
    }
}

#[test]
fn vote_majority_with_split_below_threshold() {
    let p = policy(RedundancyMode::Trio);
    let results = vec![
        ("hash-A".to_string(), "node-1".to_string()),
        ("hash-B".to_string(), "node-2".to_string()),
        ("hash-C".to_string(), "node-3".to_string()),
    ];
    let outcome = vote_majority(&p, &results);
    assert!(matches!(outcome, VoteOutcome::Disagreement { .. }));
}

#[test]
fn vote_majority_at_threshold_passes() {
    // 0.66 threshold; 2-of-3 = 0.6666... is >= 0.66.
    let p = policy(RedundancyMode::Trio);
    let results = vec![
        ("hash-A".to_string(), "node-1".to_string()),
        ("hash-A".to_string(), "node-2".to_string()),
        ("hash-B".to_string(), "node-3".to_string()),
    ];
    let outcome = vote_majority(&p, &results);
    assert!(matches!(outcome, VoteOutcome::Agreed { .. }));
}

fn decide_replicas_seeded(p: &RedundancyPolicy, tier: TrustTier, det: bool, job_id: u64) -> u8 {
    use vox_mesh_types::redundancy::decide_replicas_with_seed;
    decide_replicas_with_seed(p, tier, det, job_id)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-mesh-types --test redundancy_voting 2>&1 | tail -10`
Expected: FAIL — `redundancy` module not found.

### P6-T4b: implement `RedundancyPolicy` + voting + adaptive replication

- [ ] **Step 1: Create the redundancy module**

`crates/vox-mesh-types/src/redundancy.rs`:

```rust
//! Redundant-execution voting policy (P6-T4).
//!
//! BOINC adaptive replication: replicate every job for new (Untrusted /
//! Probationary) hosts; sample a small percentage for Established hosts;
//! skip replication entirely for Vetted / Federated peers.

use crate::task::TaskKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedundancyMode {
    /// Disabled. `decide_replicas` always returns 1.
    Off,
    /// Always 3 replicas for Untrusted/Probationary; sample for Established.
    Trio,
    /// Always 5 replicas for Untrusted; 3 for Probationary; sample for Established.
    Quintuple,
    /// BOINC-style: tier-driven, sample-rate driven, deterministic-only.
    Boinc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// First contact.
    Untrusted = 0,
    /// < 100 jobs OR any disagreement in 30 days.
    Probationary = 1,
    /// 100+ jobs and no disagreement in 30 days.
    Established = 2,
    /// Operator-promoted.
    Vetted = 3,
    /// Reciprocal vetted-tier from a peer mesh.
    Federated = 4,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedundancyPolicy {
    pub mode: RedundancyMode,
    /// Lower bound on replicas when replication is active.
    pub min_replicas: u8,
    /// Fraction of replicas that must agree for the result to be canonical.
    /// E.g. 0.66 means 2-of-3 agreement is sufficient.
    pub agreement_threshold: f32,
    /// For Established peers: percent of jobs sampled (0..=100).
    pub sample_rate_pct: u8,
    /// When true, replication is skipped for any task whose `TaskKind` is
    /// not declared `@deterministic`. (See Phase 1 task-kind annotations.)
    pub deterministic_only: bool,
}

impl RedundancyPolicy {
    /// Sensible default for a fresh public-mesh node.
    pub fn boinc_default() -> Self {
        Self {
            mode: RedundancyMode::Boinc,
            min_replicas: 3,
            agreement_threshold: 0.66,
            sample_rate_pct: 5,
            deterministic_only: true,
        }
    }
}

/// Decide how many replicas to launch for a given (policy, peer-tier,
/// determinism) combination. Returns 1 when redundancy is skipped.
pub fn decide_replicas(p: &RedundancyPolicy, tier: TrustTier, deterministic: bool) -> u8 {
    decide_replicas_with_seed(p, tier, deterministic, 0)
}

/// Same as `decide_replicas`, but seedable for deterministic sampling. The
/// orchestrator passes the job id so test cases reproduce.
pub fn decide_replicas_with_seed(
    p: &RedundancyPolicy,
    tier: TrustTier,
    deterministic: bool,
    seed: u64,
) -> u8 {
    if matches!(p.mode, RedundancyMode::Off) {
        return 1;
    }
    if p.deterministic_only && !deterministic {
        return 1;
    }
    if tier >= TrustTier::Vetted {
        return 1;
    }
    match (p.mode, tier) {
        (RedundancyMode::Trio, TrustTier::Untrusted | TrustTier::Probationary) => 3.max(p.min_replicas),
        (RedundancyMode::Quintuple, TrustTier::Untrusted) => 5.max(p.min_replicas),
        (RedundancyMode::Quintuple, TrustTier::Probationary) => 3.max(p.min_replicas),
        (RedundancyMode::Boinc, TrustTier::Untrusted | TrustTier::Probationary) => {
            p.min_replicas.max(3)
        }
        (_, TrustTier::Established) => {
            if sampled(seed, p.sample_rate_pct) {
                p.min_replicas.max(3)
            } else {
                1
            }
        }
        // Trio/Quintuple already handled the relevant tiers above.
        _ => 1,
    }
}

/// Outcome of a redundant-execution vote.
#[derive(Debug, Clone)]
pub enum VoteOutcome {
    Agreed {
        winning_hash: String,
        support: usize,
        total: usize,
        nodes: Vec<String>,
    },
    Disagreement {
        breakdown: Vec<(String, usize)>,
        total: usize,
    },
}

/// Tally `(payload_blake3_hex, node_id)` results into a vote.
pub fn vote_majority(p: &RedundancyPolicy, results: &[(String, String)]) -> VoteOutcome {
    use std::collections::HashMap;
    let total = results.len();
    let mut by_hash: HashMap<&str, Vec<&str>> = HashMap::new();
    for (h, n) in results {
        by_hash.entry(h.as_str()).or_default().push(n.as_str());
    }
    let (best_hash, best_nodes) = by_hash
        .iter()
        .max_by_key(|(_, v)| v.len())
        .map(|(h, v)| (h.to_string(), v.iter().map(|s| s.to_string()).collect::<Vec<_>>()))
        .unwrap_or_default();
    let support = best_nodes.len();
    let support_frac = if total == 0 {
        0.0
    } else {
        support as f32 / total as f32
    };
    if !best_hash.is_empty() && support_frac >= p.agreement_threshold {
        VoteOutcome::Agreed {
            winning_hash: best_hash,
            support,
            total,
            nodes: best_nodes,
        }
    } else {
        let mut breakdown: Vec<(String, usize)> = by_hash
            .into_iter()
            .map(|(h, v)| (h.to_string(), v.len()))
            .collect();
        breakdown.sort_by(|a, b| b.1.cmp(&a.1));
        VoteOutcome::Disagreement { breakdown, total }
    }
}

/// Returns true on `sample_rate_pct` percent of distinct seeds, distributed
/// uniformly over u64. Deterministic per seed.
fn sampled(seed: u64, sample_rate_pct: u8) -> bool {
    let rate = sample_rate_pct.min(100) as u64;
    if rate == 0 {
        return false;
    }
    if rate >= 100 {
        return true;
    }
    // BLAKE3 is overkill but already in the workspace; mod 100 gives the
    // bucket. Using the seed bytes directly keeps the test fully
    // reproducible.
    let h = blake3::hash(&seed.to_le_bytes());
    let first_eight = u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap());
    (first_eight % 100) < rate
}

/// Returns true when `kind` is in the canonical deterministic-task list.
/// Phase 1 ships an `@deterministic` annotation in the Vox compiler that
/// promotes additional kinds to this list at registration time; the
/// hard-coded set here is the safe baseline.
pub fn is_deterministic_baseline(kind: TaskKind) -> bool {
    matches!(kind, TaskKind::Embed | TaskKind::TrainQLoRA)
}
```

- [ ] **Step 2: Wire into `lib.rs`**

In `crates/vox-mesh-types/src/lib.rs`, append:

```rust
pub mod redundancy;
pub use redundancy::{
    decide_replicas, decide_replicas_with_seed, is_deterministic_baseline, vote_majority,
    RedundancyMode, RedundancyPolicy, TrustTier, VoteOutcome,
};
```

- [ ] **Step 3: Add `redundancy` field to `WorkerDonationPolicy`**

In `crates/vox-mesh-types/src/donation_policy.rs`, append the field:

```rust
    /// Optional redundant-execution policy (P6-T4). When None, dispatch is
    /// single-replica; when Some, the orchestrator forks N-redundant for
    /// jobs whose `TaskKind` is declared deterministic and whose peer tier
    /// is below `Vetted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redundancy: Option<crate::redundancy::RedundancyPolicy>,
```

- [ ] **Step 4: Add `serde(default)` to other optional fields if not already**

Verify with `cargo test -p vox-mesh-types 2>&1 | tail -20`. Existing serialized policies must round-trip after adding the new field. The `#[serde(default, skip_serializing_if = "Option::is_none")]` pattern on the new field guarantees this.

- [ ] **Step 5: Confirm `blake3` is already in `vox-mesh-types` deps**

Check `crates/vox-mesh-types/Cargo.toml`. If not present, add `blake3 = { workspace = true }`.

- [ ] **Step 6: Run, verify pass**

Run: `cargo test -p vox-mesh-types --test redundancy_voting 2>&1 | tail -15`
Expected: PASS for all seven tests.

### P6-T4c: commit

- [ ] **Commit**

```bash
git add crates/vox-mesh-types/src/redundancy.rs \
        crates/vox-mesh-types/src/lib.rs \
        crates/vox-mesh-types/src/donation_policy.rs \
        crates/vox-mesh-types/tests/redundancy_voting.rs \
        crates/vox-mesh-types/Cargo.toml
git commit -m "feat(mesh-types): P6-T4 RedundancyPolicy with BOINC adaptive replication

Adds Trio/Quintuple/Boinc modes, TrustTier 0..=4 with skip-for-Vetted+
behavior, vote_majority with configurable agreement threshold, and
deterministic seedable sampling for Established peers."
```

---

## Task P6-T5: TEE attestation envelope

> SSOT: "extend `TaskResult.attestation` with optional `tee_quote` field. Build the *envelope*, not the implementation."

We add an `Attestation` block to `TaskResult` and an inner `TeeQuote` type modeling the three vendor-quote formats Vox cares about. Verification is stubbed: a `TeeVerifier` trait exists, the only impl ships a `NotImplemented` error, and the planner is wired to call the verifier when the dispatch policy demands an attestation.

Real verification (against AMD SEV-SNP MSRs, Intel TDX TD-quotes, AWS Nitro PCRs, NVIDIA H100 GPU attestations) is deferred to v1.x and will live behind feature flags `tee-sev-snp`, `tee-tdx`, `tee-nitro`, `tee-h100`.

**Files:**

- Create: `crates/vox-mesh-types/src/tee_attestation.rs`
- Modify: `crates/vox-mesh-types/src/task.rs`
- Modify: `crates/vox-mesh-types/src/lib.rs`
- Append to: `crates/vox-mesh-types/tests/federation_envelope.rs` (or a new file)

### P6-T5a: failing test for `TaskResult.attestation` round-trip and `NotImplemented` verifier

- [ ] **Step 1: Write the failing test**

Create `crates/vox-mesh-types/tests/tee_attestation.rs`:

```rust
//! P6-T5 tests: TEE attestation envelope (interface only).

use vox_mesh_types::task::{Attestation, TaskResult};
use vox_mesh_types::tee_attestation::{
    StubTeeVerifier, TeeQuote, TeeQuoteKind, TeeVerifier, TeeVerifyError,
};

fn task_result_with_quote() -> TaskResult {
    TaskResult {
        task_id: "task-1".to_string(),
        node_id: "node-1".to_string(),
        success: true,
        output_b64: "AAAA".to_string(),
        duration_ms: 42,
        payload_blake3_hex: Some("00".repeat(32)),
        worker_ed25519_sig_b64: None,
        attestation: Some(Attestation {
            tee_quote: Some(TeeQuote {
                kind: TeeQuoteKind::SevSnp,
                raw: vec![0xDE, 0xAD, 0xBE, 0xEF],
                verification_endpoint: Some("https://kdsintf.amd.com/vcek/v1/".to_string()),
                expected_measurement_hex: Some("11".repeat(48)),
            }),
            replay_proof_blake3_hex: None,
            kudos_signature_b64: None,
        }),
    }
}

#[test]
fn task_result_attestation_round_trips() {
    let r = task_result_with_quote();
    let json = serde_json::to_string(&r).unwrap();
    let back: TaskResult = serde_json::from_str(&json).unwrap();
    let q = back.attestation.unwrap().tee_quote.unwrap();
    assert_eq!(q.kind, TeeQuoteKind::SevSnp);
    assert_eq!(q.raw.len(), 4);
}

#[test]
fn task_result_omits_attestation_when_none() {
    let mut r = task_result_with_quote();
    r.attestation = None;
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("attestation"), "got {json}");
}

#[test]
fn stub_verifier_returns_not_implemented_for_sev_snp() {
    let verifier = StubTeeVerifier;
    let q = TeeQuote {
        kind: TeeQuoteKind::SevSnp,
        raw: vec![0xDE, 0xAD],
        verification_endpoint: None,
        expected_measurement_hex: None,
    };
    let err = verifier.verify(&q).unwrap_err();
    assert!(matches!(err, TeeVerifyError::NotImplemented(_)));
}

#[test]
fn stub_verifier_returns_not_implemented_for_all_kinds() {
    let verifier = StubTeeVerifier;
    for kind in [TeeQuoteKind::SevSnp, TeeQuoteKind::TdxQuote, TeeQuoteKind::NitroEnclave, TeeQuoteKind::H100] {
        let q = TeeQuote {
            kind,
            raw: vec![],
            verification_endpoint: None,
            expected_measurement_hex: None,
        };
        let err = verifier.verify(&q).unwrap_err();
        assert!(matches!(err, TeeVerifyError::NotImplemented(_)), "kind={kind:?}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-mesh-types --test tee_attestation 2>&1 | tail -10`
Expected: FAIL — `tee_attestation` module not found, `Attestation` not on `TaskResult`.

### P6-T5b: implement the TEE envelope

- [ ] **Step 1: Create the tee_attestation module**

`crates/vox-mesh-types/src/tee_attestation.rs`:

```rust
//! TEE attestation envelope (P6-T5).
//!
//! This module ships only the data shape and a stub verifier. Real
//! verification against vendor quote-verification services (AMD KDS, Intel
//! PCS, AWS Nitro NSM, NVIDIA NRAS for H100) is deferred to v1.x and will
//! land behind per-vendor feature flags.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeQuoteKind {
    /// AMD SEV-SNP attestation report (1184 bytes raw).
    SevSnp,
    /// Intel TDX TD-quote.
    TdxQuote,
    /// AWS Nitro Enclaves attestation document (CBOR-encoded).
    NitroEnclave,
    /// NVIDIA H100 confidential-compute mode attestation.
    H100,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeQuote {
    pub kind: TeeQuoteKind,
    /// Vendor-specific raw bytes; the verifier interprets per `kind`.
    pub raw: Vec<u8>,
    /// Optional URL of the vendor's quote-verification endpoint. The
    /// verifier may pin against a known endpoint or accept this advisory
    /// hint depending on policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_endpoint: Option<String>,
    /// Expected platform measurement (vendor-specific format, hex). When
    /// present, the verifier compares the quote's measurement against this
    /// value and rejects on mismatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_measurement_hex: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TeeVerifyError {
    #[error("not implemented for {0:?} (P6 ships interface only)")]
    NotImplemented(TeeQuoteKind),
    #[error("quote bytes are malformed for {0:?}")]
    Malformed(TeeQuoteKind),
    #[error("verification endpoint unreachable: {0}")]
    EndpointUnreachable(String),
    #[error("measurement mismatch: expected {expected}, got {got}")]
    MeasurementMismatch { expected: String, got: String },
    #[error("vendor rejected the quote")]
    VendorRejected,
}

/// Trait for vendor-specific quote verifiers. The trait surface lets v1.x
/// drop in firecracker/SEV-SNP/etc impls behind feature flags without
/// touching the call sites that consume `TaskResult.attestation`.
pub trait TeeVerifier {
    fn verify(&self, quote: &TeeQuote) -> Result<(), TeeVerifyError>;
}

/// Stub verifier: refuses every quote with `NotImplemented`. The orchestrator
/// uses this when no vendor-specific verifier is configured; a downstream
/// caller seeing `NotImplemented` should either fall back to non-attested
/// dispatch or refuse the job depending on operator policy.
pub struct StubTeeVerifier;

impl TeeVerifier for StubTeeVerifier {
    fn verify(&self, quote: &TeeQuote) -> Result<(), TeeVerifyError> {
        Err(TeeVerifyError::NotImplemented(quote.kind))
    }
}
```

- [ ] **Step 2: Add `Attestation` and the new `TaskResult` field**

Replace `crates/vox-mesh-types/src/task.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    TextInfer,
    ImageGen,
    SpeechTranscribe,
    TrainQLoRA,
    Embed,
    VoxScript,
}

impl std::fmt::Display for TaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextInfer => write!(f, "text_infer"),
            Self::ImageGen => write!(f, "image_gen"),
            Self::SpeechTranscribe => write!(f, "speech_transcribe"),
            Self::TrainQLoRA => write!(f, "train_qlora"),
            Self::Embed => write!(f, "embed"),
            Self::VoxScript => write!(f, "vox_script"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub kind: TaskKind,
    pub model_id: Option<String>,
    pub min_vram_mb: Option<u32>,
    pub priority: u8,
    pub timeout_secs: u64,
    pub payload_b64: String,
    pub source_blake3_hex: Option<String>,
    pub required_labels: Vec<String>,
}

/// Composite attestation block on `TaskResult` (P6-T5).
///
/// Optional fields layer additively as Phase 6 ↔ v1.x adds verifiers:
/// - `tee_quote` — vendor TEE attestation (SEV-SNP / TDX / Nitro / H100).
///   Verified by a `tee_attestation::TeeVerifier`.
/// - `replay_proof_blake3_hex` — for deterministic tasks: BLAKE3 of the
///   canonical replay log (Phase 4 deliverable; populated when present).
/// - `kudos_signature_b64` — Ed25519 signature over `(task_id || node_id ||
///   payload_blake3_hex)`, used by the kudos ledger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Attestation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tee_quote: Option<crate::tee_attestation::TeeQuote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_proof_blake3_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kudos_signature_b64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub node_id: String,
    pub success: bool,
    pub output_b64: String,
    pub duration_ms: u64,
    pub payload_blake3_hex: Option<String>,
    pub worker_ed25519_sig_b64: Option<String>,
    /// Optional composite attestation (TEE quote, replay proof, kudos sig).
    /// Absent in serialized form when `None` (additive, backward-compatible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<Attestation>,
}
```

- [ ] **Step 3: Wire `tee_attestation` into `lib.rs`**

In `crates/vox-mesh-types/src/lib.rs`, append:

```rust
pub mod tee_attestation;
pub use tee_attestation::{StubTeeVerifier, TeeQuote, TeeQuoteKind, TeeVerifier, TeeVerifyError};
pub use task::Attestation;
```

- [ ] **Step 4: Update existing call-sites if needed**

Search the workspace for `TaskResult {` and confirm no exhaustive struct literal is broken: `cargo build --workspace 2>&1 | tail -20`. The new field is optional and defaulted, so existing literals compile. Anywhere we control a literal that would benefit from explicit clarity, add `attestation: None,`.

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vox-mesh-types --test tee_attestation 2>&1 | tail -10`
Expected: PASS for all four tests.

### P6-T5c: commit

- [ ] **Commit**

```bash
git add crates/vox-mesh-types/src/tee_attestation.rs \
        crates/vox-mesh-types/src/task.rs \
        crates/vox-mesh-types/src/lib.rs \
        crates/vox-mesh-types/tests/tee_attestation.rs
git commit -m "feat(mesh-types): P6-T5 TaskResult.attestation + TEE quote envelope (stub)

Adds Attestation block carrying tee_quote / replay_proof / kudos_sig and
the TeeVerifier trait with a StubTeeVerifier returning NotImplemented for
every vendor kind. Real verifiers ship in v1.x behind feature flags."
```

---

## Task P6-T6: Discovery feedback loop (Scientia)

> SSOT: "Auto-publish 'this LoRA on this node performs X% better at Y task' as a Scientia Finding. Per `scientia-mesh-integration-research-2026.md`."

A vox-skill (cron-style) aggregates `vox.workflow.*` and `vox.mesh.*` telemetry across dispatched tasks and produces a `ProviderAtlasFinding` artifact via the existing `vox-publisher` integration. The skill emits zero data when `[mesh.discovery_publishing.enabled = false]` (the default for fresh nodes). When enabled, it emits one Finding per `(model_id, node_id, task_kind)` triple per quarter.

A "fresh node never broadcasts its first observation" because the default is opt-out *and* the skill requires `--accept-publishing` on first run — the operator must affirm.

**Files:**

- Create: `crates/vox-publisher/src/atlas/provider_atlas.rs`
- Modify: `crates/vox-publisher/src/atlas/mod.rs`
- Create: `crates/vox-populi/src/mens/discovery_publish.rs`
- Modify: `crates/vox-populi/src/mens/mod.rs`
- Modify: `crates/vox-populi/Cargo.toml` (add `mesh-discovery-publish` feature)

### P6-T6a: failing test for ProviderAtlasFinding shape and opt-in default

- [ ] **Step 1: Write the failing test**

Create `crates/vox-publisher/tests/provider_atlas.rs`:

```rust
//! P6-T6 tests: Provider Atlas finding emission.

use vox_publisher::atlas::provider_atlas::{
    AtlasObservation, ProviderAtlasFinding, ProviderAtlasFindingBuilder,
};

#[test]
fn provider_atlas_finding_basic_shape() {
    let f = ProviderAtlasFindingBuilder::new("Vox Provider Atlas Q2 2026")
        .scope("did:vox:node-aaaa")
        .observation(AtlasObservation {
            model_id: "ollama:llama3-70b".to_string(),
            node_id: "node-aaaa".to_string(),
            task_kind: "text_infer".to_string(),
            metric_key: "tokens_per_sec".to_string(),
            metric_value: 47.0,
            sample_size: 1024,
            confidence_low: 44.5,
            confidence_high: 49.5,
            evidence_window_days: 90,
        })
        .build();

    assert_eq!(f.title, "Vox Provider Atlas Q2 2026");
    assert_eq!(f.observations.len(), 1);
    assert_eq!(f.observations[0].metric_value, 47.0);
}

#[test]
fn provider_atlas_finding_canonical_bytes_stable() {
    let f1 = ProviderAtlasFindingBuilder::new("Atlas")
        .observation(sample_obs())
        .build();
    let f2 = ProviderAtlasFindingBuilder::new("Atlas")
        .observation(sample_obs())
        .build();
    assert_eq!(f1.canonical_bytes(), f2.canonical_bytes());
}

#[test]
fn provider_atlas_finding_serializes_to_publishable_json() {
    let f = ProviderAtlasFindingBuilder::new("Atlas")
        .observation(sample_obs())
        .build();
    let json = f.to_publishable_json().unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["title"], "Atlas");
    assert!(v["observations"].is_array());
}

fn sample_obs() -> AtlasObservation {
    AtlasObservation {
        model_id: "ollama:llama3-70b".to_string(),
        node_id: "node-aaaa".to_string(),
        task_kind: "text_infer".to_string(),
        metric_key: "tokens_per_sec".to_string(),
        metric_value: 47.0,
        sample_size: 1024,
        confidence_low: 44.5,
        confidence_high: 49.5,
        evidence_window_days: 90,
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-publisher --test provider_atlas 2>&1 | tail -10`
Expected: FAIL — `provider_atlas` module not found.

### P6-T6b: implement ProviderAtlasFinding and the cron-skill

- [ ] **Step 1: Create the publisher type**

`crates/vox-publisher/src/atlas/provider_atlas.rs`:

```rust
//! Provider Atlas finding (P6-T6).
//!
//! A Scientia-class artifact summarizing observed model/provider behavior on
//! the local mesh. Published periodically (default quarterly) via the
//! existing `vox-publisher` pipeline.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AtlasObservation {
    pub model_id: String,
    pub node_id: String,
    pub task_kind: String,
    /// e.g. "tokens_per_sec", "p50_latency_ms", "loss_curve_auc".
    pub metric_key: String,
    pub metric_value: f64,
    pub sample_size: u64,
    pub confidence_low: f64,
    pub confidence_high: f64,
    pub evidence_window_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAtlasFinding {
    pub title: String,
    pub scope_did: Option<String>,
    pub observations: Vec<AtlasObservation>,
    pub generated_at_unix_ms: u64,
    pub source_telemetry_streams: Vec<String>,
}

impl ProviderAtlasFinding {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut clone = self.clone();
        // Strip wall-clock so the bytes are reproducible across regenerations
        // of the same evidence window.
        clone.generated_at_unix_ms = 0;
        serde_json::to_vec(&clone).unwrap_or_default()
    }

    pub fn to_publishable_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn into_publication_manifest(self) -> crate::publication::PublicationManifest {
        crate::publication::PublicationManifest {
            publication_id: format!("vox-provider-atlas-{}", self.generated_at_unix_ms),
            content_type: "vox.atlas.provider".to_string(),
            source_ref: self.scope_did.clone(),
            title: self.title.clone(),
            author: "Vox Mesh Discovery Loop".to_string(),
            abstract_text: Some(format!(
                "Observed metrics across {} (model_id, node_id, task_kind) triples; \
                 evidence window per observation (days) varies.",
                self.observations.len(),
            )),
            body_markdown: render_atlas_markdown(&self),
            citations_json: None,
            metadata_json: Some(self.to_publishable_json().unwrap_or_default()),
        }
    }
}

fn render_atlas_markdown(f: &ProviderAtlasFinding) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", f.title));
    out.push_str("| model | node | task | metric | value | n | window (d) |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for o in &f.observations {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.3} (95% CI {:.3}..{:.3}) | {} | {} |\n",
            o.model_id,
            o.node_id,
            o.task_kind,
            o.metric_key,
            o.metric_value,
            o.confidence_low,
            o.confidence_high,
            o.sample_size,
            o.evidence_window_days,
        ));
    }
    out
}

#[derive(Debug, Default)]
pub struct ProviderAtlasFindingBuilder {
    title: String,
    scope_did: Option<String>,
    observations: Vec<AtlasObservation>,
    streams: Vec<String>,
}

impl ProviderAtlasFindingBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            scope_did: None,
            observations: Vec::new(),
            streams: Vec::new(),
        }
    }
    pub fn scope(mut self, did: impl Into<String>) -> Self {
        self.scope_did = Some(did.into());
        self
    }
    pub fn observation(mut self, o: AtlasObservation) -> Self {
        self.observations.push(o);
        self
    }
    pub fn stream(mut self, s: impl Into<String>) -> Self {
        self.streams.push(s.into());
        self
    }
    pub fn build(self) -> ProviderAtlasFinding {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        ProviderAtlasFinding {
            title: self.title,
            scope_did: self.scope_did,
            observations: self.observations,
            generated_at_unix_ms: now_ms,
            source_telemetry_streams: if self.streams.is_empty() {
                vec!["vox.workflow.*".to_string(), "vox.mesh.*".to_string()]
            } else {
                self.streams
            },
        }
    }
}
```

- [ ] **Step 2: Wire into `atlas/mod.rs`**

In `crates/vox-publisher/src/atlas/mod.rs`, add:

```rust
pub mod provider_atlas;
```

(If `mod.rs` does not exist as a Rust file in the worktree, the publisher's `lib.rs` declares `pub mod atlas;`; verify and add `provider_atlas` under it analogously.)

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-publisher --test provider_atlas 2>&1 | tail -10`
Expected: PASS for all three tests.

### P6-T6c: opt-in cron-skill in vox-populi

- [ ] **Step 1: Add the feature flag**

In `crates/vox-populi/Cargo.toml` `[features]`:

```toml
mesh-discovery-publish = ["dep:vox-publisher"]
```

And under `[dependencies]` (gated):

```toml
vox-publisher = { workspace = true, optional = true }
```

- [ ] **Step 2: Create the skill module**

`crates/vox-populi/src/mens/discovery_publish.rs`:

```rust
//! Discovery-feedback cron-skill (P6-T6).
//!
//! Aggregates local telemetry into ProviderAtlasFindings. Default disabled.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryPublishConfig {
    /// Master switch. Default: false. Fresh nodes never publish without
    /// operator action.
    pub enabled: bool,
    /// Quarterly by default.
    pub interval_secs: u64,
    /// Title prefix for emitted findings.
    pub title_prefix: String,
    /// Scope DID of the local mesh (for finding's `scope_did`).
    pub scope_did: Option<String>,
}

impl Default for DiscoveryPublishConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: 90 * 86_400,
            title_prefix: "Vox Provider Atlas".to_string(),
            scope_did: None,
        }
    }
}

#[cfg(feature = "mesh-discovery-publish")]
pub async fn run_one_cycle(config: &DiscoveryPublishConfig) -> Result<usize> {
    if !config.enabled {
        tracing::info!(
            "vox.mesh.discovery_publish.skipped" = "disabled",
            "discovery publishing is disabled (default); set [mesh.discovery_publishing.enabled = true] to opt in"
        );
        return Ok(0);
    }
    let observations = collect_observations()?;
    if observations.is_empty() {
        return Ok(0);
    }
    use vox_publisher::atlas::provider_atlas::ProviderAtlasFindingBuilder;
    let mut builder = ProviderAtlasFindingBuilder::new(format!(
        "{} ({} observations)",
        config.title_prefix,
        observations.len()
    ));
    if let Some(did) = &config.scope_did {
        builder = builder.scope(did);
    }
    for o in &observations {
        builder = builder.observation(o.clone());
    }
    let finding = builder.build();
    let manifest = finding.into_publication_manifest();
    // Hand off to the publisher pipeline; this is the existing seam.
    let _digest = manifest.content_sha3_256();
    tracing::info!(
        "vox.mesh.discovery_publish.emitted" = observations.len() as u64,
        publication_id = manifest.publication_id.as_str(),
    );
    Ok(observations.len())
}

#[cfg(feature = "mesh-discovery-publish")]
fn collect_observations() -> Result<Vec<vox_publisher::atlas::provider_atlas::AtlasObservation>> {
    // Real impl: query vox-db's model_scoreboard / mesh_event_log for the
    // window. Phase 6 ships an empty collector to keep the seam testable.
    Ok(Vec::new())
}
```

- [ ] **Step 3: Wire into `mens/mod.rs`**

```rust
#[cfg(feature = "mesh-discovery-publish")]
pub mod discovery_publish;
```

- [ ] **Step 4: Build with the feature on**

Run: `cargo build -p vox-populi --features mesh-discovery-publish 2>&1 | tail -10`
Expected: clean build.

- [ ] **Step 5: Build with default features**

Run: `cargo build -p vox-populi 2>&1 | tail -10`
Expected: clean build (the new module is gated out).

### P6-T6d: commit

- [ ] **Commit**

```bash
git add crates/vox-publisher/src/atlas/provider_atlas.rs \
        crates/vox-publisher/src/atlas/mod.rs \
        crates/vox-publisher/tests/provider_atlas.rs \
        crates/vox-populi/src/mens/discovery_publish.rs \
        crates/vox-populi/src/mens/mod.rs \
        crates/vox-populi/Cargo.toml
git commit -m "feat(publisher,populi): P6-T6 Provider Atlas finding + opt-in discovery skill

Aggregates vox.workflow.* / vox.mesh.* telemetry into a Scientia-class
artifact via vox-publisher. Default disabled; fresh nodes never broadcast
without [mesh.discovery_publishing.enabled = true]."
```

---

## Task P6-T7: `vox populi join <invite>` flow + quickstart docs

> SSOT: "The 'I want to volunteer my GPU to a friend's project' experience."

The `vox populi join <invite-url>` subcommand decodes a compact invite URL, fetches the published attestation manifest, verifies it, prompts the operator for confirmation (with a non-interactive `--yes` flag), writes the peer to the local trust ledger at `TrustTier::Probationary`, and starts donating per the existing `WorkerDonationPolicy`. The invite URL is shared out-of-band (Signal / email / SMS) — there is no Vox-owned discovery service.

### Invite URL shape

```
vox+populi://join?peer_id=<did>&attestation=<base64url(url)>&bearer=<base64url(ephemeral_token)>&v=1
```

- `peer_id`: the actor DID we're pairing with.
- `attestation`: base64url-encoded URL of the manifest (Gist raw URL or `.well-known` URL).
- `bearer`: short-lived (24h) bearer the inviter generated via `vox populi attest publish --invite-bearer`. Used once for the initial handshake; subsequent traffic uses the published Ed25519 keys.
- `v=1`: schema version.

Operators paste the URL on the command line; the subcommand validates structure, fetches and verifies the manifest, prints a confirmation block, and (on `y` or `--yes`) wires the peer in.

**Files:**

- Create: `crates/vox-ml-cli/src/commands/populi_join.rs`
- Create: `docs/src/how-to/grand-network-quickstart.md`
- Modify: `crates/vox-ml-cli/src/commands/mod.rs`
- Modify: `crates/vox-ml-cli/src/commands/populi_cli.rs`

### P6-T7a: failing test for invite URL parser

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-ml-cli/src/commands/populi_join.rs` (we'll create the file in step 2):

In a `#[cfg(test)] mod tests { … }` block at the bottom of the new file, write:

```rust
#[test]
fn parse_invite_url_extracts_required_fields() {
    let url = "vox+populi://join?peer_id=did%3Avox%3Anode-bbbb\
                &attestation=aHR0cHM6Ly9naXN0LmdpdGh1Yi5jb20vYWxpY2UvYWJj\
                &bearer=ZGVtby1iZWFyZXItdG9rZW4&v=1";
    let invite = super::Invite::parse(url).unwrap();
    assert_eq!(invite.peer_id, "did:vox:node-bbbb");
    assert_eq!(invite.attestation_url, "https://gist.github.com/alice/abc");
    assert_eq!(invite.bearer, "demo-bearer-token");
    assert_eq!(invite.version, 1);
}

#[test]
fn parse_invite_rejects_unsupported_scheme() {
    let err = super::Invite::parse("https://evil.example/join?...").unwrap_err();
    assert!(err.to_string().contains("scheme"));
}

#[test]
fn parse_invite_rejects_unknown_version() {
    let url = "vox+populi://join?peer_id=did%3Avox%3Aa\
                &attestation=aHR0cHM6Ly9hLmI&bearer=eA&v=99";
    let err = super::Invite::parse(url).unwrap_err();
    assert!(err.to_string().contains("version"));
}
```

(The test mod will compile once we create the module.)

### P6-T7b: implement the join subcommand

- [ ] **Step 1: Create the module**

`crates/vox-ml-cli/src/commands/populi_join.rs`:

```rust
//! `vox populi join <invite>` (P6-T7).

use anyhow::{Context, Result};
use clap::Args;
use vox_mesh_types::attestation_manifest::PublicAttestationManifest;

#[derive(Args, Debug)]
pub struct JoinArgs {
    /// Invite URL of the form `vox+populi://join?peer_id=…&attestation=…&bearer=…&v=1`.
    #[arg(value_name = "INVITE")]
    pub invite: String,
    /// Skip the interactive y/N confirmation prompt.
    #[arg(long, default_value_t = false)]
    pub yes: bool,
    /// Override the trust tier the new peer is admitted at. Defaults to
    /// Probationary; do not promote to Vetted on first contact.
    #[arg(long, default_value = "probationary")]
    pub initial_tier: String,
}

#[derive(Debug, Clone)]
pub struct Invite {
    pub peer_id: String,
    pub attestation_url: String,
    pub bearer: String,
    pub version: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum InviteError {
    #[error("unsupported invite scheme (expected vox+populi)")]
    Scheme,
    #[error("invite is missing required field: {0}")]
    MissingField(&'static str),
    #[error("invite version is not supported: {0}")]
    Version(String),
    #[error("base64url decode failed for {0}")]
    Base64(&'static str),
    #[error("attestation URL is not utf-8")]
    AttestationNotUtf8,
}

impl Invite {
    pub fn parse(s: &str) -> Result<Self, InviteError> {
        const PREFIX: &str = "vox+populi://join?";
        let rest = s.strip_prefix(PREFIX).ok_or(InviteError::Scheme)?;
        let mut peer_id = None;
        let mut attestation = None;
        let mut bearer = None;
        let mut version = None;
        for pair in rest.split('&') {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next().unwrap_or("");
            let v = kv.next().unwrap_or("");
            match k {
                "peer_id" => peer_id = Some(percent_decode(v)),
                "attestation" => attestation = Some(v.to_string()),
                "bearer" => bearer = Some(v.to_string()),
                "v" => version = v.parse::<u8>().ok(),
                _ => {}
            }
        }
        let peer_id = peer_id.ok_or(InviteError::MissingField("peer_id"))?;
        let attestation_b64 = attestation.ok_or(InviteError::MissingField("attestation"))?;
        let bearer_b64 = bearer.ok_or(InviteError::MissingField("bearer"))?;
        let version = version.ok_or(InviteError::MissingField("v"))?;
        if version != 1 {
            return Err(InviteError::Version(version.to_string()));
        }
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let url_bytes = URL_SAFE_NO_PAD
            .decode(&attestation_b64)
            .map_err(|_| InviteError::Base64("attestation"))?;
        let attestation_url = String::from_utf8(url_bytes).map_err(|_| InviteError::AttestationNotUtf8)?;
        let bearer_bytes = URL_SAFE_NO_PAD
            .decode(&bearer_b64)
            .map_err(|_| InviteError::Base64("bearer"))?;
        let bearer = String::from_utf8(bearer_bytes).map_err(|_| InviteError::AttestationNotUtf8)?;
        Ok(Self { peer_id, attestation_url, bearer, version })
    }
}

fn percent_decode(s: &str) -> String {
    // Tiny percent-decoder for RFC 3986 unreserved + the few escapes we need
    // (`:` -> %3A, etc.). For full cases use a real URL crate; this is
    // sufficient for invites we generate.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            let hex: String = [h1, h2].iter().collect();
            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                out.push(b as char);
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub async fn run(args: JoinArgs) -> Result<()> {
    let invite = Invite::parse(&args.invite)
        .with_context(|| format!("parsing invite URL"))?;
    eprintln!("fetching attestation manifest from {}", invite.attestation_url);
    let body = reqwest::get(&invite.attestation_url)
        .await?
        .error_for_status()?
        .text()
        .await?;
    let manifest: PublicAttestationManifest = serde_json::from_str(&body)
        .context("parsing manifest")?;
    manifest.verify().context("manifest signature did not verify")?;
    if manifest.actor_did != invite.peer_id {
        anyhow::bail!(
            "manifest actor_did ({}) does not match invite peer_id ({})",
            manifest.actor_did, invite.peer_id
        );
    }

    eprintln!("--- Pairing summary ---");
    eprintln!("  peer:               {}", manifest.actor_did);
    eprintln!("  github:             {:?}", manifest.github_login);
    eprintln!("  supported tasks:    {}", manifest.supported_tasks.len());
    for t in &manifest.supported_tasks {
        eprintln!("    - {} model={:?}", t.kind, t.model_id);
    }
    eprintln!("  capabilities:       {}", manifest.peer_capabilities.join(", "));
    eprintln!("  initial trust tier: {}", args.initial_tier);
    if !args.yes {
        use std::io::Write;
        eprint!("Confirm pairing? [y/N]: ");
        std::io::stderr().flush().ok();
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer).ok();
        let answer = answer.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            eprintln!("aborted");
            return Ok(());
        }
    }
    write_peer_to_trust_ledger(&manifest, &args.initial_tier)?;
    eprintln!("paired with {} at tier {}", manifest.actor_did, args.initial_tier);
    Ok(())
}

fn write_peer_to_trust_ledger(manifest: &PublicAttestationManifest, tier: &str) -> Result<()> {
    // Phase 6 ships the seam; the trust-ledger module is owned by
    // vox-populi::mens::trust_ledger. Real impl persists to the local DB.
    tracing::info!(
        "vox.mesh.peer.paired" = manifest.actor_did.as_str(),
        "vox.mesh.peer.tier" = tier,
        "vox.mesh.peer.published_at_unix_ms" = manifest.published_at_unix_ms,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    // (See P6-T7a above.)
}
```

- [ ] **Step 2: Wire the module**

In `crates/vox-ml-cli/src/commands/mod.rs`:

```rust
pub mod populi_join;
```

In `crates/vox-ml-cli/src/commands/populi_cli.rs`, add the variant and dispatch arm:

```rust
    /// Join a public mesh from an out-of-band invite URL (P6-T7).
    Join(crate::commands::populi_join::JoinArgs),
```

```rust
    PopuliCli::Join(args) => crate::commands::populi_join::run(args).await,
```

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-populi --lib commands::populi_join 2>&1 | tail -10`
Expected: PASS for all three parser tests.

- [ ] **Step 4: CLI smoke**

Run: `cargo run -p vox-populi -- populi join --help 2>&1 | tail -10`
Expected: clap prints usage with `--yes` and `--initial-tier`.

### P6-T7c: quickstart documentation

- [ ] **Step 1: Create the howto**

`docs/src/how-to/grand-network-quickstart.md`:

```markdown
---
title: "Grand Network Quickstart — Volunteer Compute Across Friend Meshes"
description: "Pair two vox meshes via signed attestation manifests. No Vox-owned server, no token, no SaaS. Out-of-band invite, Ed25519-signed JSON manifests in a Gist or .well-known/, mutual confirmation, then donate compute."
category: "How-To Guides"
status: "current"
training_eligible: true
---

# Grand Network Quickstart

This guide walks two operators (Alice and Bob) through pairing their
private vox meshes so they can share compute. It uses Phase 6 of the
[Mesh & Language-Distribution SSOT](../architecture/mesh-and-language-distribution-ssot-2026.md)
end-to-end: federation envelope (P6-T1), public attestation registry
(P6-T2), redundant-execution voting (P6-T4), and `vox populi join` (P6-T7).

## Anti-goals

This guide does **not** rely on any of the following:

- A Vox-owned discovery server.
- A blockchain or token economy.
- A SaaS multi-tenant control plane.

The whole flow is gist + git + Ed25519 signatures.

## Prerequisites

- A working private mesh (`vox populi up` returns nodes ready).
- A GitHub identity with the ability to create a public Gist.
- The mesh node Ed25519 secret available as `VOX_MESH_NODE_ED25519_SK`
  (a 32-byte base64-encoded secret key; see
  [`vox secrets ls`](../reference/cli.md#vox-secrets-ls)).
- An out-of-band channel (Signal / email / SMS) you trust.

## Step 1 — Alice publishes her attestation manifest

```sh
vox populi attest publish \
    --node-id node-aaaa \
    --github-login alice \
    --task embed:bge-m3 \
    --task text_infer:ollama:llama3-70b \
    --cap redundant_voting \
    --cap tier_microvm_mock \
    --expiry-days 7 \
    --out alice-attestation.json
```

The file `alice-attestation.json` contains the signed manifest. Alice
uploads it to a public Gist:

```sh
gh gist create alice-attestation.json --public
```

She copies the *raw* URL (the `…/raw/…` URL, not the HTML one).

## Step 2 — Alice mints an invite

```sh
vox populi attest publish \
    --node-id node-aaaa \
    --task embed:bge-m3 \
    --invite-bearer 24h \
    --invite-out alice-invite.url
```

The file `alice-invite.url` contains a single line:

```
vox+populi://join?peer_id=did%3Avox%3Anode-aaaa&attestation=<base64url>&bearer=<base64url>&v=1
```

Alice sends this URL to Bob over Signal.

## Step 3 — Bob joins

Bob runs:

```sh
vox populi join "vox+populi://join?peer_id=...&attestation=...&bearer=...&v=1"
```

The CLI fetches the manifest, verifies the signature against the embedded
Ed25519 public key, prints a summary, and prompts:

```
Confirm pairing? [y/N]:
```

Bob confirms. The peer is admitted at `TrustTier::Probationary`.

## Step 4 — They share compute

Either side can dispatch to the other; Phase 6 redundant execution
(P6-T4) replicates Bob's first jobs to Alice's side three-way until they
graduate Bob to `Established` and then to `Vetted`.

To inspect the trust state:

```sh
vox populi status --peers
```

## Step 5 — Revoking a compromised peer

If Bob's GitHub account is compromised, Alice deletes the Gist (or
revokes via `gh gist delete`). Bob's manifest will fail to fetch on the
next refresh; gossip propagates the absence; new dispatches refuse Bob
within ≤ 5 minutes (Phase 6 acceptance criterion).

## Further reading

- [`mesh-and-language-distribution-ssot-2026.md`](../architecture/mesh-and-language-distribution-ssot-2026.md) — full SSOT.
- [`mesh-phase6-grand-network-plan-2026.md`](../architecture/mesh-phase6-grand-network-plan-2026.md) — implementation plan.
- [Populi reference](../reference/populi.md) — CLI surface.
```

- [ ] **Step 2: Verify markdownlint passes**

Run: `cargo run -p vox-cli -- check docs --no-cache 2>&1 | tail -10` (or whatever the worktree's doc gate is).
Expected: PASS, or document the file in the next regeneration of `SUMMARY.md` (auto-generated and outside the scope of this PR). `research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.

### P6-T7d: commit

- [ ] **Commit**

```bash
git add crates/vox-ml-cli/src/commands/populi_join.rs \
        crates/vox-ml-cli/src/commands/mod.rs \
        crates/vox-ml-cli/src/commands/populi_cli.rs \
        docs/src/how-to/grand-network-quickstart.md
git commit -m "feat(populi): P6-T7 vox populi join + grand-network quickstart docs

Out-of-band invite URL (vox+populi://join?…), manifest fetch+verify,
operator confirmation, peer admission at Probationary tier."
```

---

## Task P6-T8: Trust-graph snapshot self-publication

> SSOT: "every N hours (default 24), publish a snapshot of the local trust ledger as a Scientia Finding via `vox-publisher`. Per `scientia-self-publication-finalization-plan-2026.md`."

The local trust ledger (peer pubkeys, current tier, last-paired-at, last-disagreement-at) is serialized into a `TrustGraphSnapshot` finding and handed to the existing `vox-publisher` pipeline. Aggregated across opted-in nodes this becomes a public auditable trust map. No Vox-owned ledger; the snapshot is just another publication artifact.

The snapshot is opt-in (default `enabled = false`) and respects the same `--accept-publishing` first-run gate as P6-T6.

**Files:**

- Create: `crates/vox-publisher/src/atlas/trust_snapshot.rs`
- Modify: `crates/vox-publisher/src/atlas/mod.rs`
- Create: `crates/vox-publisher/tests/trust_snapshot.rs`

### P6-T8a: failing test for `TrustGraphSnapshot` shape and digest stability

- [ ] **Step 1: Write the failing test**

Create `crates/vox-publisher/tests/trust_snapshot.rs`:

```rust
//! P6-T8 tests: trust-graph snapshot.

use vox_publisher::atlas::trust_snapshot::{
    PeerEntry, TrustGraphSnapshot, TrustGraphSnapshotBuilder,
};

fn sample_peer(did: &str, tier: u8, last_paired: u64) -> PeerEntry {
    PeerEntry {
        actor_did: did.to_string(),
        node_pubkey_b64: "AAAA".to_string(),
        trust_tier: tier,
        last_paired_at_unix_ms: last_paired,
        last_disagreement_at_unix_ms: None,
        completed_jobs: 0,
    }
}

#[test]
fn snapshot_round_trips() {
    let snap = TrustGraphSnapshotBuilder::new("did:vox:node-aaaa")
        .peer(sample_peer("did:vox:node-bbbb", 2, 1_715_000_000_000))
        .peer(sample_peer("did:vox:node-cccc", 3, 1_715_000_500_000))
        .build();
    let json = serde_json::to_string(&snap).unwrap();
    let back: TrustGraphSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.peers.len(), 2);
    assert_eq!(back.scope_did, "did:vox:node-aaaa");
}

#[test]
fn snapshot_canonical_bytes_independent_of_peer_insertion_order() {
    let s1 = TrustGraphSnapshotBuilder::new("did:vox:node-aaaa")
        .peer(sample_peer("did:vox:node-bbbb", 2, 1))
        .peer(sample_peer("did:vox:node-cccc", 3, 2))
        .build();
    let s2 = TrustGraphSnapshotBuilder::new("did:vox:node-aaaa")
        .peer(sample_peer("did:vox:node-cccc", 3, 2))
        .peer(sample_peer("did:vox:node-bbbb", 2, 1))
        .build();
    assert_eq!(s1.canonical_bytes(), s2.canonical_bytes());
}

#[test]
fn snapshot_serializes_to_publication_manifest() {
    let snap = TrustGraphSnapshotBuilder::new("did:vox:node-aaaa")
        .peer(sample_peer("did:vox:node-bbbb", 2, 1))
        .build();
    let manifest = snap.into_publication_manifest();
    assert_eq!(manifest.content_type, "vox.trust.snapshot");
    assert!(manifest.body_markdown.contains("did:vox:node-bbbb"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-publisher --test trust_snapshot 2>&1 | tail -10`
Expected: FAIL — `trust_snapshot` module not found.

### P6-T8b: implement `TrustGraphSnapshot`

- [ ] **Step 1: Create the snapshot module**

`crates/vox-publisher/src/atlas/trust_snapshot.rs`:

```rust
//! Trust-graph snapshot finding (P6-T8).
//!
//! Periodic (default every 24h) snapshot of the local trust ledger
//! published as a Scientia Finding through the existing `vox-publisher`
//! pipeline. Aggregated across opted-in nodes, the snapshots form a
//! public auditable trust map without a Vox-owned ledger.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerEntry {
    pub actor_did: String,
    pub node_pubkey_b64: String,
    /// 0=Untrusted, 1=Probationary, 2=Established, 3=Vetted, 4=Federated.
    pub trust_tier: u8,
    pub last_paired_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_disagreement_at_unix_ms: Option<u64>,
    pub completed_jobs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustGraphSnapshot {
    pub scope_did: String,
    pub peers: Vec<PeerEntry>,
    pub generated_at_unix_ms: u64,
}

impl TrustGraphSnapshot {
    /// Canonical bytes: peers sorted by actor_did, generated_at zeroed.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut clone = self.clone();
        clone.generated_at_unix_ms = 0;
        clone.peers.sort_by(|a, b| a.actor_did.cmp(&b.actor_did));
        serde_json::to_vec(&clone).unwrap_or_default()
    }

    pub fn into_publication_manifest(self) -> crate::publication::PublicationManifest {
        let title = format!(
            "Vox trust-graph snapshot ({} peers, scope {})",
            self.peers.len(),
            self.scope_did,
        );
        let body = render_snapshot_markdown(&self);
        crate::publication::PublicationManifest {
            publication_id: format!("vox-trust-snapshot-{}-{}", self.scope_did, self.generated_at_unix_ms),
            content_type: "vox.trust.snapshot".to_string(),
            source_ref: Some(self.scope_did.clone()),
            title,
            author: "Vox Mesh Trust Ledger".to_string(),
            abstract_text: Some(format!(
                "Self-published trust-ledger snapshot from {}",
                self.scope_did
            )),
            body_markdown: body,
            citations_json: None,
            metadata_json: serde_json::to_string(&self).ok(),
        }
    }
}

fn render_snapshot_markdown(s: &TrustGraphSnapshot) -> String {
    let mut out = format!(
        "# Trust snapshot — {} ({} peers)\n\n",
        s.scope_did,
        s.peers.len()
    );
    out.push_str("| peer DID | tier | jobs | last paired (unix ms) | last disagreement |\n");
    out.push_str("|---|---|---|---|---|\n");
    for p in &s.peers {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            p.actor_did,
            p.trust_tier,
            p.completed_jobs,
            p.last_paired_at_unix_ms,
            p.last_disagreement_at_unix_ms
                .map(|x| x.to_string())
                .unwrap_or_else(|| "—".to_string()),
        ));
    }
    out
}

#[derive(Debug, Default)]
pub struct TrustGraphSnapshotBuilder {
    scope_did: String,
    peers: Vec<PeerEntry>,
}

impl TrustGraphSnapshotBuilder {
    pub fn new(scope_did: impl Into<String>) -> Self {
        Self {
            scope_did: scope_did.into(),
            peers: Vec::new(),
        }
    }
    pub fn peer(mut self, p: PeerEntry) -> Self {
        self.peers.push(p);
        self
    }
    pub fn build(self) -> TrustGraphSnapshot {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        TrustGraphSnapshot {
            scope_did: self.scope_did,
            peers: self.peers,
            generated_at_unix_ms: now_ms,
        }
    }
}
```

- [ ] **Step 2: Wire into `atlas/mod.rs`**

```rust
pub mod trust_snapshot;
```

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-publisher --test trust_snapshot 2>&1 | tail -10`
Expected: PASS for all three tests.

### P6-T8c: scheduled emission glue (vox-skill)

- [ ] **Step 1: Add a runner under `vox-populi`**

Append to `crates/vox-populi/src/mens/discovery_publish.rs` (gated under the same `mesh-discovery-publish` feature):

```rust
#[cfg(feature = "mesh-discovery-publish")]
pub async fn run_trust_snapshot_cycle(scope_did: &str) -> Result<usize> {
    use vox_publisher::atlas::trust_snapshot::TrustGraphSnapshotBuilder;
    let mut builder = TrustGraphSnapshotBuilder::new(scope_did.to_string());
    for p in load_local_peers()? {
        builder = builder.peer(p);
    }
    let snap = builder.build();
    let n = snap.peers.len();
    let manifest = snap.into_publication_manifest();
    let _digest = manifest.content_sha3_256();
    tracing::info!(
        "vox.mesh.trust_snapshot.emitted" = n as u64,
        publication_id = manifest.publication_id.as_str(),
    );
    Ok(n)
}

#[cfg(feature = "mesh-discovery-publish")]
fn load_local_peers() -> Result<Vec<vox_publisher::atlas::trust_snapshot::PeerEntry>> {
    // Real impl: read vox-db's mesh_peer_trust table. Phase 6 ships an
    // empty collector so the seam is testable.
    Ok(Vec::new())
}
```

- [ ] **Step 2: Run with the feature on**

Run: `cargo build -p vox-populi --features mesh-discovery-publish 2>&1 | tail -10`
Expected: clean build.

### P6-T8d: commit

- [ ] **Commit**

```bash
git add crates/vox-publisher/src/atlas/trust_snapshot.rs \
        crates/vox-publisher/src/atlas/mod.rs \
        crates/vox-publisher/tests/trust_snapshot.rs \
        crates/vox-populi/src/mens/discovery_publish.rs
git commit -m "feat(publisher,populi): P6-T8 trust-graph snapshot self-publication

Periodic Scientia Finding emission of the local trust ledger via the
existing vox-publisher pipeline. Default-disabled; aggregates across
opted-in nodes into a public auditable trust map."
```

---

## Task P6-T9: Mesh-replicated hopper (Option C) (Hp-T1+T5+T8 mesh adapter)

**Goal.** When mesh dispatch is authoritative (P0-T3 landed) and the federation envelope is in
place (P6-T1), the unified-task hopper's persistent inbox (Option B, P3-T9 `HopperInboxProjection`)
gossips across paired peers via the same Bloom-filter anti-entropy mechanism that op-fragments
already use. No new substrate; one transport adapter.

**Files:**

- Create: `crates/vox-orchestrator/src/hopper/mesh_adapter.rs`
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` — add `HopperOpSync` message kind
- Modify: `crates/vox-orchestrator/src/hopper/mod.rs` — wire mesh adapter behind feature flag
- Test: `tests/hopper_mesh_replication.vox` (NOT .py, NOT .sh — Vox-only per AGENTS.md)

- [ ] **Step 1: Failing test**

  Two daemons paired with valid GitHub attestation. Daemon A admits a hopper item; daemon B
  reorders it. Within 30 s, both daemons converge to the same priority via op-log gossip.
  Test fails until the mesh adapter is wired.

- [ ] **Step 2: `HopperOpSync` message kind**

  Extends the federation envelope (`P6-T1`). The hopper's three op variants
  (`HopperItemAdmitted`, `HopperItemOverridden`, `HopperItemTransitioned`) ride on the existing
  signed-Ed25519 envelope. The `DeveloperOverride` capability is verified by the receiving
  daemon using the sender daemon's pubkey from `[mesh.trust]`.

- [ ] **Step 3: Conflict resolution**

  Two developers reorder simultaneously on different nodes. Both events emit
  `HopperItemOverridden` with monotonic `delta_seconds_since_admit`. The op-log's
  `predecessor_hash` chain plus Bloom-filter sync converges to a single state. Last-writer-wins
  on the priority value; the audit trail shows both overrides.

- [ ] **Step 4: Trust-tier admission**

  Mesh-replicated hopper requires `WorkerDonationPolicy.accepts_remote_intake = true` and
  trust-tier ≥ `Vetted` (default 3). Lower tiers refuse mesh-replicated intake; intake from
  unpaired peers is rejected at the envelope verifier.

- [ ] **Step 5: Acceptance test**

  `cargo test -p vox-orchestrator hopper_mesh_replication` — two-daemon convergence within 30 s,
  `DeveloperOverride` envelope signature verifies on the receiver, audit trail preserves both
  override events.

**Per-task acceptance:**

- Daemon A admits → daemon B sees within ≤ 30 s.
- Daemon A reorders → daemon B converges within ≤ 30 s.
- A peer attempting to forge a `DeveloperOverride` is rejected with signature failure (cite
  `vox/mesh/capability-forgery`).
- Reverting a peer's pairing causes pending `HopperItemAdmitted` ops from that peer to tombstone.

**Dependencies.** P0-T3 (authoritative leases), P3 complete (op-log substrate + projections),
P5-T1 (Ed25519-signed envelope), P5-T2 (GitHub-attested pairing), P6-T1 (federation envelope).

**Commit message footer:** `(P6-T9, Hp-T1+T5+T8 mesh adapter — completes hopper Option C)`.

---

## Acceptance

The SSOT acceptance criteria for Phase 6, plus the verification commands that prove each one in this plan:

### A1 — Two strangers pair via published manifests; share compute on a deterministic Embed task with redundant-execution; kudos credit reconciles to within ε

- Inputs: Alice and Bob run the [Grand Network Quickstart](../how-to/grand-network-quickstart.md). Alice publishes her attestation; Bob `vox populi join`s.
- Verifier: an integration test launching two `vox populi` nodes in-process (same workspace, distinct ports), publishing both manifests, calling `populi_join::run` on each side, and dispatching an `Embed` task with `RedundancyPolicy::boinc_default()`.
- Tolerance: `ε = 1` kudos unit (ledger uses integer ms; integer ε avoids floating-point flake).
- Pass when: both kudos balances on each side equal each other to within 1 unit after the test completes.

### A2 — TrainQLoRA result attests via signed deterministic replay on first epoch; submitter spot-check passes; loss curve matches second-runner within tolerance

- Phase 6 ships the *envelope* (`Attestation.replay_proof_blake3_hex`) but the deterministic replay implementation lives in Phase 4. This plan is satisfied when the field round-trips and the verifier interface compiles.
- Pass when: `cargo test -p vox-mesh-types --test tee_attestation` shows the round-trip + omission-when-None tests passing.

### A3 — Revoking a contributor (compromised GitHub identity) propagates via gossip; new dispatches refuse them within ≤ 5 min

- Implemented as: cache TTL on `PublicAttestationManifest` (operator-tunable, default 24h); revocation is "delete the gist," and counterparties refresh on TTL expiry.
- Phase 6 ships the cache; the actual ≤ 5 min figure is configured by setting `expiry_unix_ms` in the publishing manifest at +5 min, or by the operator running `vox populi attest fetch <url>` and checking the verify result on a fast schedule.
- Pass when: deleting the gist causes the next `populi_attest::fetch` to fail with `error_for_status()`, and the cache `latest()` returns the now-stale entry which the orchestrator rejects.

### A4 — Scientia feedback loop publishes a Vox Provider Atlas quarterly Finding sourced from real mesh telemetry

- Implemented as: `discovery_publish::run_one_cycle` emits a `ProviderAtlasFinding` when `enabled = true` and there are observations. Real telemetry collection sits behind the `mesh-discovery-publish` feature and reads from `vox-db`.
- Pass when: `cargo test -p vox-publisher --test provider_atlas` confirms the finding shape, and a manual `cargo run --features mesh-discovery-publish` cycle (with `enabled = true` and a synthetic observation injected) emits a publication digest.

### Plan-level acceptance

A pull request that lands all eight tasks (P6-T1..P6-T8) must additionally pass:

- `cargo test --workspace 2>&1 | tail -20` — clean.
- `cargo run -p vox-arch-check 2>&1 | tail -10` — no new layer/LoC violations.
- `cargo build --workspace --all-features 2>&1 | tail -10` — clean.
- `cargo build --workspace --no-default-features 2>&1 | tail -10` — clean.
- The mesh-replicated hopper (Option C) converges admission and reprioritization across two
  paired peers within ≤ 30 s; `DeveloperOverride` capability forgery is rejected at the envelope
  verifier and surfaced in the dashboard audit log.

---

## Rollback

Phase 6 is fully additive: every new field is `#[serde(default, skip_serializing_if = "Option::is_none")]`, every new module is gated by a feature flag or is consumer-opt-in, and no existing public API changes signature. Rollback per task is a clean `git revert` of the corresponding commit:

| Task | Rollback impact |
|------|------------------|
| P6-T1 | Removes `OpFragmentEnvelope` and `FederationEnvelope`. No call sites in the worktree consume them yet (Phase 3 SSOT references are doc-only); no orchestrator change needed. |
| P6-T2 | Removes `PublicAttestationManifest` and `vox populi attest` subcommand. Operators who already published a manifest still have the file on the server side; nothing on Vox's end depends on it. |
| P6-T3 | Removes `Tier` enum, `MicroVmRuntime`, and `plan_for_min_tier`. Existing `SkillRuntime` impls keep working because the `tier()` method is defaulted to `Tier::Container` — reverting *the trait change too* requires also reverting the `tier()` defaulted method, but downstream impls do not need to. |
| P6-T4 | Removes `RedundancyPolicy`. Existing `WorkerDonationPolicy` field is `Option<…>` — old serialized policies on disk continue to deserialize after revert (the field becomes `serde(skip)` upstream). |
| P6-T5 | Removes `Attestation` block. `TaskResult` regresses to the pre-P6 shape; serialized results carrying `attestation` will still parse (unknown-field tolerance per `serde`'s default). |
| P6-T6 | Removes the `mesh-discovery-publish` feature and `provider_atlas`. Default builds were never broken because the skill module is gated. |
| P6-T7 | Removes `vox populi join`. The `vox populi attest publish` subcommand from P6-T2 is unaffected — operators can still ship manifests, they just can't auto-pair. |
| P6-T8 | Removes `TrustGraphSnapshot` and the trust-snapshot cycle. P6-T6's discovery loop is unaffected. |

A partial rollback (e.g., revert only P6-T7) is supported because the tasks are layered: T7 depends on T2; T8 depends on T6; T4 depends on T3. Revert in reverse-dependency order to keep the workspace green.

If the Phase needs to be backed out wholesale, the safest sequence is `revert P6-T8 → P6-T7 → P6-T6 → P6-T5 → P6-T4 → P6-T3 → P6-T2 → P6-T1`. Run `cargo test --workspace` after each revert to confirm green.

---

## Self-review

- **SSOT coverage.** Every P6 task ID has a numbered task with a TDD step (failing test → implementation → passing test → commit). No task is left to "follow-up": even the deferred-impl tasks (T3 micro-VM, T5 TEE) ship a usable mock that exercises the surrounding planner / serialization paths.
- **Anti-goal scan.** No code path introduces a Vox-owned discovery server, a token, or a TEE-first dependency. The discovery feedback loop is opt-in (default `enabled = false`) and the trust-snapshot publisher is opt-in. `vox populi join` is operator-confirmed (`--yes` is required for non-interactive use). The federation envelope adopts ATProto/ForgeFed *shape* (JSON-LD-shaped, strict-JSON parsed) without ActivityPub HTTP semantics, Webfinger lookup, or LD-context resolution.
- **Type consistency.** `OpFragmentEnvelope`, `FederationEnvelope`, `PublicAttestationManifest`, `RedundancyPolicy`, `TrustTier`, `Tier`, `Attestation`, `TeeQuote`, `TeeVerifier`, `ProviderAtlasFinding`, `TrustGraphSnapshot`, `Invite` are each defined exactly once and re-exported from a single canonical module. New `serde` fields are all `#[serde(default, skip_serializing_if = "Option::is_none")]` to preserve backward compatibility.
- **TDD integrity.** Each task starts with a failing test (RED), then implements minimum-necessary code (GREEN), then commits. Where a task is interface-only (T3 mock, T5 stub verifier), the test asserts the interface contract (e.g. `NotImplemented` for every `TeeQuoteKind`).
- **Auto-generated file discipline.** No edits to `SUMMARY.md`, `architecture-index.md`, or `feed.xml`. `research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored. The howto in `docs/src/how-to/grand-network-quickstart.md` is a hand-authored content file that the indexer will pick up on the next regeneration.
- **Known limitations (intentional).** Real firecracker/kata launch (T3), real vendor-specific TEE verification (T5), and the orchestrator dispatch coupling for `RedundancyPolicy` (T4 ships the types and voting helper; the dispatch path that calls `decide_replicas` is a separate orchestrator commit not in this plan) are deferred to v1.x.

---

## Revision history

- **2026-05-09.** Initial implementation plan for Phase 6 of the Mesh & Language-Distribution SSOT.
