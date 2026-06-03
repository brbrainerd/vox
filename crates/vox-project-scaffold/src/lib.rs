//! Shared Vox project scaffolding for **`vox init`** and MCP **`vox_project_init`**.

use anyhow::{Context, Result};
use serde::Serialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

const CHATBOT_TEMPLATE: &str = r#"# Vox Chatbot — OpenRouter-powered chat app
#
# Edit OPENROUTER_API_KEY in .env to enable live LLM responses.
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Conversation {
    id: int
    user_id: str
    started_at: str
}

@table type MessageTrace {
    id: int
    conversation_id: str
    role: str
    content: str
    request_id: str
}

@query fn recent_messages(conversation_id: str) to list[MessageTrace] {
    return []
}

@mutation fn log_message(conversation_id: str, role: str, content: str, request_id: str) to Result[bool] {
    return Ok(true)
}

// `activity` = a side-effecting step recorded in the workflow journal so
// retries/replays don't re-execute it. `workflow` = a deterministic
// orchestration of activities; on crash/restart it replays from the journal.
// ADR-028 revision 2026-05-19 made both keywords public.
activity call_provider(prompt: str) to Result[str] {
    return Ok("stub-response: " + prompt)
}

workflow chat_pipeline(prompt: str) to Result[str] {
    return call_provider(prompt)
}

@server fn chat(prompt: str, request_id: str) to Result[str] {
    let response = chat_pipeline(prompt)
    let _ = log_message("conv-default", "user", prompt, request_id)
    let _ = log_message("conv-default", "assistant", "ok", request_id)
    return response
}

component Chat() {
    view: column(raw_class="chat-root") {
        heading(level=1) { "Chat" }
        text() { "Edit src/main.vox to wire your chat UI to chat(...)." }
    }
}

routes {
    "/" to Chat
}
"#;

const DASHBOARD_TEMPLATE: &str = r#"# Vox Dashboard — data table with route params
#
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Item {
    id: int
    name: str
    value: int
    created_at: str
}

@query fn list_items() to list[Item] {
    return []
}

@mutation fn add_item(name: str, value: int) to Result[str] {
    return Ok("Added: " + name)
}

component Dashboard() {
    view: column(raw_class="dashboard") {
        heading(level=1) { "Dashboard" }
        text() { "Edit src/main.vox to add data tables, queries, and mutations." }
    }
}

component ItemDetail(id: str) {
    view: column(raw_class="item-detail") {
        heading(level=2) { "Item" }
        text() { id }
    }
}

routes {
    "/" to Dashboard
    "/items/:id" to ItemDetail
}
"#;

const API_TEMPLATE: &str = r#"# Vox API — server functions with health + metrics
#
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Task {
    id: int
    title: str
    done: bool
    created_at: str
}

fn health_check() to bool {
    return true
}

fn tasks_created() to str {
    return "ok"
}

@server fn create_task(title: str) to Result[str] {
    return Ok("Created: " + title)
}

@server fn list_tasks() to Result[list[Task]] {
    return Ok([])
}

@server fn complete_task(id: str) to Result[bool] {
    return Ok(true)
}
"#;

const DEFAULT_FULL_STACK: &str = r#"# My Vox App — a full-stack starter
#
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Note {
    id: int
    title: str
    content: str
    created_at: str
}

@server fn add_note(title: str, content: str) to Result[str] {
    return Ok("Added: " + title)
}

@server fn list_notes() to Result[str] {
    return Ok("[]")
}

component App() {
    view: column(raw_class="app") {
        heading(level=1) { "My Vox App" }
        text() { "Edit src/main.vox to get started" }
    }
}

routes {
    "/" to App
}
"#;

const AGENT_KIND: &str = "# A Vox AI agent\n\n@agent_def fn MyAgent(query: str) -> str {\n    return \"Response to: {query}\"\n}\n";
const WORKFLOW_KIND: &str =
    "# A Vox Workflow\n\n@workflow_def fn DataPipeline() {\n    return\n}\n";

const MOBILE_PWA_TEMPLATE: &str = r#"# Vox Mobile — web-first UI + native packaging via Tauri 2 (see `vox compile` / application-packaging SSOT)

import std.mobile

@table type Photo {
    id: int
    url: str
    synced: bool
}

component App() {
    state photo: str = ""
    view: column(raw_class="app") {
        heading(level=1) { "Camera Test" }
        button(on_click={fn() {
            match mobile.take_photo() {
                Ok(uri) => { photo = uri }
                Error(msg) => { let _ = mobile.notify("Camera Error", msg) }
            }
        }}) {
            "Take Photo"
        }
        if len(photo) > 0 {
            image(src=photo, alt="captured")
        }
    }
}

