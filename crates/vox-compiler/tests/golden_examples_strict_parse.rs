//! Golden examples strict-parse gate — audit item A.13.
//!
//! When `VOX_EXAMPLES_STRICT_PARSE=1` (always set in CI), every `.vox` file
//! under `examples/golden/**` **must** parse without errors using the production
//! parser.  Any parse failure is a CI-blocking error.
//!
//! Run locally:
//!   VOX_EXAMPLES_STRICT_PARSE=1 cargo test -p vox-compiler --test golden_examples_strict_parse
//!
//! The test is skipped (not failed) when the env-var is unset, so it does not
//! break local dev workflows.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // Integration tests run from crates/vox-compiler/; workspace root is two up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../.."))
}

fn collect_golden_vox_files(root: &Path) -> Vec<PathBuf> {
    let golden = root.join("examples").join("golden");
    let mut files = Vec::new();
    collect_vox_recursive(&golden, &mut files);
    files.sort();
    files
}

fn collect_vox_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_vox_recursive(&p, out);
            } else if p.extension().is_some_and(|e| e == "vox") {
                out.push(p);
            }
        }
    }
}

#[test]
fn all_golden_examples_parse_clean() {
    // Skip unless explicitly opted-in (CI sets this to "1").
    if std::env::var("VOX_EXAMPLES_STRICT_PARSE").unwrap_or_default() != "1" {
        eprintln!(
            "golden_examples_strict_parse: skipped (set VOX_EXAMPLES_STRICT_PARSE=1 to enable)"
        );
        return;
    }

    let root = repo_root();
    let files = collect_golden_vox_files(&root);

    assert!(
        !files.is_empty(),
        "No .vox files found under examples/golden/ — check that the repo root is correct (looked in {})",
        root.join("examples/golden").display()
    );

    let mut failures: Vec<(PathBuf, Vec<String>)> = Vec::new();

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                failures.push((path.clone(), vec![format!("IO error: {e}")]));
                continue;
            }
        };

        let tokens = vox_compiler::lexer::lex(&source);
        if let Err(errors) = vox_compiler::parser::parse(tokens) {
            let msgs: Vec<String> = errors
                .iter()
                .map(|e| format!("  [{:?}] {}", e.class, e.message))
                .collect();
            failures.push((path.clone(), msgs));
        }
    }

    if failures.is_empty() {
        println!(
            "golden_examples_strict_parse: {} files parsed clean ✓",
            files.len()
        );
        return;
    }

    // Print all failures before panicking so CI output is actionable.
    eprintln!("\n=== STRICT PARSE FAILURES ({} files) ===", failures.len());
    for (path, msgs) in &failures {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        eprintln!("\n  {rel}:");
        for m in msgs {
            eprintln!("{m}");
        }
    }
    eprintln!("=========================================\n");

    panic!(
        "{} of {} golden examples failed to parse — set VOX_EXAMPLES_STRICT_PARSE=0 to skip locally",
        failures.len(),
        files.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_golden_includes_nested_examples() {
        let root = repo_root();
        let files = collect_golden_vox_files(&root);
        let mesh_noop = root.join("examples/golden/mesh/noop.vox");
        assert!(
            files.iter().any(|p| p == &mesh_noop),
            "strict-parse gate must traverse examples/golden/** recursively (missing {})",
            mesh_noop.display()
        );
    }
}
