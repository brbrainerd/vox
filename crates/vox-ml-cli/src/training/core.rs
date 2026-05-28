//! File walking, construct extraction, and JSONL training records.

use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::taxonomy::construct_difficulty;

/// Schema version — must match `learn.rs` and `dogfood_train.py`.
pub const SCHEMA_VERSION: &str = "vox_dogfood_v1";

/// Walk a directory recursively and collect all `.vox` files.
pub fn walk_vox_files(dir: &Path) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ex| ex == "vox"))
        .map(|e| e.path().to_path_buf())
        .collect();
    result.sort();
    result
}

/// UTC timestamp for run IDs and logs (SSOT: [`vox_corpus::training::timestamp_string`]).
pub fn timestamp_string() -> String {
    vox_corpus::training::timestamp_string()
}

/// Extract construct tags from AST declarations for training data categorization.
pub fn extract_constructs(module: &vox_compiler::ast::decl::Module) -> Vec<String> {
    use vox_compiler::ast::decl::Decl;
    let mut constructs = Vec::new();
    for decl in &module.declarations {
        let tag = match decl {
            Decl::Function(_) => "function",
            Decl::TypeDef(_) => "type",
            Decl::Import(_) => "import",
            Decl::Const(_) => "const",
            Decl::HttpRoute(_) => "http_route",
            Decl::McpTool(_) => "mcp_tool",
            Decl::McpResource(_) => "mcp_resource",
            Decl::Test(_) => "test",
            Decl::Forall(_) => "forall",
            Decl::Table(_) => "table",
            Decl::Collection(_) => "collection",
            Decl::Index(_) => "index",
            Decl::VectorIndex(_) => "vector_index",
            Decl::SearchIndex(_) => "search_index",
            Decl::V0Component(_) => "v0_component",
            Decl::Routes(_) => "routes",

            Decl::Skill(_) => "skill",
            Decl::AgentDef(_) => "agent_def",
            Decl::Agent(_) => "agent",
            Decl::Message(_) => "message",
            Decl::Scheduled(_) => "scheduled",
            Decl::Config(_) => "config",
            Decl::Loading(_) => "loading",
            Decl::Theme(_) => "theme",
            Decl::Environment(_) => "environment",
            Decl::Page(_) => "page",
            Decl::ReactiveComponent(_) => "reactive_component",
            Decl::ReactiveModule(_) => "reactive_module",
            Decl::Fragment(_) => "fragment",
            Decl::Endpoint(_) => "endpoint",
            Decl::Url(_) => "url",
            Decl::StateMachine(_) => "state_machine",
            Decl::Workflow(_) => "workflow",
            Decl::Activity(_) => "activity",
            Decl::Actor(_) => "actor",
            Decl::Form(_) => "form",
            Decl::BackButton(_) => "back_button",
            Decl::DeepLink(_) => "deep_link",
            Decl::Push(_) => "push",
            Decl::Tokens(_) => "tokens",
        };
        constructs.push(tag.to_string());
    }
    constructs.sort();
    constructs.dedup();
    constructs
}

/// Build a training JSONL record from a successful frontend result.
pub fn build_training_record(
    file: &Path,
    result: &vox_compiler::pipeline::FrontendResult,
) -> Result<serde_json::Value> {
    let content_hash = vox_actor_runtime::builtins::vox_hash_fast(&result.source);

    let constructs = extract_constructs(&result.module);
    let difficulty = constructs
        .iter()
        .map(|c| construct_difficulty(c))
        .max()
        .unwrap_or(5);

    let record = serde_json::json!({
        "source": file.to_string_lossy(),
        "code": result.source,
        "constructs": constructs,
        "difficulty": difficulty,
        "ast_hash": content_hash,
        "compiler_version": env!("CARGO_PKG_VERSION"),
    });

    Ok(record)
}

/// Append a JSONL record to the output file (creating it if necessary).
pub fn append_jsonl(
    output_path: &Path,
    file: &Path,
    result: &vox_compiler::pipeline::FrontendResult,
) -> Result<()> {
    let record = build_training_record(file, result)?;
    let line = serde_json::to_string(&record)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_path)?;
    writeln!(f, "{}", line)?;

    Ok(())
}

pub async fn run_frontend(file: &Path) -> Result<vox_compiler::pipeline::FrontendResult> {
    let source = tokio::fs::read_to_string(file).await?;
    let res = vox_compiler::pipeline::run_frontend_str(&source, &file.to_string_lossy())
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(res)
}

pub fn has_errors(res: &vox_compiler::pipeline::FrontendResult) -> bool {
    res.diagnostics
        .iter()
        .any(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
}
