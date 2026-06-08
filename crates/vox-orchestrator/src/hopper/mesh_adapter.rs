//! Mesh-replicated hopper adapter (P6-T9, Hp-T1+T5+T8 mesh adapter).
//!
//! This module defines `HopperOpSync` — the message kind that rides on the
//! federation envelope (`OpFragmentKind::HopperSync`) for cross-daemon hopper
//! replication. When a second daemon joins the same federation scope, hopper
//! mutations on Daemon A are forwarded to Daemon B via signed federation
//! envelopes, allowing both hoppers to converge without a central broker.
//!
//! ## Architecture
//!
//! ```text
//! Daemon A                              Daemon B
//! ─────────────────────────────────    ──────────────────────────────────
//! HopperIntake::submit(item)            receive OpFragmentEnvelope
//!   → emit HopperOpSync::ItemAdmitted       {kind: HopperSync, object: …}
//!   → wrap in OpFragmentEnvelope        → verify signature
//!   → sign with node Ed25519 key        → apply_sync_op(HopperOpSync::…)
//!   → publish via federation transport  → HopperIntake::replay_admitted(…)
//! ```
//!
//! ## Current status (Phase 6, B4)
//!
//! The **apply seam is live**: [`apply_op_fragment`] verifies a `HopperSync`
//! envelope's signature + the peer's trust tier, decodes the [`HopperOpSync`],
//! and applies an `ItemAdmitted` into the local hopper via
//! [`HopperIntake::replay_admitted`]. What is NOT yet built is the *caller* — a
//! live federation inbound transport that receives `OpFragmentEnvelope`s off the
//! wire and invokes this function. That transport (and persistent durability via
//! the Hp-T5 `hopper_inbox` table) is a separate epic; this module provides the
//! complete, unit-tested logic it will call. `ItemOverridden` / `ItemTransitioned`
//! replication is not applied yet — they return a surfaced
//! [`ReplicationError::UnsupportedOpVariant`] rather than a silent no-op.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use vox_mesh_types::op_fragment::{OpFragmentEnvelope, OpFragmentKind};

use super::store::{AdmittedReplay, HopperIntake};
use super::types::IntakeItem;
use crate::types::TaskPriority;

/// Op variants that ride on the federation envelope for hopper replication.
///
/// Each variant corresponds to a state transition in the `ItemState` FSM
/// (see `crates/vox-orchestrator/src/hopper/types.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum HopperOpSync {
    /// A new item was admitted into the hopper inbox.
    ItemAdmitted {
        item_id: String,
        priority: u8,
        admitted_at_unix_ms: u64,
        task_kind: String,
        admitted_by_node_id: String,
    },
    /// A developer override changed the priority of an in-flight item.
    ItemOverridden {
        item_id: String,
        new_priority: u8,
        override_at_unix_ms: u64,
        override_by_node_id: String,
        delta_seconds_since_admit: i64,
    },
    /// An item transitioned to a new state (Assigned, Done, Cancelled, …).
    ItemTransitioned {
        item_id: String,
        new_state: String,
        transitioned_at_unix_ms: u64,
        by_node_id: String,
    },
}

/// Trust tier required for mesh-replicated hopper intake.
///
/// Peers below this tier have their `HopperOpSync` messages rejected at the
/// envelope verifier. The constant maps to `TrustTier::Vetted` (tier 3) in
/// `vox-mesh-types::redundancy::TrustTier`.
pub const MIN_INTAKE_TRUST_TIER: u8 = 3; // Vetted

// ── Apply seam ──────────────────────────────────────────────────────────────────

