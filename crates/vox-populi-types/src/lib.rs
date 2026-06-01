//! Pure-data types for the Vox Populi mesh layer.
//!
//! This crate holds the topology and node-record types that sit at L2 (same layer as
//! [`vox-repository`]). They cannot live at L0 (`vox-mesh-types`) because
//! [`NodeRecord`] embeds [`vox_repository::TaskCapabilityHints`] (also L2).
//!
//! ## Design
//!
//! - No async runtime, no database, no HTTP client.
//! - All types implement `Debug + Clone + Serialize + Deserialize`.
//! - [`vox-populi`] (L3) depends on this crate and re-exports its public surface for
//!   backwards compatibility.  Callers can depend directly on `vox-populi-types` when
//!   they only need the data types.
//!
//! ## See also
//!
//! * [ADR-042](../../docs/src/architecture/adr-042-vox-populi-types.md)

pub mod node_record;

pub use node_record::{
    MAX_MAINTENANCE_FOR_MS, NodeRecord, PopuliRegistryError, PopuliRegistryFile,
    filter_registry_by_max_stale_ms, merge_registry_by_last_seen, node_maintenance_blocks_new_work,
    sweep_expired_maintenance_on_nodes,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_maintenance_is_seven_days_in_ms() {
        // Documented contract: 7d cap on operator maintenance windows.
        assert_eq!(MAX_MAINTENANCE_FOR_MS, 7 * 24 * 60 * 60 * 1000);
    }
}
