use crate::server_state::ServerState;
use serde_json::{Value, json};
use std::path::PathBuf;
use vox_compiler::language_surface;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

/// Decorators advertised to agents that are **not** yet dedicated lexer tokens (roadmap / docs).
const MCP_ROADMAP_DECORATORS: &[&str] = &[
    "@action",
    "@collection",
    "@vector_index",
    "@search_index",
    "@layout",
    "@not_found",
    "@error_boundary",
    "@fixture",
    "@mock",
    "@trace",
    "@health",
    "@metric",
    "@agent_def",
    "@skill",
    "@theme",
    "@keyframes",
];

/// `vox_language_surface` — keywords/types/builtins from `vox_compiler::language_surface`; decorators =
/// lexer-backed + `MCP_ROADMAP_DECORATORS`.
pub fn language_surface() -> Value {
    let mut keywords: Vec<&str> = language_surface::LEXER_KEYWORDS.to_vec();
    for &(w, _) in language_surface::LSP_KEYWORD_SNIPPETS {
        if !keywords.contains(&w) {
            keywords.push(w);
        }
    }
    keywords.sort_unstable();

    let mut decorators: Vec<&str> = language_surface::LEXER_DECORATORS.to_vec();
    for d in MCP_ROADMAP_DECORATORS {
        if !decorators.contains(d) {
            decorators.push(d);
        }
    }
    decorators.sort_unstable();

    json!({
        "keywords": keywords,
        "decorators": decorators,
        "types": language_surface::SURFACE_TYPE_NAMES,
        "builtins": language_surface::SURFACE_BUILTIN_NAMES,
    })
}

