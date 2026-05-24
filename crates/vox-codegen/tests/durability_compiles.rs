//! Phase 1 closure proof: the codegen output for `workflow` + `activity`
//! resolves against `vox-workflow-runtime`.
//!
//! Approach: **Option A + compile-link belt-and-suspenders**.
//!
//! Why this approach (and not a temp-cargo-crate or `trybuild`):
//! - The existing `durability_lowering.rs` test pattern already drives `emit_fn`
//!   and asserts on the emitted Rust. We reuse that pattern for speed.
//! - To also prove the *paths* codegen emits actually resolve at link time,
//!   the top of this file `use`s every Phase 1 runtime symbol at the exact
//!   public path the emitter writes (`::vox_workflow_runtime::workflow::*`
//!   and `::vox_workflow_runtime::journal::execute`). If any symbol is removed
//!   or relocated, this test file fails to *compile* — which is stronger than
//!   the string assertion and faster than spinning up a temp crate.
//! - `vox-workflow-runtime` is added as a `[dev-dependencies]` entry of
//!   `vox-codegen` solely to enable these `use` statements. It does not
//!   reach production codegen (which still emits strings, not links).
//!
//! Closes Phase 1 of
//! `docs/superpowers/plans/2026-05-23-durable-functions-completion.md`.

// --- Compile-link proof ---------------------------------------------------
// These `use` statements fail to compile if any Phase 1 symbol is removed,
// renamed, or moved away from the path codegen emits.
#[allow(unused_imports)]
use vox_workflow_runtime::journal::execute as _phase1_journal_execute;
#[allow(unused_imports)]
use vox_workflow_runtime::workflow::{
    current_hir_module as _phase1_current_hir_module,
    extract_terminal_return as _phase1_extract_terminal_return,
};
// `interpret_workflow_durable` and `DefaultTracker` are also written by the
// emitter; pin them too so a rename of either is caught at compile time.
#[allow(unused_imports)]
use vox_workflow_runtime::workflow::interpret_workflow_durable as _phase1_interpret_workflow_durable;
#[allow(unused_imports)]
use vox_workflow_runtime::workflow::tracker::DefaultTracker as _phase1_default_tracker;

// --- Emit-string proof ----------------------------------------------------
use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::{DurabilityKind, lower_module};
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

/// Workflow emit must call every Phase 1 runtime entry-point that
/// `durability_lower.rs` writes: `current_hir_module`,
/// `interpret_workflow_durable`, and `extract_terminal_return`.
#[test]
fn workflow_emit_references_all_phase_1_workflow_symbols() {
    let src = "workflow wf() to int { return 7 }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow))
        .expect("workflow lowered");
    let rust = emit_fn(func, Some(&hir.inferred_types), &[]);

    assert!(
        rust.contains("::vox_workflow_runtime::workflow::current_hir_module"),
        "workflow emit MUST call current_hir_module (Task 1.1); got:\n{rust}"
    );
    assert!(
        rust.contains("::vox_workflow_runtime::workflow::interpret_workflow_durable"),
        "workflow emit MUST call interpret_workflow_durable; got:\n{rust}"
    );
    assert!(
        rust.contains("::vox_workflow_runtime::workflow::extract_terminal_return"),
        "workflow emit MUST call extract_terminal_return (Task 1.2); got:\n{rust}"
    );
}

/// Activity emit must wrap its body in `journal::execute` (Task 1.3).
#[test]
fn activity_emit_references_journal_execute() {
    let src = "activity act() to int { return 9 }";
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Activity))
        .expect("activity lowered");
    let rust = emit_fn(func, Some(&hir.inferred_types), &[]);

    assert!(
        rust.contains("::vox_workflow_runtime::journal::execute"),
        "activity emit MUST call journal::execute (Task 1.3); got:\n{rust}"
    );
}
