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
//! ## Current status (Phase 6)
//!
//! This is a **stub**. The `HopperOpSync` type and the `MIN_INTAKE_TRUST_TIER`
//! constant are wired in so that the federation envelope layer can route
//! `OpFragmentKind::HopperSync` messages to this module. Actual application
//! of incoming sync ops to the local hopper inbox is deferred to Hp-T5/T8
//! when the persistent `hopper_inbox` table is available.

use serde::{Deserialize, Serialize};

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
