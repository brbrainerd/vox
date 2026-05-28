use crate::VoxDb;
use crate::store::types::StoreError;
use tokio::sync::{mpsc, oneshot};

/// Command sent to the dedicated database writer task.
pub enum DbWriteCmd {
    /// Inserts a high-frequency agent event.
    InsertAgentEvent {
        agent_id: String,
        event_type: String,
        payload_json: Option<String>,
        cli_version: Option<String>,
        resp: oneshot::Sender<Result<i64, StoreError>>,
    },
    /// Records an LLM interaction cost.
    InsertCostRecord {
        agent_id: String,
        session_id: Option<String>,
        tenant_id: Option<String>,
        provider: String,
        model: Option<String>,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
        resp: oneshot::Sender<Result<i64, StoreError>>,
    },
    /// Records tool execution timing and metrics.
    InsertExecHistory {
        tool: String,
        repository_id: String,
        session_id: Option<String>,
        duration_ms: i64,
        cost_usd: Option<f64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        resp: oneshot::Sender<Result<(), StoreError>>,
    },
    /// Persists an A2A message to durable storage.
    InsertA2AMessage {
        sender: u64,
        receiver: u64,
        msg_type: String,
        payload: String,
        idempotency_key: String,
        repository_id: String,
        resp: oneshot::Sender<Result<String, StoreError>>,
    },
    /// Appends to the flattened telemetry projection table (Scientia v51).
    InsertTelemetryFlat {
        agent_id: String,
        session_id: String,
        tenant_id: Option<String>,
        repository_id: String,
        event_kind: String,
        tool_name: Option<String>,
        model_id: Option<String>,
        provider: Option<String>,
        duration_ms: Option<i64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        cost_usd: Option<f64>,
        payload_json: Option<String>,
        resp: oneshot::Sender<Result<(), StoreError>>,
    },
    /// Inserts a new entry into the scientia publication queue.
    InsertPublicationQueue {
        discovery_id: String,
        publication_id: String,
        stage: String,
        resp: oneshot::Sender<Result<(), StoreError>>,
    },
    /// Shutdown the actor task gracefully.
    Shutdown,
}

/// Handle to the dedicated writer actor.
/// Use this to serialize all write operations to a single task, eliminating lock contention.
#[derive(Clone)]
pub struct VoxWriteHandle {
    tx: mpsc::Sender<DbWriteCmd>,
}

impl VoxWriteHandle {
    pub async fn send(&self, cmd: DbWriteCmd) -> Result<(), StoreError> {
        self.tx
            .send(cmd)
            .await
            .map_err(|_| StoreError::Internal("VoxWriteActor task has terminated".to_string()))
    }

    /// Helper to insert an agent event via the actor.
    pub async fn insert_agent_event(
        &self,
        agent_id: String,
        event_type: String,
        payload_json: Option<String>,
        cli_version: Option<String>,
    ) -> Result<i64, StoreError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.send(DbWriteCmd::InsertAgentEvent {
            agent_id,
            event_type,
            payload_json,
            cli_version,
            resp: resp_tx,
        })
        .await?;
        resp_rx
            .await
            .map_err(|_| StoreError::Internal("Actor response channel dropped".to_string()))?
    }

    /// Helper to record a cost record via the actor.
    pub async fn insert_cost_record(
        &self,
        agent_id: String,
        session_id: Option<String>,
        tenant_id: Option<String>,
        provider: String,
        model: Option<String>,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<i64, StoreError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.send(DbWriteCmd::InsertCostRecord {
            agent_id,
            session_id,
            tenant_id,
            provider,
            model,
            input_tokens,
            output_tokens,
            cost_usd,
            resp: resp_tx,
        })
        .await?;
        resp_rx
            .await
            .map_err(|_| StoreError::Internal("Actor response channel dropped".to_string()))?
    }

    /// Helper to insert execution history via the actor.
    pub async fn insert_exec_history(
        &self,
        tool: String,
        repository_id: String,
        session_id: Option<String>,
        duration_ms: i64,
        cost_usd: Option<f64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    ) -> Result<(), StoreError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.send(DbWriteCmd::InsertExecHistory {
            tool,
            repository_id,
            session_id,
            duration_ms,
            cost_usd,
            input_tokens,
            output_tokens,
            resp: resp_tx,
        })
        .await?;
        resp_rx
            .await
            .map_err(|_| StoreError::Internal("Actor response channel dropped".to_string()))?
    }

    /// Helper to persist an A2A message via the actor.
    pub async fn insert_a2a_message(
        &self,
        sender: u64,
        receiver: u64,
        msg_type: String,
        payload: String,
        idempotency_key: String,
        repository_id: String,
    ) -> Result<String, StoreError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.send(DbWriteCmd::InsertA2AMessage {
            sender,
            receiver,
            msg_type,
            payload,
            idempotency_key,
            repository_id,
            resp: resp_tx,
        })
        .await?;
        resp_rx
            .await
            .map_err(|_| StoreError::Internal("Actor response channel dropped".to_string()))?
    }

    /// Helper to insert telemetry flat via the actor.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_telemetry_flat(
        &self,
        agent_id: String,
        session_id: String,
        tenant_id: Option<String>,
        repository_id: String,
        event_kind: String,
        tool_name: Option<String>,
        model_id: Option<String>,
        provider: Option<String>,
        duration_ms: Option<i64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        cost_usd: Option<f64>,
        payload_json: Option<String>,
    ) -> Result<(), StoreError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.send(DbWriteCmd::InsertTelemetryFlat {
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
            resp: resp_tx,
        })
        .await?;
        resp_rx
            .await
            .map_err(|_| StoreError::Internal("Actor response channel dropped".to_string()))?
    }

    /// Helper to insert a new entry into the scientia publication queue.
    pub async fn insert_publication_queue(
        &self,
        discovery_id: String,
        publication_id: String,
        stage: String,
    ) -> Result<(), StoreError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.send(DbWriteCmd::InsertPublicationQueue {
            discovery_id,
            publication_id,
            stage,
            resp: resp_tx,
        })
        .await?;
        resp_rx
            .await
            .map_err(|_| StoreError::Internal("Actor response channel dropped".to_string()))?
    }
}

