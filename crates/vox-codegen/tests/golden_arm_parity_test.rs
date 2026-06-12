//! CR-F2 ratchet (emit-only): `main()` goldens with `// EXPECT:` must lower through
//! `generate_script` without error. This is the fast inner loop for codegen-rust
//! parity — compile/run parity lives in `emit_compile_harness` (slow, `#[ignore]`).
//!
//! Baseline measured 2026-06-05: 3/10 script goldens compile+run; emit-only floor
//! ratchets as fixes land. Bump `EMIT_BASELINE` when adding verified goldens.

use std::path::PathBuf;

use vox_codegen::codegen_rust::generate_script;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module_with_path;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vox-actor-runtime")
}

/// Goldens known to emit cleanly (extend as CR-F2 backlog shrinks).
const EMIT_GOLDENS: &[&str] = &[
    "examples/golden/mesh/noop.vox",
    "examples/golden/decimal_math.vox",
    "examples/golden/while_loop_algorithms.vox",
    "examples/golden/regex_free_functions.vox",
    "examples/golden/tuple_destructure.vox",
    "examples/golden/closures_hof.vox",
    "examples/golden/json_as_typed.vox",
];

/// Minimum count that must pass — ratchet only upward.
const EMIT_BASELINE: usize = 7;

fn parse_expect_comment(src: &str) -> bool {
    src.lines()
        .any(|line| line.trim_start().starts_with("// EXPECT:"))
}

fn assert_has_main(src: &str) {
    assert!(
        src.contains("fn main"),
        "golden must declare fn main() for CR-F2 script arm"
    );
}

fn assert_golden_script_emits(rel: &str) {
    let path = repo_root().join(rel);
    assert!(path.is_file(), "missing golden: {}", path.display());

    let src = std::fs::read_to_string(&path).expect("read golden");
    assert!(
        parse_expect_comment(&src),
        "{} must declare at least one // EXPECT: line",
        rel
    );
    assert_has_main(&src);

    // Goldens use frontmatter; script lane accepts parse_script.
    let module = parse_script(lex(&src)).unwrap_or_else(|e| {
        panic!("parse_script failed for {}: {e:?}", rel);
    });
    let mut hir = lower_module(&module);
    let diags = typecheck_hir_module_with_path(&src, &mut hir, Some(&path));
    assert!(
        diags.is_empty(),
        "typecheck diagnostics for {}: {:?}",
        rel,
        diags
    );

    let out = generate_script(&hir, "vox-script", Some(&runtime_path()))
        .unwrap_or_else(|e| panic!("generate_script failed for {}: {e}", rel));

    assert!(
        out.files.contains_key("Cargo.toml"),
        "{rel}: missing Cargo.toml"
    );
    assert!(
        out.files.contains_key("src/lib.rs"),
        "{rel}: missing src/lib.rs"
    );
    assert!(
        out.files.contains_key("src/main.rs"),
        "{rel}: missing src/main.rs"
    );
    let main_rs = &out.files["src/main.rs"];
    assert!(main_rs.contains("fn main"), "{rel}: main.rs lacks fn main");
}

#[test]
fn golden_script_emit_noop_smoke() {
    assert_golden_script_emits("examples/golden/mesh/noop.vox");
}

#[test]
fn golden_script_emit_expect_corpus() {
    let mut passed = 0usize;
    let mut failures = Vec::new();

    for rel in EMIT_GOLDENS {
        match std::panic::catch_unwind(|| assert_golden_script_emits(rel)) {
            Ok(()) => passed += 1,
            Err(_) => failures.push(*rel),
        }
    }

    assert!(
        passed >= EMIT_BASELINE,
        "CR-F2 emit ratchet: {passed}/{} passed (baseline {EMIT_BASELINE}); failures: {failures:?}",
        EMIT_GOLDENS.len()
    );

    // When all listed goldens pass, this documents the new floor for the next bump.
    if !failures.is_empty() {
        eprintln!(
            "golden_arm_parity emit corpus: {passed}/{} green; still failing: {failures:?}",
            EMIT_GOLDENS.len()
        );
    }
}

#[test]
fn golden_script_emit_regex_patterns_use_valid_rust_escapes() {
    let path = repo_root().join("examples/golden/regex_free_functions.vox");
    let src = std::fs::read_to_string(&path).expect("read regex golden");
    let module = parse_script(lex(&src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module_with_path(&src, &mut hir, Some(&path));
    let out = generate_script(&hir, "vox-script", Some(&runtime_path())).expect("emit");
    let lib = out.files.get("src/lib.rs").expect("lib.rs");
    // `\w` in Vox regex strings must not appear as invalid Rust `\w` escapes.
    assert!(
        lib.contains("\\\\w") || lib.contains("r\""),
        "regex patterns must be Rust-safe; sample:\n{}",
        &lib[..lib.len().min(800)]
    );
}
