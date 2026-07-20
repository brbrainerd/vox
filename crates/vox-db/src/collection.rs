//! NoSQL-style document collection backed by SQLite JSON columns.
//!
//! A `Collection` wraps a single SQLite table with the schema:
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS <name> (
//!     _id INTEGER PRIMARY KEY AUTOINCREMENT,
//!     _data TEXT NOT NULL,
//!     _created_at TEXT DEFAULT (datetime('now')),
//!     _updated_at TEXT DEFAULT (datetime('now'))
//! );
//! ```
//!
//! Documents are stored as JSON in the `_data` column.
//! Queries use `json_extract()` for filtering and indexing.

use std::sync::Arc;

use serde_json::Value;
use turso::params;

use crate::sql_util::validate_identifier;

use crate::{DbCircuitBreaker, GuardedConnection};

/// A handle to a schemaless document collection.
///
/// All CRUD operations translate to SQL under the hood:
/// - `insert` → `INSERT INTO <name> (_data) VALUES (?1)`
/// - `get`    → `SELECT _data FROM <name> WHERE _id = ?1`
/// - `find`   → `SELECT _id, _data FROM <name> WHERE json_extract(_data, '$.<key>') = <value>`
/// - `patch`  → `UPDATE <name> SET _data = json_patch(_data, ?1) WHERE _id = ?2`
/// - `delete` → `DELETE FROM <name> WHERE _id = ?1`
pub struct Collection {
    name: String,
    conn: GuardedConnection,
    breaker: Arc<DbCircuitBreaker>,
}

/// Error type for collection operations.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    /// Underlying SQL or libSQL failure.
    #[error("turso error: {0}")]
    Turso(#[from] turso::Error),
    /// Document serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Caller or invariant error (e.g. missing row after insert).
    #[error("{0}")]
    InvalidInput(String),
    /// [`DbCircuitBreaker`] is open.
    #[error(transparent)]
    CircuitBreaker(#[from] crate::CircuitBreakerError),
}

impl Collection {
    /// Create a new collection handle. Does NOT create the underlying table.
    pub fn new(
        name: impl Into<String>,
        conn: GuardedConnection,
        breaker: Arc<DbCircuitBreaker>,
    ) -> Self {
        Self {
            name: name.into(),
            conn,
            breaker,
        }
    }

    /// Borrow the validated table name.
    ///
    /// All callsites that interpolate `self.name` into a SQL string MUST go
    /// through this accessor, never `&self.name` directly, so attacker- or
    /// user-controlled collection names cannot inject SQL.
    fn validated_table(&self) -> Result<&str, CollectionError> {
        validate_identifier(&self.name).map_err(|e| {
            CollectionError::InvalidInput(format!("collection name {:?} {}", self.name, e))
        })
    }

    /// Ensure the underlying table exists.
    pub async fn ensure_table(&self) -> Result<(), CollectionError> {
        let table = self.validated_table()?;
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {table} (
                _id INTEGER PRIMARY KEY AUTOINCREMENT,
                _data TEXT NOT NULL,
                _created_at TEXT DEFAULT (datetime('now')),
                _updated_at TEXT DEFAULT (datetime('now'))
            )"
        );
        self.conn.execute(&sql, ()).await?;
        Ok(())
    }

    /// Insert a JSON document into the collection. Returns the new document's `_id`.
    pub async fn insert(&self, doc: &Value) -> Result<i64, CollectionError> {
        let table = self.validated_table()?;
        let json_str = serde_json::to_string(doc)?;
        let sql = format!("INSERT INTO {table} (_data) VALUES (?1)");
        self.conn.execute(&sql, params![json_str]).await?;

        // Get last inserted row id
        let mut rows = self.conn.query("SELECT last_insert_rowid()", ()).await?;
        let row = rows.next().await?;
        match row {
            Some(r) => Ok(r.get::<i64>(0)?),
            None => Err(CollectionError::InvalidInput(
                "last_insert_rowid returned no rows".into(),
            )),
        }
    }

    /// Get a document by its `_id`. Returns `None` if not found.
    pub async fn get(&self, id: i64) -> Result<Option<Value>, CollectionError> {
        let table = self.validated_table()?;
        let sql = format!("SELECT _data FROM {table} WHERE _id = ?1");
        let mut rows = self.conn.query(&sql, params![id]).await?;
        let row = rows.next().await?;
        match row {
            Some(r) => {
                let json_str: String = r.get::<String>(0)?;
                let val: Value = serde_json::from_str(&json_str)?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// Find documents matching a flat filter object.
    ///
    /// The filter is a JSON object where each key-value pair becomes a
    /// `json_extract(_data, '$.<key>') = <literal>` clause joined by AND.
    ///
    /// Values are safely serialized into SQL literals (no parameter binding needed
    /// since json_extract returns text and we compare against text/integer literals).
    pub async fn find(&self, filter: &Value) -> Result<Vec<(i64, Value)>, CollectionError> {
        let table = self.validated_table()?;
        let obj = filter
            .as_object()
            .ok_or_else(|| CollectionError::InvalidInput("filter must be a JSON object".into()))?;

        if obj.is_empty() {
            return self.list(100, 0).await;
        }

        let mut conditions = Vec::new();
        for (key, val) in obj {
            // JSON-path keys are interpolated directly into the SQL string,
            // so they MUST be validated against the same identifier rule as
            // table names — otherwise a key like `name'); DROP TABLE x; --`
            // breaks out of the json_extract path.
            let key = validate_identifier(key)
                .map_err(|e| CollectionError::InvalidInput(format!("filter key {key:?} {e}")))?;
            let sql_lit = json_to_sql_literal(val);
            conditions.push(format!("json_extract(_data, '$.{key}') = {sql_lit}"));
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!("SELECT _id, _data FROM {table} WHERE {where_clause} LIMIT 100");

        self.query_id_data(&sql).await
    }

    /// Patch a document by merging partial JSON into the existing document.
    ///
    /// Uses SQLite's `json_patch()` function to merge the partial update.
    pub async fn patch(&self, id: i64, partial: &Value) -> Result<(), CollectionError> {
        let _ = partial
            .as_object()
            .ok_or_else(|| CollectionError::InvalidInput("patch must be a JSON object".into()))?;

        let patch_str = serde_json::to_string(partial)?;
        let table = self.validated_table()?.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let sql = format!(
                    "UPDATE {table} SET _data = json_patch(_data, ?1), _updated_at = datetime('now') WHERE _id = ?2"
                );
                conn.execute(&sql, params![patch_str, id]).await?;
                Ok::<(), CollectionError>(())
            })
            .await
    }

    /// Replace the entire document at the given `_id`.
    pub async fn replace(&self, id: i64, doc: &Value) -> Result<(), CollectionError> {
        let json_str = serde_json::to_string(doc)?;
        let table = self.validated_table()?.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let sql = format!(
                    "UPDATE {table} SET _data = ?1, _updated_at = datetime('now') WHERE _id = ?2"
                );
                conn.execute(&sql, params![json_str, id]).await?;
                Ok::<(), CollectionError>(())
            })
            .await
    }

    /// Delete a document by its `_id`.
    pub async fn delete(&self, id: i64) -> Result<(), CollectionError> {
        let table = self.validated_table()?.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let sql = format!("DELETE FROM {table} WHERE _id = ?1");
                conn.execute(&sql, params![id]).await?;
                Ok::<(), CollectionError>(())
            })
            .await
    }

    /// List documents with pagination.
    pub async fn list(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(i64, Value)>, CollectionError> {
        let table = self.validated_table()?;
        let sql = format!("SELECT _id, _data FROM {table} ORDER BY _id ASC LIMIT ?1 OFFSET ?2");
        let mut rows = self.conn.query(&sql, params![limit, offset]).await?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get::<i64>(0)?;
            let json_str: String = row.get::<String>(1)?;
            let val: Value = serde_json::from_str(&json_str)?;
            results.push((id, val));
        }
        Ok(results)
    }

    /// Count documents, optionally filtered.
    pub async fn count(&self, filter: Option<&Value>) -> Result<i64, CollectionError> {
        let table = self.validated_table()?;
        let sql = if let Some(f) = filter {
            let obj = f.as_object().ok_or_else(|| {
                CollectionError::InvalidInput("filter must be a JSON object".into())
            })?;
            if obj.is_empty() {
                format!("SELECT COUNT(*) FROM {table}")
            } else {
                let mut conditions = Vec::new();
                for (key, val) in obj {
                    let key = validate_identifier(key).map_err(|e| {
                        CollectionError::InvalidInput(format!("filter key {key:?} {e}"))
                    })?;
                    let sql_lit = json_to_sql_literal(val);
                    conditions.push(format!("json_extract(_data, '$.{key}') = {sql_lit}"));
                }
                format!(
                    "SELECT COUNT(*) FROM {table} WHERE {}",
                    conditions.join(" AND ")
                )
            }
        } else {
            format!("SELECT COUNT(*) FROM {table}")
        };

        let mut rows = self.conn.query(&sql, ()).await?;
        let row = rows.next().await?;
        match row {
            Some(r) => Ok(r.get::<i64>(0)?),
            None => Ok(0),
        }
    }

    // ── Internal helpers ─────────────────────────────────

    async fn query_id_data(&self, sql: &str) -> Result<Vec<(i64, Value)>, CollectionError> {
        let mut rows = self.conn.query(sql, ()).await?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get::<i64>(0)?;
            let json_str: String = row.get::<String>(1)?;
            let val: Value = serde_json::from_str(&json_str)?;
            results.push((id, val));
        }
        Ok(results)
    }
}

