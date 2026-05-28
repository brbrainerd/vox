//! Phase 5.1 — snapshot the `emit_main_boot` output to lock the generated
//! binary's `main()` shape.
//!
//! Two snapshots cover the meaningful branches:
//! 1. A module with one `@scheduled` fn, one `actor`, and one `@server`
//!    endpoint — exercises the scheduler-register + start path AND the
//!    HTTP-endpoints TODO branch.
//! 2. A module with no `@scheduled` and no endpoints — exercises the
//!    "no scheduler started, no HTTP server" branch so the empty-module
//!    behavior is locked too.
//!
//! See `docs/superpowers/plans/2026-05-23-durable-functions-completion.md`
//! (Task 5.1) for the parent plan.

use insta::assert_snapshot;
use vox_codegen::codegen_rust::emit::emit_main_boot;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn emit_main_boot_with_scheduled_actor_server() {
    let src = r#"
        @scheduled("1m")
        fn tick() to int { return 1 }

        actor Counter {
            on inc() to int { return 1 }
        }

        @server
        fn hello() to str { return "hi" }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let main_rs = emit_main_boot(&hir);
    assert_snapshot!("main_boot_scheduled_actor_server", main_rs);
}

#[test]
fn emit_main_boot_with_no_scheduled_or_endpoint() {
    // A module that does nothing requiring boot-time wiring: no @scheduled,
    // no @query/@mutation/@server, no actor. The generated main() should
    // still compile-shape: DB init + ctrl-C + Ok(()). No `scheduled_handle`
    // is referenced because none was bound.
    let src = r#"
        fn plain() to int { return 1 }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let main_rs = emit_main_boot(&hir);
    assert_snapshot!("main_boot_no_scheduled_no_endpoints", main_rs);
}
