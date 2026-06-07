//! Integration gate: golden `@test` functions must actually *verify* something.
//!
//! The golden `@test` runner ([`golden_vox_test_runner`]) proves the tests *pass*,
//! but a test that asserts a tautology (`assert(true)`, `assert(1 is 1)`,
//! `assert(x is x)`) passes while verifying nothing — false confidence that the
//! behavioral-coverage push is explicitly meant to guard against (plan track A3,
//! `docs/superpowers/plans/2026-06-02-vox-golden-corpus-and-compiler-reality.md`).
//!
//! This gate lowers every golden to HIR and inspects each `@test` body for
//! **unambiguous** tautological assertions. The detection is intentionally
//! conservative (zero false positives): it flags only
//!   * `assert(true)`, and
//!   * `assert(a is a)` where both operands are the *same* identifier or the same
//!     literal value (`assert(1 is 1)`, `assert("x" is "x")`).
//! It never flags `assert(false)` (a legitimate unreachable-branch guard) or
//! `assert(v is 1)` (a real comparison of distinct operands).
//!
//! Mechanism: HIR derives `Serialize`, so we walk the serialized JSON generically
//! rather than hand-writing a full `HirStmt`/`HirExpr` visitor. The `Call`/`Ident`/
//! `Binary`/literal shapes are the default serde tuple-variant encodings. A
//! self-test (`gate_catches_planted_tautology`) plants `assert(1 is 1)` and proves
//! the detector fires, so a future change to the HIR encoding can't silently turn
//! this gate into a no-op.

use std::path::{Path, PathBuf};

use serde_json::Value;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../.."))
}

fn collect_golden_vox(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_vox_recursive(&root.join("examples").join("golden"), &mut files);
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

/// The value carried by a single-key tuple-variant object (e.g. `{"Ident": ["x", span]}`)
/// — returns the inner array if `value` is exactly `{ key: [..] }`.
fn variant<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    let obj = value.as_object()?;
    if obj.len() != 1 {
        return None;
    }
    obj.get(key)?.as_array()
}

/// True when `a` and `b` are the same atom (same identifier name or same literal
/// value), ignoring spans. Only atoms compare — nested expressions never match, so
/// the tautology check stays conservative.
fn same_atom(a: &Value, b: &Value) -> bool {
    const ATOMS: [&str; 6] = [
        "Ident",
        "IntLit",
        "FloatLit",
        "StringLit",
        "BoolLit",
        "DecimalLit",
    ];
    for key in ATOMS {
        if let (Some(av), Some(bv)) = (variant(a, key), variant(b, key)) {
            // First tuple element is the value; trailing element is the span.
            return av.first() == bv.first();
        }
    }
    false
}

/// True when `cond` (the argument to `assert`) is an unambiguous tautology.
fn is_tautological_cond(cond: &Value) -> bool {
    // `assert(true)`
    if let Some(arr) = variant(cond, "BoolLit")
        && arr.first() == Some(&Value::Bool(true))
    {
        return true;
    }
    // `assert(a is a)` — same identifier or same literal on both sides.
    if let Some(arr) = variant(cond, "Binary")
        && arr.first().and_then(Value::as_str) == Some("Is")
        && arr.len() >= 3
        && same_atom(&arr[1], &arr[2])
    {
        return true;
    }
    false
}

/// Recursively walk a serialized HIR value, collecting a short description of every
/// tautological `assert(..)` call found.
fn collect_tautologies(value: &Value, out: &mut Vec<String>) {
    // Is this node a `Call(callee, args, ..)` to the `assert` builtin?
    if let Some(call) = variant(value, "Call")
        && call.len() >= 2
        && let Some(ident) = variant(&call[0], "Ident")
        && ident.first().and_then(Value::as_str) == Some("assert")
        && let Some(args) = call[1].as_array()
        && let Some(first) = args.first()
        // HirArg serializes as `{ "name": .., "value": <expr> }`.
        && let Some(cond) = first.get("value")
        && is_tautological_cond(cond)
    {
        let kind = if variant(cond, "BoolLit").is_some() {
            "assert(true)"
        } else {
            "assert(<x> is <x>) — both operands identical"
        };
        out.push(kind.to_string());
    }

    // Recurse into every child value.
    match value {
        Value::Array(items) => items.iter().for_each(|v| collect_tautologies(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect_tautologies(v, out)),
        _ => {}
    }
}

/// Lower one `.vox` source and return `(test_fn_name, tautology_description)` for
/// every tautological assertion found in a `@test` body.
fn tautological_tests_in(src: &str) -> Vec<(String, String)> {
    let module = match parse(lex(src)) {
        Ok(m) => m,
        // Parse failures are surfaced by the dedicated parse gate; ignore here.
        Err(_) => return vec![],
    };
    let hir = lower_module(&module);

    let mut found = Vec::new();
    for test_fn in &hir.tests {
        let Ok(json) = serde_json::to_value(test_fn) else {
            continue;
        };
        let mut hits = Vec::new();
        collect_tautologies(&json, &mut hits);
        for hit in hits {
            found.push((test_fn.name.clone(), hit));
        }
    }
    found
}

#[test]
fn golden_at_tests_are_not_tautological() {
    let root = repo_root();
    let files = collect_golden_vox(&root);
    assert!(
        !files.is_empty(),
        "No golden .vox files found under {}",
        root.join("examples/golden").display()
    );

    let mut offenders: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        if !src.contains("@test") {
            continue;
        }
        checked += 1;
        let label = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        for (name, kind) in tautological_tests_in(&src) {
            offenders.push(format!("  {label}::{name} — {kind}"));
        }
    }

    if offenders.is_empty() {
        println!(
            "[golden_test_quality_gate] no tautological @test assertions across {checked} golden file(s) ✓"
        );
    } else {
        panic!(
            "{} tautological @test assertion(s) found — these pass while verifying nothing:\n{}",
            offenders.len(),
            offenders.join("\n")
        );
    }
}

/// Proves the detector is not a no-op: a planted `assert(1 is 1)` (and `assert(true)`)
/// must be caught. Guards against a future HIR-encoding change silently disabling
/// the gate.
#[test]
fn gate_catches_planted_tautology() {
    let tautological = r#"
        @test
        fn fake_check() {
            assert(1 is 1)
            assert(true)
        }
    "#;
    let hits = tautological_tests_in(tautological);
    assert_eq!(
        hits.len(),
        2,
        "detector must catch both planted tautologies, got: {hits:?}"
    );

    // And a real assertion of distinct operands must NOT be flagged.
    let real = r#"
        @test
        fn real_check() {
            let x = 1 + 1
            assert(x is 2)
            assert(false)
        }
    "#;
    assert!(
        tautological_tests_in(real).is_empty(),
        "real comparisons and unreachable-guard assert(false) must not be flagged"
    );
}
