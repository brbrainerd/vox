//! `vox ci mcp-vox-surface-parity`
//!
//! Verifies workspace `@tool` / `@resource` declarations appear in the federated MCP surface,
//! carry JSON schemas, and round-trip through dispatch with fixture args.

use anyhow::{Context, Result};

#[derive(serde::Deserialize)]
struct FixturesFile {
    fixtures: Vec<FixtureRow>,
}

#[derive(serde::Deserialize)]
struct FixtureRow {
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    args: Option<serde_json::Value>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    expected_body: Option<String>,
}

pub fn run() -> Result<()> {
    let repo = crate::repo_root();
    let load = vox_orchestrator_mcp::workspace_mcp::WorkspaceMcpLoader::load_repo(
        &repo,
        &vox_orchestrator_mcp::workspace_mcp::load_scan_config(&repo),
    )
    .map_err(|e| anyhow::anyhow!("workspace MCP load: {e}"))?;
    let surface = &load.surface;

    let mut errors = Vec::new();
    if !load.errors.is_empty() {
        errors.push(format!(
            "workspace MCP scan reported {} file error(s)",
            load.errors.len()
        ));
    }

    for tool in &surface.tools {
        if tool.input_schema.get("type").and_then(|v| v.as_str()) != Some("object") {
            errors.push(format!("{}: input_schema missing type=object", tool.name));
        }
        if tool.description.is_empty() {
            errors.push(format!("{}: empty description", tool.name));
        }
    }

    for res in &surface.resources {
        if res.description.is_empty() {
            errors.push(format!("{}: empty resource description", res.uri));
        }
    }

    let fixtures_path = repo.join("contracts/mcp/workspace-tool-fixtures.v1.json");
    let fixtures: FixturesFile = serde_json::from_str(
        &std::fs::read_to_string(&fixtures_path)
            .with_context(|| format!("read {}", fixtures_path.display()))?,
    )?;

    let mut tool_fixtures = 0usize;
    let mut resource_fixtures = 0usize;

    for row in &fixtures.fixtures {
        if let Some(tool) = &row.tool {
            tool_fixtures += 1;
            let args = row.args.clone().unwrap_or(serde_json::json!({}));
            if surface.tool_by_name(tool).is_none() {
                errors.push(format!("fixture tool '{tool}' not in federated surface"));
                continue;
            }
            let resp =
                vox_orchestrator_mcp::workspace_mcp::dispatch_workspace_tool(surface, tool, &args);
            match resp {
                Ok(json) => {
                    let v: serde_json::Value = serde_json::from_str(&json)?;
                    if v.get("success").and_then(|s| s.as_bool()) != Some(true) {
                        errors.push(format!("fixture {tool} returned success!=true: {json}"));
                    }
                }
                Err(e) => errors.push(format!("fixture {tool} dispatch failed: {e}")),
            }
        }

        if let Some(uri) = &row.resource {
            resource_fixtures += 1;
            if surface.resource_by_uri(uri).is_none() {
                errors.push(format!("fixture resource '{uri}' not in federated surface"));
                continue;
            }
            match vox_orchestrator_mcp::workspace_mcp::dispatch_workspace_resource(surface, uri) {
                Ok(body) => {
                    if let Some(expected) = &row.expected_body
                        && body != *expected
                    {
                        errors.push(format!(
                            "fixture resource {uri} expected body {expected:?}, got {body:?}"
                        ));
                    }
                }
                Err(e) => errors.push(format!("fixture resource {uri} read failed: {e}")),
            }
        }
    }

    if errors.is_empty() {
        println!(
            "✓ mcp-vox-surface-parity ok ({} workspace tools, {} resources, {} tool fixtures, {} resource fixtures)",
            surface.tools.len(),
            surface.resources.len(),
            tool_fixtures,
            resource_fixtures,
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("mcp-vox-surface-parity: {e}");
        }
        anyhow::bail!(
            "mcp-vox-surface-parity failed with {} error(s)",
            errors.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_gate_passes_on_repo_fixtures() {
        run().expect("mcp-vox-surface-parity");
    }
}
