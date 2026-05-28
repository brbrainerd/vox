//! Exhaustiveness and structural checks for `state_machine` declarations (TASK-4.1).
//!
//! Checks performed on each non-`partial` machine:
//! 1. No duplicate state names.
//! 2. No duplicate (from, event) pairs (ambiguous transitions).
//! 3. Terminal states have no outgoing transitions.
//! 4. Every terminal state is reachable from at least one non-terminal state.
//! 5. Every (non-terminal state, event) pair has a covering transition (`from Named` or `from any`).

use std::collections::{HashMap, HashSet};

use crate::hir::{HirModule, HirSmFrom, HirStateMachineDecl};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory};

pub fn check_state_machines(module: &HirModule, source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for sm in &module.state_machines {
        check_one(sm, source, &mut diags);
    }
    diags
}

fn check_one(sm: &HirStateMachineDecl, source: &str, diags: &mut Vec<Diagnostic>) {
    // ── 1. No duplicate state names ───────────────────────────────────────
    let mut seen_states: HashSet<&str> = HashSet::new();
    for st in &sm.states {
        if !seen_states.insert(st.name.as_str()) {
            diags.push(err(
                format!("State machine `{}`: duplicate state `{}`", sm.name, st.name),
                sm.span,
                source,
            ));
        }
    }

    let terminal_names: HashSet<&str> = sm
        .states
        .iter()
        .filter(|s| s.is_terminal)
        .map(|s| s.name.as_str())
        .collect();

    let non_terminal_names: Vec<&str> = sm
        .states
        .iter()
        .filter(|s| !s.is_terminal)
        .map(|s| s.name.as_str())
        .collect();

    // ── 2a. Unknown `from` states ─────────────────────────────────────────────
    for tr in &sm.transitions {
        if let HirSmFrom::Named(from_state) = &tr.from {
            if !seen_states.contains(from_state.as_str()) {
                let mut d = Diagnostic::error(
                    format!(
                        "State machine `{}`: transition references unknown state `{}`",
                        sm.name, from_state
                    ),
                    tr.span,
                    source,
                );
                d.category = DiagnosticCategory::StateMachineCheck;
                d.code = Some("E_SM_UNKNOWN_STATE".to_string());
                diags.push(d);
            }
        }
    }

    // ── 2. No duplicate (from, event) pairs ───────────────────────────────
    let mut seen_pairs: HashSet<(String, &str)> = HashSet::new();
    for tr in &sm.transitions {
        let key = match &tr.from {
            HirSmFrom::Named(s) => s.clone(),
            HirSmFrom::Any => "__any__".to_string(),
        };
        let pair = (key, tr.event_name.as_str());
        if !seen_pairs.insert(pair) {
            diags.push(err(
                format!(
                    "State machine `{}`: duplicate transition `on {} from {:?}`",
                    sm.name, tr.event_name, tr.from
                ),
                tr.span,
                source,
            ));
        }
    }

    // ── 3. Terminal states have no outgoing transitions ───────────────────
    for tr in &sm.transitions {
        if let HirSmFrom::Named(from_state) = &tr.from {
            if terminal_names.contains(from_state.as_str()) {
                diags.push(err(
                    format!(
                        "State machine `{}`: terminal state `{}` cannot have outgoing transition (event `{}`)",
                        sm.name, from_state, tr.event_name
                    ),
                    tr.span,
                    source,
                ));
            }
        }
    }

    // ── 4. Every terminal state is reachable ──────────────────────────────
    let reachable_targets: HashSet<&str> = sm
        .transitions
        .iter()
        .map(|tr| tr.to_state.as_str())
        .collect();

    for term in &terminal_names {
        if !reachable_targets.contains(*term) {
            diags.push(err(
                format!(
                    "State machine `{}`: terminal state `{}` is unreachable — no transition leads to it",
                    sm.name, term
                ),
                sm.span,
                source,
            ));
        }
    }

    // ── 5. (Non-terminal state, event) exhaustiveness ─────────────────────
    if sm.is_partial {
        return; // `partial state_machine` explicitly opts out.
    }

    // Collect all unique event names declared across all transitions.
    let all_events: HashSet<&str> = sm
        .transitions
        .iter()
        .map(|tr| tr.event_name.as_str())
        .collect();

    // Build a coverage map: state_name → set of covered events.
    let mut covered: HashMap<&str, HashSet<&str>> = HashMap::new();
    for non_term in &non_terminal_names {
        covered.insert(non_term, HashSet::new());
    }

    for tr in &sm.transitions {
        for event in &all_events {
            if tr.event_name.as_str() != *event {
                continue;
            }
            match &tr.from {
                HirSmFrom::Any => {
                    // Covers every non-terminal state for this event.
                    for non_term in &non_terminal_names {
                        if let Some(set) = covered.get_mut(*non_term) {
                            set.insert(event);
                        }
                    }
                }
                HirSmFrom::Named(state_name) => {
                    if let Some(set) = covered.get_mut(state_name.as_str()) {
                        set.insert(event);
                    }
                }
            }
        }
    }

    for (state, covered_events) in &covered {
        for event in &all_events {
            if !covered_events.contains(*event) {
                diags.push(err(
                    format!(
                        "State machine `{}`: non-exhaustive — state `{}` has no transition for event `{}`. \
                         Add `on {} from {} -> …` or use `partial state_machine` to allow gaps.",
                        sm.name, state, event, event, state
                    ),
                    sm.span,
                    source,
                ));
            }
        }
    }
}

