use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use vox_codegen::syntax_k::{SyntaxKInput, canonical_web_ir_bytes, measure_syntax_k_event};
use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

#[derive(Debug, Serialize, Deserialize, Default)]
struct ComplexityBudget {
    #[serde(default)]
    fixtures: HashMap<String, usize>,
}

pub(crate) fn run_k_complexity_budget(root: &Path, tolerance: f64, update: bool) -> Result<()> {
    let budget_path = root.join("contracts/eval/complexity-budget.v1.json");
    let mut budget = if budget_path.exists() {
        let content = fs::read_to_string(&budget_path)?;
        serde_json::from_str::<ComplexityBudget>(&content)?
    } else {
        ComplexityBudget::default()
    };

    let ladder = vox_codegen::canonical_ladder::CanonicalLadder::load_from_repo_root(root)
        .map_err(|e| anyhow!("failed to load canonical ladder: {e}"))?;
    let ladder_ids = ladder.fixture_ids();

    let golden_dir = root.join("examples/golden");
    if !golden_dir.is_dir() {
        return Err(anyhow!("examples/golden directory not found"));
    }

    let mut failures = Vec::new();
    let mut new_budgets = HashMap::new();

    // Only ladder fixtures participate in the k-complexity budget gate.
    for entry in fs::read_dir(golden_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("vox") {
            let fixture_id = path.file_stem().unwrap().to_str().unwrap().to_string();
            if !ladder_ids.contains(&fixture_id) {
                continue;
            }
            let source = fs::read_to_string(&path)?;

            // Measure K-complexity of WebIR
            let tokens = lex(&source);
            let module =
                parse(tokens).map_err(|e| anyhow!("Failed to parse {}: {:?}", fixture_id, e))?;
            let hir = lower_module(&module);
            let web_ir = lower_hir_to_web_ir(&hir);
            let ir_bytes = canonical_web_ir_bytes(&web_ir)
                .map_err(|e| anyhow!("Failed to serialize IR {}: {:?}", fixture_id, e))?;

            let input = SyntaxKInput {
                fixture_id: &fixture_id,
                target_kind: "web_ir",
                bytes: &ir_bytes,
                source_hash: None,
                web_ir_hash: None,
                baseline_bytes: None,
                support_metrics: None,
            };

            let event = measure_syntax_k_event(input)
                .map_err(|e| anyhow!("Failed to measure K-complexity {}: {:?}", fixture_id, e))?;
            let current_k = event.k_est_bytes;

            new_budgets.insert(fixture_id.clone(), current_k);

            if let Some(&allowed) = budget.fixtures.get(&fixture_id) {
                let limit = (allowed as f64 * (1.0 + tolerance / 100.0)).ceil() as usize;
                if current_k > limit {
                    failures.push(format!(
                        "Fixture '{}' exceeded budget: {} > {} (allowed: {}, tolerance: {}%)",
                        fixture_id, current_k, limit, allowed, tolerance
                    ));
                }
            } else if !update {
                failures.push(format!(
                    "Fixture '{}' has no budget defined in {}",
                    fixture_id,
                    budget_path.display()
                ));
            }
        }
    }

    let total_fixtures = new_budgets.len();
    if update {
        budget.fixtures = new_budgets;
        let content = serde_json::to_string_pretty(&budget)?;
        if let Some(parent) = budget_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&budget_path, content)?;
        println!(
            "Updated complexity budget baseline: {}",
            budget_path.display()
        );
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("  [K-Complexity] ERROR: {}", f);
        }
        anyhow::bail!(
            "K-complexity budget audit failed ({} violations): {}",
            failures.len(),
            failures.join("; ")
        );
    }

    println!(
        "K-complexity budget OK ({} ladder fixtures validated)",
        total_fixtures
    );
    Ok(())
}