/// Why a `HopperSync` envelope was not applied.
#[derive(Debug, thiserror::Error)]
pub enum ReplicationError {
    /// Envelope kind was not `HopperSync`.
    #[error("envelope is not a HopperSync op")]
    WrongKind,
    /// Ed25519 signature did not verify against the supplied peer key.
    #[error("federation signature verification failed")]
    Signature,
    /// Peer's trust tier is below `MIN_INTAKE_TRUST_TIER`.
    #[error("peer trust tier {tier} below required {MIN_INTAKE_TRUST_TIER}")]
    Untrusted { tier: u8 },
    /// `object` did not decode into a `HopperOpSync`.
    #[error("could not decode HopperOpSync: {0}")]
    Decode(String),
    /// A `HopperOpSync` variant that replication does not apply yet.
    #[error("unsupported HopperOpSync variant: {0}")]
    UnsupportedOpVariant(&'static str),
}

/// Verify and apply a `HopperSync` federation envelope into the local hopper (B4).
///
/// `peer_vk` and `peer_trust_tier` are supplied by the (future) federation
/// transport / trust-graph layer; keeping them as inputs makes this function pure
/// and unit-testable. Steps: kind check → Ed25519 signature over
/// [`OpFragmentEnvelope::canonical_signing_bytes`] → trust-tier gate → decode →
/// apply. Only `ItemAdmitted` is applied today (see module docs).
pub async fn apply_op_fragment(
    env: &OpFragmentEnvelope,
    peer_vk: &VerifyingKey,
    peer_trust_tier: u8,
    hopper: &dyn HopperIntake,
) -> Result<IntakeItem, ReplicationError> {
    if env.kind != OpFragmentKind::HopperSync {
        return Err(ReplicationError::WrongKind);
    }

    // Verify signature over canonical bytes (signature_b64 blanked).
    let sig_bytes = B64
        .decode(env.signature.signature_b64.as_bytes())
        .map_err(|_| ReplicationError::Signature)?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ReplicationError::Signature)?;
    let signature = Signature::from_bytes(&sig_arr);
    peer_vk
        .verify(&env.canonical_signing_bytes(), &signature)
        .map_err(|_| ReplicationError::Signature)?;

    // Trust-tier gate (after signature so we only trust an authenticated tier).
    if peer_trust_tier < MIN_INTAKE_TRUST_TIER {
        return Err(ReplicationError::Untrusted {
            tier: peer_trust_tier,
        });
    }

    let op: HopperOpSync = serde_json::from_value(env.object.clone())
        .map_err(|e| ReplicationError::Decode(e.to_string()))?;

