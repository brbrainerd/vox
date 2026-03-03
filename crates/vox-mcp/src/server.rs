//! MCP Server state and protocol handler implementation.

use std::sync::Arc;
use tokio::sync::Mutex;
use rmcp::{model as protocol, ServerHandler, RoleServer, ErrorData};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, InitializeRequestParams,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
    ToolsCapability,
};
use rmcp::service::RequestContext;

use vox_orchestrator::{Orchestrator, OrchestratorConfig, SessionManager, SessionConfig, AgentEvent};
use vox_db::VoxDb;
use vox_skills::{SkillRegistry, install_builtins};

use crate::params::{ToolResult, SubmitTaskParams, TaskStatusParams};
use crate::tools;

#[derive(Clone)]
pub struct ServerState {
    pub orchestrator: Arc<Mutex<Orchestrator>>,
    pub db: Option<Arc<VoxDb>>,
    pub session_manager: Arc<Mutex<SessionManager>>,
    pub skill_registry: Arc<SkillRegistry>,
    pub transient_events: Arc<Mutex<Vec<AgentEvent>>>,
    /// Root directory of the workspace, used for @mention resolution and PLAN.md writing.
    pub workspace_root: Option<std::path::PathBuf>,
}

impl ServerState {
    pub fn new(config: OrchestratorConfig) -> Self {
        let session_cfg = SessionConfig::default();
        let session_manager =
            SessionManager::new(session_cfg).unwrap_or_else(|_| {
                // Fallback: in-memory only if disk is unavailable
                SessionManager::new(SessionConfig {
                    persist: false,
                    ..Default::default()
                })
                .unwrap_or_else(|e| {
                    panic!(
                        "in-memory session manager (fallback when persist fails): {}",
                        e
                    )
                })
            });
        let registry = Arc::new(SkillRegistry::new());

        // Auto-install built-in skills in the background
        let registry_for_builtins = registry.clone();
        tokio::spawn(async move {
            match install_builtins(&registry_for_builtins).await {
                Ok(n) if n > 0 => tracing::info!("Auto-installed {} built-in skill(s)", n),
                Ok(_) => {} // already installed
                Err(e) => tracing::warn!("Failed to auto-install built-in skills: {}", e),
            }
        });

        // Auto-detect workspace root from CWD or a Vox.toml search
        let workspace_root = find_workspace_root();

        Self {
            orchestrator: Arc::new(Mutex::new(Orchestrator::new(config))),
            db: None,
            session_manager: Arc::new(Mutex::new(session_manager)),
            skill_registry: registry,
            transient_events: Arc::new(Mutex::new(Vec::new())),
            workspace_root,
        }
    }

    /// Override the workspace root (useful for tests or when the extension provides it explicitly).
    pub fn with_workspace_root(mut self, path: std::path::PathBuf) -> Self {
        self.workspace_root = Some(path);
        self
    }

    pub async fn new_test() -> Self {
        let config = OrchestratorConfig::default();
        Self::new(config)
    }

