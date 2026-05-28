use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirFn, HirForall, HirModule, HirType};

use super::tables::{collect_table_select_projections, emit_table_struct};
use super::types::emit_type;

pub fn emit_lib(module: &HirModule) -> String {
    let mut out = String::new();
    out.push_str("use serde::{Serialize, Deserialize};\n");

    if !module.tables.is_empty() {
        out.push_str("use vox_db::Codex;\n");
    }

    out.push('\n');

    // Helper for casts
    out.push_str("pub fn as_string<T: serde::Serialize>(v: &T) -> String {\n");
    out.push_str("    let val = serde_json::to_value(v).expect(\"vox codegen: serde_json::to_value failed\");\n");
    out.push_str("    if let Some(s) = val.as_str() { s.to_string() } else { val.to_string() }\n");
    out.push_str("}\n\n");

    // Re-export variants (only for sum types — struct typedefs are top-level structs).
    for typedef in &module.types {
        if !typedef.variants.is_empty() {
            out.push_str(&format!("pub use self::{}::*;\n", typedef.name));
        }
    }

    // Types
    for typedef in &module.types {
        emit_typedef(typedef, &mut out);
    }

    // State Machines
    out.push_str(&super::state_machine::emit_state_machine_decls(module));

    // Actor state structs
    out.push_str(&emit_actor_state_structs(module));

    // Table structs
    let table_projections = collect_table_select_projections(module);
    for table in &module.tables {
        let projs = table_projections
            .get(&table.name)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        out.push_str(&emit_table_struct(table, projs));
    }

    for func in &module.functions {
        out.push_str(&emit_fn_with_actor_handlers(func, module));
    }

    // MCP tools and resources — must be `pub` so `mcp_server` binary can `use crate::*`.
    for t in &module.mcp_tools {
        let mut f = t.func.clone();
        f.is_pub = true;
        out.push_str(&emit_fn(&f, Some(&module.inferred_types), &[]));
    }
    for r in &module.mcp_resources {
        let mut f = r.func.clone();
        f.is_pub = true;
        out.push_str(&emit_fn(&f, Some(&module.inferred_types), &[]));
    }

    // Tests
    for test in &module.tests {
        if test.is_async {
            out.push_str("#[tokio::test]\n");
        } else {
            out.push_str("#[test]\n");
        }
        out.push_str(&emit_fn(test, Some(&module.inferred_types), &[]));
    }

    // Property-based Tests (@forall)
    for forall in &module.foralls {
        out.push_str(&emit_forall(forall, Some(&module.inferred_types)));
    }

    out
}

/// Emit a single HIR typedef (struct or ADT) as a Rust type definition.
/// Extracted from `emit_lib` per CR-A1: the two-branch if-chain inside the
/// for-loop contributed ~8 DPs (variants-empty/fields-empty, inner loops).
fn emit_typedef(typedef: &vox_compiler::hir::HirTypeDef, out: &mut String) {
    // Struct typedef → `pub struct Foo { pub f: T, ... }`.
    if typedef.variants.is_empty() && !typedef.fields.is_empty() {
        out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
        out.push_str(&format!("pub struct {} {{\n", typedef.name));
        for (fname, ftype) in &typedef.fields {
            out.push_str(&format!("    pub {}: {},\n", fname, emit_type(ftype)));
        }
        out.push_str("}\n\n");
        return;
    }
    // Sum type / ADT.
    out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
    out.push_str(&format!("pub enum {} {{\n", typedef.name));
    for variant in &typedef.variants {
        if variant.fields.is_empty() {
            out.push_str(&format!("    {},\n", variant.name));
        } else {
            out.push_str(&format!("    {}(", variant.name));
            for (_fname, ftype) in &variant.fields {
                out.push_str(&format!("{}, ", emit_type(ftype)));
            }
            out.push_str("),\n");
        }
    }
    out.push_str("}\n\n");
}

