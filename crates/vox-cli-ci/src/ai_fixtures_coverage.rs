//! `vox ci ai-fixtures-coverage` — catalog parity against lexer, HIR, typecheck, and TS surfaces.

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
pub fn run(repo_root: &Path) -> Result<()> {
    let catalog = repo_root.join("contracts/agentos/ai-first-fixtures.v1.yaml");
    let token_rs = repo_root.join("crates/vox-compiler/src/lexer/token.rs");
    let hir_grafts = repo_root.join("crates/vox-compiler/src/hir/nodes/boilerplate_grafts.rs");
    let typeck_ai = repo_root.join("crates/vox-compiler/src/typeck/boilerplate_grafts.rs");
    let ts_emitter = repo_root.join("crates/vox-codegen-ts/src/emitter.rs");

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn require_contains_accepts_present_marker() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("file.txt");
        let mut f = fs::File::create(&path).expect("create");
        writeln!(f, "class: agent_control").expect("write");
        require_contains(&path, "class: agent_control").expect("marker present");
    }

    #[test]
    fn require_contains_rejects_missing_marker() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("file.txt");
        fs::write(&path, "other content").expect("write");
        let err = require_contains(&path, "class: agent_control").unwrap_err();
        assert!(err.to_string().contains("missing required marker"));
    }

    #[test]
    fn run_passes_on_workspace_repo() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");
        run(repo_root).expect("ai-fixtures-coverage must pass on current repo");
    }
}