    pub fn with_db(mut self, db: VoxDb) -> Self {
        let db_arc = Arc::new(db);
        self.db = Some(db_arc.clone());

        // Stream orchestrator events to gamify database
        let mut rx = {
            let orch = self
                .orchestrator
                .try_lock()
                .unwrap_or_else(|e| panic!("orchestrator lock busy/poisoned: {}", e));
            orch.event_bus().subscribe()
        };

        let db_for_task = db_arc.clone();
        let transient = self.transient_events.clone();
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let agent_and_type = match &event.kind {
                    vox_orchestrator::AgentEventKind::AgentSpawned { agent_id, .. } => {
                        Some((agent_id.0, "AgentSpawned"))
                    }
                    vox_orchestrator::AgentEventKind::AgentRetired { agent_id } => {
                        Some((agent_id.0, "AgentRetired"))
                    }
                    vox_orchestrator::AgentEventKind::ActivityChanged { agent_id, .. } => {
                        Some((agent_id.0, "ActivityChanged"))
                    }
                    vox_orchestrator::AgentEventKind::TaskSubmitted { agent_id, .. } => {
                        Some((agent_id.0, "TaskSubmitted"))
                    }
                    vox_orchestrator::AgentEventKind::TaskStarted { agent_id, .. } => {
                        Some((agent_id.0, "TaskStarted"))
                    }
                    vox_orchestrator::AgentEventKind::TaskCompleted { agent_id, .. } => {
                        Some((agent_id.0, "TaskCompleted"))
                    }
                    vox_orchestrator::AgentEventKind::TaskFailed { agent_id, .. } => {
                        Some((agent_id.0, "TaskFailed"))
                    }
                    vox_orchestrator::AgentEventKind::LockAcquired { agent_id, .. } => {
                        Some((agent_id.0, "LockAcquired"))
                    }
                    vox_orchestrator::AgentEventKind::LockReleased { agent_id, .. } => {
                        Some((agent_id.0, "LockReleased"))
                    }
                    vox_orchestrator::AgentEventKind::AgentIdle { agent_id } => {
                        Some((agent_id.0, "AgentIdle"))
                    }
                    vox_orchestrator::AgentEventKind::AgentBusy { agent_id } => {
                        Some((agent_id.0, "AgentBusy"))
                    }
                    vox_orchestrator::AgentEventKind::MessageSent { from, .. } => {
                        Some((from.0, "MessageSent"))
                    }
                    vox_orchestrator::AgentEventKind::CostIncurred { agent_id, .. } => {
                        Some((agent_id.0, "CostIncurred"))
                    }
                    vox_orchestrator::AgentEventKind::ContinuationTriggered {
                        agent_id, ..
                    } => Some((agent_id.0, "ContinuationTriggered")),
                    vox_orchestrator::AgentEventKind::PlanHandoff { from, .. } => {
                        Some((from.0, "PlanHandoff"))
                    }
                    vox_orchestrator::AgentEventKind::AgentHandoffAccepted { agent_id, .. } => {
                        Some((agent_id.0, "AgentHandoffAccepted"))
                    }
                    vox_orchestrator::AgentEventKind::AgentHandoffRejected { from, .. } => {
                        Some((from.0, "AgentHandoffRejected"))
                    }
                    vox_orchestrator::AgentEventKind::ScopeViolation { agent_id, .. } => {
                        Some((agent_id.0, "ScopeViolation"))
                    }
                    vox_orchestrator::AgentEventKind::PromptConflictDetected { .. } => {
                        Some((0, "PromptConflictDetected"))
                    }
                    vox_orchestrator::AgentEventKind::InjectionDetected { .. } => {
                        Some((0, "InjectionDetected"))
                    }
                    vox_orchestrator::AgentEventKind::CompactionTriggered { agent_id, .. } => {
                        Some((agent_id.0, "CompactionTriggered"))
                    }
                    vox_orchestrator::AgentEventKind::MemoryFlushed { agent_id, .. } => {
                        Some((agent_id.0, "MemoryFlushed"))
                    }
                    vox_orchestrator::AgentEventKind::SessionCreated { agent_id, .. } => {
                        Some((agent_id.0, "SessionCreated"))
                    }
                    vox_orchestrator::AgentEventKind::SessionReset { agent_id, .. } => {
                        Some((agent_id.0, "SessionReset"))
                    }
                    vox_orchestrator::AgentEventKind::WorkflowStarted { .. } => {
                        Some((0, "WorkflowStarted"))
                    }
                    vox_orchestrator::AgentEventKind::WorkflowCompleted { .. } => {
                        Some((0, "WorkflowCompleted"))
                    }
                    vox_orchestrator::AgentEventKind::WorkflowFailed { .. } => {
                        Some((0, "WorkflowFailed"))
                    }
                    vox_orchestrator::AgentEventKind::ActivityStarted { .. } => {
                        Some((0, "ActivityStarted"))
                    }
                    vox_orchestrator::AgentEventKind::ActivityCompleted { .. } => {
                        Some((0, "ActivityCompleted"))
                    }
                    vox_orchestrator::AgentEventKind::ActivityRetried { .. } => {
                        Some((0, "ActivityRetried"))
                    }
                    // JJ-inspired VCS events
                    vox_orchestrator::AgentEventKind::SnapshotCaptured { agent_id, .. } => {
                        Some((agent_id.0, "SnapshotCaptured"))
                    }
                    vox_orchestrator::AgentEventKind::OperationUndone { agent_id, .. } => {
                        Some((agent_id.0, "OperationUndone"))
                    }
                    vox_orchestrator::AgentEventKind::OperationRedone { agent_id, .. } => {
                        Some((agent_id.0, "OperationRedone"))
                    }
                    vox_orchestrator::AgentEventKind::ConflictDetected { agent_ids, .. } => Some((
                        agent_ids.first().map(|a| a.0).unwrap_or(0),
                        "ConflictDetected",
                    )),
                    vox_orchestrator::AgentEventKind::ConflictResolved { .. } => {
                        Some((0, "ConflictResolved"))
                    }
                    vox_orchestrator::AgentEventKind::WorkspaceCreated { agent_id, .. } => {
                        Some((agent_id.0, "WorkspaceCreated"))
                    }
                    vox_orchestrator::AgentEventKind::UrgentRebalanceTriggered { .. } => {
                        Some((0, "UrgentRebalanceTriggered"))
                    }
                    vox_orchestrator::AgentEventKind::TokenStreamed { .. } => {
                        // Keep transient events in memory
                        if let Ok(mut q) = transient.try_lock() {
                            q.push(event.clone());
                        }
                        None
                    }
                };

                if let Some((agent_id, event_type)) = agent_and_type {
                    let payload = serde_json::to_string(&event.kind).unwrap_or_default();
                    let _ = vox_gamify::db::insert_event(
                        &db_for_task,
                        &agent_id.to_string(),
                        event_type,
                        Some(&payload),
                    )
                    .await;

                    // Process rewards
                    let kind_json = serde_json::to_value(&event.kind).unwrap_or_default();
                    let _ = vox_gamify::db::process_event_rewards(
                        &db_for_task,
                        vox_gamify::util::DEFAULT_USER_ID,
                        &kind_json,
                    )
                    .await;
                }
            }
        });

        // Wire DB into skill registry for persistence
        self.skill_registry.set_db(db_arc.clone());

        self
    }
}

