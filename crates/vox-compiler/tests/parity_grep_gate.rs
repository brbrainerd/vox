//! CI grep-gate: forbids silent `_ => String::new()` / `_ => "".to_string()`
//! catch-alls in the HIR emitter files.
//!
//! These are the exact patterns that existed before Wave 2 was applied and
//! caused WorkflowVersion to silently emit empty strings in the TypeScript path.
//! If anyone reintroduces such a pattern, this test catches it at `cargo test` time.
//!
//! **Why file-reading instead of compile-time?**  The compile-time gate (the
//! exhaustive `support()` match in `feature_matrix.rs`) prevents *new* Feature
//! variants from being unsupported silently. But it cannot retroactively enforce
//! that existing emitter arms emit diagnostics rather than empty strings. This
//! grep-gate is the complementary "no regression" check.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/vox-compiler; workspace root is 3 levels up.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    PathBuf::from(manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Files that must not contain silent-empty-string drops.
const EMITTER_FILES: &[&str] = &[
    "crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs",
    "crates/vox-compiler/src/eval/expr.rs",
    "crates/vox-codegen-ts/src/hir_emit/mod.rs",
];

/// Patterns that indicate a silent empty-string drop from an emitter.
/// These are NOT flagged in test code (the grep is limited to non-test sections).
const SILENT_DROP_PATTERNS: &[&str] = &[
    "=> String::new()",
    "=> \"\".to_string()",
];

#[test]
fn no_silent_empty_string_drops_in_emitters() {
    let root = workspace_root();
    let mut violations: Vec<String> = Vec::new();

    for rel_path in EMITTER_FILES {
        let path = root.join(rel_path);
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => panic!("parity_grep_gate: could not read {rel_path}: {e}"),
        };

        // Only scan non-test sections: stop at the first `#[cfg(test)]` line.
        let non_test_src = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(&src);

        for (line_no, line) in non_test_src.lines().enumerate() {
            // Skip comment lines.
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            for pattern in SILENT_DROP_PATTERNS {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: silent empty-string drop `{}` — use unsupported_diagnostic() instead",
                        rel_path,
                        line_no + 1,
                        pattern.trim(),
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "parity grep-gate: silent empty-string drops detected in emitters.\n\
             Each must use unsupported_diagnostic() to derive a code+message from the\n\
             parity matrix instead of returning String::new() / \"\".to_string().\n\n\
             Violations:\n{}\n\n\
             See docs/src/architecture/pipeline-parity-ssot-2026-06-14.md §3.3 for guidance.",
            violations.join("\n")
        );
    }
}
