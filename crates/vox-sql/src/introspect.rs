#[cfg(any(feature = "postgres", feature = "mysql"))]
use std::collections::BTreeMap;

use crate::{
    AnySqlBackend, SqlBackendError, SqlRow, SqlValue,
    schema_model::{IntrospectedColumn, IntrospectedSchema, IntrospectedTable},
};

impl AnySqlBackend {
    pub async fn introspect_schema(&self) -> Result<IntrospectedSchema, SqlBackendError> {
        match self {
            #[cfg(feature = "libsql")]
            AnySqlBackend::Libsql(_) => introspect_sqlite(self).await,
            #[cfg(feature = "postgres")]
            AnySqlBackend::Postgres(_) => introspect_postgres(self).await,
            #[cfg(feature = "mysql")]
            AnySqlBackend::MySql(_) => introspect_mysql(self).await,
        }
    }
}

#[cfg(feature = "libsql")]
async fn introspect_sqlite(backend: &AnySqlBackend) -> Result<IntrospectedSchema, SqlBackendError> {
    let table_rows = backend
        .query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            &[],
        )
        .await?;
    let mut tables = Vec::new();
    for row in table_rows {
        let Some(name) = text_at(&row, 0) else {
            continue;
        };
        let pragma = format!("PRAGMA table_info(\"{}\")", name.replace('"', "\"\""));
        let cols_rows = backend.query(&pragma, &[]).await?;
        let mut columns = Vec::new();
        for c in cols_rows {
            let Some(col_name) = text_at(&c, 1) else {
                continue;
            };
            let data_type = text_at(&c, 2).unwrap_or_else(|| "TEXT".to_string());
            let nullable = int_at(&c, 3).unwrap_or(0) == 0;
            let default_expr = text_at(&c, 4);
            let is_primary_key = int_at(&c, 5).unwrap_or(0) > 0;
            columns.push(IntrospectedColumn {
                name: col_name,
                data_type,
                nullable,
                is_primary_key,
                default_expr,
            });
        }
        tables.push(IntrospectedTable { name, columns });
    }
    Ok(IntrospectedSchema {
        backend: "sqlite".to_string(),
        tables,
    })
}

#[cfg(feature = "postgres")]
async fn introspect_postgres(
    backend: &AnySqlBackend,
) -> Result<IntrospectedSchema, SqlBackendError> {
    let rows = backend
        .query(
            "SELECT table_name, column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = current_schema() ORDER BY table_name, ordinal_position",
            &[],
        )
        .await?;

    let pk_rows = backend
        .query(
            "SELECT kcu.table_name, kcu.column_name FROM information_schema.table_constraints tc JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = current_schema()",
            &[],
        )
        .await?;
    let mut pk_set = std::collections::BTreeSet::new();
    for r in pk_rows {
        if let (Some(t), Some(c)) = (text_at(&r, 0), text_at(&r, 1)) {
            pk_set.insert((t, c));
        }
    }
    rows_to_schema("postgres", rows, &pk_set)
}

#[cfg(feature = "mysql")]
async fn introspect_mysql(backend: &AnySqlBackend) -> Result<IntrospectedSchema, SqlBackendError> {
    let rows = backend
        .query(
            "SELECT table_name, column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = DATABASE() ORDER BY table_name, ordinal_position",
            &[],
        )
        .await?;
    let pk_rows = backend
        .query(
            "SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = DATABASE() AND column_key = 'PRI'",
            &[],
        )
        .await?;
    let mut pk_set = std::collections::BTreeSet::new();
    for r in pk_rows {
        if let (Some(t), Some(c)) = (text_at(&r, 0), text_at(&r, 1)) {
            pk_set.insert((t, c));
        }
    }
    rows_to_schema("mysql", rows, &pk_set)
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn rows_to_schema(
    backend_name: &str,
    rows: Vec<SqlRow>,
    pk_set: &std::collections::BTreeSet<(String, String)>,
) -> Result<IntrospectedSchema, SqlBackendError> {
    let mut by_table: BTreeMap<String, Vec<IntrospectedColumn>> = BTreeMap::new();
    for row in rows {
        let table_name = text_at(&row, 0).ok_or_else(|| {
            SqlBackendError::new(format!(
                "{backend_name} introspection row missing table name"
            ))
        })?;
        let column_name = text_at(&row, 1).ok_or_else(|| {
            SqlBackendError::new(format!(
                "{backend_name} introspection row missing column name"
            ))
        })?;
        let data_type = text_at(&row, 2).unwrap_or_else(|| "text".to_string());
        let nullable_raw = text_at(&row, 3).unwrap_or_else(|| "YES".to_string());
        let default_expr = text_at(&row, 4);
        let is_primary_key = pk_set.contains(&(table_name.clone(), column_name.clone()));
        by_table
            .entry(table_name)
            .or_default()
            .push(IntrospectedColumn {
                name: column_name,
                data_type,
                nullable: nullable_raw.eq_ignore_ascii_case("yes"),
                is_primary_key,
                default_expr,
            });
    }
    let tables = by_table
        .into_iter()
        .map(|(name, columns)| IntrospectedTable { name, columns })
        .collect::<Vec<_>>();
    Ok(IntrospectedSchema {
        backend: backend_name.to_string(),
        tables,
    })
}

fn text_at(row: &SqlRow, idx: usize) -> Option<String> {
    match row.get(idx).map(|(_k, v)| v) {
        Some(SqlValue::Text(s)) => Some(s.clone()),
        Some(SqlValue::Int(v)) => Some(v.to_string()),
        Some(SqlValue::Float(v)) => Some(v.to_string()),
        Some(SqlValue::Bool(v)) => Some(if *v { "1".to_string() } else { "0".to_string() }),
        Some(SqlValue::Null) | Some(SqlValue::Bytes(_)) | None => None,
    }
}

#[cfg(feature = "libsql")]
fn int_at(row: &SqlRow, idx: usize) -> Option<i64> {
    match row.get(idx).map(|(_k, v)| v) {
        Some(SqlValue::Int(v)) => Some(*v),
        Some(SqlValue::Bool(v)) => Some(i64::from(*v)),
        Some(SqlValue::Text(s)) => s.parse::<i64>().ok(),
        Some(SqlValue::Float(v)) => Some(*v as i64),
        _ => None,
    }
}
