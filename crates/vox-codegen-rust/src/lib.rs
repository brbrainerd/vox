pub mod emit;

use vox_hir::HirModule;
use std::collections::HashMap;

pub struct CodegenOutput {
    pub files: HashMap<String, String>,
    /// TypeScript API client for server functions (empty if no server fns)
    pub api_client_ts: String,
}

pub fn generate(module: &HirModule, package_name: &str) -> Result<CodegenOutput, miette::Error> {
    let mut files = HashMap::new();

    // Cargo.toml
    files.insert("Cargo.toml".to_string(), emit::emit_cargo_toml(package_name));

    // src/main.rs (Entry point + Routes)
    files.insert("src/main.rs".to_string(), emit::emit_main(module, package_name));

    // src/lib.rs (Types, Actors, Workflows, Functions)
    files.insert("src/lib.rs".to_string(), emit::emit_lib(module));

    // TypeScript API client
    let api_client_ts = emit::emit_api_client(module);

    // MCP server (if @mcp.tool declarations are present)
    if !module.mcp_tools.is_empty() {
        files.insert(
            "src/mcp_server.rs".to_string(),
            emit::emit_mcp_server(module, package_name),
        );
    }

    Ok(CodegenOutput { files, api_client_ts })
}
