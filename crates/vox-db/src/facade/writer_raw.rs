use crate::StoreError;
use crate::VoxDb;
use turso::params;

impl VoxDb {
    /// Internal raw insert for agent events, bypassing application-level circuit breakers.
    pub async fn insert_agent_event_raw(
        &self,
        agent_id: &str,
        event_type: &str,
        payload_json: Option<&str>,
        cli_version: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO agent_events (agent_id, event_type, payload_json, cli_version, timestamp)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![
                agent_id,
                event_type,
                payload_json.unwrap_or("{}"),
                cli_version.unwrap_or("unknown"),
            ],
        )
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Internal raw insert for cost records.
    pub async fn insert_cost_record(
        &self,
        agent_id: &str,
        session_id: Option<&str>,
        tenant_id: Option<&str>,
        provider: &str,
        model: Option<&str>,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO cost_records (agent_id, session_id, tenant_id, provider, model, input_tokens, output_tokens, cost_usd, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            params![
                agent_id,
                session_id,
                tenant_id,
                provider,
                model,
                input_tokens,
                output_tokens,
                cost_usd,
            ],
        )
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Internal raw insert for execution history.
    pub async fn insert_exec_history_raw(
        &self,
        tool_key: &str,
        repository_id: &str,
        session_id: Option<&str>,
        duration_ms: i64,
        cost_usd: Option<f64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO agent_exec_history 
                (tool_key, repository_id, session_id, duration_ms, vendor_cost_usd_micros, compute_tokens_used, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch('now') * 1000)",
            params![
                tool_key,
                repository_id,
                session_id,
                duration_ms,
                cost_usd.map(|c| (c * 1_000_000.0) as i64),
                input_tokens.unwrap_or(0) + output_tokens.unwrap_or(0),
            ],
        )
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(())
    }

    /// Internal raw insert for A2A messages.
    pub async fn insert_a2a_message_raw(
        &self,
        sender_id: u64,
        receiver_id: u64,
        msg_type: &str,
        payload: &str,
        idempotency_key: &str,
        repository_id: &str,
    ) -> Result<String, StoreError> {
        let msg_id = format!("msg_{}", uuid::Uuid::new_v4());
        self.conn.execute(
            "INSERT INTO a2a_messages (id, sender_id, receiver_id, msg_type, payload, idempotency_key, repository_id, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch('now') * 1000)",
            params![
                msg_id.clone(),
                sender_id as i64,
                receiver_id as i64,
                msg_type,
                payload,
                idempotency_key,
                repository_id,
            ],
        )
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(msg_id)
    }

    /// Internal raw insert for the flattened telemetry projection (v51).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_telemetry_flat_raw(
        &self,
        agent_id: &str,
        session_id: &str,
        tenant_id: Option<&str>,
        repository_id: &str,
        event_kind: &str,
        tool_name: Option<&str>,
        model_id: Option<&str>,
        provider: Option<&str>,
        duration_ms: Option<i64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        cost_usd: Option<f64>,
        payload_json: Option<&str>,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO agent_telemetry_flat (
                agent_id, session_id, tenant_id, repository_id, event_kind, tool_name, model_id, provider, 
                duration_ms, input_tokens, output_tokens, cost_usd, payload_json, recorded_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, unixepoch('now') * 1000)",
                params![
                    agent_id,
                    session_id,
                    tenant_id,
                    repository_id,
                    event_kind,
                    tool_name,
                    model_id,
                    provider,
                    duration_ms,
                    input_tokens,
                    output_tokens,
                    cost_usd,
                    payload_json,
                ],
            )
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(())
    }

    /// Internal raw insert for the scientia publication queue.
    pub async fn insert_publication_queue_raw(
        &self,
        discovery_id: &str,
        publication_id: &str,
        stage: &str,
    ) -> Result<(), StoreError> {
        let now = crate::now_unix_ms();
        self.conn.execute(
            "INSERT INTO scientia_publication_queue (discovery_id, publication_id, stage, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![discovery_id, publication_id, stage, now as i64],
        )
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(())
    }
}