/// Per-fixture source-token budget: structural lexer-token count plus raw source
/// byte count (a coarse BPE correlate; `bytes/4` ≈ model tokens).
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
struct SourceTokenEntry {
    tokens: usize,
    bytes: usize,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct SourceTokenBudgetFile {
    #[serde(default)]
    fixtures: HashMap<String, SourceTokenEntry>,
}

/// Source-token budget gate (ladder-scoped), mirroring [`run_k_complexity_budget`].
///
/// Measures `lex(source).len()` (structural lexer tokens — a lower bound on grammar
/// verbosity, NOT model BPE tokens) and the raw source byte count for each golden
/// ladder fixture, comparing both against `contracts/eval/source-token-budget.v1.json`.
/// Ratchet-down: fail when a fixture exceeds its budget beyond `tolerance`. `--update`
/// rebaselines (run after the decorator→keyword codemod to record the shrunk counts).
pub(crate) fn run_source_token_budget(root: &Path, tolerance: f64, update: bool) -> Result<()> {
    // Extra headroom (percentage points) the byte budget gets on top of the token
    // tolerance — raw bytes are noisier than structural tokens, so they gate loosely.
    const BYTE_HEADROOM_PCT: f64 = 10.0;
    let budget_path = root.join("contracts/eval/source-token-budget.v1.json");
    let mut budget = if budget_path.exists() {
        serde_json::from_str::<SourceTokenBudgetFile>(&fs::read_to_string(&budget_path)?)?
    } else {
        SourceTokenBudgetFile::default()
    };

    let ladder = vox_codegen::canonical_ladder::CanonicalLadder::load_from_repo_root(root)
        .map_err(|e| anyhow!("failed to load canonical ladder: {e}"))?;
    let ladder_ids = ladder.fixture_ids();

    let golden_dir = root.join("examples/golden");
    if !golden_dir.is_dir() {
        return Err(anyhow!("examples/golden directory not found"));
    }

    let mut failures = Vec::new();
    let mut new_budgets = HashMap::new();

    for entry in fs::read_dir(golden_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("vox") {
            continue;
        }
        let fixture_id = path.file_stem().unwrap().to_str().unwrap().to_string();
        if !ladder_ids.contains(&fixture_id) {
            continue;
        }
        let source = fs::read_to_string(&path)?;
        let measured = SourceTokenEntry {
            tokens: lex(&source).len(),
            bytes: source.as_bytes().len(),
        };
        new_budgets.insert(fixture_id.clone(), measured);

        if let Some(allowed) = budget.fixtures.get(&fixture_id) {
            // TOKEN count is the tight ratchet (structural; comment-independent because
            // lex() strips comments). BYTE count is also enforced, but with generous
            // headroom (+BYTE_HEADROOM_PCT beyond the token tolerance) so a benign
            // comment/whitespace edit to a golden does not fail CI while a gross size
            // regression still does. `--update` rebaselines on legitimate growth.
            let tok_limit = (allowed.tokens as f64 * (1.0 + tolerance / 100.0)).ceil() as usize;
            if measured.tokens > tok_limit {
                failures.push(format!(
                    "Fixture '{fixture_id}' exceeded token budget: {} > {tok_limit} (allowed: {})",
                    measured.tokens, allowed.tokens
                ));
            }
            let byte_limit = (allowed.bytes as f64
                * (1.0 + (tolerance + BYTE_HEADROOM_PCT) / 100.0))
                .ceil() as usize;
            if measured.bytes > byte_limit {
                failures.push(format!(
                    "Fixture '{fixture_id}' exceeded byte budget: {} > {byte_limit} (allowed: {} + {BYTE_HEADROOM_PCT}% headroom)",
                    measured.bytes, allowed.bytes
                ));
            }
        } else if !update {
            failures.push(format!(
                "Fixture '{fixture_id}' has no source-token budget defined in {}",
                budget_path.display()
            ));
        }
    }

    let total_fixtures = new_budgets.len();
    if update {
        budget.fixtures = new_budgets;
        let content = serde_json::to_string_pretty(&budget)?;
        if let Some(parent) = budget_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&budget_path, content)?;
        println!(
            "Updated source-token budget baseline: {}",
            budget_path.display()
        );
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("  [SourceToken] ERROR: {f}");
        }
        anyhow::bail!(
            "Source-token budget audit failed ({} violations): {}",
            failures.len(),
            failures.join("; ")
        );
    }

    println!("Source-token budget OK ({total_fixtures} ladder fixtures validated)");
    Ok(())
}

