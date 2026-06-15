use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirConst, HirFn, HirForall, HirModule, HirType};

use super::script_db;
use super::tables::{collect_table_select_projections, emit_table_struct};
use super::types::emit_type;

/// Script-mode lib.rs: optional Codex prelude + mit_lib under script DB emit mode.
pub fn emit_script_lib(module: &HirModule) -> String {
    script_db::refresh_script_async_metadata(module);
    let mut out = String::new();
    if script_db::module_uses_db(module) {
        out.push_str(&script_db::emit_script_db_prelude(module));
    }
    out.push_str(&script_db::with_script_db_emit_mode(|| emit_lib(module)));
    out
}

pub fn emit_lib(module: &HirModule) -> String {
    let mut out = String::new();
    out.push_str("use serde::{Serialize, Deserialize};\n");

    if !module.tables.is_empty() && !script_db::script_db_emit_mode() {
        out.push_str("use vox_db::Codex;\n");
    }

    if super::types::module_uses_vox_json_type(module) {
        out.push_str(&super::types::vox_json_type_alias_prelude());
    }

    out.push('\n');

    // Helper for casts
    out.push_str("pub fn as_string<T: serde::Serialize>(v: &T) -> String {\n");
    out.push_str("    let val = serde_json::to_value(v).expect(\"vox codegen: serde_json::to_value failed\");\n");
    out.push_str("    if let Some(s) = val.as_str() { return s.to_string(); }\n");
    // Vox stringifies whole-number floats without the trailing `.0`
    // (`str(5.0) == \"5\"`); match the interpreter. Only f64 values are touched.
    out.push_str("    if val.is_f64() {\n");
    out.push_str("        if let Some(f) = val.as_f64() {\n");
    out.push_str(
        "            if f.is_finite() && f.fract() == 0.0 { return format!(\"{}\", f as i64); }\n",
    );
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    val.to_string()\n");
    out.push_str("}\n\n");

    // Module-level constants
    for c in &module.consts {
        out.push_str(&emit_const(c));
    }

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

/// Emit a single `HirConst` as a Rust `const` declaration.
///
/// Rust `const` items require a concrete type — `_` is forbidden (E0121).
/// When no type annotation is present we infer from the literal kind.
/// String consts use `&'static str` with a borrowed literal (not `.to_string()`
/// which is non-const, E0015). Non-literal initialisers that can't be expressed
/// as a `const` produce a `compile_error!` rather than uncompilable code.
fn emit_const(c: &HirConst) -> String {
    use vox_compiler::hir::HirExpr;
    let vis = if c.is_pub { "pub " } else { "" };

    // Helper: escape a string value for a Rust `"..."` literal.
    let escape_str = |s: &str| -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    };

    let (ty, value) = match &c.type_ann {
        Some(vox_compiler::hir::HirType::Named(n)) if n == "str" => {
            // str-annotated: borrow the literal so it's const-evaluable.
            let v = match &c.value {
                HirExpr::StringLit(s, _) => format!("\"{}\"", escape_str(s)),
                other => super::stmt_expr::emit_expr(other),
            };
            ("&'static str".to_string(), v)
        }
        Some(ty) => (emit_type(ty), super::stmt_expr::emit_expr(&c.value)),
        None => {
            // Infer Rust type from the literal kind.
            match &c.value {
                HirExpr::IntLit(n, _) => ("i64".to_string(), n.to_string()),
                HirExpr::FloatLit(f, _) => ("f64".to_string(), format!("{f}f64")),
                HirExpr::BoolLit(b, _) => ("bool".to_string(), b.to_string()),
                HirExpr::StringLit(s, _) => {
                    ("&'static str".to_string(), format!("\"{}\"", escape_str(s)))
                }
                _ => {
                    return format!(
                        "compile_error!(\"vox.codegen_rust.const_requires_literal: \
                         cannot emit const `{}` from a non-literal initialiser \
                         — add an explicit type annotation\");\n",
                        c.name
                    );
                }
            }
        }
    };
    format!("{vis}const {name}: {ty} = {value};\n", name = c.name)
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

/// For a `@json_as`-generated `<Type>_from_json` function, return the decoded
/// struct name (the `T` in the `Result[T]` return type). `None` for any other
/// function — including `<Type>_to_json` (whose `ObjectLit` is a real JSON
/// value) and ADT `from_json` (which returns variant constructor calls, not an
/// `ObjectLit`). Builtin/JSON-ish names are excluded so only user structs win.
fn from_json_struct_hint(func: &HirFn) -> Option<String> {
    if !func.name.ends_with("_from_json") {
        return None;
    }
    match func.return_type.as_ref()? {
        HirType::Generic(outer, args) if outer == "Result" => match args.first()? {
            HirType::Named(n)
                if !matches!(
                    n.as_str(),
                    "Json" | "JsonBody" | "Any" | "Result" | "Element"
                ) =>
            {
                Some(n.clone())
            }
            _ => None,
        },
        _ => None,
    }
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
    if func.is_traced {
        out.push_str(&format!(
            "#[tracing::instrument(skip_all, name = \"{}\", fields(trace_id = tracing::field::Empty))]\n",
            func.name.replace("::", "_")
        ));
    }
    out.push_str(&format!(
        "{}{}fn {}(",
        pub_kw,
        async_kw,
        func.name.replace("::", "_")
    ));
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
    if func.is_traced {
        out.push_str(
            "if let Some(__tc) = vox_telemetry::current_trace_context() { \
             tracing::Span::current().record(\"trace_id\", tracing::field::display(&__tc.trace_id)); }\n",
        );
    }
    if func.is_llm {
        super::ai_fixture::emit_llm_function_body(&mut out, func);
    } else {
        // `@json_as` `<Type>_from_json` bodies return `Ok(Type { .. })`. In the
        // script lane there is no typeck, so record the struct name (the inner
        // arg of the `Result[Type]` return type) for the tail-emitter's
        // `ObjectLit` arm to ascribe a struct literal instead of `json!`.
        let _from_json_guard =
            from_json_struct_hint(func).map(|name| super::json_as_ctx::enter_from_json(&name));
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
        let is_actor_shell =
            matches!(func.durability, Some(DurabilityKind::Actor)) && !func.name.contains("::");
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