routes {
    "/" to App
}
"#;

/// Render a Red-then-Green Vox fn stub paired with an `@test` block.
///
/// The output is intentionally non-runnable: the test references undefined
/// placeholders (`_expected`, `_` per param) so the user is *forced* to fill
/// in the expected behavior before the function compiles. This is the
/// friction reducer for AGENTS.md §Test-First Policy.
///
/// `name`: validated Vox identifier (alphanumeric + underscore, not starting with a digit).
/// `params`: optional, e.g. `"a: int, b: int"`. Empty means no parameters.
/// `returns`: optional return type, e.g. `"int"`. None means `Unit`.
pub fn render_fn_stub(name: &str, params: Option<&str>, returns: Option<&str>) -> Result<String> {
    if !is_valid_vox_identifier(name) {
        anyhow::bail!(
            "'{name}' is not a valid Vox identifier (alphanumeric + underscore, not starting with a digit)"
        );
    }
    let params_str = params
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let returns_str = returns
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unit");
    let example_args = call_args_for_params(params_str);

    Ok(format!(
        "\n@test\nfn test_{name}() to Unit {{\n    \
         // RED step: replace `_` and `_expected` with concrete values\n    \
         //            that capture the intended behavior of `{name}`.\n    \
         let result = {name}({example_args})\n    \
         assert(result is _expected)\n\
         }}\n\n\
         fn {name}({params_str}) to {returns_str} {{\n    \
         // GREEN step: implement until the @test above passes.\n\
         }}\n"
    ))
}

fn is_valid_vox_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some(c) if !(c.is_ascii_alphabetic() || c == '_') => false,
        _ => chars.all(|c| c.is_ascii_alphanumeric() || c == '_'),
    }
}

/// Convert a param list like `"a: int, b: str"` into a comma-separated `_, _`
/// for the test call site. Each placeholder triggers a compile-error nudge.
fn call_args_for_params(params: &str) -> String {
    let count = params
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .count();
    if count == 0 {
        String::new()
    } else {
        std::iter::repeat_n("_", count)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Append a rendered fn-stub to a target file (creating it if missing).
/// Returns the bytes written. Refuses to clobber a fn of the same name unless
/// `force` is true.
pub fn append_fn_stub(
    target: &Path,
    name: &str,
    params: Option<&str>,
    returns: Option<&str>,
    force: bool,
) -> Result<usize> {
    let stub = render_fn_stub(name, params, returns)?;

    let existing = if target.exists() {
        std::fs::read_to_string(target).with_context(|| format!("read {}", target.display()))?
    } else {
        String::new()
    };

    if !force && file_defines_fn(&existing, name) {
        anyhow::bail!(
            "fn `{name}` already defined in {}; pass --force to replace (not yet supported) or pick another name",
            target.display()
        );
    }

    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent of {}", target.display()))?;
    }

    let combined = if existing.is_empty() {
        stub.clone()
    } else if existing.ends_with('\n') {
        format!("{existing}{stub}")
    } else {
        format!("{existing}\n{stub}")
    };
    std::fs::write(target, &combined).with_context(|| format!("write {}", target.display()))?;
    Ok(stub.len())
}

fn file_defines_fn(content: &str, name: &str) -> bool {
    let needle = format!("fn {name}");
    content.lines().any(|line| {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(&needle) {
            return false;
        }
        let after = &trimmed[needle.len()..];
        matches!(after.chars().next(), Some('(') | Some(' ') | Some('\t'))
    })
}

/// Summary of files and directories created under the scaffold root.
#[derive(Debug, Clone, Serialize)]
pub struct ScaffoldSummary {
    pub package_kind: String,
    pub project_name: String,
    pub created_relative_paths: Vec<String>,
    /// Template key when `template` was honored (known keys only).
    pub template_applied: Option<String>,
}

/// Resolve target directory strictly under `workspace_repo_root` (empty / missing `target_subdir` → workspace root).
pub fn resolve_scaffold_target_under_repo(
    workspace_repo_root: &Path,
    target_subdir: Option<&str>,
) -> Result<PathBuf> {
    match target_subdir.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(workspace_repo_root.to_path_buf()),
        Some(rel) => vox_repository::resolve_strict_repo_relative_path(workspace_repo_root, rel)
            .map_err(|e| anyhow::anyhow!("{e}")),
    }
}

