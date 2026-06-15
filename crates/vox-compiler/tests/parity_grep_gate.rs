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
/// These are NOT flagged in test code (the grep skips `#[cfg(test)]` blocks).
const SILENT_DROP_PATTERNS: &[&str] = &["=> String::new()", "=> \"\".to_string()"];

/// Strip the content of `#[cfg(test)]` blocks from `src`, returning the
/// non-test portions concatenated.  Uses brace-counting so that test modules
/// appearing *anywhere* in the file (not just at the end) are excluded, while
/// production code that follows the last test block is still scanned.
fn strip_test_blocks(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let chars = src.chars().peekable();
    // We scan line-by-line so we can detect `#[cfg(test)]`.
    let mut remaining = src;

    while !remaining.is_empty() {
        // Look for the next `#[cfg(test)]` marker.
        if let Some(idx) = remaining.find("#[cfg(test)]") {
            // Everything before the marker is production code.
            out.push_str(&remaining[..idx]);
            remaining = &remaining[idx + "#[cfg(test)]".len()..];
            // Skip optional whitespace / newlines until we hit `{` or `mod`.
            // The common pattern is `#[cfg(test)]\nmod tests {\n...`.
            // We need to find the opening brace of the block (depth 0→1) and
            // then skip until the matching closing brace (depth back to 0).
            let open = match remaining.find('{') {
                Some(p) => p,
                None => {
                    // No opening brace — malformed; just include the rest.
                    out.push_str(remaining);
                    break;
                }
            };
            remaining = &remaining[open + 1..]; // consume the `{`
            let mut depth: usize = 1;
            let mut close_pos = 0;
            let bytes = remaining.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            close_pos = i;
                            break;
                        }
                    }
                    // Skip string literals to avoid counting braces inside them.
                    b'"' => {
                        i += 1;
                        while i < bytes.len() {
                            if bytes[i] == b'\\' {
                                i += 1; // skip escaped char
                            } else if bytes[i] == b'"' {
                                break;
                            }
                            i += 1;
                        }
                    }
                    // Skip char literals.
                    b'\'' => {
                        i += 1;
                        while i < bytes.len() {
                            if bytes[i] == b'\\' {
                                i += 1;
                            } else if bytes[i] == b'\'' {
                                break;
                            }
                            i += 1;
                        }
                    }
                    // Skip line comments.
                    b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                        while i < bytes.len() && bytes[i] != b'\n' {
                            i += 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            if depth == 0 {
                // Skip past the closing brace of the test block.
                remaining = &remaining[close_pos + 1..];
            } else {
                // Unbalanced — stop scanning.
                break;
            }
        } else {
            // No more `#[cfg(test)]` markers; include the rest.
            out.push_str(remaining);
            break;
        }
    }
    // Suppress unused variable warning from the `chars` binding above.
    drop(chars);
    out
}

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

        // Strip #[cfg(test)] blocks with brace-counting — avoids false
        // negatives from production code that appears after a test module.
        let non_test_src = strip_test_blocks(&src);

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
