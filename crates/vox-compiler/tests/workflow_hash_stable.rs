//! P2-T1 acceptance: `generated_hash` on `HirFn` is stable across whitespace
//! changes and changes on body edits.

use vox_compiler::hir::{DurabilityKind, lower_module};
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn first_workflow_hash(src: &str) -> String {
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    hir.functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow))
        .and_then(|f| f.generated_hash.clone())
        .expect("workflow should have generated_hash")
}

#[test]
fn workflow_hash_is_stable_across_whitespace() {
    let src_a = "workflow wf() to int { return 7 }";
    let src_b = "workflow wf()   to  int  {  return 7  }";
    let h1 = first_workflow_hash(src_a);
    let h2 = first_workflow_hash(src_b);
    assert_eq!(h1, h2, "whitespace must not affect the generated hash");
}

#[test]
fn workflow_hash_changes_on_body_change() {
    let h1 = first_workflow_hash("workflow wf() to int { return 7 }");
    let h2 = first_workflow_hash("workflow wf() to int { return 8 }");
    assert_ne!(h1, h2, "body change MUST bust the hash");
}

#[test]
fn activity_also_gets_generated_hash() {
    let src = "activity act() to str { return \"ok\" }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let act = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Activity))
        .expect("activity present");
    assert!(
        act.generated_hash.is_some(),
        "activity must have generated_hash; got None"
    );
}
