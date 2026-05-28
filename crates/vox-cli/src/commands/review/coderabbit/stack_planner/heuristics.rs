//! Path ignore rules and coarse semantic bucket assignment for full-repo stack scans.

use super::super::path_policy;

/// Determines if a file should be hidden from CodeRabbit entirely.
pub(crate) fn is_ignored(path: &str) -> bool {
    if path_policy::is_coderabbit_local_tool_path(path) {
        return true;
    }
    let ignored_dirs = [
        ".git",
        ".github",
        "target",
        "target-toestub", // TOESTUB build artifacts
        "node_modules",
        "dist",
        ".next",
        "tmp",
        ".gemini",
        ".cursor",
        ".vox-research-data", // local SQLite cache
    ];
    // Separately: generated mdBook HTML (in .gitignore but sometimes still tracked)
    let ignored_prefixes = ["docs/book/"];
    let ignored_exts = [
        ".png", ".jpg", ".jpeg", ".webp", ".ico", ".dll", ".exe", ".so", ".dylib", ".bin", ".lock",
        ".svg", ".db", ".db-wal", ".db-shm",
    ];
    let ignored_files = [
        "Cargo.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
    ];

    let path_norm = path.replace('\\', "/");
    let path_lower = path_norm.to_lowercase();
    let path_parts: Vec<&str> = path_lower.split('/').collect();

    if ignored_dirs.iter().any(|d| path_parts.contains(d)) {
        return true;
    }
    if ignored_prefixes.iter().any(|p| path_norm.starts_with(p)) {
        return true;
    }
    if ignored_exts.iter().any(|e| path_lower.ends_with(e)) {
        return true;
    }
    let filename = path_norm.split('/').filter(|s| !s.is_empty()).last();
    if let Some(filename) = filename
        && ignored_files.contains(&filename)
    {
        return true;
    }

    false
}

/// Maps a file to its semantic layer and chunk order (used for all-file stack scans).
///
/// For changed-only reviews use [`super::super::semantic_planner::SemanticPlanner`] instead.
pub(crate) fn get_chunk_id(path: &str) -> (u32, &'static str) {
    let p = path.replace('\\', "/");

    // ── Scaffolding / project metadata ───────────────────────────────────
    if p.contains("Cargo.toml") || p.contains("package.json") {
        return (5, "01_scaffold_manifests");
    }
    if p.ends_with(".md") || p.ends_with(".toml") && !p.contains("crates/") {
        return (8, "01_scaffold_docs");
    }

    // ── CI / agents ──────────────────────────────────────────────────────
    if p.contains(".github/") || p.contains(".agents/") {
        return (10, "02_github_agents");
    }

    // ── Dotfiles / config ────────────────────────────────────────────────
    if p.contains(".cargo/") || p.contains(".config/") {
        return (12, "03_dotfiles_config");
    }

    // ── Docs ─────────────────────────────────────────────────────────────
    if p.contains("docs/src/") {
        return (15, "04_docs_src");
    }
    if p.contains("docs/") {
        return (17, "04_docs_other");
    }

    // ── Frontend / UI ────────────────────────────────────────────────────
    if p.contains("frontend/") || p.contains("packages/") {
        return (20, "05_frontend");
    }
    if p.contains("apps/editor/vox-vscode/") {
        return (22, "05_vscode_ext");
    }

    // ── Examples ─────────────────────────────────────────────────────────
    if p.contains("examples/") {
        return (25, "06_examples");
    }

    // ── ML / Mens ──────────────────────────────────────────────────────
    // candle_model_qwen.rs deleted (SP3 Unit 1); canonical copy in vox-plugin-mens-candle-cuda
    if p.starts_with("crates/vox-populi/src/mens/tensor/lora.rs")
        || p.starts_with("crates/vox-plugin-mens-candle-cuda/src/model.rs")
    {
        return (30, "07_populi_lora_model");
    }
    if p.starts_with("crates/vox-populi/src/mens/tensor/") {
        return (32, "07_populi_tensor");
    }
    if p.starts_with("crates/vox-populi/src/mens/") || p.starts_with("mens/") {
        return (35, "07_populi_ml");
    }

    // ── Scripts / xtask ──────────────────────────────────────────────────
    if p.starts_with("scripts/")
        || p.starts_with("xtask/")
        || p.starts_with("fuzz/")
        || p.starts_with("infra/")
        || p.starts_with("tree-sitter-vox/")
    {
        return (38, "08_scripts_xtask");
    }

    // ── Tests / fixtures ─────────────────────────────────────────────────
    if p.contains("tests/") {
        return (40, "09_tests");
    }

    // ── Compiler (monolith) ────────────────────────────────────────────────
    if p.contains("crates/vox-compiler/src/parser") || p.contains("crates/vox-compiler/src/lexer") {
        return (50, "10_compiler_parser_lexer");
    }
    if p.contains("crates/vox-compiler/src/ast") {
        return (52, "10_compiler_ast");
    }
    if p.contains("crates/vox-compiler/src/hir") {
        return (54, "10_compiler_hir");
    }
    if p.contains("crates/vox-compiler/src/typeck") {
        return (56, "10_compiler_typeck");
    }

    // ── Code generation ──────────────────────────────────────────────────
    if p.contains("crates/vox-codegen") {
        return (60, "11_crate_codegen");
    }

    // ── Runtime / LSP ────────────────────────────────────────────────────
    if p.contains("crates/vox-actor-runtime/")
        || p.contains("crates/vox-lsp/")
        || p.contains("crates/vox-dap/")
        || p.contains("crates/vox-fmt/")
        || p.contains("crates/vox-code-audit/")
        || p.contains("crates/vox-doc-pipeline/")
    {
        return (65, "12_crate_runtime_lsp");
    }

    // ── Agent / MCP ───────────────────────────────────────────────────────
    if p.contains("crates/vox-orchestrator-mcp/")
        || p.contains("crates/vox-mcp-registry/")
        || p.contains("crates/vox-codex")
        || p.contains("crates/vox-capability")
    {
        return (70, "13_crate_mcp_dei");
    }

    // ── CLI ───────────────────────────────────────────────────────────────
    if p == "crates/vox-cli/src/main.rs"
        || p == "crates/vox-cli/src/lib.rs"
        || p == "crates/vox-cli/src/diagnostics.rs"
    {
        return (75, "14_crate_cli_core");
    }
    if p.starts_with("crates/vox-cli/src/training/") {
        return (77, "14_crate_cli_training");
    }
    if p.starts_with("crates/vox-cli/src/commands/") {
        return (79, "14_crate_cli_commands");
    }
    if p.starts_with("crates/vox-cli/") {
        return (80, "14_crate_cli_other");
    }

    // ── All other Rust crates ─────────────────────────────────────────────
    if p.ends_with(".rs") || p.contains("crates/") {
        return (85, "15_crate_other");
    }

    (99, "99_unassigned")
}
