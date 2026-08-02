use thiserror::Error;

use crate::CircuitBreakerError;

/// Store operation failure (Turso, not-found, or serialization).
#[derive(Error, Debug)]
pub enum StoreError {
    /// Generic database-layer message.
    #[error("Database error: {0}")]
    Db(String),
    /// Underlying Turso / libSQL error.
    #[error(transparent)]
    Turso(#[from] turso::Error),
    /// Local filesystem I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Requested row or binding was missing.
    #[error("Not found: {0}")]
    NotFound(String),
    /// Invalid migration version or sequence.
    #[error("Invalid migration: {0}")]
    InvalidMigration(String),
    /// Stable identity columns would change for an existing natural key (`idempotency_key` / adapter id).
    #[error("upsert_identity_mismatch: {0}")]
    UpsertIdentityMismatch(String),
    /// JSON or other serialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Invalid UTF-8 in blob payload.
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    /// Database `schema_version` is not the current Arca baseline (see [`crate::schema::BASELINE_VERSION`]).
    #[error(
        "legacy or non-baseline Arca schema (schema_version max={max_version}, expected baseline {}): export with `vox codex export-legacy`, initialize a fresh Codex database, then `vox codex import-legacy`",
        crate::schema::BASELINE_VERSION
    )]
    LegacySchemaChain {
        /// Highest `schema_version.version` present before baseline migration.
        max_version: i64,
    },
    /// [`crate::DbCircuitBreaker`] is open (too many consecutive write failures).
    #[error(transparent)]
    CircuitBreaker(#[from] CircuitBreakerError),
    /// Internal actor or system failure.
    #[error("Internal error: {0}")]
    Internal(String),
    /// Phase 1 of [`crate::migration::migrate_dropping_quarantine`] found rows in one or more
    /// tables slated for `DROP TABLE`; the drop was refused and the database was left untouched.
    #[error(
        "quarantine drop aborted: non-empty quarantined table(s) found: {}. \
         The database is pinned below schema version {} until this is resolved (this will recur on every \
         connect that attempts this migration). Remediation: export the row(s) in the listed table(s) \
         (e.g. `SELECT * FROM <table>`), then either delete them or move them to their new home, then retry.",
        tables.join(", "),
        crate::schema::BASELINE_VERSION
    )]
    QuarantineDropAborted {
        /// Names of the non-empty tables that blocked the drop.
        tables: Vec<String>,
    },
}
