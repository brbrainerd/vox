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

use serde_json::Value;
use turso::{params, Connection};

/// A handle to a schemaless document collection.
///
/// All CRUD operations translate to SQL under the hood:
/// - `insert` → `INSERT INTO <name> (_data) VALUES (?1)`
/// - `get`    → `SELECT _data FROM <name> WHERE _id = ?1`
/// - `find`   → `SELECT _id, _data FROM <name> WHERE json_extract(_data, '$.<key>') = <value>`
/// - `patch`  → `UPDATE <name> SET _data = json_patch(_data, ?1) WHERE _id = ?2`
/// - `delete` → `DELETE FROM <name> WHERE _id = ?1`
pub struct Collection<'a> {
    name: String,
    conn: &'a Connection,
}

/// Error type for collection operations.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    #[error("turso error: {0}")]
    Turso(#[from] turso::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    InvalidInput(String),
}

impl<'a> Collection<'a> {
    /// Create a new collection handle. Does NOT create the underlying table.
    pub fn new(name: impl Into<String>, conn: &'a Connection) -> Self {
        Self {
            name: name.into(),
            conn,
        }
    }

    /// Ensure the underlying table exists.
    pub async fn ensure_table(&self) -> Result<(), CollectionError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                _id INTEGER PRIMARY KEY AUTOINCREMENT,
                _data TEXT NOT NULL,
                _created_at TEXT DEFAULT (datetime('now')),
                _updated_at TEXT DEFAULT (datetime('now'))
            )",
            self.name
        );
        self.conn.execute(&sql, ()).await?;
        Ok(())
    }

    /// Insert a JSON document into the collection. Returns the new document's `_id`.
    pub async fn insert(&self, doc: &Value) -> Result<i64, CollectionError> {
        let json_str = serde_json::to_string(doc)?;
        let sql = format!("INSERT INTO {} (_data) VALUES (?1)", self.name);
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
        let sql = format!("SELECT _data FROM {} WHERE _id = ?1", self.name);
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
        let obj = filter
            .as_object()
            .ok_or_else(|| CollectionError::InvalidInput("filter must be a JSON object".into()))?;

        if obj.is_empty() {
            return self.list(100, 0).await;
        }

        let mut conditions = Vec::new();
        for (key, val) in obj {
            let sql_lit = json_to_sql_literal(val);
            conditions.push(format!("json_extract(_data, '$.{key}') = {sql_lit}"));
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT _id, _data FROM {} WHERE {} LIMIT 100",
            self.name, where_clause
        );

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
        let sql = format!(
            "UPDATE {} SET _data = json_patch(_data, ?1), _updated_at = datetime('now') WHERE _id = ?2",
            self.name
        );
        self.conn.execute(&sql, params![patch_str, id]).await?;
        Ok(())
    }

    /// Replace the entire document at the given `_id`.
    pub async fn replace(&self, id: i64, doc: &Value) -> Result<(), CollectionError> {
        let json_str = serde_json::to_string(doc)?;
        let sql = format!(
            "UPDATE {} SET _data = ?1, _updated_at = datetime('now') WHERE _id = ?2",
            self.name
        );
        self.conn.execute(&sql, params![json_str, id]).await?;
        Ok(())
    }

    /// Delete a document by its `_id`.
    pub async fn delete(&self, id: i64) -> Result<(), CollectionError> {
        let sql = format!("DELETE FROM {} WHERE _id = ?1", self.name);
        self.conn.execute(&sql, params![id]).await?;
        Ok(())
    }

    /// List documents with pagination.
    pub async fn list(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(i64, Value)>, CollectionError> {
        let sql = format!(
            "SELECT _id, _data FROM {} ORDER BY _id ASC LIMIT ?1 OFFSET ?2",
            self.name
        );
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
        let sql = if let Some(f) = filter {
            let obj = f.as_object().ok_or_else(|| {
                CollectionError::InvalidInput("filter must be a JSON object".into())
            })?;
            if obj.is_empty() {
                format!("SELECT COUNT(*) FROM {}", self.name)
            } else {
                let mut conditions = Vec::new();
                for (key, val) in obj {
                    let sql_lit = json_to_sql_literal(val);
                    conditions.push(format!("json_extract(_data, '$.{key}') = {sql_lit}"));
                }
                format!(
                    "SELECT COUNT(*) FROM {} WHERE {}",
                    self.name,
                    conditions.join(" AND ")
                )
            }
        } else {
            format!("SELECT COUNT(*) FROM {}", self.name)
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
pub fn collection_ddl(name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {name} (
    _id INTEGER PRIMARY KEY AUTOINCREMENT,
    _data TEXT NOT NULL,
    _created_at TEXT DEFAULT (datetime('now')),
    _updated_at TEXT DEFAULT (datetime('now'))
)"
    )
}

/// Generate DDL for a `json_extract` expression index on a collection field.
pub fn collection_index_ddl(collection_name: &str, field_name: &str) -> String {
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{collection_name}_{field_name} \
         ON {collection_name}(json_extract(_data, '$.{field_name}'))"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_ddl() {
        let ddl = collection_ddl("notes");
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS notes"));
        assert!(ddl.contains("_id INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(ddl.contains("_data TEXT NOT NULL"));
        assert!(ddl.contains("_created_at TEXT DEFAULT"));
        assert!(ddl.contains("_updated_at TEXT DEFAULT"));
    }

    #[test]
    fn test_collection_index_ddl() {
        let ddl = collection_index_ddl("notes", "author");
        assert!(ddl.contains("idx_notes_author"));
        assert!(ddl.contains("json_extract(_data, '$.author')"));
    }

    #[test]
    fn test_json_to_sql_literal() {
        assert_eq!(json_to_sql_literal(&Value::Null), "NULL");
        assert_eq!(json_to_sql_literal(&Value::Bool(true)), "1");
        assert_eq!(json_to_sql_literal(&Value::Bool(false)), "0");
        assert_eq!(json_to_sql_literal(&serde_json::json!(42)), "42");
        assert_eq!(json_to_sql_literal(&serde_json::json!(3.14)), "3.14");
        assert_eq!(json_to_sql_literal(&serde_json::json!("hello")), "'hello'");
        // SQL injection safety: quotes escaped
        assert_eq!(
            json_to_sql_literal(&serde_json::json!("O'Brien")),
            "'O''Brien'"
        );
    }
}
