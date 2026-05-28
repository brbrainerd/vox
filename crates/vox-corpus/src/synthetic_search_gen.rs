use serde_json::json;
use std::io::Write;

include!(concat!(env!("OUT_DIR"), "/dynamic_registry.rs"));

/// Generate dynamic multi-turn codebase search conversation traces.
///
/// Instead of 3 hardcoded scenarios, this dynamically generates traces from:
/// 1. The `TOOL_REGISTRY_SLIM` (where each MCP tool is implemented)
/// 2. The `TAXONOMY_FROM_AST` (how each construct is parsed/compiled)
/// 3. The `CLI_COMMANDS` (how to use each CLI command)
/// 4. Cross-crate architecture questions
pub fn generate_search_traces(out: &mut impl Write) -> anyhow::Result<usize> {
    let mut count = 0;

    // ── 1. Tool implementation search ────────────────────────────────────────
    let tool_search_templates = [
        (
            "Where is the `{tool}` MCP tool implemented?",
            "{tool} implementation",
        ),
        (
            "How does `{tool}` handle its arguments?",
            "{tool} handler arguments",
        ),
        ("Show me the source code for `{tool}`", "{tool} source code"),
    ];
    for &tool in TOOL_REGISTRY_SLIM.iter().take(30) {
        let tmpl = &tool_search_templates[count % tool_search_templates.len()];
        let query = tmpl.0.replace("{tool}", tool);
        let search_query = tmpl.1.replace("{tool}", tool);
        let fake_file = format!(
            "crates/vox-mcp/src/tools/{}.rs",
            tool.trim_start_matches("vox_")
        );
        emit_search_trace(out, &query, &search_query, &fake_file, tool)?;
        count += 1;
    }

    // ── 2. Construct parsing search ──────────────────────────────────────────
    let construct_templates = [
        (
            "How does the Vox parser handle `{construct}` declarations?",
            "{construct} parser grammar",
        ),
        (
            "Where is `{construct}` lowered to HIR?",
            "{construct} HIR lowering",
        ),
        (
            "How does type checking work for `{construct}`?",
            "{construct} typeck",
        ),
        (
            "What codegen does Vox produce for a `{construct}`?",
            "{construct} codegen",
        ),
    ];
    for construct in TAXONOMY_FROM_AST.iter().take(20) {
        let tmpl = &construct_templates[count % construct_templates.len()];
        let query = tmpl.0.replace("{construct}", construct);
        let search_query = tmpl.1.replace("{construct}", construct);
        let fake_file = format!("crates/vox-compiler/src/parser/{}.md", construct);
        emit_search_trace(out, &query, &search_query, &fake_file, construct)?;
        count += 1;
    }

    // ── 3. CLI command usage search ──────────────────────────────────────────
    let cli_templates = [
        ("How do I use `vox {cmd}`?", "vox {cmd} usage"),
        (
            "What flags does `vox {cmd}` accept?",
            "vox {cmd} flags arguments",
        ),
        ("Show me an example of `vox {cmd}`", "vox {cmd} example"),
    ];
    for (cmd, desc) in CLI_COMMANDS {
        let tmpl = &cli_templates[count % cli_templates.len()];
        let query = tmpl.0.replace("{cmd}", cmd);
        let search_query = tmpl.1.replace("{cmd}", cmd);
        let fake_file = "crates/vox-cli/src/commands/mod.rs".to_string();
        let conversation = json!({
            "prompt": query,
            "response": format!(
                "The `vox {}` command is used to {}. It is implemented in `{}`. Usage:\n\n```\nvox {}\n```",
                cmd, desc, fake_file, cmd
            ),
            "category": "codebase_search",
            "record_type": "conversation",
            "schema_version": "vox_dogfood_v1",
            "format": "conversation",
            "messages": [
                { "role": "user", "content": query },
                {
                    "role": "assistant",
                    "content": json!({
                        "tool": "vox_db_codebase_search",
                        "arguments": { "query": search_query }
                    }).to_string()
                },
                {
                    "role": "system",
                    "content": format!("--- Search Results ---\n[1] {}\n// CLI handler for `vox {}`", fake_file, cmd)
                },
                {
                    "role": "assistant",
                    "content": format!(
                        "The `vox {}` command is used to {}. It is implemented in `{}`. Usage:\n\n```\nvox {}\n```",
                        cmd, desc, fake_file, cmd
                    )
                }
            ]
        });
        writeln!(out, "{}", serde_json::to_string(&conversation)?)?;
        count += 1;
    }

    // ── 4. Architecture cross-crate questions ────────────────────────────────
    let arch_scenarios = [
        (
            "How does data flow from a .vox file to running Rust code?",
            "compiler pipeline source to runtime",
            "crates/vox-compiler/src/lib.rs",
            "The Vox compiler is the `vox-compiler` workspace crate: Source → Lexer (`logos` in `lexer/token.rs`) → Parser (`parser`) → AST (`ast`) → HIR (`hir`) → TypeCheck (`typeck`) → Rust/TS codegen (`codegen_rust`, `codegen_ts`, `web_ir`). See `docs/src/architecture/nomenclature-migration-map.md` for retired split-crate names.",
        ),
        (
            "How does the actor system work in Vox?",
            "actor mailbox runtime tokio",
            "crates/vox-actor-runtime/src/actor.rs",
            "Every `actor` declaration compiles to a Tokio task with an `mpsc` channel mailbox. Messages are dispatched via pattern matching on message types. State is isolated per actor instance, preventing data races.",
        ),
        (
            "How does Vox handle database schemas?",
            "@table schema codegen SQL",
            "crates/vox-compiler/src/codegen_rust/",
            "`@table` and related DB declarations are lowered through `vox-compiler` HIR and Rust codegen (Codex/SQL surfaces). Field and index declarations stay type-checked in-tree before emit.",
        ),
        (
            "How does the MCP server expose tools?",
            "MCP tool registration server",
            "crates/vox-mcp/src/server.rs",
            "MCP tools are registered in `vox-mcp-registry` and served via Axum. Each `@mcp.tool` declaration in .vox becomes a handler function. Tool discovery is automatic via the dynamic registry.",
        ),
        (
            "How does QLoRA training work with Vox Mens?",
            "QLoRA candle training pipeline",
            "crates/vox-populi/src/mens/tensor/candle_qlora_train.rs",
            "QLoRA training uses Candle + qlora-rs: NF4 frozen projection/LM-head linears + trainable LoRA adapters. Training data is loaded from JSONL, tokenized with ChatML format, and trained with causal language modeling loss.",
        ),
        (
            "How does the orchestrator route tasks to agents?",
            "orchestrator routing affinity groups",
            "crates/vox-orchestrator/src/routing.rs",
            "The orchestrator uses affinity groups (from Vox.toml or detected from workspace layout) to route tasks. Socrates reputation scores blend with routing decisions. File ownership and lock status determine agent assignment.",
        ),
        (
            "What is the Codex/Arca data architecture?",
            "Codex VoxDb Arca CodeStore Turso",
            "crates/vox-db/src/lib.rs",
            "Codex is the public unified data API (VoxDb type). Arca is the internal schema + CodeStore on Turso. VoxDb::store() returns &CodeStore. Migrations use execute_batch (no row-returning statements).",
        ),
        (
            "How does Vox handle component rendering?",
            "component JSX React SSR",
            "crates/vox-compiler/src/web_ir/",
            "UI constructs lower to `web_ir` and emit TS/TSX via `vox-compiler` (`emit_tsx`, React interop). Routing and client data patterns follow the current web stack docs (`docs/src/reference/vox-web-stack.md`).",
        ),
    ];

    for (query, search_query, file, answer) in &arch_scenarios {
        emit_search_trace_with_answer(out, query, search_query, file, answer)?;
        count += 1;
    }

    Ok(count)
}

