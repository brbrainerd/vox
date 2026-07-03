//! Thin TCP client for [`super::dispatch_request`] (newline-delimited [`vox_foundation::protocol::DispatchRequest`]).

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use vox_foundation::protocol::orch_daemon_method;
use vox_foundation::protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

use super::normalize_tcp_bind_addr;

/// Well-known file the daemon writes its auth token to at startup, and that
/// [`OrchDaemonClient::new`] best-effort reads to auto-resolve a token (T0.2).
/// Mirrors `<user_home_dir>/.vox/run/orchestrator-daemon.token`, written by
/// `vox-orchestrator-d`'s `main()`.
fn token_file_path() -> std::path::PathBuf {
    vox_config::paths::user_home_dir()
        .join(".vox")
        .join("run")
        .join("orchestrator-daemon.token")
}

/// Best-effort read of the well-known daemon token file. Missing or unreadable
/// file (or empty contents) yields `None` rather than an error — callers treat
/// an absent token as "let the daemon's auth check reject the connection".
fn read_token_file() -> Option<String> {
    let contents = std::fs::read_to_string(token_file_path()).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Connect to `vox-orchestrator-d` and exchange one request/response pair per connection method call.
#[derive(Debug, Clone)]
pub struct OrchDaemonClient {
    addr: String,
    token: Option<String>,
    /// GUI-selected `PermissionMode` wire string (T0.3), threaded onto every
    /// [`DispatchRequest`] this client sends. `None` by default — the
    /// dispatch-side gate treats an absent mode as the fail-safe `ask`
    /// default (today's always-park behavior). Set via
    /// [`Self::with_permission_mode`].
    permission_mode: Option<String>,
}

impl OrchDaemonClient {
    /// Auto-resolves the daemon auth token by best-effort reading the
    /// well-known token file (`<user_home_dir>/.vox/run/orchestrator-daemon.token`).
    /// If the file is missing or unreadable, `token` stays `None` and the
    /// daemon's auth check will reject the connection with a clear error
    /// rather than this constructor panicking client-side.
    #[must_use]
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: normalize_tcp_bind_addr(&addr.into()),
            token: read_token_file(),
            permission_mode: None,
        }
    }

    /// Construct with an explicitly known token (e.g. the GUI's
    /// `PersistentDaemon`, which generates the token itself and injects it
    /// into the spawned daemon's environment — avoiding a race with reading a
    /// file the daemon may not have written yet).
    #[must_use]
    pub fn with_token(addr: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            addr: normalize_tcp_bind_addr(&addr.into()),
            token: Some(token.into()),
            permission_mode: None,
        }
    }

    /// Set the `PermissionMode` wire string (T0.3 — `"ask" | "accept_edits"
    /// | "accept_all" | "plan"`) to carry on every subsequent request from
    /// this client. Mirrors [`Self::with_token`]'s builder shape. Callers
    /// (the GUI's `invoke_mcp_tool`) set this from UI-selected state, never
    /// from tool-call `params` — see `DispatchRequest::permission_mode`'s
    /// doc comment for the isolation rationale.
    #[must_use]
    pub fn with_permission_mode(mut self, mode: impl Into<String>) -> Self {
        self.permission_mode = Some(mode.into());
        self
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
            auth_token: self.token.clone(),
            permission_mode: self.permission_mode.clone(),
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

    /// [`orch_daemon_method::SAFETY_BUDGET_SIGNALS`] — `{"agents": [{"id", "name", "signal"}]}`.
    pub async fn safety_budget_signals(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::SAFETY_BUDGET_SIGNALS, serde_json::json!({}))
            .await
    }

    /// [`orch_daemon_method::SAFETY_LEDGER`] — `{"receipts": [{"receipt_id", "agent_id", "tool_name"}]}`.
    pub async fn safety_ledger(
        &self,
        agent_id: Option<u64>,
    ) -> anyhow::Result<serde_json::Value> {
        self.call(
            orch_daemon_method::SAFETY_LEDGER,
            serde_json::json!({ "agent_id": agent_id }),
        )
        .await
    }

    /// [`orch_daemon_method::SAFETY_LOCKS`] — `{"locks": [{"resource_id", "kind", "holder", "expires_ms"}]}`.
    pub async fn safety_locks(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::SAFETY_LOCKS, serde_json::json!({}))
            .await
    }

    /// [`orch_daemon_method::ATTENTION_SNAPSHOT`] — `{"snapshot": AttentionBudget, "config": {...}}`.
    pub async fn attention_snapshot(&self) -> anyhow::Result<serde_json::Value> {
        self.call(orch_daemon_method::ATTENTION_SNAPSHOT, serde_json::json!({}))
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

    /// [`orch_daemon_method::SUBSCRIBE`] — open a long-lived status-snapshot
    /// stream, forwarding each pushed [`DispatchPayload::Event`] value into `tx`.
    /// Returns when the daemon closes the stream (`Done`) or the receiver drops.
    pub async fn subscribe(
        &self,
        tx: tokio::sync::mpsc::Sender<serde_json::Value>,
    ) -> anyhow::Result<()> {
        self.subscribe_with_method(orch_daemon_method::SUBSCRIBE, serde_json::json!({}), tx)
            .await
    }

    /// [`orch_daemon_method::SUBSCRIBE_EVENTS`] — open the live agent-event
    /// stream (token streaming + task/agent lifecycle). Each forwarded value is
    /// a serialized `AgentEvent` (`{ id, timestamp_ms, kind: { type, … } }`).
    pub async fn subscribe_events(
        &self,
        tx: tokio::sync::mpsc::Sender<serde_json::Value>,
    ) -> anyhow::Result<()> {
        self.subscribe_with_method(
            orch_daemon_method::SUBSCRIBE_EVENTS,
            serde_json::json!({}),
            tx,
        )
        .await
    }

    /// [`orch_daemon_method::SUBSCRIBE_EVENTS`] with a replay-from-offset
    /// cursor (T1.3): the daemon first pushes every durable Tier-A op with
    /// `op_id > from_offset` as a replay-envelope frame
    /// (`{ replay: true, op_id, agent_id, timestamp_ms, description, kind }`
    /// — distinct in shape from a live `AgentEvent` frame, see
    /// `orch_daemon::replay_frame_value`'s doc comment for why it isn't
    /// disguised as one), then transitions to the same live tail as
    /// [`Self::subscribe_events`]. A separate method (rather than an
    /// additional parameter on `subscribe_events`) so the existing zero-arg
    /// call sites (`vox-gui`'s `spawn_agent_event_stream`) are untouched.
    pub async fn subscribe_events_from_offset(
        &self,
        from_offset: u64,
        tx: tokio::sync::mpsc::Sender<serde_json::Value>,
    ) -> anyhow::Result<()> {
        self.subscribe_with_method(
            orch_daemon_method::SUBSCRIBE_EVENTS,
            serde_json::json!({ "from_offset": from_offset }),
            tx,
        )
        .await
    }

    /// Shared body for the `Event`-frame subscription methods: connect, send one
    /// request for `method` (with `params`), and forward each pushed `Event`
    /// value into `tx` until the daemon closes the stream or the receiver drops.
    /// An `Error` payload (T1.3: e.g. the Lagged-reconnect signal) ends the
    /// stream with an error rather than being silently swallowed, so a caller
    /// using `subscribe_events_from_offset` can detect "you lagged, reconnect"
    /// and re-subscribe with an updated offset.
    async fn subscribe_with_method(
        &self,
        method: &str,
        params: serde_json::Value,
        tx: tokio::sync::mpsc::Sender<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let mut stream = TcpStream::connect(&self.addr).await?;
        let (read_half, mut write_half) = stream.split();
        let req = DispatchRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.to_string(),
            params,
            auth_token: self.token.clone(),
            permission_mode: self.permission_mode.clone(),
        };
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');
        write_half.write_all(line.as_bytes()).await?;
        write_half.flush().await?;

        let mut reader = BufReader::new(read_half);
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = reader.read_line(&mut buf).await?;
            if n == 0 {
                break; // daemon closed the connection
            }
            let trimmed = buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            let resp: DispatchResponse = serde_json::from_str(trimmed)?;
            match resp.payload {
                DispatchPayload::Event { value } => {
                    if tx.send(value).await.is_err() {
                        break; // receiver dropped — stop consuming
                    }
                }
                DispatchPayload::Error { message, code } => {
                    anyhow::bail!("orchestrator daemon subscribe error ({code}): {message}")
                }
                DispatchPayload::Done { .. } => break,
                _ => {}
            }
        }
        Ok(())
    }
}