/// Walk up from CWD to find the workspace root (directory containing Vox.toml or Cargo.toml).
fn find_workspace_root() -> Option<std::path::PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join("Vox.toml").exists() || current.join("Cargo.toml").exists() {
            return Some(current);
        }
        if !current.pop() {
            return std::env::current_dir().ok();
        }
    }
}

pub struct VoxMcpServer {
    state: ServerState,
}

impl VoxMcpServer {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }
}

impl ServerHandler for VoxMcpServer {
    async fn initialize(
        &self,
        params: InitializeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        let tool_count = tools::TOOL_REGISTRY.len();
        let vox_version = env!("CARGO_PKG_VERSION");
        Ok(InitializeResult {
            protocol_version: params.protocol_version.clone(),
            server_info: Implementation {
                name: "vox-mcp".to_string(),
                version: vox_version.to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                experimental: {
                    let mut map = std::collections::BTreeMap::new();
                    let mut inner = serde_json::Map::new();
                    inner.insert("messagepack".to_string(), serde_json::Value::Bool(true));
                    inner.insert("inmem_transport".to_string(), serde_json::Value::Bool(true));
                    map.insert("transport_capabilities".to_string(), inner);
                    Some(map)
                },
                ..Default::default()
            },
            instructions: Some(format!(
                "vox-mcp v{} | tools: {} | protocol: {}",
                vox_version, tool_count, params.protocol_version,
            )),
        })
    }

    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut tools = tools::tool_registry();

        // Auto-register tools from installed skills
        let skills = self.state.skill_registry.list(None);
        for skill in skills {
            for tool_name in &skill.tools {
                if !tools.iter().any(|t| t.name == *tool_name) {
                    tools.push(rmcp::model::Tool {
                        name: std::borrow::Cow::Owned(tool_name.clone()),
                        description: Some(std::borrow::Cow::Owned(format!(
                            "Instructional macro tool from skill: {}",
                            skill.name
                        ))),
                        input_schema: std::sync::Arc::new(serde_json::Map::new()),
                        output_schema: None,
                        meta: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        title: None,
                    });
                }
            }
        }

        Ok(ListToolsResult {
            meta: None,
            tools,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
        let name_str = params.name.to_string();
        let result_json = tools::handle_tool_call(&self.state, &name_str, args)
            .await
            .unwrap_or_else(|e| format!("{{\"success\":false,\"error\":\"{}\"}}", e));

        Ok(CallToolResult {
            content: vec![Content::text(result_json)],
            is_error: Some(false),
            meta: None,
            structured_content: None,
        })
    }
}