fn err(message: String, span: crate::ast::span::Span, source: &str) -> Diagnostic {
    let mut d = Diagnostic::error(message, span, source);
    d.category = DiagnosticCategory::StateMachineCheck;
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::lower::lower_module;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn check(src: &str) -> Vec<Diagnostic> {
        let tokens = lex(src);
        let module = parse(tokens).expect("parse error");
        let hir = lower_module(&module);
        check_state_machines(&hir, src)
    }

    const SIMPLE_MACHINE: &str = "
state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
  on Toggle from Off -> On
}";

    #[test]
    fn test_complete_two_state_machine_ok() {
        let diags = check(SIMPLE_MACHINE);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_missing_transition_is_error() {
        let src = "
state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
}";
        let diags = check(src);
        // Off has no transition for Toggle
        assert!(!diags.is_empty(), "expected exhaustiveness error");
        assert!(diags[0].message.contains("Off"));
        assert!(diags[0].message.contains("Toggle"));
    }

    #[test]
    fn test_partial_machine_skips_exhaustiveness() {
        let src = "
partial state_machine Light {
  state On
  state Off

  on Toggle from On -> Off
}";
        let diags = check(src);
        assert!(
            diags.is_empty(),
            "partial machine should skip check: {diags:?}"
        );
    }

    #[test]
    fn test_terminal_state_unreachable_is_error() {
        let src = "
state_machine Door {
  state Open
  terminal state Destroyed

  on Close from Open -> Open
}";
        let diags = check(src);
        let unreachable = diags.iter().any(|d| d.message.contains("unreachable"));
        assert!(
            unreachable,
            "expected unreachable terminal error: {diags:?}"
        );
    }

    #[test]
    fn test_terminal_state_with_outgoing_transition_is_error() {
        let src = "
partial state_machine Door {
  state Open
  terminal state Locked

  on Break from Locked -> Open
}";
        let diags = check(src);
        let terminal_err = diags.iter().any(|d| d.message.contains("terminal"));
        assert!(terminal_err, "expected terminal-outgoing error: {diags:?}");
    }

    #[test]
    fn test_from_any_covers_all_states() {
        let src = "
state_machine Lifecycle {
  state Active
  state Idle
  terminal state Done

  on Stop from any -> Done
}";
        let diags = check(src);
        // `from any` covers Active and Idle for Stop; Done is terminal and reachable
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_duplicate_state_name_is_error() {
        let src = "
partial state_machine Broken {
  state A
  state A
}";
        let diags = check(src);
        assert!(!diags.is_empty(), "expected duplicate state error");
        assert!(diags[0].message.contains("duplicate"));
    }

    #[test]
    fn transition_from_undeclared_state_produces_error() {
        use crate::ast::span::Span;
        use crate::hir::nodes::{HirSmState, HirSmTransition, HirStateMachineDecl};
        use crate::hir::{DefId, HirModule, HirSmFrom};

        let sp = Span::new(0, 0);

        let make_state = |name: &str, is_terminal: bool| HirSmState {
            name: name.to_string(),
            fields: vec![],
            is_terminal,
            span: sp,
        };

        let make_transition = |event: &str, from: HirSmFrom, to: &str| HirSmTransition {
            event_name: event.to_string(),
            event_params: vec![],
            from,
            to_state: to.to_string(),
            span: sp,
        };

        // The `from` state "Ghost" is not declared — should produce E_SM_UNKNOWN_STATE.
        let mut module = HirModule::default();
        module.state_machines.push(HirStateMachineDecl {
            id: DefId(0),
            name: "Bad".to_string(),
            states: vec![make_state("Idle", false), make_state("Done", false)],
            transitions: vec![make_transition(
                "Phantom",
                HirSmFrom::Named("Ghost".to_string()),
                "Done",
            )],
            is_partial: false,
            is_pub: false,
            span: sp,
        });
        let diags = check_state_machines(&module, "");
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("E_SM_UNKNOWN_STATE")),
            "Expected E_SM_UNKNOWN_STATE for unknown `from` state, got: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("Ghost")),
            "Diagnostic should name the unknown state 'Ghost'"
        );
    }
}
