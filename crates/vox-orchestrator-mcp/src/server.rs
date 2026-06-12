//! RMCP [`ServerHandler`] for tool listing and `call_tool` dispatch.

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, GetPromptRequestParams, GetPromptResult,
    Implementation, InitializeRequestParams, InitializeResult, ListPromptsResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt, PromptMessage,
    PromptMessageRole, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_TOOL_DISPATCH: &str = "Retry the call; if it persists, check tool arguments against the schema and restart the MCP server.";

const RESOURCE_LLMS_TXT_URI: &str = "resource://vox/llms.txt";
const RESOURCE_AGENTS_MD_URI: &str = "resource://vox/agents.md";

const LLMS_TXT_REL: &str = "docs/src/.well-known/llms.txt";
const AGENTS_MD_REL: &str = "AGENTS.md";

const PROMPT_AGENTS_POLICY: &str = "agents-policy";
const PROMPT_VOX_CHECK: &str = "vox-check";
const PROMPT_LLMS_DISCOVERY: &str = "llms-discovery";

fn static_vox_resources() -> Vec<Resource> {
    vec![
        Resource::new(
            RawResource::new(RESOURCE_LLMS_TXT_URI, "llms.txt")
                .with_title("Vox LLM discovery index")
                .with_description("Agent discovery index for the Vox repository")
                .with_mime_type("text/plain"),
            None,
        ),
        Resource::new(
            RawResource::new(RESOURCE_AGENTS_MD_URI, "agents.md")
                .with_title("AGENTS.md")
                .with_description("Cross-tool agent policy surface for the Vox repository")
                .with_mime_type("text/markdown"),
            None,
        ),
    ]
}

fn read_vox_resource(
    repo_root: &std::path::Path,
    uri: &str,
) -> Result<ReadResourceResult, ErrorData> {
    let (rel_path, mime_type) = match uri {
        RESOURCE_LLMS_TXT_URI => (LLMS_TXT_REL, "text/plain"),
        RESOURCE_AGENTS_MD_URI => (AGENTS_MD_REL, "text/markdown"),
        other => {
            return Err(ErrorData::invalid_params(
                format!("unknown resource uri: {other}"),
                None,
            ));
        }
    };

    let path = repo_root.join(rel_path);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        ErrorData::internal_error(format!("failed to read {}: {e}", path.display()), None)
    })?;

    Ok(ReadResourceResult::new(vec![
        ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some(mime_type.to_string()),
            text,
            meta: None,
        },
    ]))
}

fn static_vox_prompts() -> Vec<Prompt> {
    vec![
        Prompt::new(
            PROMPT_AGENTS_POLICY,
            Some("Load cross-tool agent policy from AGENTS.md"),
            None,
        )
        .with_title("Agents policy"),
        Prompt::new(
            PROMPT_VOX_CHECK,
            Some("Validate .vox source before edits using vox_check"),
            None,
        )
        .with_title("Vox check workflow"),
        Prompt::new(
            PROMPT_LLMS_DISCOVERY,
            Some("Repository discovery index from llms.txt"),
            None,
        )
        .with_title("LLM discovery index"),
    ]
}

fn get_vox_prompt(repo_root: &std::path::Path, name: &str) -> Result<GetPromptResult, ErrorData> {
    match name {
        PROMPT_AGENTS_POLICY => {
            let path = repo_root.join(AGENTS_MD_REL);
            let text = std::fs::read_to_string(&path).map_err(|e| {
                ErrorData::internal_error(format!("failed to read {}: {e}", path.display()), None)
            })?;
            Ok(GetPromptResult::new(vec![PromptMessage::new_text(
                PromptMessageRole::User,
                format!("Follow this repository agent policy:\n\n{text}"),
            )]))
        }
        PROMPT_VOX_CHECK => Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            "Before committing .vox changes, call the `vox_check` MCP tool \
             (or `vox check --output-format json`) and fix all reported errors.",
        )])),
        PROMPT_LLMS_DISCOVERY => {
            let path = repo_root.join(LLMS_TXT_REL);
            let text = std::fs::read_to_string(&path).map_err(|e| {
                ErrorData::internal_error(format!("failed to read {}: {e}", path.display()), None)
            })?;
            Ok(GetPromptResult::new(vec![PromptMessage::new_text(
                PromptMessageRole::User,
                format!("Use this discovery index:\n\n{text}"),
            )]))
        }
        other => Err(ErrorData::invalid_params(
            format!("unknown prompt: {other}"),
            None,
        )),
    }
}

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
            .enable_resources()
            .enable_prompts()
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

    async fn list_resources(
        &self,
        _params: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            meta: None,
            resources: static_vox_resources(),
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        read_vox_resource(&self.state.repository.root, &params.uri)
    }

    async fn list_prompts(
        &self,
        _params: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult {
            meta: None,
            prompts: static_vox_prompts(),
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        get_vox_prompt(&self.state.repository.root, &params.name)
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

#[cfg(test)]
mod prompt_tests {
    use super::*;

    #[test]
    fn static_prompt_catalog_lists_three() {
        assert_eq!(static_vox_prompts().len(), 3);
        let names: Vec<_> = static_vox_prompts().into_iter().map(|p| p.name).collect();
        assert!(names.contains(&PROMPT_AGENTS_POLICY.to_string()));
        assert!(names.contains(&PROMPT_VOX_CHECK.to_string()));
    }

    #[test]
    fn get_vox_check_prompt_has_message() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let result = get_vox_prompt(&root, PROMPT_VOX_CHECK).expect("vox-check prompt");
        assert!(!result.messages.is_empty());
    }

    #[test]
    fn get_unknown_prompt_is_invalid_params() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let err = get_vox_prompt(&root, "no-such-prompt").expect_err("unknown prompt");
        assert!(err.message.contains("unknown prompt"));
    }
}