/// Emit a function together with its actor handler set (if it is an actor shell).
/// Extracted from `emit_lib` per CR-A1: the `if !func.actor_state_fields.is_empty()`
/// guard + the `filter` closure contributed ~3 DPs inside the for-loop.
fn emit_fn_with_actor_handlers(func: &HirFn, module: &vox_compiler::hir::HirModule) -> String {
    let handlers: Vec<&HirFn> = if !func.actor_state_fields.is_empty() {
        module
            .functions
            .iter()
            .filter(|f| f.name.starts_with(&format!("{}::", func.name)))
            .collect()
    } else {
        vec![]
    };
    emit_fn(func, Some(&module.inferred_types), &handlers)
}

fn emit_forall(forall: &HirForall, inferred_types: Option<&HashMap<Span, HirType>>) -> String {
    let mut out = String::new();
    out.push_str("proptest::proptest! {\n");
    if forall.iterations > 0 {
        out.push_str(&format!(
            "    #![proptest_config(proptest::prelude::ProptestConfig::with_cases({}))]\n",
            forall.iterations
        ));
    }
    out.push_str("    #[test]\n");
    // Indent the function emit to map inside the macro bounds cleanly
    let func_code = emit_fn(&forall.func, inferred_types, &[]);
    for line in func_code.lines() {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("}\n\n");
    out
}

/// Emit a single HIR function (or test) as Rust source.
pub fn emit_fn(
    func: &HirFn,
    inferred_types: Option<&HashMap<Span, HirType>>,
    actor_handlers: &[&HirFn],
) -> String {
    let mut out = String::new();
    let pub_kw = if func.is_pub { "pub " } else { "" };
    let async_kw = if func.is_async || func.is_llm {
        "async "
    } else {
        ""
    };
    out.push_str(&format!("{}{}fn {}(", pub_kw, async_kw, func.name.replace("::", "_")));
    if func.name.contains("::") {
        let actor_name = func.name.split("::").next().unwrap();
        out.push_str(&format!("state: &mut {}State, ", actor_name));
    }
    for param in &func.params {
        out.push_str(&format!(
            "{}: {}, ",
            param.name,
            emit_type(
                param
                    .type_ann
                    .as_ref()
                    .unwrap_or(&HirType::Named("serde_json::Value".into()))
            )
        ));
    }
    out.push_str(") ");
    if let Some(ret) = &func.return_type {
        out.push_str(&format!("-> {} ", emit_type(ret)));
    }
    out.push_str("{\n");
    if func.is_llm {
        super::ai_fixture::emit_llm_function_body(&mut out, func);
    } else {
        let usage = super::usage::UsageTracker::build(&func.body);
        out.push_str(&super::durability_lower::emit_durable_body(
            func,
            inferred_types,
            Some(&usage),
            actor_handlers,
        ));
    }
    out.push_str("}\n\n");
    out
}

fn emit_actor_state_structs(module: &HirModule) -> String {
    use vox_compiler::hir::DurabilityKind;
    let mut out = String::new();
    for func in &module.functions {
        // Emit a state struct for every actor SHELL — i.e. an actor
        // function whose name has no `::` (handlers are named
        // "ActorName::event"). The emit_actor_body lowering refers to
        // `<ActorName>State::default()` unconditionally; without a
        // struct definition, rustc errors with E0412 / E0433. State
        // fields are optional in the Vox surface — when absent, emit
        // a unit-like struct so `::default()` still resolves. Per the
        // 2026-05-23 slot-3 chat bring-up.
        let is_actor_shell = matches!(func.durability, Some(DurabilityKind::Actor))
            && !func.name.contains("::");
        if !is_actor_shell {
            continue;
        }
        if func.actor_state_fields.is_empty() {
            out.push_str(&format!(
                "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]\npub struct {}State;\n\n",
                func.name
            ));
        } else {
            out.push_str(&format!(
                "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]\npub struct {}State {{\n",
                func.name
            ));
            for field in &func.actor_state_fields {
                out.push_str(&format!(
                    "    pub {}: {},\n",
                    field.name,
                    super::types::emit_type(&field.type_ann)
                ));
            }
            out.push_str("}\n\n");
        }
    }
    out
}