fn emit_search_trace(
    out: &mut impl Write,
    query: &str,
    search_query: &str,
    fake_file: &str,
    context_name: &str,
) -> anyhow::Result<()> {
    let final_answer = format!(
        "Based on the search results, `{}` is implemented in `{}`. The implementation handles the core logic for this feature in the Vox ecosystem.",
        context_name, fake_file
    );
    let conversation = json!({
        "prompt": query,
        "response": final_answer,
        "category": "codebase_search",
        "record_type": "conversation",
        "schema_version": "vox_dogfood_v1",
        "format": "conversation",
        "messages": [
            { "role": "user", "content": query },
            {
                "role": "assistant",
                "content": json!({
                    "tool": "vox_db_codebase_search",
                    "arguments": { "query": search_query }
                }).to_string()
            },
            {
                "role": "system",
                "content": format!("--- Search Results ---\n[1] {}\n// Implementation of {}", fake_file, context_name)
            },
            {
                "role": "assistant",
                "content": final_answer
            }
        ]
    });
    writeln!(out, "{}", serde_json::to_string(&conversation)?)?;
    Ok(())
}

fn emit_search_trace_with_answer(
    out: &mut impl Write,
    query: &str,
    search_query: &str,
    fake_file: &str,
    answer: &str,
) -> anyhow::Result<()> {
    let conversation = json!({
        "prompt": query,
        "response": answer,
        "category": "codebase_search",
        "record_type": "conversation",
        "schema_version": "vox_dogfood_v1",
        "format": "conversation",
        "messages": [
            { "role": "user", "content": query },
            {
                "role": "assistant",
                "content": json!({
                    "tool": "vox_db_codebase_search",
                    "arguments": { "query": search_query }
                }).to_string()
            },
            {
                "role": "system",
                "content": format!("--- Search Results ---\n[1] {}", fake_file)
            },
            {
                "role": "assistant",
                "content": answer
            }
        ]
    });
    writeln!(out, "{}", serde_json::to_string(&conversation)?)?;
    Ok(())
}
