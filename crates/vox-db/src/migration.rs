//! User-defined SQL migrations sharing the same `schema_version` table as built-in Arca migrations.
//!
//! Prefer [`crate::builtin_migrations`] when you need the canonical baseline snapshot as a single
//! migration row at [`crate::schema::BASELINE_VERSION`]. For custom migrations, ensure [`Migration::up_sql`] is compatible with
//! [`turso::Connection::execute_batch`] (no row-returning statements).
//!
//! **Warning:** versions other than [`crate::schema::BASELINE_VERSION`] make `MAX(schema_version) != BASELINE_VERSION`,
//! so the next normal [`crate::VoxDb::connect`] will report [`crate::StoreError::LegacySchemaChain`]. Use custom
//! [`Migration`] rows only on ephemeral DBs, tests, or with a plan to re-baseline the file.

use crate::store::StoreError;

/// One forward migration applied in monotonically increasing [`Self::version`] order.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Must be unique, greater than zero, and strictly increasing across the slice passed to [`crate::VoxDb::apply_migrations`].
    pub version: i64,
    /// Human-readable label (logging only; not stored in DB).
    pub name: String,
    /// Semicolon-separated SQL executed via `execute_batch` when `version` is ahead of the DB.
    pub up_sql: String,
}

impl Migration {
    /// Construct a migration entry (does not run SQL until [`crate::VoxDb::apply_migrations`]).
    pub fn new(version: i64, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            up_sql: up_sql.into(),
        }
    }
}

/// Validate strictly increasing versions and no duplicates.
///
/// **Note:** validation failures are reported as [`StoreError::InvalidMigration`].
pub fn validate_migrations(migrations: &[Migration]) -> Result<(), StoreError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut last = 0i64;
    for migration in migrations {
        if migration.version <= 0 {
            return Err(StoreError::InvalidMigration(
                "migration version must be > 0".to_string(),
            ));
        }
        if migration.version <= last {
            return Err(StoreError::InvalidMigration(
                "migrations must be sorted by increasing version".to_string(),
            ));
        }
        if !seen.insert(migration.version) {
            return Err(StoreError::InvalidMigration(format!(
                "duplicate migration version {}",
                migration.version
            )));
        }
        last = migration.version;
    }
    Ok(())
}

/// Returns the canonical baseline migration at [`crate::schema::BASELINE_VERSION`] from [`crate::schema::baseline_sql`].
pub fn builtin_migrations() -> Vec<Migration> {
    vec![Migration::new(
        crate::schema::BASELINE_VERSION,
        "arca_baseline",
        crate::schema::baseline_sql().to_string(),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_sorted_unique() {
        let migrations = vec![
            Migration::new(1, "one", "CREATE TABLE a(id INTEGER);"),
            Migration::new(2, "two", "CREATE TABLE b(id INTEGER);"),
        ];
        assert!(validate_migrations(&migrations).is_ok());
    }
}
