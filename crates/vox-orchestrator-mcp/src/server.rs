//! RMCP [`ServerHandler`] for tool listing and `call_tool` dispatch.

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, InitializeRequestParams,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_TOOL_DISPATCH: &str = "Retry the call; if it persists, check tool arguments against the schema and restart the MCP server.";

/// RMCP [`ServerHandler`] implementation listing tools and dispatching `call_tool`.
pub struct VoxMcpServer {
    state: ServerState,
}

impl VoxMcpServer {
    /// Wrap `state` for use with `rmcp` transport loops.
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
        let tool_count = crate::TOOL_REGISTRY.len();
        let vox_version = env!("CARGO_PKG_VERSION");
        let mut experimental = std::collections::BTreeMap::new();
        let mut inner = serde_json::Map::new();
        inner.insert("messagepack".to_string(), serde_json::Value::Bool(true));
        inner.insert("inmem_transport".to_string(), serde_json::Value::Bool(true));
        experimental.insert("transport_capabilities".to_string(), inner);
        // Skills may append tools after startup; `enable_tool_list_changed` tells
        // clients to refresh their tool list occasionally.
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tool_list_changed()
            .enable_experimental_with(experimental)
            .build();
        Ok(InitializeResult::new(capabilities)
            .with_protocol_version(params.protocol_version.clone())
            .with_server_info(Implementation::new("vox-mcp", vox_version))
            .with_instructions(format!(
                "vox-mcp v{} | tools: {} | protocol: {}",
                vox_version, tool_count, params.protocol_version,
            )))
    }

    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut tool_list = crate::tool_registry();

        // E.56 MCP tool tiering: load core by default, dev/advanced require opt-in
        let allowed_tiers_env =
            std::env::var("VOX_MCP_TIERS").unwrap_or_else(|_| "core".to_string());
        let allowed_tiers: Vec<&str> = allowed_tiers_env.split(',').collect();

        if !allowed_tiers.contains(&"all") {
            tool_list.retain(|t| {
                if let Some(meta) = &t.meta {
                    if let Some(serde_json::Value::String(tier)) = meta.0.get("vox_tier") {
                        return allowed_tiers.contains(&tier.as_str());
                    }
                }
                true
            });
        }

        // Auto-register tools from installed skills
        let skills = self.state.skill_registry.list(None);
        for skill in skills {
            for tool_name in &skill.tools {
                if !tool_list.iter().any(|t| t.name == *tool_name) {
                    tool_list.push(rmcp::model::Tool::new_with_raw(
                        std::borrow::Cow::Owned(tool_name.clone()),
                        Some(std::borrow::Cow::Owned(format!(
                            "Instructional macro tool from skill: {}",
                            skill.name
                        ))),
                        std::sync::Arc::new(serde_json::Map::new()),
                    ));
                }
            }
        }

        Ok(ListToolsResult {
            meta: None,
            tools: tool_list,
            next_cursor: None,
        })
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.state.orchestrator.record_activity();
        let args = params
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
        let name_str = params.name.to_string();
        let (result_json, is_error): (String, bool) =
            match crate::handle_tool_call(&self.state, &name_str, args).await {
                Ok(json) => {
                    let is_err = tool_json_envelope_is_error(&json);
                    (json, is_err)
                }
                Err(e) => {
                    let msg = format!("{e}");
                    (
                        ToolResult::<serde_json::Value>::err_with_remediation(
                            msg,
                            REM_TOOL_DISPATCH,
                        )
                        .to_json(),
                        true,
                    )
                }
            };

        let content = vec![Content::text(result_json)];
        Ok(if is_error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        })
    }
}

/// Returns true when JSON looks like `ToolResult` with `success: false` (MCP `is_error` signal).
pub fn tool_json_envelope_is_error(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("success").and_then(|s| s.as_bool()))
        == Some(false)
}

#[cfg(test)]
mod call_tool_tests {
    use super::tool_json_envelope_is_error;

    #[test]
    fn envelope_error_when_success_false() {
        assert!(tool_json_envelope_is_error(
            r#"{"success":false,"error":"nope"}"#
        ));
    }

    #[test]
    fn envelope_ok_when_success_true() {
        assert!(!tool_json_envelope_is_error(
            r#"{"success":true,"data":"x"}"#
        ));
    }

    #[test]
    fn envelope_ok_when_not_tool_result_shape() {
        assert!(!tool_json_envelope_is_error("not json"));
        assert!(!tool_json_envelope_is_error(r#"{"foo":1}"#));
    }
}
