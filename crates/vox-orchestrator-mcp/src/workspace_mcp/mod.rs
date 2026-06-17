//! Federate workspace @tool / @resource declarations into vox-mcp (Option A).

mod dispatch;
mod loader;
mod schema;

pub use dispatch::{dispatch_workspace_resource, dispatch_workspace_tool};
pub use loader::{WorkspaceMcpLoader, WorkspaceMcpScanConfig, load_scan_config};
pub use schema::param_required_in_schema;

/// One federated workspace MCP tool discovered from HIR.
#[derive(Debug, Clone)]
pub struct WorkspaceMcpToolEntry {
    pub name: String,
    pub description: String,
    pub signature: String,
    pub source_path: std::path::PathBuf,
    pub repo_relative: String,
    pub input_schema: serde_json::Value,
}

/// One federated workspace MCP resource discovered from HIR.
#[derive(Debug, Clone)]
pub struct WorkspaceMcpResourceEntry {
    pub uri: String,
    pub description: String,
    pub source_path: std::path::PathBuf,
    pub repo_relative: String,
    pub func_name: String,
}

/// Merged workspace MCP surface loaded at bind / refresh time.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceMcpSurface {
    pub tools: Vec<WorkspaceMcpToolEntry>,
    pub resources: Vec<WorkspaceMcpResourceEntry>,
    /// Workspace tool names shadowed by static catalog entries.
    pub shadowed: Vec<String>,
    /// Tool names seen more than once across scan globs (first wins).
    pub duplicate_tools: Vec<String>,
    /// Resource URIs seen more than once (first wins).
    pub duplicate_resources: Vec<String>,
}

impl WorkspaceMcpSurface {
    pub fn tool_by_name(&self, name: &str) -> Option<&WorkspaceMcpToolEntry> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn resource_by_uri(&self, uri: &str) -> Option<&WorkspaceMcpResourceEntry> {
        self.resources.iter().find(|r| r.uri == uri)
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }
}

/// Per-file compile failure during workspace MCP scan.
#[derive(Debug, Clone)]
pub struct WorkspaceMcpLoadError {
    pub path: std::path::PathBuf,
    pub message: String,
}

/// Result of a workspace MCP scan (partial success when `errors` is non-empty).
#[derive(Debug, Clone)]
pub struct WorkspaceMcpLoadResult {
    pub surface: WorkspaceMcpSurface,
    pub errors: Vec<WorkspaceMcpLoadError>,
}

impl Default for WorkspaceMcpLoadResult {
    fn default() -> Self {
        Self {
            surface: WorkspaceMcpSurface::default(),
            errors: Vec::new(),
        }
    }
}