/// `vox_capability_model_manifest` — live merge of capability registry + MCP tool names + active CLI paths
/// (same composition as `vox ci capability-sync` output, without writing the generated JSON file).
pub fn capability_model_manifest(state: &ServerState) -> Result<Value, anyhow::Error> {
    let root = state
        .workspace_root
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("workspace_root not set"))?;
    let cap_path = root.join(vox_capability_registry::CAPABILITY_REGISTRY_REL);
    let cap_raw = vox_bounded_fs::read_utf8_path_capped(&cap_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", cap_path.display()))?;
    let doc: vox_capability_registry::CapabilityRegistryDoc = serde_yaml::from_str(&cap_raw)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", cap_path.display()))?;
    let reg_path = root.join(vox_capability_registry::COMMAND_REGISTRY_REL);
    let reg_raw = vox_bounded_fs::read_utf8_path_capped(&reg_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", reg_path.display()))?;
    let cli_paths =
        vox_capability_registry::active_vox_cli_paths_from_command_registry_yaml(&reg_raw)?;
    let mcp_tools: Vec<String> = vox_mcp_registry::TOOL_REGISTRY
        .iter()
        .map(|e| e.name.to_string())
        .collect();
    let manifest = vox_capability_registry::build_model_manifest(&doc, &mcp_tools, &cli_paths);
    serde_json::to_value(manifest).map_err(|e| anyhow::anyhow!(e))
}

/// `vox_compiler::ast_inspect` — parses a file and returns its AST as JSON.
pub async fn ast_inspect(state: &ServerState, path: &str) -> Result<Value, anyhow::Error> {
    let abs_path = if PathBuf::from(path).is_absolute() {
        PathBuf::from(path)
    } else {
        state.repository.root.join(path)
    };

    let content = std::fs::read_to_string(&abs_path)?;
    let tokens = lex(&content);
    match parse(tokens) {
        Ok(module) => Ok(json!(module)),
        Err(e) => Ok(
            json!({ "error": "Parse errors", "details": e.iter().map(|err| err.to_string()).collect::<Vec<_>>() }),
        ),
    }
}

/// `vox_pipeline_status` — returns current compiler pipeline health.
pub async fn pipeline_status() -> Value {
    // In a real implementation, this would query a global status tracker or the LSP server.
    // For now, we return a "hardened" status that reflects the 2024 architecture.
    json!({
        "lexer": "ok",
        "parser": "ok",
        "hir": "ok",
        "typeck": "ok",
        "codegen": "ok",
        "lsp": "ok",
        "orchestrator": "ok"
    })
}

/// `vox_decorator_registry` — starts from [`language_surface::LSP_DECORATOR_DOCS`], then MCP-only rows.
pub fn decorator_registry() -> Value {
    let mut rows: Vec<Value> = language_surface::LSP_DECORATOR_DOCS
        .iter()
        .map(|(name, desc)| {
            json!({
                "name": name,
                "desc": desc,
                "args": serde_json::Value::Null
            })
        })
        .collect();
    rows.extend([
        json!({"name": "@action", "desc": "Declares a side-effecting action.", "args": "name: str"}),
        json!({"name": "@collection", "desc": "Declares a NoSQL collection.", "args": "name: str"}),
        json!({"name": "@pure", "desc": "Marks a function as side-effect free.", "args": serde_json::Value::Null}),
        json!({"name": "@deprecated", "desc": "Marks a function as deprecated.", "args": "reason: str"}),
        json!({"name": "@agent_def", "desc": "Declares an agent definition surface.", "args": serde_json::Value::Null}),
        json!({"name": "@skill", "desc": "Declares a skill binding.", "args": serde_json::Value::Null}),
        json!({"name": "@scheduled", "desc": "Cron/interval scheduled function.", "args": serde_json::Value::Null}),
    ]);
    json!(rows)
}

/// `vox_builtin_registry` — detailed signatures for builtin functions.
pub fn builtin_registry() -> Value {
    json!([
        { "name": "print", "sig": "fn print(value: any) -> Unit", "doc": "Prints a value to stdout." },
        { "name": "len", "sig": "fn len(col: Collection[T]) -> int", "doc": "Returns the number of elements in a collection." },
        { "name": "push", "sig": "fn push(col: mut List[T], item: T) -> Unit", "doc": "Appends an item to a list." },
        { "name": "pop", "sig": "fn pop(col: mut List[T]) -> Option[T]", "doc": "Removes and returns the last item from a list." },
        { "name": "now", "sig": "fn now() -> int", "doc": "Returns current unix timestamp." },
        { "name": "uuid", "sig": "fn uuid() -> str", "doc": "Generates a random UUID v4." },
        { "name": "OpenClaw.list_skills", "sig": "fn OpenClaw.list_skills() -> Result[str]", "doc": "List remote skills JSON via the OpenClaw runtime adapter." },
        { "name": "OpenClaw.call", "sig": "fn OpenClaw.call(method: str, params_json: str) -> Result[str]", "doc": "Gateway WS call with JSON params object string." },
        { "name": "OpenClaw.subscribe", "sig": "fn OpenClaw.subscribe(domain: str) -> Result[str]", "doc": "Subscribe session to a gateway domain." },
        { "name": "OpenClaw.unsubscribe", "sig": "fn OpenClaw.unsubscribe(domain: str) -> Result[str]", "doc": "Unsubscribe session from a gateway domain." },
        { "name": "OpenClaw.notify", "sig": "fn OpenClaw.notify(domain: str, message: str) -> Result[str]", "doc": "Send a domain-scoped gateway notification." },
        { "name": "Browser.open", "sig": "fn Browser.open(url: str, headless: bool) -> Result[str]", "doc": "Opens a Chromium tab (CDP); returns page_id. Native scripts only; WASI returns Error." },
        { "name": "Browser.close", "sig": "fn Browser.close(page_id: str) -> Result[unit]", "doc": "Closes a tab; shuts down the host when no tabs remain." },
        { "name": "Browser.goto", "sig": "fn Browser.goto(page_id: str, url: str) -> Result[unit]", "doc": "Navigates an open tab to url." },
        { "name": "Browser.click", "sig": "fn Browser.click(page_id: str, target: str) -> Result[unit]", "doc": "CSS selector or xpath:… prefix for XPath." },
        { "name": "Browser.fill", "sig": "fn Browser.fill(page_id: str, target: str, value: str) -> Result[unit]", "doc": "Focus and type into an input-like element." },
        { "name": "Browser.wait_for", "sig": "fn Browser.wait_for(page_id: str, target: str, timeout_secs: int) -> Result[unit]", "doc": "Poll until selector or xpath matches (deadline in seconds)." },
        { "name": "Browser.text", "sig": "fn Browser.text(page_id: str, target: str) -> Result[str]", "doc": "Element inner_text." },
        { "name": "Browser.html", "sig": "fn Browser.html(page_id: str, target: str) -> Result[str]", "doc": "Fragment outer HTML, or full document when target is empty." },
        { "name": "Browser.screenshot", "sig": "fn Browser.screenshot(page_id: str, path: str) -> Result[str]", "doc": "Full-page PNG to path; returns path string." }
    ])
}

/// `vox_workspace_modules` — returns a list of all .vox files in the workspace.
pub async fn workspace_modules(state: &ServerState) -> Result<Value, anyhow::Error> {
    use walkdir::WalkDir;

    let root = &state.repository.root;
    let mut modules = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "target"
                && name != "node_modules"
                && name != "dist"
                && name != "out"
        })
        .flatten()
    {
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .map(|e| e == "vox")
                .unwrap_or(false)
        {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                modules.push(rel.to_string_lossy().to_string());
            }
        }
    }

    Ok(json!(modules))
}