// ── Helpers ─────────────────────────────────────────────

/// Convert a `serde_json::Value` to a SQL literal string for embedding in queries.
///
/// This is safe because the values are always derived from user-provided filter
/// objects and are formatted as typed SQL literals, not concatenated as identifiers.
fn json_to_sql_literal(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            // Escape single quotes by doubling them
            let escaped = s.replace('\'', "''");
            format!("'{escaped}'")
        }
        // Nested objects/arrays are stored as JSON text
        Value::Array(_) | Value::Object(_) => {
            let json_text = serde_json::to_string(v).unwrap_or_default();
            let escaped = json_text.replace('\'', "''");
            format!("'{escaped}'")
        }
    }
}

/// Generate DDL for a collection table.
///
/// Returns `Err(CollectionError::InvalidInput)` if `name` is not a safe SQL
/// identifier (alphanumeric + underscore, must start with letter or
/// underscore, ≤ 64 bytes). This guard is what makes the function safe to
/// hand attacker- or user-controlled names.
pub fn collection_ddl(name: &str) -> Result<String, CollectionError> {
    let name = validate_identifier(name)
        .map_err(|e| CollectionError::InvalidInput(format!("collection name {name:?} {e}")))?;
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {name} (
    _id INTEGER PRIMARY KEY AUTOINCREMENT,
    _data TEXT NOT NULL,
    _created_at TEXT DEFAULT (datetime('now')),
    _updated_at TEXT DEFAULT (datetime('now'))
)"
    ))
}

