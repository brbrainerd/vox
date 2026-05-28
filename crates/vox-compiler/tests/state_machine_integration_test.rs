use vox_codegen::codegen_ts::state_machine_emit::emit_state_machine_decls;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::{diagnostics::TypeckSeverity, typecheck_hir_module};

fn parse_and_lower(src: &str) -> vox_compiler::hir::HirModule {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse failed");
    lower_module(&module)
}

// ── Parser round-trip ─────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_parse_simple_state_machine() {
    let src = "
state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
  on Toggle from Off -> On
}";
    let hir = parse_and_lower(src);
    assert_eq!(hir.state_machines.len(), 1);
    let sm = &hir.state_machines[0];
    assert_eq!(sm.name, "Light");
    assert_eq!(sm.states.len(), 2);
    assert_eq!(sm.transitions.len(), 2);
    assert!(!sm.is_partial);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_parse_partial_state_machine() {
    let src = "
partial state_machine Lifecycle {
  state Active
  state Idle
  terminal state Done

  on Stop from any -> Done
}";
    let hir = parse_and_lower(src);
    assert_eq!(hir.state_machines.len(), 1);
    let sm = &hir.state_machines[0];
    assert!(sm.is_partial);
    let done = sm.states.iter().find(|s| s.name == "Done").unwrap();
    assert!(done.is_terminal);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_parse_state_with_fields() {
    let src = "
state_machine Counter {
  state Active(count: int)
  terminal state Done

  on Finish from Active -> Done
}";
    let hir = parse_and_lower(src);
    let sm = &hir.state_machines[0];
    let active = sm.states.iter().find(|s| s.name == "Active").unwrap();
    assert_eq!(active.fields.len(), 1);
    assert_eq!(active.fields[0].name, "count");
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_parse_event_with_params() {
    let src = "
partial state_machine Flow {
  state Idle
  state Running

  on Start(task_id) from Idle -> Running
}";
    let hir = parse_and_lower(src);
    let sm = &hir.state_machines[0];
    let tr = sm
        .transitions
        .iter()
        .find(|t| t.event_name == "Start")
        .unwrap();
    assert_eq!(tr.event_params, vec!["task_id"]);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_parse_from_any() {
    let src = "
state_machine Lifecycle {
  state Active
  state Idle
  terminal state Done

  on Stop from any -> Done
}";
    let hir = parse_and_lower(src);
    let sm = &hir.state_machines[0];
    let tr = &sm.transitions[0];
    assert!(matches!(tr.from, vox_compiler::hir::HirSmFrom::Any));
}

// ── Type-checker integration ───────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_typecheck_complete_machine_no_errors() {
    let src = "
state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
  on Toggle from Off -> On
}";
    let mut hir = parse_and_lower(src);
    let diags = typecheck_hir_module(src, &mut hir);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_typecheck_non_exhaustive_machine_is_error() {
    let src = "
state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
}";
    let mut hir = parse_and_lower(src);
    let diags = typecheck_hir_module(src, &mut hir);
    let error_msgs: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .map(|d| d.message.as_str())
        .collect();
    assert!(!error_msgs.is_empty(), "expected exhaustiveness error");
    assert!(
        error_msgs
            .iter()
            .any(|m| m.contains("Off") && m.contains("Toggle"))
    );
}

// ── Codegen emit ──────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_emit_includes_state_machines_ts() {
    let src = "
state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
  on Toggle from Off -> On
}";
    let hir = parse_and_lower(src);
    let out = emit_state_machine_decls(&hir);
    insta::assert_snapshot!("light_state_machine_ts_emit", out);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_emit_terminal_state_not_in_reducer_switch() {
    let src = "
partial state_machine Door {
  state Open
  terminal state Locked

  on Lock from Open -> Locked
}";
    let hir = parse_and_lower(src);
    let out = emit_state_machine_decls(&hir);
    // Locked is terminal — the reducer should not emit a case for it
    insta::assert_snapshot!("door_terminal_state_machine_emit", out);
    let reducer_start = out.find("switch (state._tag)").unwrap();
    let reducer_body = &out[reducer_start..];
    assert!(
        !reducer_body.contains("case \"Locked\""),
        "terminal state must not appear in reducer switch"
    );
}
