//! Arca SQL: Package registry and components.
pub const SCHEMA_PACKAGES: &str = "
CREATE TABLE IF NOT EXISTS packages (
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    hash TEXT NOT NULL REFERENCES objects(hash),
    description TEXT,
    author TEXT,
    license TEXT,
    yanked INTEGER NOT NULL DEFAULT 0,
    published_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (name, version)
);

-- package_deps: quarantined (DEAD, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS components (
    name TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    schema_hash TEXT REFERENCES objects(hash),
    description TEXT,
    version TEXT NOT NULL DEFAULT '0.1.0',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_packages_hash ON packages(hash);
CREATE INDEX IF NOT EXISTS idx_components_namespace ON components(namespace);
";
