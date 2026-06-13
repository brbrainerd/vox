//! Unified task hopper — Hp-T1 (L1 module, Option A single-machine).
//!
//! The hopper is the intake funnel for all developer-sourced work. Items flow:
//!   submit → Inbox → Assigned → Done
//!   (any non-terminal state can be Overridden by a DeveloperOverride cap)
//!
//! ## Architecture
//!
//! - `types` — pure domain types (IntakeItem, ItemState, PriorityHint, …)
//! - `capability` — DeveloperOverride capability token + minter
//! - `store` — HopperIntake trait + InMemoryHopper (Option A)
//!
//! When Hp-T5 lands (vox-db hopper_inbox table), swap `InMemoryHopper` for
//! the persistent impl through `Arc<dyn HopperIntake>` — the dashboard adapter
//! and HTTP handlers need no changes.

pub mod capability;
/// P6-T9: mesh-replicated hopper adapter (Hp-T1+T5+T8 over the federation envelope).
pub mod mesh_adapter;
pub mod store;
pub mod types;

pub use crate::types::PrioritySource;
pub use capability::{DeveloperOverride, DeveloperOverrideMint};
pub use mesh_adapter::{HopperOpSync, MIN_INTAKE_TRUST_TIER, ReplicationError, apply_op_fragment};
pub use store::{AdmittedReplay, HopperError, HopperIntake, InMemoryHopper};
pub use types::{
    HopperItemId, IntakeItem, IntakeSource, ItemState, PriorityHint, PriorityOverrideRecord,
};
