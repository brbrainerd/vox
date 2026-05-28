//! Agent events, cost records, sessions, metrics.

use anyhow::Result;
use turso::params;
use vox_db::Codex;

/// Persistent record of an agent lifecycle or state-change event.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentEventRecord {
    /// Monotonic database row ID.
    pub id: i64,
    /// Identifier of the agent that emitted the event.
    pub agent_id: String,
    /// Discriminant string for the event kind (e.g. `"task_completed"`).
    pub event_type: String,
    /// Optional JSON payload attached to the event.
    pub payload: Option<String>,
    /// CLI / crate version string stored with the event when present.
    #[serde(default)]
    pub cli_version: Option<String>,
    /// SQLite `datetime` string when the event was recorded.
    pub timestamp: String,
}

/// Load recent events for an agent.
pub async fn get_events(
    db: &Codex,
    agent_id: &str,
    limit: Option<i64>,
) -> Result<Vec<AgentEventRecord>> {
    let lim = limit.unwrap_or(50);
    let mut rows = db
        .connection()
        .query(
            "SELECT id, agent_id, event_type, payload_json, cli_version, timestamp
             FROM agent_events WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            params![agent_id, lim],
        )
        .await?;
    let mut events = Vec::new();
    while let Some(row) = rows.next().await? {
        events.push(AgentEventRecord {
            id: row.get::<i64>(0)?,
            agent_id: row.get::<String>(1)?,
            event_type: row.get::<String>(2)?,
            payload: row.get::<Option<String>>(3).unwrap_or(None),
            cli_version: row.get::<Option<String>>(4).unwrap_or(None),
            timestamp: row.get::<String>(5)?,
        });
    }
    Ok(events)
}