fn main_vox_content(package_kind: &str, template: Option<&str>) -> Cow<'static, str> {
    if let Some(tmpl) = template {
        return match tmpl {
            "chatbot" => Cow::Borrowed(CHATBOT_TEMPLATE),
            "web" | "dashboard" => Cow::Borrowed(DASHBOARD_TEMPLATE),
            "api" => Cow::Borrowed(API_TEMPLATE),
            "mobile-pwa" | "tauri-mobile" => Cow::Borrowed(MOBILE_PWA_TEMPLATE),
            other => Cow::Owned(format!(
                "# My Vox App\n\ncomponent App() {{\n    <div><h1>\"My Vox App\"</h1></div>\n}}\n\nroutes:\n    \"/\" to App\n\n# Unknown template '{other}'; use web, chatbot, dashboard, api, mobile-pwa, or tauri-mobile.\n"
            )),
        };
    }
    match package_kind {
        "agent" => Cow::Borrowed(AGENT_KIND),
        "workflow" => Cow::Borrowed(WORKFLOW_KIND),
        "chatbot" => Cow::Borrowed(CHATBOT_TEMPLATE),
        _ => Cow::Borrowed(DEFAULT_FULL_STACK),
    }
}

/// Scaffold under an **absolute** directory (e.g. CLI current dir). Creates the directory tree if needed.
pub fn scaffold_vox_project_at(
    root: &Path,
    project_name: &str,
    package_kind: &str,
    template: Option<&str>,
) -> Result<ScaffoldSummary> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("create scaffold root {}", root.display()))?;

    let mut created_relative_paths = Vec::new();

    if package_kind == "skill" {
        let skill_file_name = vox_repository::skill_markdown_filename(project_name);
        let target_path = root.join(&skill_file_name);
        if target_path.exists() {
            anyhow::bail!("{} already exists in this directory", skill_file_name);
        }
        let content = vox_repository::skill_markdown_for_project(project_name);
        std::fs::write(&target_path, content)
            .with_context(|| format!("Failed to write {}", skill_file_name))?;
        created_relative_paths.push(skill_file_name.replace('\\', "/"));
        return Ok(ScaffoldSummary {
            package_kind: package_kind.to_string(),
            project_name: project_name.to_string(),
            created_relative_paths,
            template_applied: None,
        });
    }

    let manifest_path = root.join("Vox.toml");
    if manifest_path.exists() {
        anyhow::bail!("Vox.toml already exists in this directory");
    }

    let manifest = vox_package_types::VoxManifest::scaffold(project_name, package_kind);
    let mut toml_content = manifest
        .to_toml_string()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if template == Some("web") {
        // Default to a portable OCI container build; uncomment the [deploy.fly] block
        // (or switch `target` to "coolify" / "bare-metal" / "compose" / "kubernetes")
        // to target a specific platform. See `vox deploy --help`.
        toml_content.push_str("\n[deploy]\ntarget = \"container\"   # \"container\" | \"compose\" | \"kubernetes\" | \"bare-metal\" | \"fly\" | \"coolify\"\nruntime = \"auto\"\n");
        toml_content.push_str("\n# [deploy.fly]\n# app_name = \"my-app\"\n# region = \"iad\"\n");
        toml_content.push_str("\n# [deploy.coolify]\n# base_url = \"https://coolify.example.com\"\n# app_uuid = \"...\"\n");
    }

    std::fs::write(&manifest_path, &toml_content).with_context(|| "Failed to write Vox.toml")?;
    created_relative_paths.push("Vox.toml".to_string());

    let src_dir = root.join("src");
    if !src_dir.exists() {
        std::fs::create_dir_all(&src_dir).with_context(|| "Failed to create src/ directory")?;
    }

    let main_file = src_dir.join("main.vox");
    if !main_file.exists() {
        let main_content = main_vox_content(package_kind, template);
        std::fs::write(&main_file, main_content.as_ref())
            .with_context(|| "Failed to write src/main.vox")?;
        created_relative_paths.push("src/main.vox".to_string());
    }

    let modules_dir = root.join(".vox_modules");
    if !modules_dir.exists() {
        std::fs::create_dir_all(&modules_dir).with_context(|| "Failed to create .vox_modules/")?;
        created_relative_paths.push(".vox_modules".to_string());
    }

    let template_applied = template.map(str::to_string);

    Ok(ScaffoldSummary {
        package_kind: package_kind.to_string(),
        project_name: project_name.to_string(),
        created_relative_paths,
        template_applied,
    })
}

