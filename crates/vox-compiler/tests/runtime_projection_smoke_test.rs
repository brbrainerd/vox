//! Runtime projection + canonical JSON stability (IR unification).

use vox_codegen::syntax_k::{canonical_web_ir_bytes, sha3_hex, sort_json_value_keys};
use vox_compiler::hir::lower_module;
use vox_compiler::parser::parse;
use vox_compiler::runtime_projection::{
    canonical_runtime_projection_bytes, project_runtime_from_core,
};

fn lower_src(src: &str) -> vox_compiler::hir::TypedCoreIR_v2 {
    let tokens = vox_compiler::lexer::lex(src);
    let module = parse(tokens).expect("parse");
    lower_module(&module)
}

#[test]
fn canonical_web_ir_bytes_sorted_is_stable() {
    let src = r#"
import react.use_state

fn Dash() to str {
    let n = 0
    return "div"
}

routes {
    "/" to Dash
}
"#;
    let hir = lower_src(src);
    let web = vox_codegen::web_ir::lower::project_web_from_core(&hir);
    let a = sha3_hex(&canonical_web_ir_bytes(&web).expect("web ir json"));
    let b = sha3_hex(&canonical_web_ir_bytes(&web).expect("web ir json"));
    assert_eq!(a, b, "canonical web IR hash must be stable across calls");
}

#[test]
fn runtime_projection_sort_json_roundtrip() {
    let src = r#"
table T { a: str }

fn f() to Unit {
    db.T.all().sync()
}
"#;
    let hir = lower_src(src);
    let rp = project_runtime_from_core(&hir);
    let bytes = canonical_runtime_projection_bytes(&rp).expect("runtime json");
    let mut v = serde_json::from_slice::<serde_json::Value>(&bytes).expect("parse");
    sort_json_value_keys(&mut v);
    let again = serde_json::to_vec(&v).expect("serialize");
    assert_eq!(bytes, again);
}

#[test]
fn module_task_capability_hints_inferred_from_db_using_and_scope() {
    let src = r#"
table T { a: str }

fn q() to Unit {
    db.T.all().using("vector").scope("training").sync()
}
"#;
    let hir = lower_src(src);
    let rp = project_runtime_from_core(&hir);
    let hints = rp
        .module_task_capability_hints
        .as_ref()
        .expect("expected inferred hints");
    assert!(hints.prefer_gpu_compute);
    assert!(
        hints
            .labels
            .contains(&"vox:db_retrieval:vector".to_string())
    );
    assert!(
        hints
            .labels
            .contains(&"vox:orchestration_scope:training".to_string())
    );
}

#[test]
fn module_task_capability_hints_none_without_db_metadata() {
    let src = r#"
fn f() to int { return 1 }
"#;
    let hir = lower_src(src);
    let rp = project_runtime_from_core(&hir);
    assert!(rp.module_task_capability_hints.is_none());
}
