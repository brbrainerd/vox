//! Arca SQL: context-archive dedup/compression support (design 2026-06-20 §4).
pub const SCHEMA_CONTEXT_ARCHIVE: &str = r#"
CREATE TABLE IF NOT EXISTS chunk_members (
    item_hash  TEXT NOT NULL,
    ordinal    INTEGER NOT NULL,
    chunk_hash TEXT NOT NULL REFERENCES objects(hash),
    PRIMARY KEY (item_hash, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_chunk_members_chunk ON chunk_members(chunk_hash);

CREATE TABLE IF NOT EXISTS archive_membership (
    window_id TEXT NOT NULL,
    ref_hash  TEXT NOT NULL REFERENCES objects(hash),
    PRIMARY KEY (window_id, ref_hash)
);
CREATE INDEX IF NOT EXISTS idx_archive_membership_hash ON archive_membership(ref_hash);

CREATE TABLE IF NOT EXISTS zstd_dictionaries (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    version      INTEGER NOT NULL,
    bytes        BLOB    NOT NULL,
    sample_count INTEGER NOT NULL DEFAULT 0,
    trained_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    notes        TEXT
);
"#;