/// Insert a new agent event.
pub async fn insert_event(
    db: &Codex,
    agent_id: &str,
    event_type: &str,
    payload: Option<&str>,
) -> Result<()> {
    let agent_id = agent_id.to_string();
    let event_type = event_type.to_string();
    let payload_json = payload.unwrap_or("{}").to_string();
    let cli_version = env!("CARGO_PKG_VERSION").to_string();
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO agent_events (agent_id, event_type, payload_json, cli_version, timestamp)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                params![
                    agent_id.as_str(),
                    event_type.as_str(),
                    payload_json.as_str(),
                    cli_version.as_str(),
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// A recorded LLM cost event for a single agent inference call.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CostRecord {
    /// Monotonic database row ID.
    pub id: i64,
    /// Identifier of the agent that incurred the cost.
    pub agent_id: String,
    /// Optional session correlation ID.
    pub session_id: Option<String>,
    /// Provider backend name (e.g. `"google"`, `"openai"`).
    pub provider: String,
    /// Model identifier used for the inference.
    pub model: Option<String>,
    /// Number of prompt (input) tokens consumed.
    pub input_tokens: i64,
    /// Number of completion (output) tokens produced.
    pub output_tokens: i64,
    /// Estimated cost in USD for this call.
    pub cost_usd: f64,
    /// SQLite `datetime` string when the record was inserted.
    pub timestamp: String,
}

impl CostRecord {
    /// Create a new cost record for in-memory tracking.
    pub fn new_ephemeral(
        agent_id: impl Into<String>,
        provider: impl Into<String>,
        model: Option<String>,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Self {
        Self {
            id: 0,
            agent_id: agent_id.into(),
            session_id: None,
            provider: provider.into(),
            model,
            input_tokens,
            output_tokens,
            cost_usd,
            timestamp: String::new(),
        }
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Insert a cost record.
#[allow(clippy::too_many_arguments)]
pub async fn insert_cost_record(
    db: &Codex,
    agent_id: &str,
    session_id: Option<&str>,
    provider: &str,
    model: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
    tenant_id: Option<&str>,
) -> Result<()> {
    let agent_id = agent_id.to_string();
    let session_id = session_id.map(str::to_string);
    let provider = provider.to_string();
    let model = model.map(str::to_string);
    let tenant_id = tenant_id.map(str::to_string);
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO cost_records (agent_id, session_id, provider, model,
                 input_tokens, output_tokens, cost_usd, tenant_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    agent_id.as_str(),
                    session_id.as_deref(),
                    provider.as_str(),
                    model.as_deref(),
                    input_tokens,
                    output_tokens,
                    cost_usd,
                    tenant_id.as_deref(),
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Get total cost for an agent.
pub async fn get_agent_cost_usd(db: &Codex, agent_id: &str) -> Result<f64> {
    let mut rows = db
        .connection()
        .query(
            "SELECT COALESCE(SUM(cost_usd), 0.0) FROM cost_records WHERE agent_id = ?1",
            params![agent_id],
        )
        .await?;
    Ok(rows
        .next()
        .await?
        .map(|r| r.get::<f64>(0).unwrap_or(0.0))
        .unwrap_or(0.0))
}

/// Get total token usage for a tenant in the current month.
pub async fn get_tenant_monthly_token_usage(db: &Codex, tenant_id: &str) -> Result<i64> {
    let mut rows = db
        .connection()
        .query(
            "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM cost_records 
             WHERE tenant_id = ?1 AND timestamp >= datetime('now', 'start of month')",
            params![tenant_id],
        )
        .await?;
    Ok(rows
        .next()
        .await?
        .map(|r| r.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0))
}

/// Get cost records for an agent, most recent first.
pub async fn list_cost_records(db: &Codex, agent_id: &str, limit: i64) -> Result<Vec<CostRecord>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT id, agent_id, session_id, provider, model, input_tokens, output_tokens, cost_usd, timestamp
             FROM cost_records WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            params![agent_id, limit],
        )
        .await?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().await? {
        records.push(CostRecord {
            id: row.get::<i64>(0)?,
            agent_id: row.get::<String>(1)?,
            session_id: row.get::<Option<String>>(2)?,
            provider: row.get::<String>(3)?,
            model: row.get::<Option<String>>(4)?,
            input_tokens: row.get::<i64>(5)?,
            output_tokens: row.get::<i64>(6)?,
            cost_usd: row.get::<f64>(7)?,
            timestamp: row.get::<String>(8)?,
        });
    }
    Ok(records)
}

/// Acknowledge an A2A message by ID.
pub async fn acknowledge_message(db: &Codex, id: i64) -> Result<()> {
    db.acknowledge_a2a_message_by_id(id).await?;
    Ok(())
}

/// Persisted snapshot of an agent's run session for lifecycle tracking.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentSessionRecord {
    /// Unique session identifier (UUID string).
    pub id: String,
    /// Numeric or named agent identifier.
    pub agent_id: String,
    /// Human-readable name of the agent, if assigned.
    pub agent_name: Option<String>,
    /// SQLite `datetime` when the session began.
    pub started_at: String,
    /// SQLite `datetime` when the session ended, or `None` if still active.
    pub ended_at: Option<String>,
    /// Session lifecycle status: `"active"`, `"archived"`, etc.
    pub status: String,
    /// JSON snapshot of the task being processed at compaction time.
    pub task_snapshot: Option<String>,
    /// Summarized context carried across compaction boundaries.
    pub context_summary: Option<String>,
    /// Optional tenant identifier.
    pub tenant_id: Option<String>,
}

/// Insert a new agent session.
pub async fn insert_agent_session(
    db: &Codex,
    id: &str,
    agent_id: &str,
    agent_name: Option<&str>,
) -> Result<()> {
    let id = id.to_string();
    let agent_id = agent_id.to_string();
    let agent_name = agent_name.map(str::to_string);
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT OR IGNORE INTO agent_sessions (id, agent_id, agent_name) VALUES (?1, ?2, ?3)",
                params![id.as_str(), agent_id.as_str(), agent_name.as_deref()],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Update session status and optional context.
pub async fn update_agent_session(
    db: &Codex,
    id: &str,
    status: &str,
    task_snapshot: Option<&str>,
    context_summary: Option<&str>,
) -> Result<()> {
    let id = id.to_string();
    let status = status.to_string();
    let task_snapshot = task_snapshot.map(str::to_string);
    let context_summary = context_summary.map(str::to_string);
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE agent_sessions SET status=?1, task_snapshot=?2, context_summary=?3 WHERE id=?4",
                params![
                    status.as_str(),
                    task_snapshot.as_deref(),
                    context_summary.as_deref(),
                    id.as_str(),
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// End a session by setting ended_at and status.
pub async fn end_agent_session(db: &Codex, id: &str, status: &str) -> Result<()> {
    let id = id.to_string();
    let status = status.to_string();
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE agent_sessions SET status=?1, ended_at=datetime('now') WHERE id=?2",
                params![status.as_str(), id.as_str()],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Get active sessions.
pub async fn list_active_sessions(db: &Codex) -> Result<Vec<AgentSessionRecord>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT id, agent_id, agent_name, started_at, ended_at, status, task_snapshot, context_summary, tenant_id
             FROM agent_sessions WHERE status='active' ORDER BY started_at DESC",
            (),
        )
        .await?;
    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await? {
        sessions.push(AgentSessionRecord {
            id: row.get::<String>(0)?,
            agent_id: row.get::<String>(1)?,
            agent_name: row.get::<Option<String>>(2)?,
            started_at: row.get::<String>(3)?,
            ended_at: row.get::<Option<String>>(4)?,
            status: row.get::<String>(5)?,
            task_snapshot: row.get::<Option<String>>(6)?,
            context_summary: row.get::<Option<String>>(7)?,
            tenant_id: row.get::<Option<String>>(8)?,
        });
    }
    Ok(sessions)
}

/// Get a single agent session by ID.
pub async fn get_agent_session(db: &Codex, id: &str) -> Result<Option<AgentSessionRecord>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT id, agent_id, agent_name, started_at, ended_at, status, task_snapshot, context_summary, tenant_id
             FROM agent_sessions WHERE id = ?1",
            params![id],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(Some(AgentSessionRecord {
            id: row.get::<String>(0)?,
            agent_id: row.get::<String>(1)?,
            agent_name: row.get::<Option<String>>(2)?,
            started_at: row.get::<String>(3)?,
            ended_at: row.get::<Option<String>>(4)?,
            status: row.get::<String>(5)?,
            task_snapshot: row.get::<Option<String>>(6)?,
            context_summary: row.get::<Option<String>>(7)?,
            tenant_id: row.get::<Option<String>>(8)?,
        }))
    } else {
        Ok(None)
    }
}

/// Upsert an aggregated metric for an agent.
pub async fn upsert_agent_metric(
    db: &Codex,
    agent_id: &str,
    metric_name: &str,
    metric_value: f64,
    period: &str,
) -> Result<()> {
    let agent_id = agent_id.to_string();
    let metric_name = metric_name.to_string();
    let period = period.to_string();
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO agent_metrics (agent_id, metric_name, metric_value, period)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(agent_id, metric_name, period) DO UPDATE SET
                   metric_value=excluded.metric_value, timestamp=datetime('now')",
                params![
                    agent_id.as_str(),
                    metric_name.as_str(),
                    metric_value,
                    period.as_str(),
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Get all metrics for an agent in a given period.
pub async fn get_agent_metrics(
    db: &Codex,
    agent_id: &str,
    period: &str,
) -> Result<std::collections::HashMap<String, f64>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT metric_name, metric_value FROM agent_metrics
             WHERE agent_id=?1 AND period=?2",
            params![agent_id, period],
        )
        .await?;
    let mut map = std::collections::HashMap::new();
    while let Some(row) = rows.next().await? {
        map.insert(row.get::<String>(0)?, row.get::<f64>(1).unwrap_or(0.0));
    }
    Ok(map)
}
