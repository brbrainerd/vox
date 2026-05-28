use anyhow::{Result, bail};
use std::path::Path;

fn require_contains(path: &Path, needle: &str) -> Result<()> {
    let body = std::fs::read_to_string(path)?;
    if !body.contains(needle) {
        bail!("{} missing required marker `{needle}`", path.display());
    }
    Ok(())
}

/// Verify AI fixture catalog parity against shipped parser + HIR surfaces.
pub fn run(root: &Path) -> Result<()> {
    let catalog = root.join("contracts/agentos/ai-first-fixtures.v1.yaml");
    let token_rs = root.join("crates/vox-compiler/src/lexer/token.rs");
    let hir_grafts = root.join("crates/vox-compiler/src/hir/nodes/boilerplate_grafts.rs");
    let typeck_ai = root.join("crates/vox-compiler/src/typeck/boilerplate_grafts.rs");
    let ts_emitter = root.join("crates/vox-codegen/src/codegen_ts/emitter.rs");

    // Catalog coverage checks (fixture classes we claim to ship).
    require_contains(&catalog, "class: agent_control")?;
    require_contains(&catalog, "class: model_selection")?;
    require_contains(&catalog, "class: query_template")?;
    require_contains(&catalog, "class: search_substitution")?;
    require_contains(&catalog, "class: deferred_fill")?;

    // Lexer parity.
    require_contains(&token_rs, "AtPrompt")?;
    require_contains(&token_rs, "AtSubagent")?;
    require_contains(&token_rs, "AtSearch")?;
    require_contains(&token_rs, "AtHole")?;

    // HIR parity.
    require_contains(&hir_grafts, "HirAiFixture")?;
    require_contains(&hir_grafts, "IntentRouted")?;
    require_contains(&hir_grafts, "Prompt")?;
    require_contains(&hir_grafts, "Subagent")?;
    require_contains(&hir_grafts, "Search")?;
    require_contains(&hir_grafts, "Hole")?;

    // Typecheck / TS surfaces for catalog-backed diagnostic IDs.
    require_contains(&typeck_ai, "collect_ai_fixture_diagnostics")?;
    require_contains(&typeck_ai, "vox/ai/unknown-task-category")?;
    require_contains(&typeck_ai, "vox/prompt/invalid-stage")?;
    require_contains(&typeck_ai, "vox/subagent/chain-depth-exceeded")?;
    require_contains(&typeck_ai, "vox/search/corpus-denied")?;
    require_contains(&typeck_ai, "vox/subagent/distributed-not-wired")?;

    require_contains(&ts_emitter, "vox/codegen/missing-ts-ai-lowering")?;

    println!("ai-fixtures-coverage: catalog ↔ lexer/HIR parity OK");
    Ok(())
}
