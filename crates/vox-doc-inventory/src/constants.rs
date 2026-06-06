//! Static lists and description strings for doc inventory.

pub const HOTSPOT_TIER1: &[&str] = &[
    "AGENTS.md",
    "docs/src/api/DOC_GAPS.md",
    "docs/src/api/vox-ast.md",
    "crates/vox-compiler/src/ast/expr.rs",
    "crates/vox-cli/src/lib.rs",
    "crates/vox-compiler/src/hir/mod.rs",
    "crates/vox-orchestrator-mcp/src/memory_tools/mod.rs",
    "crates/vox-orchestrator/src/events.rs",
    "crates/vox-orchestrator/src/oplog.rs",
    "crates/vox-orchestrator/src/orchestrator.rs",
    "crates/vox-orchestrator/src/session.rs",
    "crates/vox-orchestrator/src/types.rs",
    "crates/vox-package/src/store/types.rs",
    "crates/vox-populi/src/mens/tensor/qlora_preflight.rs",
    "crates/vox-orchestrator-mcp/src/input_schemas.rs",
    "docs/src/ci/rust-modernization-baseline.md",
];

pub const HOTSPOT_TIER2_RUST: &[&str] = &[
    "crates/vox-populi/src/mens/tensor/lora.rs",
    "crates/vox-cli/src/commands/mens/mod.rs",
    "crates/vox-orchestrator/src/memory.rs",
    "crates/vox-db/src/lib.rs",
    "crates/vox-forge/src/types.rs",
    "crates/vox-package/src/store/ops.rs",
    "crates/vox-codegen-rust/src/emit.rs",
    "crates/vox-dei-shim/src/research/orchestrator/mod.rs",
    "crates/vox-gamify/src/db.rs",
    "crates/vox-orchestrator-mcp/src/chat_tools/mod.rs",
    // selection/mod.rs removed: selection/ was WIP and not part of the active module tree
    // (see vox-dei-shim/src/lib.rs for context)
    "crates/vox-db/src/schema_digest.rs",
    "crates/vox-cli/src/cli_actions.rs",
    "crates/vox-compiler/src/ast/decl/mod.rs",
    "crates/vox-orchestrator/src/compaction.rs",
    "crates/vox-compiler/src/lexer/token.rs",
    "crates/vox-orchestrator-mcp/src/params.rs",
    "crates/vox-orchestrator/src/config.rs",
    "crates/vox-orchestrator/src/snapshot.rs",
];

pub const SYMBOL_HINT_PATHS: &[&str] = &[
    "crates/vox-compiler/src/ast/expr.rs",
    "crates/vox-cli/src/lib.rs",
    "crates/vox-compiler/src/hir/mod.rs",
    "crates/vox-orchestrator-mcp/src/memory_tools/mod.rs",
    "crates/vox-orchestrator/src/events.rs",
    "crates/vox-orchestrator/src/oplog.rs",
    "crates/vox-orchestrator/src/orchestrator.rs",
    "crates/vox-orchestrator/src/session.rs",
    "crates/vox-orchestrator/src/types.rs",
    "crates/vox-package/src/store/types.rs",
    "crates/vox-populi/src/mens/tensor/qlora_preflight.rs",
];

pub const SKIP_DIR_NAMES: &[&str] = &[
    "target",
    ".git",
    ".venv",
    "node_modules",
    "dist",
    "build",
    "__pycache__",
];

pub const INVENTORY_DESCRIPTION: &str = "Per-file comment/doc counts for LLM batch targeting. Rust: lines_triple_slash=///, lines_inner_doc=//!, lines_plain_comment=// excluding doc. Markdown: lines_total=all lines; lines_other_doc_signal=# heading count. hotspot_tier: 1=plan-listed path, 2=high-density heuristic, 0=other. symbol_hints (schema v3): plan-hotspot Rust only; /// or //! linked to next item; containing_symbol, doc_preview, comment_type, quality_tag (mechanical|operational|section_divider|user_help|narrative|ssot_sensitive).";

/// Default output path relative to repository root.
pub const DEFAULT_INVENTORY_PATH: &str = "docs/agents/doc-inventory.json";
