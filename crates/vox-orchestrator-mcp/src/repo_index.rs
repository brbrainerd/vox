//! Lightweight repository file index for MCP (path counts + optional JSON cache).

use serde::Serialize;
use std::fs;
use walkdir::WalkDir;

use crate::params::ToolResult;
use crate::server_state::ServerState;

const MAX_FILES_WALKED: usize = 50_000;
const REM_REPO_INDEX: &str = "Confirm `VOX_REPOSITORY_ROOT` / MCP workspace binding points at a readable directory with expected permissions.";
const REM_REPO_INDEX_CACHE: &str =
    "Ensure `.vox/cache` (or repo tooling cache dir) is writable and not locked by another tool.";

/// Bounded repository walk statistics for MCP clients and on-disk cache files.
#[derive(Debug, Serialize)]
pub struct RepoIndexSummary {
    /// Stable blake3-based id from `vox_repository`.
    pub repository_id: String,
    /// Absolute/resolved root path string.
    pub root: String,
    /// Files visited (excluding skipped dirs).
    pub files_scanned: usize,
    /// True when the walker stopped after scanning 50,000 files (hard cap in this module).
    pub stopped_at_cap: bool,
    /// Top file extensions by count (trimmed list).
    pub by_extension_top: Vec<(String, usize)>,
    /// Number of SKILL.md files discovered.
    pub skills_discovered: usize,
    /// Number of workflows discovered (`workflows/**/*.md` or `*.yaml`).
    pub workflows_discovered: usize,
    /// Number of files under test-like paths (`tests/`, `*_test.*`, `test_*.*`).
    pub tests_discovered: usize,
    /// Number of files under `contracts/`.
    pub contracts_discovered: usize,
    /// Number of markdown/docs anchors under `docs/`.
    pub docs_discovered: usize,
}

fn index_cache_path(state: &ServerState) -> std::path::PathBuf {
    vox_config::repo_tooling_cache_dir(&state.repository.root, &state.repository.repository_id)
        .join("repo_index.json")
}

