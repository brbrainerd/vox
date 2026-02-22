use crate::hash::content_hash;
use crate::schema::{MIGRATIONS, PRAGMAS};
use rusqlite::{params, Connection};
use thiserror::Error;

/// Parameters for logging an execution
pub struct LogExecutionParams<'a> {
    pub workflow_id: &'a str,
    pub activity_name: &'a str,
    pub status: &'a str,
    pub attempt: u32,
    pub input: Option<&'a [u8]>,
    pub output: Option<&'a [u8]>,
    pub error: Option<&'a str>,
    pub options: Option<&'a str>,
}

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Content-addressed code store backed by SQLite.
/// Inspired by Unison's codebase format and Convex's data model.
pub struct CodeStore {
    conn: Connection,
}

impl CodeStore {
    /// Apply SQLite pragmas for performance and concurrency.
    fn apply_pragmas(conn: &Connection) -> Result<(), StoreError> {
        conn.execute_batch(PRAGMAS)?;
        Ok(())
    }

    /// Run schema migrations, applying only those not yet applied.
    fn migrate(conn: &Connection) -> Result<(), StoreError> {
        // Ensure schema_version table exists (bootstrap)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;

        let current_version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;

        for &(version, sql) in MIGRATIONS {
            if version > current_version {
                conn.execute_batch(sql)?;
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    params![version],
                )?;
            }
        }

