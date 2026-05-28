//! Rust emission for `state_machine` declarations (TASK-4.1).
//!
//! Mirrors `codegen_ts/state_machine_emit.rs` to ensure server-side parity
//! and eliminate the split-plane issue for stateful logic.

use super::types::emit_type;
use vox_compiler::hir::{HirModule, HirSmFrom, HirStateMachineDecl};

/// Emit Rust types and reducer functions for all state machines in the module.
pub fn emit_state_machine_decls(hir: &HirModule) -> String {
    let mut out = String::new();
    for sm in &hir.state_machines {
        emit_one(sm, &mut out);
        out.push('\n');
    }
    out
}

fn emit_one(sm: &HirStateMachineDecl, out: &mut String) {
    let name = &sm.name;

    // ── State Enum ─────────────────────────────────────────────────────────
    out.push_str("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n");
    out.push_str(&format!("pub enum {} {{\n", name));
    for st in &sm.states {
        if st.fields.is_empty() {
            out.push_str(&format!("    {},\n", st.name));
        } else {
            out.push_str(&format!("    {} {{\n", st.name));
            for field in &st.fields {
                let ty_str = field
                    .ty
                    .as_ref()
                    .map(emit_type)
                    .unwrap_or_else(|| "serde_json::Value".to_string());
                out.push_str(&format!("        {}: {},\n", field.name, ty_str));
            }
            out.push_str("    },\n");
        }
    }
    out.push_str("}\n\n");

    // ── Event Enum ─────────────────────────────────────────────────────────
    let mut seen_events: Vec<String> = Vec::new();
    for tr in &sm.transitions {
        if !seen_events.contains(&tr.event_name) {
            seen_events.push(tr.event_name.clone());
        }
    }

    if !seen_events.is_empty() {
        out.push_str("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n");
        out.push_str(&format!("pub enum {}Event {{\n", name));
        for event in &seen_events {
            // Collect params for this event from the first matching transition.
            let params: Vec<&str> = sm
                .transitions
                .iter()
                .filter(|tr| &tr.event_name == event)
                .flat_map(|tr| tr.event_params.iter().map(String::as_str))
                .collect();

            if params.is_empty() {
                out.push_str(&format!("    {},\n", event));
            } else {
                out.push_str(&format!("    {} {{\n", event));
                for param in &params {
                    out.push_str(&format!("        {}: serde_json::Value,\n", param));
                }
                out.push_str("    },\n");
            }
        }
        out.push_str("}\n\n");

        // ── Reducer Function ───────────────────────────────────────────────
        emit_reducer_fn(out, sm, name);
    }
}

/// Emit the `{name}_reducer(state, event) -> {name}` function.
/// Extracted from `emit_one` per CR-A1: the match-state + nested match-event
/// loops contributed ~10 DPs inline.
fn emit_reducer_fn(out: &mut String, sm: &HirStateMachineDecl, name: &str) {
    let reducer_name = to_snake_case(name);
    out.push_str(&format!(
        "pub fn {}_reducer(state: {}, event: {}Event) -> {} {{\n",
        reducer_name, name, name, name
    ));
    out.push_str("    match state {\n");
    for st in &sm.states {
        if st.is_terminal {
            out.push_str(&format!(
                "        {}::{} {{ .. }} => state,\n",
                name, st.name
            ));
            continue;
        }
        let fields_suffix = if st.fields.is_empty() { "" } else { " { .. }" };
        out.push_str(&format!(
            "        {}::{}{} => match event {{\n",
            name, st.name, fields_suffix
        ));
        let state_transitions: Vec<_> = sm
            .transitions
            .iter()
            .filter(|tr| {
                matches!(&tr.from, HirSmFrom::Named(n) if n == &st.name)
                    || matches!(&tr.from, HirSmFrom::Any)
            })
            .collect();
        for tr in &state_transitions {
            let event_fields_suffix = if tr.event_params.is_empty() {
                ""
            } else {
                " { .. }"
            };
            out.push_str(&format!(
                "            {}Event::{}{} => {}::{},\n",
                name, tr.event_name, event_fields_suffix, name, tr.to_state
            ));
        }
        out.push_str("            _ => state,\n");
        out.push_str("        },\n");
    }
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_lowercase().next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::hir::lower::lower_module;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse;

    fn emit(src: &str) -> String {
        let tokens = lex(src);
        let module = parse(tokens).expect("parse error");
        let hir = lower_module(&module);
        emit_state_machine_decls(&hir)
    }

    #[test]
    fn test_emit_rust_state_machine() {
        let out = emit(
            "state_machine Light { state On\nstate Off\non Toggle from On -> Off\non Toggle from Off -> On\n}",
        );
        assert!(out.contains("pub enum Light {"));
        assert!(out.contains("On,"));
        assert!(out.contains("Off,"));
        assert!(out.contains("pub enum LightEvent {"));
        assert!(out.contains("Toggle,"));
        assert!(out.contains("pub fn light_reducer"));
        assert!(out.contains("match state {"));
        assert!(out.contains("Light::On => match event {"));
        assert!(out.contains("LightEvent::Toggle => Light::Off,"));
    }

    #[test]
    fn test_emit_rust_state_machine_with_fields() {
        let out =
            emit("state_machine Counter { state Count(val: int)\non Inc from Count -> Count\n}");
        assert!(out.contains("Count {"));
        assert!(out.contains("val: i64,"));
        assert!(out.contains("Counter::Count { .. } => match event {"));
    }
}
