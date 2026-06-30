//! Legacy-only migration surfaces.
//!
//! This module is the explicit boundary for transitional pathways that should not
//! grow as first-class peers to baseline schema surfaces.
//!
//! Deletion criteria:
//! - pre-baseline `schema_version` chain upgrades are no longer supported in release policy
//! - legacy JSONL import/export workflows have no remaining operator dependents
//! - extended Ludus cutover DDL is fully represented by baseline schema fragments

use turso::Connection;

/// Canonical legacy import/export planning surface.
#[cfg(feature = "legacy-import")]
#[allow(deprecated)] // intentional re-export of the deprecated legacy surface
pub use crate::codex_legacy as codex;
/// Canonical optional extra importer surface.
#[cfg(feature = "legacy-import")]
pub use crate::legacy_import_extras as import_extras;

/// Apply legacy schema-cutover alignment for pre-baseline databases.
pub async fn apply_schema_cutover(conn: &Connection) -> Result<(), crate::StoreError> {
    crate::schema_extensions::apply_schema_extensions(conn).await
}