#[cfg(test)]
mod k_complexity_budget_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const HELLO_VOX: &str = r#"fn hello(name: str) to str {
    return "Hello " + name + "!"
}
"#;

    const LADDER_YAML: &str = r#"x-vox-version: 1
fixtures:
  - id: hello
    path: examples/golden/hello.vox
    proves: [parse, lower, typecheck]
    targets: [interp, rust-script]
"#;

    fn write_minimal_ladder_workspace(root: &Path, budget_fixtures_json: &str) {
        fs::create_dir_all(root.join("contracts/pipeline")).expect("pipeline dir");
        fs::create_dir_all(root.join("contracts/eval")).expect("eval dir");
        fs::create_dir_all(root.join("examples/golden")).expect("golden dir");

        fs::write(
            root.join("contracts/pipeline/canonical-ladder.v1.yaml"),
            LADDER_YAML,
        )
        .expect("ladder yaml");
        fs::write(root.join("examples/golden/hello.vox"), HELLO_VOX).expect("hello fixture");
        let budget = format!(
            r#"{{
  "x-vox-version": 1,
  "fixtures": {budget_fixtures_json}
}}"#
        );
        fs::write(
            root.join("contracts/eval/complexity-budget.v1.json"),
            budget,
        )
        .expect("budget json");
    }

    #[test]
    fn missing_ladder_fixture_budget_fails_when_not_updating() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_minimal_ladder_workspace(root, "{}");

        let err = run_k_complexity_budget(root, 0.0, false).expect_err("missing budget must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("hello") && msg.contains("no budget defined"),
            "expected missing-budget failure for hello, got: {msg}"
        );
    }

    #[test]
    fn all_ladder_fixtures_have_budget_entries_in_repo() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        run_k_complexity_budget(&root, 0.0, false)
            .expect("real repo k-complexity budget should pass");
    }

    fn write_source_token_budget(root: &Path, fixtures_json: &str) {
        fs::create_dir_all(root.join("contracts/eval")).expect("eval dir");
        fs::write(
            root.join("contracts/eval/source-token-budget.v1.json"),
            format!(r#"{{ "fixtures": {fixtures_json} }}"#),
        )
        .expect("source-token budget json");
    }

    #[test]
    fn source_token_budget_passes_under_generous_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_minimal_ladder_workspace(root, "{}");
        write_source_token_budget(root, r#"{ "hello": { "tokens": 9999, "bytes": 99999 } }"#);
        run_source_token_budget(root, 0.0, false).expect("under-budget hello must pass");
    }

    #[test]
    fn source_token_budget_fails_over_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_minimal_ladder_workspace(root, "{}");
        write_source_token_budget(root, r#"{ "hello": { "tokens": 1, "bytes": 1 } }"#);
        let err =
            run_source_token_budget(root, 0.0, false).expect_err("over-budget hello must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("hello") && msg.contains("exceeded"),
            "expected over-budget failure for hello, got: {msg}"
        );
    }

    #[test]
    fn source_token_budget_missing_entry_fails_when_not_updating() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_minimal_ladder_workspace(root, "{}");
        write_source_token_budget(root, "{}");
        let err = run_source_token_budget(root, 0.0, false).expect_err("missing budget must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("hello") && msg.contains("no source-token budget"),
            "expected missing-budget failure for hello, got: {msg}"
        );
    }
}