    match op {
        HopperOpSync::ItemAdmitted {
            item_id,
            priority,
            admitted_at_unix_ms,
            task_kind,
            admitted_by_node_id,
        } => {
            let replay = AdmittedReplay {
                item_id: crate::events::HopperItemId(item_id),
                classified_priority: TaskPriority::from_u8(priority),
                submitted_at_micros: admitted_at_unix_ms.saturating_mul(1_000),
                task_kind,
                origin_node_id: admitted_by_node_id,
            };
            Ok(hopper.replay_admitted(replay).await)
        }
        HopperOpSync::ItemOverridden { .. } => {
            Err(ReplicationError::UnsupportedOpVariant("ItemOverridden"))
        }
        HopperOpSync::ItemTransitioned { .. } => {
            Err(ReplicationError::UnsupportedOpVariant("ItemTransitioned"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::store::InMemoryHopper;
    use crate::hopper::types::{IntakeSource, ItemState};
    use ed25519_dalek::{Signer, SigningKey};
    use vox_mesh_types::op_fragment::FederationSignature;

    fn signing_key() -> SigningKey {
        // Deterministic key (no rand dependency needed in the test build).
        SigningKey::from_bytes(&[7u8; 32])
    }

    /// Build a signed HopperSync envelope carrying `op`.
    fn signed_envelope(op: &HopperOpSync, sk: &SigningKey) -> OpFragmentEnvelope {
        let mut env = OpFragmentEnvelope {
            context: "https://www.w3.org/ns/activitystreams".to_string(),
            id: "urn:uuid:00000000-0000-0000-0000-0000000000aa".to_string(),
            kind: OpFragmentKind::HopperSync,
            actor: "did:key:zPeerA".to_string(),
            object: serde_json::to_value(op).expect("serialize op"),
            signature: FederationSignature::placeholder("did:key:zPeerA#key-1"),
        };
        let sig = sk.sign(&env.canonical_signing_bytes());
        env.signature.signature_b64 = B64.encode(sig.to_bytes());
        env
    }

    fn admitted() -> HopperOpSync {
        HopperOpSync::ItemAdmitted {
            item_id: "remote-item-001".to_string(),
            priority: 2, // Urgent
            admitted_at_unix_ms: 1_700_000_000_000,
            task_kind: "bugfix".to_string(),
            admitted_by_node_id: "did:key:zPeerA".to_string(),
        }
    }

    #[tokio::test]
    async fn happy_path_applies_admission_with_origin_id() {
        let sk = signing_key();
        let env = signed_envelope(&admitted(), &sk);
        let hopper = InMemoryHopper::headless();

        let item = apply_op_fragment(&env, &sk.verifying_key(), 3, &hopper)
            .await
            .expect("apply ok");

        assert_eq!(item.item_id.0, "remote-item-001");
        assert_eq!(item.classified_priority, TaskPriority::Urgent);
        assert!(matches!(item.source, IntakeSource::Mesh { .. }));
        assert!(matches!(item.state, ItemState::Inbox));
        let inbox = hopper.inbox().await;
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].item_id.0, "remote-item-001");
    }

    #[tokio::test]
    async fn idempotent_on_redelivery() {
        let sk = signing_key();
        let env = signed_envelope(&admitted(), &sk);
        let hopper = InMemoryHopper::headless();

        apply_op_fragment(&env, &sk.verifying_key(), 3, &hopper)
            .await
            .unwrap();
        apply_op_fragment(&env, &sk.verifying_key(), 3, &hopper)
            .await
            .unwrap();

        assert_eq!(hopper.inbox().await.len(), 1, "second apply is a no-op");
    }

    #[tokio::test]
    async fn untrusted_tier_is_rejected() {
        let sk = signing_key();
        let env = signed_envelope(&admitted(), &sk);
        let hopper = InMemoryHopper::headless();

        let err = apply_op_fragment(&env, &sk.verifying_key(), 2, &hopper)
            .await
            .unwrap_err();
        assert!(matches!(err, ReplicationError::Untrusted { tier: 2 }));
        assert_eq!(hopper.inbox().await.len(), 0, "nothing applied");
    }

    #[tokio::test]
    async fn tampered_object_fails_signature() {
        let sk = signing_key();
        let mut env = signed_envelope(&admitted(), &sk);
        // Tamper after signing.
        env.object = serde_json::to_value(HopperOpSync::ItemAdmitted {
            item_id: "evil".into(),
            priority: 2,
            admitted_at_unix_ms: 1,
            task_kind: "x".into(),
            admitted_by_node_id: "did:key:zPeerA".into(),
        })
        .unwrap();
        let hopper = InMemoryHopper::headless();

        let err = apply_op_fragment(&env, &sk.verifying_key(), 3, &hopper)
            .await
            .unwrap_err();
        assert!(matches!(err, ReplicationError::Signature));
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected() {
        let sk = signing_key();
        let mut env = signed_envelope(&admitted(), &sk);
        env.kind = OpFragmentKind::TaskResult;
        let hopper = InMemoryHopper::headless();

        let err = apply_op_fragment(&env, &sk.verifying_key(), 3, &hopper)
            .await
            .unwrap_err();
        assert!(matches!(err, ReplicationError::WrongKind));
    }

    #[tokio::test]
    async fn unsupported_variant_is_surfaced() {
        let sk = signing_key();
        let op = HopperOpSync::ItemTransitioned {
            item_id: "remote-item-001".into(),
            new_state: "done".into(),
            transitioned_at_unix_ms: 1,
            by_node_id: "did:key:zPeerA".into(),
        };
        let env = signed_envelope(&op, &sk);
        let hopper = InMemoryHopper::headless();

        let err = apply_op_fragment(&env, &sk.verifying_key(), 3, &hopper)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ReplicationError::UnsupportedOpVariant("ItemTransitioned")
        ));
    }
}