/// Scaffold under `workspace_repo_root` or a strict repo-relative subdirectory (for MCP).
pub fn scaffold_vox_project_under_repo(
    workspace_repo_root: &Path,
    target_subdir: Option<&str>,
    project_name: &str,
    package_kind: &str,
    template: Option<&str>,
) -> Result<ScaffoldSummary> {
    let root = resolve_scaffold_target_under_repo(workspace_repo_root, target_subdir)?;
    scaffold_vox_project_at(&root, project_name, package_kind, template)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn skill_scaffold_writes_file() {
        let d = TempDir::new().unwrap();
        let s = scaffold_vox_project_at(d.path(), "my-skill", "skill", None).unwrap();
        assert_eq!(s.created_relative_paths.len(), 1);
        assert!(d.path().join("my-skill.skill.md").is_file());
    }

    #[test]
    fn application_scaffold_creates_manifest_and_main() {
        let d = TempDir::new().unwrap();
        let s = scaffold_vox_project_at(d.path(), "app1", "application", None).unwrap();
        assert!(d.path().join("Vox.toml").is_file());
        assert!(d.path().join("src/main.vox").is_file());
        assert!(s.created_relative_paths.contains(&"Vox.toml".to_string()));
    }

    // --- render_fn_stub / append_fn_stub coverage ---

    #[test]
    fn fn_stub_no_params_no_returns_emits_unit_and_empty_call() {
        let s = render_fn_stub("greet", None, None).unwrap();
        assert!(s.contains("fn test_greet() to Unit {"), "test sig: {s}");
        assert!(s.contains("let result = greet()"), "empty-arg call: {s}");
        assert!(s.contains("fn greet() to Unit {"), "fn sig: {s}");
    }

    #[test]
    fn fn_stub_with_params_emits_underscore_per_arg() {
        let s = render_fn_stub("add", Some("a: int, b: int"), Some("int")).unwrap();
        assert!(s.contains("let result = add(_, _)"), "two underscores: {s}");
        assert!(
            s.contains("fn add(a: int, b: int) to int {"),
            "param echo: {s}"
        );
    }

    #[test]
    fn fn_stub_rejects_invalid_identifier() {
        assert!(render_fn_stub("123abc", None, None).is_err());
        assert!(render_fn_stub("", None, None).is_err());
        assert!(render_fn_stub("has space", None, None).is_err());
        assert!(render_fn_stub("kebab-case", None, None).is_err());
        assert!(render_fn_stub("_underscore_ok", None, None).is_ok());
        assert!(render_fn_stub("snake_case_ok", None, None).is_ok());
    }

    #[test]
    fn fn_stub_test_block_uses_assert_is() {
        let s = render_fn_stub("foo", None, None).unwrap();
        assert!(
            s.contains("assert(result is _expected)"),
            "canonical assert: {s}"
        );
    }

    #[test]
    fn append_fn_stub_creates_file_when_missing() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("subdir/new.vox");
        let n = append_fn_stub(&p, "first", None, None, false).unwrap();
        assert!(p.is_file());
        assert!(n > 0);
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("fn first()"), "stub written: {body}");
    }

    #[test]
    fn append_fn_stub_appends_without_clobbering_existing_content() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("existing.vox");
        std::fs::write(&p, "fn other() to Unit {\n    // pre-existing\n}\n").unwrap();
        append_fn_stub(&p, "added", None, None, false).unwrap();
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("fn other()"), "preserved original: {body}");
        assert!(body.contains("fn added()"), "appended new: {body}");
    }

    #[test]
    fn append_fn_stub_refuses_to_clobber_same_name() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("dup.vox");
        std::fs::write(&p, "fn dup() to int {\n    return 1\n}\n").unwrap();
        let err = append_fn_stub(&p, "dup", None, None, false).unwrap_err();
        assert!(err.to_string().contains("already defined"), "got: {err}");
    }

    #[test]
    fn append_fn_stub_inserts_blank_line_before_when_file_lacks_trailing_newline() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("noeol.vox");
        std::fs::write(&p, "fn other() to Unit {}").unwrap();
        append_fn_stub(&p, "added", None, None, false).unwrap();
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(
            body.contains("fn other() to Unit {}\n"),
            "newline inserted: {body:?}"
        );
    }

    #[test]
    fn file_defines_fn_does_not_match_substring() {
        // `fn add` should not match `fn adder`
        assert!(!file_defines_fn("fn adder() to int { return 0 }", "add"));
        assert!(file_defines_fn("fn add() to int { return 0 }", "add"));
        assert!(file_defines_fn(
            "    fn add(x: int) to int { return x }",
            "add"
        ));
    }
}