fn build_summary(state: &ServerState) -> Result<RepoIndexSummary, String> {
    let root = &state.repository.root;
    if !root.is_dir() {
        return Err(format!(
            "repository root is not a directory: {}",
            root.display()
        ));
    }

    let mut files_scanned = 0usize;
    let mut ext_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut stopped = false;
    let mut skills_discovered = 0usize;
    let mut workflows_discovered = 0usize;
    let mut tests_discovered = 0usize;
    let mut contracts_discovered = 0usize;
    let mut docs_discovered = 0usize;
    const SKIP_DIRS: &[&str] = &[
        ".git",
        "target",
        "node_modules",
        "dist",
        "build",
        ".venv",
        ".vox",
        ".next",
    ];

    for ent in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !SKIP_DIRS.contains(&name.as_ref())
        })
        .filter_map(Result::ok)
    {
        if files_scanned >= MAX_FILES_WALKED {
            stopped = true;
            break;
        }
        let p = ent.path();
        if p == root.as_path() {
            continue;
        }
        if ent.file_type().is_file() {
            files_scanned += 1;
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();

            let name_str = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name_str == "SKILL.md" {
                skills_discovered += 1;
            } else if p.components().any(|c| {
                let s = c.as_os_str().to_str().unwrap_or("");
                s == "workflows" || s == "_workflows" || s == ".agents" || s == "_agents"
            }) && (ext == "md" || ext == "yaml" || ext == "yml")
            {
                workflows_discovered += 1;
            }
            let path_str = p.to_string_lossy().replace('\\', "/");
            if path_str.contains("/tests/")
                || name_str.contains("_test.")
                || name_str.starts_with("test_")
            {
                tests_discovered += 1;
            }
            if path_str.contains("/contracts/") {
                contracts_discovered += 1;
            }
            if path_str.contains("/docs/") && (ext == "md" || ext == "mdx" || ext == "rst") {
                docs_discovered += 1;
            }

            let key = if ext.is_empty() {
                "(no ext)".to_string()
            } else {
                format!(".{ext}")
            };
            *ext_counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut pairs: Vec<(String, usize)> = ext_counts.into_iter().collect();
    pairs.sort_by_key(|p| std::cmp::Reverse(p.1));
    pairs.truncate(24);

    Ok(RepoIndexSummary {
        repository_id: state.repository.repository_id.clone(),
        root: root.display().to_string(),
        files_scanned,
        stopped_at_cap: stopped,
        by_extension_top: pairs,
        skills_discovered,
        workflows_discovered,
        tests_discovered,
        contracts_discovered,
        docs_discovered,
    })
}

/// Walk `state.repository.root` (bounded), return JSON summary of file counts.
pub async fn repo_index_status(state: &ServerState) -> String {
    let orch = &state.orchestrator;
    let fresh_check = {
        let handle = orch.context_handle();
        match crate::sync_poison::poison_rw_read(handle.read(), "orchestrator context") {
            Ok(ctx) => {
                if ctx.is_fresh("workspace_index_status", 30) {
                    let entry = ctx.get_entry("workspace_index_status");
                    entry.map(|e| e.value)
                } else {
                    None
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "repo_index_status: context poisoned");
                None
            }
        }
    };
    if let Some(cached) = fresh_check {
        return cached;
    }

    let summary = match build_summary(state) {
        Ok(s) => ToolResult::ok(s).to_json(),
        Err(e) => ToolResult::<RepoIndexSummary>::err_with_remediation(e, REM_REPO_INDEX).to_json(),
    };

    match crate::sync_poison::poison_rw_write(
        state.orchestrator.context_handle().write(),
        "orchestrator context",
    ) {
        Ok(ctx) => {
            ctx.set(
                vox_orchestrator::AgentId(0),
                "workspace_index_status",
                summary.clone(),
                30,
            );
        }
        Err(e) => tracing::warn!(error = %e, "repo_index_status: context poisoned on cache write"),
    }
    summary
}

/// Refresh on-disk cache under `.vox/cache/repos/<repository_id>/repo_index.json`.
pub async fn repo_index_refresh(state: &ServerState) -> String {
    let orch = &state.orchestrator;
    let fresh_check = {
        let handle = orch.context_handle();
        match crate::sync_poison::poison_rw_read(handle.read(), "orchestrator context") {
            Ok(ctx) => {
                if ctx.is_fresh("workspace_index_refresh", 30) {
                    let entry = ctx.get_entry("workspace_index_refresh");
                    entry.map(|e| e.value)
                } else {
                    None
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "repo_index_refresh: context poisoned");
                None
            }
        }
    };
    if let Some(cached) = fresh_check {
        return cached;
    }

    let summary = match build_summary(state) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(e, REM_REPO_INDEX).to_json();
        }
    };
    let path = index_cache_path(state);
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return ToolResult::<String>::err_with_remediation(
                format!("mkdir {}: {e}", parent.display()),
                REM_REPO_INDEX_CACHE,
            )
            .to_json();
        }
    }
    let text = match serde_json::to_string_pretty(&summary) {
        Ok(t) => t,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("serialize: {e}"),
                REM_REPO_INDEX_CACHE,
            )
            .to_json();
        }
    };
    let res = match fs::write(&path, &text) {
        Ok(()) => ToolResult::ok(format!("wrote {}", path.display())).to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("write {}: {e}", path.display()),
            REM_REPO_INDEX_CACHE,
        )
        .to_json(),
    };

    let ctx_handle = state.orchestrator.context_handle();
    vox_orchestrator::sync_lock::rw_write(&*ctx_handle).set(
        vox_orchestrator::AgentId(0),
        "workspace_index_refresh",
        res.clone(),
        30,
    );
    res
}
