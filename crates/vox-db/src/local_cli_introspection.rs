//! Read-only SQLite helpers for `vox db audit` / `vox db sample`.
//!
//! Keeps direct `turso` crate usage inside `vox-db` (data-storage policy).

use crate::VoxDb;
use crate::sql_util::validate_identifier;
use crate::store::types::StoreError;
use serde_json::{Map, Value, json};
use turso::Connection;

fn sqlite_quote_ident(name: &str) -> String {
    let mut s = String::with_capacity(name.len() + 2);
    s.push('"');
    for c in name.chars() {
        if c == '"' {
            s.push_str("\"\"");
        } else {
            s.push(c);
        }
    }
    s.push('"');
    s
}

async fn sqlite_pragma_i64(conn: &Connection, sql: &str) -> Result<i64, StoreError> {
    let mut rows = conn.query(sql, ()).await?;
    let Some(row) = rows.next().await? else {
        return Ok(0i64);
    };
    Ok(row.get(0)?)
}

async fn sqlite_pragma_text(conn: &Connection, sql: &str) -> Result<String, StoreError> {
    let mut rows = conn.query(sql, ()).await?;
    let Some(row) = rows.next().await? else {
        return Ok(String::new());
    };
    Ok(row.get(0)?)
}

fn pick_time_audit_column(col_names: &[String]) -> Option<String> {
    const PREFERRED: &[&str] = &[
        "updated_at_ms",
        "created_at_ms",
        "updated_at",
        "created_at",
        "recorded_at_ms",
        "submitted_at_ms",
        "attempted_at_ms",
        "timestamp",
        "ts",
    ];
    for name in PREFERRED {
        if col_names.iter().any(|n| n == name) {
            return Some((*name).to_string());
        }
    }
    for n in col_names {
        let l = n.to_lowercase();
        if l.contains("at_ms") || l.ends_with("_at") || l.contains("timestamp") {
            return Some(n.clone());
        }
    }
    None
}

/// Maps a single SQLite cell to JSON for CLI display (`vox db sample`).
#[must_use]
pub(crate) fn turso_cell_value_to_json(v: turso::Value) -> Value {
    match v {
        turso::Value::Null => Value::Null,
        turso::Value::Integer(i) => i.into(),
        turso::Value::Real(f) => f.into(),
        turso::Value::Text(s) => s.into(),
        turso::Value::Blob(b) => format!("(blob {} bytes)", b.len()).into(),
    }
}

/// JSON document printed by `vox db audit`.
pub async fn audit_database_json(db: &VoxDb, timestamps: bool) -> Result<Value, StoreError> {
    let version = db.schema_version().await?;
    let data_dir = crate::VoxDb::data_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());
    let db_path = crate::paths::default_db_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let conn = db.connection();

    let page_count = sqlite_pragma_i64(conn, "PRAGMA page_count").await?;
    let page_size = sqlite_pragma_i64(conn, "PRAGMA page_size").await?;
    let freelist_count = sqlite_pragma_i64(conn, "PRAGMA freelist_count").await?;
    let journal_mode = sqlite_pragma_text(conn, "PRAGMA journal_mode").await?;

    let mut name_rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            (),
        )
        .await?;
    let mut tables: Vec<Value> = Vec::new();
    while let Some(row) = name_rows.next().await? {
        let name: String = row.get(0)?;
        let q = sqlite_quote_ident(&name);
        let sql = format!("SELECT COUNT(*) FROM {q}");
        let mut c = conn.query(&sql, ()).await?;
        let count: i64 = c
            .next()
            .await?
            .ok_or_else(|| StoreError::Db(format!("missing count for {name}")))?
            .get(0)?;
        let mut entry = json!({"name": name, "row_count": count});
        if timestamps && count > 0 {
            let info_sql = format!("PRAGMA table_info({q})");
            let mut info_rows = conn.query(&info_sql, ()).await?;
            let mut col_names = Vec::new();
            while let Some(r) = info_rows.next().await? {
                col_names.push(r.get::<String>(1)?);
            }
            if let Some(tc) = pick_time_audit_column(&col_names) {
                let tq = sqlite_quote_ident(&tc);
                let rng_sql =
                    format!("SELECT MIN({tq}), MAX({tq}) FROM {q} WHERE {tq} IS NOT NULL");
                if let Ok(mut rng) = conn.query(&rng_sql, ()).await {
                    if let Some(rr) = rng.next().await? {
                        let vmin: Option<String> = rr.get(0).ok();
                        let vmax: Option<String> = rr.get(1).ok();
                        entry["time_column"] = json!(tc);
                        entry["time_min"] = json!(vmin);
                        entry["time_max"] = json!(vmax);
                    }
                }
            }
        }
        tables.push(entry);
    }

    Ok(json!({
        "schema_version": version,
        "data_dir": data_dir,
        "db_path": db_path,
        "pragma": {
            "page_count": page_count,
            "page_size": page_size,
            "freelist_count": freelist_count,
            "journal_mode": journal_mode,
        },
        "table_count": tables.len(),
        "tables": tables,
    }))
}

/// Rows from [`sample_table_json_objects`] as printable JSON maps.
pub async fn sample_table_json_objects(
    db: &VoxDb,
    table: &str,
    limit: i64,
) -> Result<Vec<Map<String, Value>>, StoreError> {
    validate_identifier(table).map_err(|e| StoreError::Db(e.to_string()))?;
    let conn = db.connection();

    let info_sql = format!("PRAGMA table_info({table})");
    let mut info_rows = conn.query(&info_sql, ()).await?;
    let mut col_names = Vec::new();
    while let Some(row) = info_rows.next().await? {
        col_names.push(row.get::<String>(1)?);
    }

    let sql = format!("SELECT * FROM {table} LIMIT {limit}");
    let mut rows = conn.query(&sql, ()).await?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let mut map = Map::new();
        for (i, name) in col_names.iter().enumerate() {
            let val: Value = match row.get_value(i) {
                Ok(v) => turso_cell_value_to_json(v),
                Err(_) => Value::String("error".into()),
            };
            map.insert(name.to_string(), val);
        }
        out.push(map);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turso_cell_value_to_json_variants() {
        assert_eq!(turso_cell_value_to_json(turso::Value::Null), Value::Null);
        assert_eq!(
            turso_cell_value_to_json(turso::Value::Integer(42)),
            json!(42)
        );
        assert_eq!(
            turso_cell_value_to_json(turso::Value::Real(1.5)),
            json!(1.5)
        );
        assert_eq!(
            turso_cell_value_to_json(turso::Value::Text("x".into())),
            json!("x")
        );
        let blob = turso_cell_value_to_json(turso::Value::Blob(vec![1, 2]));
        assert!(blob.as_str().unwrap().contains("blob"));
    }
}
