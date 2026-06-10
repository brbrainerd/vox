//! P9 (2026-05-24) smoke test — production `emit_main` now embeds the
//! durable-functions boot prelude (HIR registration + `@scheduled` runner)
//! alongside the existing HTTP wiring. Before P9 these were two separate
//! `main()` emitters and generated axum binaries silently missed the
//! durable side.
//!
//! See `crates/vox-codegen/src/codegen_rust/emit/main_boot.rs` for the
//! shared `emit_durable_boot_prelude` / `emit_durable_boot_helpers` and
//! `crates/vox-codegen/src/codegen_rust/emit/http.rs::emit_main` for the
//! injection sites.

use vox_codegen::codegen_rust::emit::emit_main;
use vox_codegen::projection_bundle::project_bundle_from_hir;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn emit_main_registers_hir_module_and_scheduled_runner() {
    // A module that exercises both the durable side (`@scheduled`) and the
    // HTTP side (`@server`) — the whole point of P9 is that BOTH paths
    // get wired into one generated `main()`.
    let src = r#"
        @scheduled("5m")
        fn tick() to int { return 1 }

        @server
        fn hello() to str { return "hi" }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let bundle = project_bundle_from_hir(&hir);
    let main_rs = emit_main(&hir, "p9-smoke", &bundle.app);

    // HIR module is registered for `current_hir_module()` lookups
    // (ADR-041 §6(b)) — without this, workflow bodies panic at runtime.
    assert!(
        main_rs.contains("set_current_hir_module"),
        "emit_main must call set_current_hir_module — durable boot is wired",
    );
    assert!(
        main_rs.contains("load_hir_module_from_embedded"),
        "emit_main must emit the HIR-embed helper",
    );

    // `@scheduled` is registered and the scheduler is started.
    assert!(
        main_rs.contains("scheduled::register"),
        "emit_main must register @scheduled functions",
    );
    assert!(
        main_rs.contains("scheduled::start"),
        "emit_main must start the scheduler",
    );

    // The durable DB uses a distinct binding from the existing Codex `db`
    // produced by emit_db_setup — confirms no name collision.
    assert!(
        main_rs.contains("vox_durable_db"),
        "emit_main durable boot must bind a distinct `vox_durable_db` to \
         avoid colliding with the Codex `db` from emit_db_setup",
    );

    // The HTTP side still emits its routes (we didn't break anything).
    assert!(
        main_rs.contains("axum::serve"),
        "emit_main must still serve HTTP — durable boot is additive, \
         not a replacement",
    );
}

#[test]
fn emit_main_includes_durable_boot_even_without_scheduled() {
    // The prelude must be safe to call even when the module has no
    // `@scheduled` functions — HIR registration is still useful for
    // workflows (and harmless otherwise).
    let src = r#"
        @server
        fn ping() to str { return "pong" }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let bundle = project_bundle_from_hir(&hir);
    let main_rs = emit_main(&hir, "p9-smoke-noscheduled", &bundle.app);

    assert!(
        main_rs.contains("set_current_hir_module"),
        "HIR registration must still happen even without @scheduled fns",
    );
    assert!(
        main_rs.contains("load_hir_module_from_embedded"),
        "HIR-embed helper must still be emitted",
    );
    // No scheduler started — the prelude emits a comment in this branch.
    assert!(
        !main_rs.contains("scheduled::start"),
        "scheduler must not be started when no @scheduled fns are present",
    );
}

#[test]
fn emit_main_includes_healthz_for_db_backed_modules() {
    let src = r#"
        @table type Task {
            title: str
        }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let bundle = project_bundle_from_hir(&hir);
    let main_rs = emit_main(&hir, "p9-healthz", &bundle.app);

    assert!(
        main_rs.contains(".route(\"/healthz\", get(handle_healthz))"),
        "db-backed emit_main must expose /healthz"
    );
    assert!(
        main_rs.contains(".route(\"/readyz\", get(handle_healthz))"),
        "db-backed emit_main must expose /readyz"
    );
    assert!(
        main_rs.contains("evaluate_codex_api_readiness"),
        "health handler must run codex readiness evaluation"
    );
    assert!(
        main_rs.contains("async fn vox_health_probe_for_backend"),
        "health emit should route readiness through a backend probe abstraction"
    );
    assert!(
        main_rs.contains("vox_health_probe_for_backend(backend, db.as_ref()).await"),
        "health handler should dispatch via backend probe helper"
    );
    assert!(
        main_rs.contains(
            "generated Axum health/readiness probe does not yet include a backend-specific readiness evaluator"
        ),
        "health handler should emit a clear degraded message for backends without a probe yet"
    );
    assert!(
        main_rs.contains("fn vox_health_backend_kind() -> &'static str"),
        "health emit should include backend-kind helper for status payload"
    );
    assert!(
        main_rs.contains("generated Axum table runtime still boots Codex while backend-specific table dispatch is completed incrementally"),
        "db setup should warn (not hard fail) when VOX_APP_DB_URL points at non-libsql backends in this phase"
    );
    assert!(
        !main_rs.contains("generated Axum table runtime currently requires libsql/Codex"),
        "legacy hard-fail wording should be removed from Axum db setup emit"
    );
}
