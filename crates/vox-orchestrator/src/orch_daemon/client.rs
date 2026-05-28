//! Thin TCP client for [`super::dispatch_request`] (newline-delimited [`vox_foundation::protocol::DispatchRequest`]).

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use vox_foundation::protocol::orch_daemon_method;
use vox_foundation::protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

use super::normalize_tcp_bind_addr;

/// Connect to `vox-orchestrator-d` and exchange one request/response pair per connection method call.
#[derive(Debug, Clone)]
pub struct OrchDaemonClient {
    addr: String,
}

impl OrchDaemonClient {
    #[must_use]
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: normalize_tcp_bind_addr(&addr.into()),
        }
    }

    /// Send one line, read one line (blocking for this request).
    pub async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let mut stream = TcpStream::connect(&self.addr).await?;
        let (read_half, mut write_half) = stream.split();
        let id = uuid::Uuid::new_v4().to_string();
        let req = DispatchRequest {
            id,
            method: method.to_string(),
            params,
        };
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');
        write_half.write_all(line.as_bytes()).await?;
        write_half.flush().await?;

        let mut reader = BufReader::new(read_half);
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await?;
        let resp: DispatchResponse = serde_json::from_str(resp_line.trim())?;
        match resp.payload {
            DispatchPayload::Result { value } => Ok(value),
            DispatchPayload::Error { message, code } => {
                anyhow::bail!("orchestrator daemon error ({code}): {message}")
            }
            _ => anyhow::bail!("unexpected orchestrator daemon payload (not a Result)"),
        }
    }

    /// [`orch_daemon_method::PING`].
    pub async fn ping(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::PING, serde_json::json!({}))
            .await
    }

    /// [`orch_daemon_method::STATUS`] — full orchestrator status JSON.
    pub async fn orchestrator_status(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::STATUS, serde_json::json!({}))
            .await
    }

    /// [`orch_daemon_method::TASK_STATUS`] — `{"status": "..."}` or error payload.
    pub async fn task_status(&self, task_id: u64) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::TASK_STATUS,
            serde_json::json!({ "task_id": task_id }),
        )
        .await
    }

    /// [`orch_daemon_method::SPAWN_AGENT`] — `{"agent_id": u64}`.
    pub async fn spawn_agent_named(&self, name: &str) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::SPAWN_AGENT,
            serde_json::json!({ "name": name }),
        )
        .await
    }

    /// [`orch_daemon_method::AGENT_IDS`] — `{"agent_ids": [u64, ...]}`.
    pub async fn agent_ids(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::AGENT_IDS, serde_json::json!({}))
            .await
    }

    /// [`orch_daemon_method::SUBMIT_TASK`] — returns `{"task_id": u64}`.
    pub async fn submit_task(
        &self,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::SUBMIT_TASK, params).await
    }

    /// [`orch_daemon_method::COMPLETE_TASK`] — returns `{"ok": true}`.
    pub async fn complete_task(
        &self,
        task_id: u64,
        attestation: Option<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::COMPLETE_TASK,
            serde_json::json!({ "task_id": task_id, "attestation": attestation }),
        )
        .await
    }

    /// [`orch_daemon_method::FAIL_TASK`] — returns `{"ok": true}`.
    pub async fn fail_task(
        &self,
        task_id: u64,
        reason: String,
    ) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::FAIL_TASK,
            serde_json::json!({ "task_id": task_id, "reason": reason }),
        )
        .await
    }

    /// [`orch_daemon_method::CANCEL_TASK`] — returns `{"ok": true}`.
    pub async fn cancel_task(&self, task_id: u64) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::CANCEL_TASK,
            serde_json::json!({ "task_id": task_id }),
        )
        .await
    }

    /// [`orch_daemon_method::REORDER_TASK`] — returns `{"ok": true}`.
    pub async fn reorder_task(
        &self,
        task_id: u64,
        priority: &str,
    ) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::REORDER_TASK,
            serde_json::json!({ "task_id": task_id, "priority": priority }),
        )
        .await
    }

    /// [`orch_daemon_method::DRAIN_AGENT`] — returns `{"drained_count": u64}`.
    pub async fn drain_agent(&self, agent_id: u64) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::DRAIN_AGENT,
            serde_json::json!({ "agent_id": agent_id }),
        )
        .await
    }

    /// [`orch_daemon_method::REBALANCE`] — returns `{"rebalanced": u64}`.
    pub async fn rebalance(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::REBALANCE, serde_json::json!({}))
            .await
    }

    /// [`orch_daemon_method::SPAWN_AGENT_EXT`] — returns `{"agent_id": u64}`.
    pub async fn spawn_agent_ext(
        &self,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::SPAWN_AGENT_EXT, params).await
    }

    /// [`orch_daemon_method::RETIRE_AGENT`] — returns `{"remaining_tasks": u64}`.
    pub async fn retire_agent(&self, agent_id: u64) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::RETIRE_AGENT,
            serde_json::json!({ "agent_id": agent_id }),
        )
        .await
    }

    /// [`orch_daemon_method::PAUSE_AGENT`] — returns `{"ok": true}`.
    pub async fn pause_agent(&self, agent_id: u64) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::PAUSE_AGENT,
            serde_json::json!({ "agent_id": agent_id }),
        )
        .await
    }

    /// [`orch_daemon_method::RESUME_AGENT`] — returns `{"ok": true}`.
    pub async fn resume_agent(&self, agent_id: u64) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::RESUME_AGENT,
            serde_json::json!({ "agent_id": agent_id }),
        )
        .await
    }

    /// [`orch_daemon_method::WORKSPACE_JOURNEY`] — workspace store diagnostics JSON.
    pub async fn workspace_journey(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::WORKSPACE_JOURNEY, serde_json::json!({}))
            .await
    }
}