/// Generate DDL for a `json_extract` expression index on a collection field.
///
/// Both `collection_name` and `field_name` are validated against the same
/// identifier rule as `collection_ddl`.
pub fn collection_index_ddl(
    collection_name: &str,
    field_name: &str,
) -> Result<String, CollectionError> {
    let collection_name = validate_identifier(collection_name).map_err(|e| {
        CollectionError::InvalidInput(format!("collection name {collection_name:?} {e}"))
    })?;
    let field_name = validate_identifier(field_name)
        .map_err(|e| CollectionError::InvalidInput(format!("field name {field_name:?} {e}")))?;
    Ok(format!(
        "CREATE INDEX IF NOT EXISTS idx_{collection_name}_{field_name} \
         ON {collection_name}(json_extract(_data, '$.{field_name}'))"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_ddl() {
        let ddl = collection_ddl("notes").expect("valid identifier");
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS notes"));
        assert!(ddl.contains("_id INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(ddl.contains("_data TEXT NOT NULL"));
        assert!(ddl.contains("_created_at TEXT DEFAULT"));
        assert!(ddl.contains("_updated_at TEXT DEFAULT"));
    }

    #[test]
    fn test_collection_index_ddl() {
        let ddl = collection_index_ddl("notes", "author").expect("valid identifiers");
        assert!(ddl.contains("idx_notes_author"));
        assert!(ddl.contains("json_extract(_data, '$.author')"));
    }

    #[test]
    fn validate_identifier_accepts_normal_names() {
        for ok in [
            "notes",
            "Notes",
            "user_facts",
            "_internal",
            "a1",
            "x",
            "a_very_long_identifier_with_64_characters_____________________",
        ] {
            assert!(
                validate_identifier(ok).is_ok(),
                "{ok:?} should be accepted but was rejected"
            );
        }
    }

    #[test]
    fn validate_identifier_rejects_injection_vectors() {
        for bad in [
            "",
            "1leading_digit",
            "has space",
            "has-dash",
            "has.dot",
            "has;semicolon",
            "has'quote",
            "has\"dquote",
            "has`backtick",
            "drop table x; --",
            "users); DROP TABLE users; --",
            "name\x00null",
            "üñíçødé",
            // 65 chars — over the limit
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(
                validate_identifier(bad).is_err(),
                "{bad:?} should be rejected but was accepted"
            );
        }
    }

    #[test]
    fn collection_ddl_rejects_injection_in_name() {
        let err = collection_ddl("users); DROP TABLE users; --")
            .expect_err("attacker-shaped name must not return Ok");
        assert!(matches!(err, CollectionError::InvalidInput(_)));
    }

    #[test]
    fn collection_index_ddl_rejects_injection_in_either_arg() {
        assert!(collection_index_ddl("notes; DROP TABLE x", "author").is_err());
        assert!(collection_index_ddl("notes", "author'); DROP TABLE x; --").is_err());
    }

    #[test]
    fn test_json_to_sql_literal() {
        assert_eq!(json_to_sql_literal(&Value::Null), "NULL");
        assert_eq!(json_to_sql_literal(&Value::Bool(true)), "1");
        assert_eq!(json_to_sql_literal(&Value::Bool(false)), "0");
        assert_eq!(json_to_sql_literal(&serde_json::json!(42)), "42");
        let f: serde_json::Value = serde_json::from_str("3.14").expect("valid f64 json");
        assert_eq!(json_to_sql_literal(&f), "3.14");
        assert_eq!(json_to_sql_literal(&serde_json::json!("hello")), "'hello'");
        // SQL injection safety: quotes escaped
        assert_eq!(
            json_to_sql_literal(&serde_json::json!("O'Brien")),
            "'O''Brien'"
        );
    }
}
