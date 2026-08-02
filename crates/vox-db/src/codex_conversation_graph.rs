//! Structured **research sessions**, **conversation version** snapshots, **conversation edges**, and
//! **topic evolution** events (Arca manifest fragment `v17`).

use crate::store::StoreError;

use crate::VoxDb;

impl VoxDb {
    /// Upsert `research_sessions` by `session_key`.
    pub async fn research_session_upsert(
        &self,
        session_key: &str,
        title: &str,
        status: &str,
        repository_id: &str,
        config_json: Option<&str>,
        summary_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.upsert_research_session(
            session_key,
            title,
            status,
            repository_id,
            config_json,
            summary_json,
        )
        .await
    }

    /// Append a `conversation_versions` row.
    pub async fn conversation_version_append(
        &self,
        conversation_id: i64,
        version_index: i64,
        label: &str,
        snapshot_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.append_conversation_version(conversation_id, version_index, label, snapshot_json)
            .await
    }

    /// Insert a `conversation_edges` row (no self-edges).
    pub async fn conversation_edge_insert(
        &self,
        from_conversation_id: i64,
        to_conversation_id: i64,
        edge_kind: &str,
        weight: f64,
        metadata_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.insert_conversation_edge(
            from_conversation_id,
            to_conversation_id,
            edge_kind,
            weight,
            metadata_json,
        )
        .await
    }

    /// Append `topic_evolution_events`.
    pub async fn topic_evolution_event_append(
        &self,
        topic_id: i64,
        event_kind: &str,
        prior_label: Option<&str>,
        new_label: Option<&str>,
        detail_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.append_topic_evolution_event(topic_id, event_kind, prior_label, new_label, detail_json)
            .await
    }

    /// Ensure a `research_sessions` row exists for `session_key`, then append `research_metrics`
    /// with the same `session_id` string (legacy table uses TEXT, not a FK).
    pub async fn research_metric_append_linked(
        &self,
        session_key: &str,
        metric_type: &str,
        metric_value: Option<f64>,
        metadata_json: Option<&str>,
        repository_id: &str,
    ) -> Result<serde_json::Value, StoreError> {
        let session_row_id = self
            .research_session_upsert(session_key, "", "active", repository_id, None, None)
            .await?;
        let metric_row_id = self
            .append_research_metric(session_key, metric_type, metric_value, metadata_json)
            .await?;
        Ok(serde_json::json!({
            "research_session_row_id": session_row_id,
            "research_metric_row_id": metric_row_id,
            "session_key": session_key,
        }))
    }
}

#[cfg(all(test, feature = "local"))]
mod tests {
    // Only used by the quarantine-gated test below; unused (and would warn)
    // when the `quarantine` feature is off.
    #[cfg(feature = "quarantine")]
    use crate::research_metrics_contract::METRIC_TYPE_SOCRATES_SURFACE;
    #[cfg(feature = "quarantine")]
    use crate::{DbConfig, VoxDb};

    // Exercises conversation_versions, conversation_edges, and
    // topic_evolution_events, all quarantined (Task 4, VoxDB audit
    // condensation plan) — off by default, see domains/quarantine.rs.
    #[tokio::test]
    #[cfg(feature = "quarantine")]
    async fn v17_research_session_conversation_graph_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

        db.connection()
            .execute(
                "INSERT OR IGNORE INTO users (id, display_name, role) VALUES ('u1', 'u1', 'user')",
                (),
            )
            .await
            .expect("user");

        let c1 = db
            .chat_create_conversation(Some("u1"), "first")
            .await
            .expect("c1");
        let c2 = db
            .chat_create_conversation(Some("u1"), "second")
            .await
            .expect("c2");

        let sid = db
            .research_session_upsert(
                "sess-key-1",
                "Study",
                "active",
                "repo-deadbeef",
                Some(r#"{"model":"x"}"#),
                None,
            )
            .await
            .expect("rs");
        assert!(sid > 0);

        let again = db
            .research_session_upsert(
                "sess-key-1",
                "Study",
                "closed",
                "repo-deadbeef",
                None,
                Some(r#"{"closed":true}"#),
            )
            .await
            .expect("rs2");
        assert_eq!(again, sid);

        let vrow = db
            .conversation_version_append(c1, 1, "v1", Some(r#"{"a":1}"#))
            .await
            .expect("cv");
        assert!(vrow > 0);

        let eid = db
            .conversation_edge_insert(c1, c2, "fork", 1.0, None)
            .await
            .expect("edge");
        assert!(eid > 0);

        db.connection()
            .execute(
                "INSERT OR IGNORE INTO topics (slug, label) VALUES ('t1', 'T1')",
                (),
            )
            .await
            .expect("topic");
        let mut rows = db
            .connection()
            .query("SELECT id FROM topics WHERE slug = 't1'", ())
            .await
            .expect("q");
        let topic_id: i64 = rows
            .next()
            .await
            .expect("n")
            .expect("r")
            .get(0)
            .expect("id");

        let te = db
            .topic_evolution_event_append(topic_id, "rename", Some("T1"), Some("T1-prime"), None)
            .await
            .expect("te");
        assert!(te > 0);

        let linked = db
            .research_metric_append_linked(
                "sess-metric-1",
                METRIC_TYPE_SOCRATES_SURFACE,
                Some(0.42),
                Some(r#"{"k":1}"#),
                "repo-x",
            )
            .await
            .expect("linked");
        assert_eq!(linked["session_key"], "sess-metric-1");
        assert!(linked["research_session_row_id"].as_i64().unwrap_or(0) > 0);
        assert!(linked["research_metric_row_id"].as_i64().unwrap_or(0) > 0);
    }
}