        Ok(())
    }

    /// Open or create a code store at the given path.
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Self::apply_pragmas(&conn)?;
        Self::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory code store (for testing).
    pub fn open_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        // WAL mode doesn't apply to in-memory, but other pragmas do
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Self::migrate(&conn)?;
        Ok(Self { conn })
    }

    // ── Core CAS Operations ──────────────────────────────

    /// Store a definition, returning its content hash.
    pub fn store(&self, kind: &str, data: &[u8]) -> Result<String, StoreError> {
        let hash = content_hash(data);
        self.conn.execute(
            "INSERT OR IGNORE INTO objects (hash, kind, data) VALUES (?1, ?2, ?3)",
            params![hash, kind, data],
        )?;
        Ok(hash)
    }

    /// Retrieve a definition by its content hash.
    pub fn get(&self, hash: &str) -> Result<Vec<u8>, StoreError> {
        let data: Vec<u8> = self.conn.query_row(
            "SELECT data FROM objects WHERE hash = ?1",
            params![hash],
            |row| row.get(0),
        )?;
        Ok(data)
    }

    // ── Name Binding ─────────────────────────────────────

    /// Bind a name to a hash in a namespace.
    pub fn bind_name(&self, namespace: &str, name: &str, hash: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO names (namespace, name, hash) VALUES (?1, ?2, ?3)",
            params![namespace, name, hash],
        )?;
        Ok(())
    }

    /// Look up a name in a namespace, returning its hash.
    pub fn lookup_name(&self, namespace: &str, name: &str) -> Result<String, StoreError> {
        let hash: String = self
            .conn
            .query_row(
                "SELECT hash FROM names WHERE namespace = ?1 AND name = ?2",
                params![namespace, name],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::NotFound(format!("{namespace}.{name}")))?;
        Ok(hash)
    }

    /// Rename a definition without changing its hash.
    pub fn rename(
        &self,
        namespace: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), StoreError> {
        let hash = self.lookup_name(namespace, old_name)?;
        self.bind_name(namespace, new_name, &hash)?;
        self.conn.execute(
            "DELETE FROM names WHERE namespace = ?1 AND name = ?2",
            params![namespace, old_name],
        )?;
        Ok(())
    }

    /// List all names in a namespace.
    pub fn list_names(&self, namespace: &str) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, hash FROM names WHERE namespace = ?1 ORDER BY name")?;
        let rows = stmt.query_map(params![namespace], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Causal Dependencies ──────────────────────────────

    /// Record a causal dependency (this hash depends on parent_hash).
    pub fn add_dependency(&self, hash: &str, parent_hash: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO causal (hash, parent_hash) VALUES (?1, ?2)",
            params![hash, parent_hash],
        )?;
        Ok(())
    }

    /// Get all dependencies of a hash.
    pub fn get_dependencies(&self, hash: &str) -> Result<Vec<String>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT parent_hash FROM causal WHERE hash = ?1")?;
        let rows = stmt.query_map(params![hash], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Metadata ─────────────────────────────────────────

    /// Set a metadata key-value pair on an object.
    pub fn set_metadata(&self, hash: &str, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (hash, key, value) VALUES (?1, ?2, ?3)",
            params![hash, key, value],
        )?;
        Ok(())
    }

    /// Get a metadata value for an object.
    pub fn get_metadata(&self, hash: &str, key: &str) -> Result<String, StoreError> {
        let value: String = self
            .conn
            .query_row(
                "SELECT value FROM metadata WHERE hash = ?1 AND key = ?2",
                params![hash, key],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::NotFound(format!("metadata {hash}.{key}")))?;
        Ok(value)
    }

    /// Get all metadata for an object.
    pub fn get_all_metadata(&self, hash: &str) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM metadata WHERE hash = ?1 ORDER BY key")?;
        let rows = stmt.query_map(params![hash], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Package Registry ─────────────────────────────────

    /// Publish a package version.
    pub fn publish_package(
        &self,
        name: &str,
        version: &str,
        hash: &str,
        description: Option<&str>,
        author: Option<&str>,
        license: Option<&str>,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO packages (name, version, hash, description, author, license)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![name, version, hash, description, author, license],
        )?;
        Ok(())
    }

    /// Add a dependency to a package.
    pub fn add_package_dep(
        &self,
        package_name: &str,
        package_version: &str,
        dep_name: &str,
        dep_version_req: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO package_deps (package_name, package_version, dep_name, dep_version_req)
             VALUES (?1, ?2, ?3, ?4)",
            params![package_name, package_version, dep_name, dep_version_req],
        )?;
        Ok(())
    }

    /// Get all versions of a package.
    pub fn get_package_versions(&self, name: &str) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT version, hash FROM packages WHERE name = ?1 ORDER BY published_at DESC",
        )?;
        let rows = stmt.query_map(params![name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Get dependencies of a specific package version.
    pub fn get_package_deps(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT dep_name, dep_version_req FROM package_deps
             WHERE package_name = ?1 AND package_version = ?2
             ORDER BY dep_name",
        )?;
        let rows = stmt.query_map(params![package_name, package_version], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Execution Log ────────────────────────────────────

    // Moved out

    /// Append an entry to the execution log (append-only).
    pub fn log_execution<'a>(&self, params: LogExecutionParams<'a>) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO execution_log
             (workflow_id, activity_name, status, attempt, input, output, error, options)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                params.workflow_id,
                params.activity_name,
                params.status,
                params.attempt,
                params.input,
                params.output,
                params.error,
                params.options
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get execution history for a workflow.
    pub fn get_execution_history(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<ExecutionEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, activity_name, status, attempt, error, created_at
             FROM execution_log WHERE workflow_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Ok(ExecutionEntry {
                id: row.get(0)?,
                activity_name: row.get(1)?,
                status: row.get(2)?,
                attempt: row.get(3)?,
                error: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Scheduled Functions ──────────────────────────────

    /// Schedule a function for future execution.
    pub fn schedule_function(
        &self,
        function_hash: &str,
        args: Option<&[u8]>,
        run_at: &str,
        cron_expr: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO scheduled (function_hash, args, run_at, cron_expr)
             VALUES (?1, ?2, ?3, ?4)",
            params![function_hash, args, run_at, cron_expr],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get pending scheduled functions that are due.
    pub fn get_due_scheduled(&self, now: &str) -> Result<Vec<ScheduledEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, function_hash, args, run_at, cron_expr
             FROM scheduled WHERE status = 'pending' AND run_at <= ?1
             ORDER BY run_at ASC",
        )?;
        let rows = stmt.query_map(params![now], |row| {
            Ok(ScheduledEntry {
                id: row.get(0)?,
                function_hash: row.get(1)?,
                args: row.get(2)?,
                run_at: row.get(3)?,
                cron_expr: row.get(4)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Mark a scheduled function as completed.
    pub fn complete_scheduled(&self, id: i64) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE scheduled SET status = 'completed' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    // ── Component Registry ───────────────────────────────

    /// Register a component (skill, workflow, package bundle).
    pub fn register_component(
        &self,
        name: &str,
        namespace: &str,
        schema_hash: Option<&str>,
        description: Option<&str>,
        version: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO components (name, namespace, schema_hash, description, version)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, namespace, schema_hash, description, version],
        )?;
        Ok(())
    }

    /// List components in a namespace.
    pub fn list_components(&self, namespace: &str) -> Result<Vec<ComponentEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT name, namespace, version, description FROM components
             WHERE namespace = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![namespace], |row| {
            Ok(ComponentEntry {
                name: row.get(0)?,
                namespace: row.get(1)?,
                version: row.get(2)?,
                description: row.get(3)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Get the current schema version.
    pub fn schema_version(&self) -> Result<i64, StoreError> {
        let version: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;
        Ok(version)
    }
}

// ── Data types for query results ─────────────────────────

/// An entry from the execution log.
#[derive(Debug, Clone)]
pub struct ExecutionEntry {
    pub id: i64,
    pub activity_name: String,
    pub status: String,
    pub attempt: u32,
    pub error: Option<String>,
    pub created_at: String,
}

/// A scheduled function entry.
#[derive(Debug, Clone)]
pub struct ScheduledEntry {
    pub id: i64,
    pub function_hash: String,
    pub args: Option<Vec<u8>>,
    pub run_at: String,
    pub cron_expr: Option<String>,
}

/// A component registry entry.
#[derive(Debug, Clone)]
pub struct ComponentEntry {
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_store() -> CodeStore {
        CodeStore::open_memory().unwrap()
    }

    // ── Original tests ───────────────────────────────────

    #[test]
    fn test_store_and_retrieve() {
        let store = new_store();
        let hash = store.store("fn", b"fn add(a, b): ret a + b").unwrap();
        let data = store.get(&hash).unwrap();
        assert_eq!(data, b"fn add(a, b): ret a + b");
    }

    #[test]
    fn test_name_binding() {
        let store = new_store();
        let hash = store.store("fn", b"fn greet(): ret \"hello\"").unwrap();
        store.bind_name(".", "greet", &hash).unwrap();
        let found = store.lookup_name(".", "greet").unwrap();
        assert_eq!(found, hash);
    }

    #[test]
    fn test_rename() {
        let store = new_store();
        let hash = store.store("fn", b"fn foo(): ret 42").unwrap();
        store.bind_name(".", "foo", &hash).unwrap();
        store.rename(".", "foo", "bar").unwrap();
        assert!(store.lookup_name(".", "foo").is_err());
        assert_eq!(store.lookup_name(".", "bar").unwrap(), hash);
    }

    #[test]
    fn test_content_addressing_dedup() {
        let store = new_store();
        let h1 = store.store("fn", b"fn id(x): ret x").unwrap();
        let h2 = store.store("fn", b"fn id(x): ret x").unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_list_names() {
        let store = new_store();
        let h1 = store.store("fn", b"fn a(): ret 1").unwrap();
        let h2 = store.store("fn", b"fn b(): ret 2").unwrap();
        store.bind_name("math", "a", &h1).unwrap();
        store.bind_name("math", "b", &h2).unwrap();
        let names = store.list_names("math").unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].0, "a");
        assert_eq!(names[1].0, "b");
    }

    #[test]
    fn test_dependencies() {
        let store = new_store();
        let h1 = store.store("fn", b"fn base(): ret 1").unwrap();
        let h2 = store.store("fn", b"fn derived(): ret base()").unwrap();
        store.add_dependency(&h2, &h1).unwrap();
        let deps = store.get_dependencies(&h2).unwrap();
        assert_eq!(deps, vec![h1]);
    }

    // ── Migration tests ──────────────────────────────────

    #[test]
    fn test_schema_version() {
        let store = new_store();
        let version = store.schema_version().unwrap();
        assert_eq!(version, 2); // V1 + V2 both applied
    }

    // ── Metadata tests ───────────────────────────────────

    #[test]
    fn test_metadata() {
        let store = new_store();
        let hash = store.store("fn", b"fn meta_test(): ret 1").unwrap();

        store.set_metadata(&hash, "author", "alice").unwrap();
        store
            .set_metadata(&hash, "description", "A test function")
            .unwrap();

        assert_eq!(store.get_metadata(&hash, "author").unwrap(), "alice");
        assert_eq!(
            store.get_metadata(&hash, "description").unwrap(),
            "A test function"
        );

        let all = store.get_all_metadata(&hash).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], ("author".to_string(), "alice".to_string()));
    }

    #[test]
    fn test_metadata_not_found() {
        let store = new_store();
        let hash = store.store("fn", b"fn no_meta(): ret 0").unwrap();
        assert!(store.get_metadata(&hash, "missing").is_err());
    }

    // ── Package registry tests ───────────────────────────

    #[test]
    fn test_publish_and_list_package() {
        let store = new_store();
        let hash = store.store("pkg", b"package contents v1").unwrap();

        store
            .publish_package(
                "my-utils",
                "0.1.0",
                &hash,
                Some("Utility functions"),
                Some("alice"),
                Some("MIT"),
            )
            .unwrap();

        let versions = store.get_package_versions("my-utils").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].0, "0.1.0");
        assert_eq!(versions[0].1, hash);
    }

    #[test]
    fn test_package_dependencies() {
        let store = new_store();
        let h1 = store.store("pkg", b"package core").unwrap();
        let h2 = store.store("pkg", b"package app").unwrap();

        store
            .publish_package("core", "1.0.0", &h1, None, None, None)
            .unwrap();
        store
            .publish_package("app", "0.1.0", &h2, None, None, None)
            .unwrap();
        store
            .add_package_dep("app", "0.1.0", "core", ">=1.0.0")
            .unwrap();

        let deps = store.get_package_deps("app", "0.1.0").unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].0, "core");
        assert_eq!(deps[0].1, ">=1.0.0");
    }

    // ── Execution log tests ──────────────────────────────

    #[test]
    fn test_execution_log() {
        let store = new_store();
        let id1 = store
            .log_execution(LogExecutionParams {
                workflow_id: "wf-1",
                activity_name: "build",
                status: "completed",
                attempt: 1,
                input: Some(b"input-data"),
                output: Some(b"output-data"),
                error: None,
                options: None,
            })
            .unwrap();
        let id2 = store
            .log_execution(LogExecutionParams {
                workflow_id: "wf-1",
                activity_name: "test",
                status: "failed",
                attempt: 2,
                input: None,
                output: None,
                error: Some("timeout"),
                options: Some(r#"{"retries":3}"#),
            })
            .unwrap();

        assert!(id2 > id1);

        let history = store.get_execution_history("wf-1").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].activity_name, "build");
        assert_eq!(history[0].status, "completed");
        assert_eq!(history[1].activity_name, "test");
        assert_eq!(history[1].status, "failed");
        assert_eq!(history[1].error, Some("timeout".to_string()));
    }

    // ── Scheduled function tests ─────────────────────────

    #[test]
    fn test_schedule_and_query() {
        let store = new_store();
        let hash = store.store("fn", b"fn cleanup(): pass").unwrap();

        store
            .schedule_function(&hash, None, "2026-02-17T00:00:00", None)
            .unwrap();
        store
            .schedule_function(
                &hash,
                Some(b"args"),
                "2026-02-18T00:00:00",
                Some("0 0 * * *"),
            )
            .unwrap();

        // Query before first schedule — nothing due
        let due = store.get_due_scheduled("2026-02-16T00:00:00").unwrap();
        assert_eq!(due.len(), 0);

        // Query after first schedule — one due
        let due = store.get_due_scheduled("2026-02-17T12:00:00").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].function_hash, hash);

        // Complete it
        store.complete_scheduled(due[0].id).unwrap();

        // Now only the second one is pending
        let due = store.get_due_scheduled("2026-02-19T00:00:00").unwrap();
        assert_eq!(due.len(), 1);
        assert!(due[0].cron_expr.is_some());
    }

    // ── Component registry tests ─────────────────────────

    #[test]
    fn test_components() {
        let store = new_store();

        store
            .register_component(
                "http-utils",
                "skills",
                None,
                Some("HTTP helper functions"),
                "1.0.0",
            )
            .unwrap();
        store
            .register_component(
                "deploy-pipeline",
                "workflows",
                None,
                Some("CI/CD workflow"),
                "0.2.0",
            )
            .unwrap();

        let skills = store.list_components("skills").unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "http-utils");
        assert_eq!(skills[0].version, "1.0.0");

        let workflows = store.list_components("workflows").unwrap();
        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].name, "deploy-pipeline");
    }
}
