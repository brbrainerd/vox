//! Property-based tests for vox-codegen (Rust + TS codegen).
//!
//! Catches non-determinism in codegen and ensures generated Rust output
//! does not contain error markers that would cause downstream compilation
//! failures.
//!
//! Case budget: 64 per test to keep CI within time budget.

use proptest::prelude::*;
use vox_codegen::codegen_rust;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Simple Vox snippets that are known-valid and produce Rust output.
/// Using a fixed set lets us focus the property tests on determinism rather
/// than input generation (grammar generation would require its own strategy).
const SIMPLE_SNIPPETS: &[&str] = &[
    // Minimal — no declarations
    "",
    // Single endpoint function
    r#"@query fn get_count() to int { return 0 }"#,
    // Mutation endpoint
    r#"@mutation fn update_count(n: int) to int { return n }"#,
    // Two functions
    r#"@query fn ping() to int { return 1 }
@mutation fn pong(x: int) to int { return x }"#,
];

fn generate_rust_output(src: &str) -> Result<String, String> {
    let m = parse(lex(src)).map_err(|e| format!("parse error: {e:?}"))?;
    let hir = lower_module(&m);
    let out = codegen_rust::generate(
        &hir,
        "test_pkg",
        codegen_rust::RustAppShell::AxumLocalServer,
    )
    .map_err(|e| format!("codegen error: {e}"))?;
    // Concatenate all file contents to a single comparable string.
    let mut keys: Vec<_> = out.files.keys().collect();
    keys.sort();
    Ok(keys
        .iter()
        .map(|k| format!("=== {} ===\n{}", k, out.files[*k]))
        .collect::<Vec<_>>()
        .join("\n\n"))
}

// ── Test 1: codegen is deterministic ────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn codegen_deterministic(idx in 0usize..SIMPLE_SNIPPETS.len()) {
        let src = SIMPLE_SNIPPETS[idx];
        let first = generate_rust_output(src)
            .expect("first codegen run must succeed");
        let second = generate_rust_output(src)
            .expect("second codegen run must succeed");
        prop_assert_eq!(
            first,
            second,
            "Codegen produced different output on two runs for snippet index {}",
            idx
        );
    }
}

// ── Test 2: generated Rust contains no error markers ────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn codegen_no_error_markers(idx in 0usize..SIMPLE_SNIPPETS.len()) {
        let src = SIMPLE_SNIPPETS[idx];
        let output = generate_rust_output(src)
            .expect("codegen must succeed for known-valid snippet");
        prop_assert!(
            !output.contains("compile_error!"),
            "Generated Rust must not contain compile_error! for snippet index {idx}.\n\
             Output excerpt:\n{}",
            &output[..output.len().min(500)]
        );
        prop_assert!(
            !output.contains("todo!()"),
            "Generated Rust must not contain todo!() for snippet index {idx}.\n\
             Output excerpt:\n{}",
            &output[..output.len().min(500)]
        );
    }
}