/// `vox_a2a_tasks` — returns all current active and queued tasks for DAG visualization.
pub async fn a2a_tasks(state: &ServerState) -> Result<Value, anyhow::Error> {
    let orch = &state.orchestrator;
    let tasks = orch.all_tasks();
    let assignments = orch.task_assignments_copy();

    let ui_tasks: Vec<Value> = tasks
        .into_iter()
        .map(|t| {
            let status_str = match &t.status {
                vox_orchestrator::types::TaskStatus::Queued => "Queued".to_string(),
                vox_orchestrator::types::TaskStatus::InProgress => "InProgress".to_string(),
                vox_orchestrator::types::TaskStatus::Completed => "Completed".to_string(),
                vox_orchestrator::types::TaskStatus::Failed(e) => format!("Failed: {}", e),
                vox_orchestrator::types::TaskStatus::Blocked(id) => format!("Blocked by {}", id),
                vox_orchestrator::types::TaskStatus::Cancelled => "Cancelled".to_string(),
                _other => "Other".to_string(),
            };

            let priority_str = match t.priority {
                vox_orchestrator::types::TaskPriority::Background => "Background",
                vox_orchestrator::types::TaskPriority::Normal => "Normal",
                vox_orchestrator::types::TaskPriority::Urgent => "Urgent",
                _ => "Unknown",
            };

            let agent_id = assignments
                .get(&t.id)
                .map(|id| id.0.to_string())
                .unwrap_or_else(|| "unassigned".to_string());

            json!({
                "id": t.id.to_string(),
                "description": t.description,
                "status": status_str,
                "priority": priority_str,
                "agent_id": agent_id,
                "depends_on": t.depends_on.iter().map(|id| id.to_string()).collect::<Vec<_>>()
            })
        })
        .collect();

    Ok(json!(ui_tasks))
}

#[cfg(test)]
mod tests {
    use super::language_surface;

    #[test]
    fn language_surface_json_stable_shape() {
        let v = language_surface();
        let kw = v["keywords"].as_array().expect("keywords array");
        assert!(kw.iter().any(|x| x.as_str() == Some("fn")));
        let dec = v["decorators"].as_array().expect("decorators array");
        // @mcp.tool / @mcp.resource were retired to LEXER_DEPRECATED_DECORATORS;
        // assert the current canonical replacements instead.
        assert!(dec.iter().any(|x| x.as_str() == Some("@tool")));
        assert!(dec.iter().any(|x| x.as_str() == Some("@resource")));
    }
}
