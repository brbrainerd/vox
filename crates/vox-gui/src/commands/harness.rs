//! Harness panel helpers (diff review, MCP text extraction, repo file picker).

use std::sync::Arc;

use serde_json::Value;
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;
use vox_repository::discover_repository_or_fallback;

/// Extract a string payload from a daemon `orch.tool_call` JSON envelope.
pub fn extract_mcp_tool_string(result: &Value) -> Result<String, String> {
    if let Some(s) = result.as_str() {
        return Ok(s.to_string());
    }
    if result.get("success") == Some(&Value::Bool(true))
        && let Some(data) = result.get("data")
        && let Some(s) = data.as_str()
    {
        return Ok(s.to_string());
    }
    if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
        return Err(err.to_string());
    }
    Err("MCP tool returned no text payload".to_string())
}

/// Return staged/working-tree diff text via `vox_git_diff` (harness diff review).
#[tauri::command]
pub async fn get_task_diff(
    path: Option<String>,
    daemon: tauri::State<'_, Arc<crate::commands::daemon::PersistentDaemon>>,
) -> Result<String, String> {
    let addr = daemon.ensure().await?;
    let mut args = serde_json::Map::new();
    if let Some(p) = path.filter(|s| !s.trim().is_empty()) {
        args.insert("path".to_string(), Value::String(p));
    }
    let value = OrchDaemonClient::new(addr)
        .call(
            orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": "vox_git_diff", "args": Value::Object(args) }),
        )
        .await
        .map_err(|e| format!("vox_git_diff failed: {e}"))?;

    if value.get("success") == Some(&Value::Bool(false)) {
        return Err(value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("git diff failed")
            .to_string());
    }

    extract_mcp_tool_string(&value)
}

/// Filter tracked repo paths by substring match (case-insensitive), preferring prefix hits.
pub fn filter_repo_paths(paths: &[String], query: &str, limit: usize) -> Vec<String> {
    let q = query.trim();
    if limit == 0 {
        return Vec::new();
    }
    if q.is_empty() {
        return paths.iter().take(limit).cloned().collect();
    }
    let q_lower = q.to_lowercase();
    let mut matches: Vec<String> = paths
        .iter()
        .filter(|p| p.to_lowercase().contains(&q_lower))
        .cloned()
        .collect();
    matches.sort_by(|a, b| {
        let a_prefix = a.to_lowercase().starts_with(&q_lower);
        let b_prefix = b.to_lowercase().starts_with(&q_lower);
        b_prefix.cmp(&a_prefix).then_with(|| a.cmp(b))
    });
    matches.truncate(limit);
    matches
}

/// List tracked files under the discovered repo root (`git ls-files`), filtered by `query`.
#[tauri::command]
pub fn list_repo_files(query: Option<String>, limit: Option<usize>) -> Result<Vec<String>, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = discover_repository_or_fallback(&cwd);
    let repo_root = repo_ctx.root;

    let stdout = vox_git::read_only(&repo_root, &["ls-files"]).map_err(|e| e.to_string())?;

    let all: Vec<String> = stdout
        .lines()
        .map(|line| line.replace('\\', "/"))
        .filter(|line| !line.is_empty())
        .collect();

    let q = query.unwrap_or_default();
    let lim = limit.unwrap_or(50);
    Ok(filter_repo_paths(&all, &q, lim))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_repo_paths_empty_query_returns_first_n() {
        let paths = vec!["a.rs".to_string(), "b.rs".to_string(), "c.rs".to_string()];
        assert_eq!(
            filter_repo_paths(&paths, "", 2),
            vec!["a.rs".to_string(), "b.rs".to_string()]
        );
    }

    #[test]
    fn filter_repo_paths_substring_match_prefers_prefix() {
        let paths = vec![
            "crates/vox-gui/src/main.rs".to_string(),
            "docs/src/main.md".to_string(),
            "crates/vox-cli/src/main.rs".to_string(),
        ];
        let got = filter_repo_paths(&paths, "vox-gui", 10);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], "crates/vox-gui/src/main.rs");
    }

    #[test]
    fn filter_repo_paths_respects_limit() {
        let paths = vec!["a.rs".to_string(), "b.rs".to_string(), "c.rs".to_string()];
        assert_eq!(filter_repo_paths(&paths, ".rs", 1).len(), 1);
    }

    #[test]
    fn extract_mcp_tool_string_reads_tool_result_data() {
        let v = serde_json::json!({ "success": true, "data": "diff --git a/x" });
        assert_eq!(extract_mcp_tool_string(&v).unwrap(), "diff --git a/x");
    }

    #[test]
    fn extract_mcp_tool_string_surfaces_errors() {
        let v = serde_json::json!({ "success": false, "error": "git missing" });
        assert_eq!(extract_mcp_tool_string(&v).unwrap_err(), "git missing");
    }
}
