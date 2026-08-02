// Fixture: stand-in for the real crates/vox-db/src/migration.rs — its
// QUARANTINE_DROP_TABLES const lists every quarantined table generically so
// the existing-DB migration can DROP them by name. The census tool must
// exclude this file from the usage signal too, so a table that ONLY appears
// here (fixture_migration_excluded_table) still classifies as DEAD rather
// than "laundering" into DORMANT (see scripts/db-table-census.vox's
// exclusion-list comment for how this was discovered during Task 9).
pub const QUARANTINE_DROP_TABLES: &[&str] = &["fixture_migration_excluded_table"];