/// Starts the writer actor task.
pub fn spawn_writer(db: VoxDb) -> VoxWriteHandle {
    let (tx, mut rx) = mpsc::channel(1024);
    let tx_for_actor = tx.clone();
    let writer_db = db.clone();

    tokio::spawn(async move {
        let tx_clone = tx_for_actor.clone();
        let mut msg_count = 0u64;
        while let Some(cmd) = rx.recv().await {
            msg_count += 1;
            // Every 100 messages, check capacity as a proxy for backpressure
            if msg_count.is_multiple_of(100) {
                let cap = tx_clone.capacity();
                if cap < 100 {
                    // Emit backpressure warning to flat telemetry
                    let _ = writer_db
                        .insert_telemetry_flat_raw(
                            "writer_actor",
                            "null",
                            None,
                            "null",
                            "backpressure_warning",
                            None,
                            None,
                            None,
                            Some(cap as i64),
                            None,
                            None,
                            None,
                            Some(&format!("{{\"capacity\":{}}}", cap)),
                        )
                        .await;
                }
            }
            match cmd {
                DbWriteCmd::InsertAgentEvent {
                    agent_id,
                    event_type,
                    payload_json,
                    cli_version,
                    resp,
                } => {
                    let res = db
                        .insert_agent_event_raw(
                            &agent_id,
                            &event_type,
                            payload_json.as_deref(),
                            cli_version.as_deref(),
                        )
                        .await;
                    let _ = resp.send(res);
                }
                DbWriteCmd::InsertCostRecord {
                    agent_id,
                    session_id,
                    tenant_id,
                    provider,
                    model,
                    input_tokens,
                    output_tokens,
                    cost_usd,
                    resp,
                } => {
                    let res = db
                        .insert_cost_record(
                            &agent_id,
                            session_id.as_deref(),
                            tenant_id.as_deref(),
                            &provider,
                            model.as_deref(),
                            input_tokens,
                            output_tokens,
                            cost_usd,
                        )
                        .await;
                    let _ = resp.send(res);
                }
                DbWriteCmd::InsertExecHistory {
                    tool,
                    repository_id,
                    session_id,
                    duration_ms,
                    cost_usd,
                    input_tokens,
                    output_tokens,
                    resp,
                } => {
                    let res = db
                        .insert_exec_history_raw(
                            &tool,
                            &repository_id,
                            session_id.as_deref(),
                            duration_ms,
                            cost_usd,
                            input_tokens,
                            output_tokens,
                        )
                        .await;
                    let _ = resp.send(res);
                }
                DbWriteCmd::InsertA2AMessage {
                    sender,
                    receiver,
                    msg_type,
                    payload,
                    idempotency_key,
                    repository_id,
                    resp,
                } => {
                    let res = db
                        .insert_a2a_message_raw(
                            sender,
                            receiver,
                            &msg_type,
                            &payload,
                            &idempotency_key,
                            &repository_id,
                        )
                        .await;
                    let _ = resp.send(res);
                }
                DbWriteCmd::InsertTelemetryFlat {
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
                    resp,
                } => {
                    let res = db
                        .insert_telemetry_flat_raw(
                            &agent_id,
                            &session_id,
                            tenant_id.as_deref(),
                            &repository_id,
                            &event_kind,
                            tool_name.as_deref(),
                            model_id.as_deref(),
                            provider.as_deref(),
                            duration_ms,
                            input_tokens,
                            output_tokens,
                            cost_usd,
                            payload_json.as_deref(),
                        )
                        .await;
                    let _ = resp.send(res);
                }
                DbWriteCmd::InsertPublicationQueue {
                    discovery_id,
                    publication_id,
                    stage,
                    resp,
                } => {
                    let res = db
                        .insert_publication_queue_raw(&discovery_id, &publication_id, &stage)
                        .await;
                    let _ = resp.send(res);
                }
                DbWriteCmd::Shutdown => break,
            }
        }
    });

    VoxWriteHandle { tx }
}

impl VoxDb {
    // These methods should be implemented in facade/ or lib.rs to wrap the raw SQL.
    // Assuming they exist or adding them to the facade.
}
