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
    let rust = emit_fn(func, Some(&hir.inferred_types), &[]);
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
    let rust = emit_fn(func, Some(&hir.inferred_types), &[]);
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
    let rust = emit_fn(func, Some(&hir.inferred_types), &[]);
    assert!(
        rust.contains("vox_actor_runtime::spawn_process"),
        "actor MUST lower to vox_actor_runtime::spawn_process; got:\n{rust}"
    );
}

/// D3: the actor shell emits a real `Envelope` dispatch table that routes each
/// `on <event>` handler — the no-op `// dispatch not yet wired` marker is gone.
#[test]
fn actor_dispatch_table_routes_to_handlers() {
    let src = "actor MyActor { on greet(name: str) to str { return name } on tick(n: int) { return } }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let shell = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Actor) && !f.name.contains("::"))
        .expect("actor shell present");
    let handlers: Vec<&_> = hir
        .functions
        .iter()
        .filter(|f| f.name.starts_with("MyActor::"))
        .collect();
    let rust = emit_fn(shell, Some(&hir.inferred_types), &handlers);

    assert!(
        !rust.contains("dispatch not yet wired"),
        "the no-op dispatch marker must be gone; got:\n{rust}"
    );
    for needle in [
        "::vox_actor_runtime::Envelope::Message(__m)",
        "::vox_actor_runtime::Envelope::Request(__req)",
        "match __ev",
        "\"greet\" =>",
        "\"tick\" =>",
        "MyActor_greet(&mut _state, name)",
        "MyActor_tick(&mut _state, n)",
        "::vox_actor_runtime::ProcessContext::reply(__req",
    ] {
        assert!(
            rust.contains(needle),
            "dispatch table must contain `{needle}`; got:\n{rust}"
        );
    }
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
    let rust = emit_fn(func, Some(&hir.inferred_types), &[]);
    assert!(
        !rust.contains("interpret_workflow_durable"),
        "plain fn must not use workflow runtime"
    );
    assert!(
        !rust.contains("journal::execute"),
        "plain fn must not use journal execute"
    );
    assert!(
        !rust.contains("vox_actor_runtime::spawn_process"),
        "plain fn must not use actor spawn_process"
    );
}
