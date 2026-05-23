//! P2-T7 acceptance: emit_fn lowers DurabilityKind to distinct runtime call shapes.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::{DurabilityKind, lower_module};
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn workflow_lowers_to_interpret_workflow_durable() {
    let src = "workflow wf() to int { return 7 }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow))
        .expect("workflow present");
    let rust = emit_fn(func, None, &[]);
    assert!(
        rust.contains("interpret_workflow_durable"),
        "workflow MUST lower to interpret_workflow_durable; got:\n{rust}"
    );
}

#[test]
fn activity_lowers_to_journal_execute() {
    let src = "activity act() to int { return 9 }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Activity))
        .expect("activity present");
    let rust = emit_fn(func, None, &[]);
    assert!(
        rust.contains("journal::execute"),
        "activity MUST lower to journal::execute; got:\n{rust}"
    );
    assert!(
        rust.contains("activity_id") || rust.contains("journal::execute"),
        "activity body must reference journal::execute; got:\n{rust}"
    );
}

#[test]
fn actor_lowers_to_mailbox_spawn() {
    let src = "actor MyActor { on greet(name: str) to str { return name } }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Actor))
        .expect("actor handler present");
    let rust = emit_fn(func, None, &[]);
    // The codegen targets vox_actor_runtime::spawn_process (the
    // current public entrypoint re-exported by vox-actor-runtime
    // crate root, see process.rs::spawn_process). The earlier
    // assertion `mailbox::spawn` was a pre-existing test/code drift
    // that pre-dated the spawn_process API consolidation.
    assert!(
        rust.contains("spawn_process"),
        "actor MUST lower to vox_actor_runtime::spawn_process; got:\n{rust}"
    );
}

#[test]
fn plain_fn_unchanged() {
    let src = "fn add(a: int, b: int) to int { return a + b }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability.is_none())
        .expect("plain fn present");
    let rust = emit_fn(func, None, &[]);
    assert!(
        !rust.contains("interpret_workflow_durable"),
        "plain fn must not use workflow runtime"
    );
    assert!(
        !rust.contains("journal::execute"),
        "plain fn must not use journal execute"
    );
    assert!(
        !rust.contains("mailbox::spawn"),
        "plain fn must not use mailbox spawn"
    );
}
