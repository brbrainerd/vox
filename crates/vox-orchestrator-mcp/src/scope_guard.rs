//! Scope enforcement for write-capable MCP tools.
//!
//! When an agent has declared `.vox/agents/{agent_id}.md` with a `scope:` front-matter
//! block, write tool calls that reference paths outside that scope are rejected at the
//! admission layer — before any filesystem operation occurs.

use crate::server_state::ServerState;

/// Write-capable tools that accept a path argument and must respect agent scope.
pub(crate) const WRITE_TOOLS: &[&str] = &[
    "vox_write_file",
    "vox_patch_file",
    "vox_inline_edit_file",
    "vox_multi_replace",
    "vox_multi_replace_file",
];

/// Path argument key names for write tools (checked in order).
pub(crate) const PATH_ARG_KEYS: &[&str] = &["path", "file_path", "target_file"];

/// Returns `Some(rejection message)` when the tool call is outside declared scope;
/// `None` when the call is allowed.
pub fn check_scope(
    state: &ServerState,
    tool_name: &str,
    agent_id: Option<&str>,
    args: &serde_json::Value,
) -> Option<String> {
    if !WRITE_TOOLS.contains(&tool_name) {
        return None;
    }
    let agent_name = agent_id?;
    let scopes = vox_repository::load_agent_scopes(&state.repository.root, agent_name)?;
    let path = PATH_ARG_KEYS
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()))?;
    let norm = vox_repository::normalize_task_path(&state.repository.root, path);
    let allowed = scopes.iter().any(|pat| {
        glob::Pattern::new(pat)
            .ok()
            .map(|p: glob::Pattern| p.matches(&norm))
            .unwrap_or(true)
    });
    if allowed {
        return None;
    }
    Some(format!(
        "SCOPE_VIOLATION: Path '{path}' is outside the declared scope for agent '{agent_name}'. \
         Allowed patterns: {scopes:?}. \
         Expand scope in `.vox/agents/{agent_name}.md` or use a path within the declared scope.",
    ))
}
