//! P5: `@versioned` / `@tracked` decorator — parse, lower, capability injection,
//! and interpreter auto-snapshot-on-success acceptance tests.

use vox_compiler::ast::decl::Decl;
use vox_compiler::eval::Interpreter;
use vox_compiler::eval::value::VoxValue;
use vox_compiler::hir::HirCapability;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::{parse, parse_script};

fn interp_for(src: &str) -> Interpreter {
    let module = lower_module(&parse_script(lex(src)).expect("parse_script"));
    let mut interp = Interpreter::new(1_000_000);
    interp.run_module(&module).expect("run_module");
    interp
}

#[test]
fn parses_versioned_decorator_sets_flag() {
    let src = "@versioned fn save() uses vcs { repo.snapshot(\"x\") }";
    let m = parse(lex(src)).expect("parse");
    let f = m
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "save" => Some(f),
            _ => None,
        })
        .expect("fn save");
    assert!(f.is_versioned, "@versioned must set FnDecl.is_versioned");
}

#[test]
fn parses_tracked_alias_sets_same_flag() {
    let src = "@tracked fn save() uses vcs { repo.snapshot(\"x\") }";
    let m = parse(lex(src)).expect("parse");
    let f = m
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "save" => Some(f),
            _ => None,
        })
        .expect("fn save");
    assert!(f.is_versioned, "@tracked must set FnDecl.is_versioned");
}

// ── Task 3: HIR + VoxValue::Fn carry the flag ──────────────────────────────

#[test]
fn versioned_fn_value_carries_flag() {
    let interp = interp_for("@versioned fn save() uses vcs { repo.snapshot(\"x\") }");
    match interp.scope.get("save") {
        Some(VoxValue::Fn {
            is_versioned, name, ..
        }) => {
            assert!(*is_versioned, "registered fn must carry is_versioned");
            assert_eq!(name, "save");
        }
        other => panic!("expected versioned Fn, got {other:?}"),
    }
}

// ── Task 4: @versioned implies `uses vcs` (capability injection) ────────────

#[test]
fn versioned_decorator_grants_vcs_capability() {
    // No explicit `uses vcs` clause — the decorator supplies it.
    let src = "@versioned fn save() { repo.snapshot(\"x\") }";
    let hir = lower_module(&parse(lex(src)).expect("parse"));
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "save")
        .expect("fn save");
    assert!(f.is_versioned, "HirFn must carry is_versioned");
    assert!(
        f.capabilities.contains(&HirCapability::Vcs),
        "@versioned must imply uses vcs; caps = {:?}",
        f.capabilities
    );
    // The effect checker must report no E_EFFECT violation for the bare repo.* call.
    let diags = vox_compiler::typeck::effect_check::check_effect_compliance(&hir, src);
    assert!(
        diags.iter().all(|d| !format!("{d:?}").contains("E_EFFECT")),
        "no effect violation expected, got {diags:?}"
    );
}

// ── Task 5: interpreter auto-snapshot on successful return ──────────────────

#[test]
fn versioned_fn_auto_snapshots_on_success() {
    // The body does NO explicit repo.snapshot — the decorator supplies it.
    let mut interp = interp_for("@versioned fn save() { let x = 1 }\nfn main() { save() }");
    interp.call("main", vec![]).unwrap();
    let changes = interp.repo.changes();
    assert_eq!(
        changes.len(),
        1,
        "exactly one auto-snapshot per @versioned call"
    );
    assert_eq!(changes[0].label.as_deref(), Some("@versioned save"));
}

#[test]
fn non_versioned_fn_does_not_auto_snapshot() {
    let mut interp = interp_for("fn save() { let x = 1 }\nfn main() { save() }");
    interp.call("main", vec![]).unwrap();
    assert!(
        interp.repo.changes().is_empty(),
        "no decorator -> no auto-snapshot"
    );
}

#[test]
fn versioned_fn_error_records_no_snapshot() {
    // assert(false) raises; the snapshot hook is after the body loop and must be skipped.
    let mut interp = interp_for("@versioned fn save() { assert(false) }\nfn main() { save() }");
    let _ = interp.call("main", vec![]); // expected Err
    assert!(
        interp.repo.changes().is_empty(),
        "a failing @versioned call must not leave a checkpoint"
    );
}

#[test]
fn nested_versioned_calls_each_snapshot_once() {
    let mut interp = interp_for(
        "@versioned fn inner() { let x = 1 }\n\
         @versioned fn outer() { inner() }\n\
         fn main() { outer() }",
    );
    interp.call("main", vec![]).unwrap();
    let labels: Vec<_> = interp
        .repo
        .changes()
        .iter()
        .map(|c| c.label.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        labels,
        vec![
            "@versioned inner".to_string(),
            "@versioned outer".to_string()
        ],
        "inner snapshots before outer (snapshot-on-success ordering)"
    );
}
