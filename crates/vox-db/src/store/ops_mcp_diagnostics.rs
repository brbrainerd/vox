//! MCP-facing introspection and legacy transcript writes (`db_sample_data`, chat persistence).
//!
//! Any path that persists **full MCP transcripts** or tool payloads is **S3** content-bearing; redact before
//! treating rows as telemetry (`docs/src/architecture/telemetry-trust-ssot.md`).

use turso::params;

use crate::store::types::StoreError;

fn mcp_safe_sqlite_table_name(name: &str) -> Result<&str, StoreError> {
    if name.is_empty() || name.len() > 128 {
        return Err(StoreError::Db("table name empty or too long".into()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(StoreError::Db(format!(
            "table name must be [A-Za-z0-9_]+ only (got {name:?})"
        )));
    }
    Ok(name)
}

impl crate::VoxDb {
    /// `PRAGMA table_info` + `SELECT * … LIMIT` for MCP `db_sample_data` (identifier-validated `table`).
    pub async fn mcp_diagnostic_sample_table(
        &self,
        table: &str,
        limit: u64,
    ) -> Result<Vec<serde_json::Value>, StoreError> {
        let table = mcp_safe_sqlite_table_name(table)?;
        let lim = limit.clamp(1, 1000) as i64;

        let pragma = format!("PRAGMA table_info({table})");
        let mut info_rows = self.conn.query(&pragma, ()).await?;
        let mut col_names = Vec::new();
        while let Some(row) = info_rows.next().await? {
            if let Ok(name) = row.get::<String>(1) {
                col_names.push(name);
            }
        }
        if col_names.is_empty() {
            return Err(StoreError::Db(format!(
                "table '{table}' does not exist or has no columns"
            )));
        }

        let sql = format!("SELECT * FROM {table} LIMIT ?1");
        let mut rows = self.conn.query(&sql, params![lim]).await?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            let mut map = serde_json::Map::new();
            for (i, col_name) in col_names.iter().enumerate() {
                let val = match row.get_value(i) {
                    Ok(v) => match v {
                        turso::Value::Null => serde_json::Value::Null,
                        turso::Value::Integer(i) => serde_json::Value::Number(i.into()),
                        turso::Value::Real(f) => serde_json::Number::from_f64(f)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                        turso::Value::Text(s) => serde_json::Value::String(s),
                        turso::Value::Blob(b) => {
                            serde_json::Value::String(format!("(blob {} bytes)", b.len()))
                        }
                    },
                    Err(_) => serde_json::Value::String("<error>".to_string()),
                };
                map.insert(col_name.to_string(), val);
            }
            results.push(serde_json::Value::Object(map));
        }
        Ok(results)
    }

    /// Insert one MCP chat transcript row (`chat_transcripts` legacy / VS Code history API).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_chat_transcript_turn(
        &self,
        id: &str,
        session_id: &str,
        role: &str,
        content: &str,
        model_used: Option<&str>,
        tokens: Option<i64>,
        context_files_json: &str,
        repository_id: &str,
    ) -> Result<(), StoreError> {
        let id = id.to_string();
        let session_id = session_id.to_string();
        let role = role.to_string();
        let content = content.to_string();
        let model_used = model_used.map(str::to_string);
        let context_files_json = context_files_json.to_string();
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO chat_transcripts (id, session_id, role, content, model_used, tokens, context_files, repository_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        id.as_str(),
                        session_id.as_str(),
                        role.as_str(),
                        content.as_str(),
                        model_used.as_deref(),
                        tokens,
                        context_files_json.as_str(),
                        repository_id.as_str(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Persist a bounded routing summary row (local-first; joins `journey_id` across telemetry).
    pub async fn record_routing_decision(
        &self,
        journey_id: Option<&str>,
        repository_id: &str,
        session_id: Option<&str>,
        surface: &str,
        model_id: Option<&str>,
        reason_json: Option<&str>,
    ) -> Result<(), StoreError> {
        let journey_id = journey_id.map(str::to_string);
        let repository_id = repository_id.to_string();
        let session_id = session_id.map(str::to_string);
        let surface = surface.to_string();
        let model_id = model_id.map(str::to_string);
        let reason_json = reason_json.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO routing_decisions
                    (journey_id, repository_id, session_id, surface, model_id, reason_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        journey_id.as_deref(),
                        repository_id.as_str(),
                        session_id.as_deref(),
                        surface.as_str(),
                        model_id.as_deref(),
                        reason_json.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}
