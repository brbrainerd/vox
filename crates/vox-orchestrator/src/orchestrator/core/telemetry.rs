use crate::types::AgentId;

impl crate::orchestrator::Orchestrator {
    /// Record a flat telemetry event to `agent_telemetry_flat` in the database.
    pub async fn record_telemetry(
        &self,
        agent_id: AgentId,
        tenant_id: Option<&str>,
        event_kind: &str,
        model_id: Option<&str>,
        provider: Option<&str>,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        cost_usd: Option<f64>,
        payload: Option<serde_json::Value>,
    ) {
        let Some(db) = self.db() else { return };
        let repo = crate::lineage::repository_id();
        let aid = agent_id.0.to_string();
        let sid = "canonical-session";
        let payload_json = payload.map(|p| p.to_string());

        let res = db
            .insert_telemetry_flat_raw(
                &aid,
                sid,
                tenant_id,
                &repo,
                event_kind,
                None, // tool_name
                model_id,
                provider,
                None, // duration_ms
                input_tokens.map(|t| t as i64),
                output_tokens.map(|t| t as i64),
                cost_usd,
                payload_json.as_deref(),
            )
            .await;

        if let Err(e) = res {
            tracing::warn!(error = %e, kind = event_kind, "failed to record flat telemetry; outbox enqueue pending");
            self.enqueue_telemetry_outbox(
                agent_id,
                tenant_id,
                sid,
                event_kind,
                model_id,
                provider,
                input_tokens,
                output_tokens,
                cost_usd,
                payload_json.as_deref(),
            )
            .await;
        }
    }

    pub async fn enqueue_telemetry_outbox(
        &self,
        agent_id: AgentId,
        tenant_id: Option<&str>,
        session_id: &str,
        event_kind: &str,
        model_id: Option<&str>,
        provider: Option<&str>,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        cost_usd: Option<f64>,
        payload_json: Option<&str>,
    ) {
        let store = crate::sync_lock::rw_read(&*self.context_store);
        let key = crate::orchestrator::persistence::PERSISTENCE_OUTBOX_KEY.to_string();
        let mut queue = store
            .get(&key)
            .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
            .unwrap_or_default();

        let entry = serde_json::json!({
            "lane": "telemetry/flat",
            "error": "db_unavailable",
            "first_seen_unix_ms": crate::types::now_unix_ms(),
            "retry_count": 0,
            "replay": {
                "op": "insert_telemetry_flat_raw",
                "agent_id": agent_id.0.to_string(),
                "tenant_id": tenant_id,
                "session_id": session_id,
                "event_kind": event_kind,
                "model_id": model_id,
                "provider": provider,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cost_usd": cost_usd,
                "payload_json": payload_json,
            }
        });

        queue.push(entry);
        if let Ok(raw) = serde_json::to_string(&queue) {
            store.set(AgentId(0), key, raw, 0);
        }
    }

    /// Pull the latest model scoreboard from the database and inject it into the registry
    /// to enable data-driven model selection.
    pub async fn refresh_model_scoreboard(&self) {
        let Some(db) = self.db() else { return };

        // Use a 7-day window for routing decisions as defined in schema.
        match db.get_model_scoreboard(7).await {
            Ok(rows) => {
                let mut scoreboard = std::collections::HashMap::new();
                for row in rows {
                    scoreboard.insert(row.model_id.clone(), crate::models::ModelScore::from(row));
                }

                let mut registry = self.models.write().unwrap();
                registry.inject_scoreboard(scoreboard);
                tracing::debug!(
                    "Refreshed model scoreboard with {} entries",
                    registry.scoreboard_len()
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to refresh model scoreboard from database");
            }
        }

        match db.list_model_arm_stats(7).await {
            Ok(arms) => {
                let mut registry = self.models.write().unwrap();
                registry.inject_arm_stats(arms);
                tracing::debug!(
                    "Refreshed model arm stats with {} model ids",
                    registry.arm_stats_snapshot().len()
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to refresh model arm stats from database");
            }
        }

        match db.get_pricing_catalog().await {
            Ok(pricing) => {
                let count = pricing.len();
                let mut registry = self.models.write().unwrap();
                registry.inject_pricing_catalog(pricing);
                tracing::debug!(
                    "Refreshed model pricing catalog with {} confident entries",
                    count
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to refresh model pricing catalog from database");
            }
        }
    }
}
