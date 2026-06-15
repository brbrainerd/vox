//! RMCP client wiring for outbound connections to third-party MCP servers.

use anyhow::{Context, Result, anyhow, bail};
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation, Tool};
use rmcp::service::{RunningService, ServiceError};
use rmcp::{RoleClient, ServiceExt};

use crate::mcp_client::ToolDefinition;

/// Configuration for an external MCP server reachable via stdio subprocess or HTTP.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternalMcpServer {
    /// Stable identifier used in caches and telemetry (`server_id`).
    pub name: String,
    /// Stdio transport: executable to spawn (mutually exclusive with [`Self::url`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Arguments passed to [`Self::command`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Streamable HTTP transport endpoint (mutually exclusive with [`Self::command`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl ExternalMcpServer {
    /// Validate transport fields: exactly one of stdio (`command`) or HTTP (`url`).
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("external MCP server name must be non-empty");
        }
        match (&self.command, &self.url) {
            (Some(cmd), None) if !cmd.trim().is_empty() => Ok(()),
            (None, Some(url)) if !url.trim().is_empty() => Ok(()),
            (Some(_), Some(_)) => bail!(
                "external MCP server '{}' must specify either command or url, not both",
                self.name
            ),
            _ => bail!(
                "external MCP server '{}' requires a non-empty command or url",
                self.name
            ),
        }
    }

    fn client_info(&self) -> ClientInfo {
        ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("vox-external-mcp-client", env!("CARGO_PKG_VERSION"))
                .with_title(format!("vox external MCP client ({})", self.name)),
        )
    }
}

fn tool_to_definition(tool: &Tool) -> ToolDefinition {
    ToolDefinition {
        name: tool.name.to_string(),
        description: tool.description.as_deref().unwrap_or_default().to_string(),
        input_schema: serde_json::Value::Object((*tool.input_schema).clone()),
    }
}

async fn connect(config: &ExternalMcpServer) -> Result<RunningService<RoleClient, ClientInfo>> {
    config.validate()?;

    let client = config.client_info();

    if let Some(command) = &config.command {
        use rmcp::transport::TokioChildProcess;
        use tokio::process::Command as TokioCommand;

        let mut cmd = TokioCommand::new(command);
        cmd.args(&config.args);
        let transport = TokioChildProcess::new(cmd)
            .with_context(|| format!("spawn external MCP server '{}'", config.name))?;
        client
            .serve(transport)
            .await
            .map_err(|e| anyhow!("initialize external MCP server '{}': {e}", config.name))
    } else {
        use rmcp::transport::StreamableHttpClientTransport;

        let url = config
            .url
            .as_deref()
            .expect("validate() ensures url is present when command is absent");
        let transport = StreamableHttpClientTransport::from_uri(url);
        client
            .serve(transport)
            .await
            .map_err(|e| anyhow!("initialize external MCP server '{}': {e}", config.name))
    }
}

/// Connect to `config`, list all tools, then tear down the session.
pub async fn connect_and_list_tools(config: &ExternalMcpServer) -> Result<Vec<ToolDefinition>> {
    let mut session = connect(config).await?;
    let tools = session
        .peer()
        .list_all_tools()
        .await
        .map_err(map_service_error)?;
    session.close().await.ok();
    Ok(tools.iter().map(tool_to_definition).collect())
}

fn map_service_error(err: ServiceError) -> anyhow::Error {
    anyhow!("external MCP tool listing failed: {err}")
}

#[cfg(test)]
mod tests {
    use super::ExternalMcpServer;

    #[test]
    fn parse_stdio_config_from_json() {
        // /tmp is a test fixture arg — not portable runtime code.
        // vox-arch-check: allow abs-path
        let raw = r#"{ "name": "filesystem", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"] }"#;
        let cfg: ExternalMcpServer = serde_json::from_str(raw).expect("parse");
        assert_eq!(cfg.name, "filesystem");
        assert_eq!(cfg.command.as_deref(), Some("npx"));
        assert_eq!(cfg.args.len(), 3);
        assert!(cfg.url.is_none());
        cfg.validate().expect("valid stdio config");
    }

    #[test]
    fn parse_http_config_from_json() {
        let raw = r#"{
            "name": "remote",
            "url": "http://127.0.0.1:8000/mcp"
        }"#;
        let cfg: ExternalMcpServer = serde_json::from_str(raw).expect("parse");
        assert_eq!(cfg.name, "remote");
        assert!(cfg.command.is_none());
        assert_eq!(cfg.url.as_deref(), Some("http://127.0.0.1:8000/mcp"));
        cfg.validate().expect("valid http config");
    }

    #[test]
    fn reject_empty_name_and_dual_transport() {
        let bad_name = ExternalMcpServer {
            name: "  ".into(),
            command: Some("vox".into()),
            args: vec![],
            url: None,
        };
        assert!(bad_name.validate().is_err());

        let dual = ExternalMcpServer {
            name: "both".into(),
            command: Some("vox".into()),
            args: vec![],
            url: Some("http://localhost/mcp".into()),
        };
        assert!(dual.validate().is_err());
    }
}
