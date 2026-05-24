//! P2-T7: lower `DurabilityKind` into specific runtime call shapes.
//!
//! Driven by `HirFn::durability`. The branch lives here so `emit_fn` stays
//! readable: header emit + delegate-by-kind.

use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{DurabilityKind, HirFn, HirType};

use super::stmt_expr::emit_stmt;
use super::types::emit_type;

/// Emit the body of a workflow / activity / actor handler.
///
/// The function header (params, return type) is emitted by the caller; this
/// function owns everything inside the `{ ... }`.
pub(super) fn emit_durable_body(
    func: &HirFn,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    actor_handlers: &[&HirFn],
) -> String {
    match func.durability {
        Some(DurabilityKind::Workflow) => emit_workflow_body(func),
        Some(DurabilityKind::Activity) => emit_activity_body(func, inferred_types, usage),
        Some(DurabilityKind::Actor) => emit_actor_body(func, inferred_types, usage, actor_handlers),
        None => emit_plain_body(func, inferred_types, usage),
    }
}

fn emit_workflow_body(func: &HirFn) -> String {
    let name = &func.name;
    let hash = func.generated_hash.as_deref().unwrap_or("UNSTAMPED");
    let mut out = String::new();
    out.push_str("    // P2-T7: workflow body lowered to interpret_workflow_durable\n");
    out.push_str(&format!(
        "    let __vox_fn_hash: &'static str = \"{hash}\";\n"
    ));
    out.push_str("    let _ = __vox_fn_hash;\n");
    out.push_str("    let __vox_hir = ::vox_workflow_runtime::workflow::current_hir_module();\n");
    out.push_str(
        "    let mut __vox_tracker = ::vox_workflow_runtime::workflow::tracker::DefaultTracker;\n",
    );
    out.push_str(&format!(
        "    let __vox_journal = ::vox_workflow_runtime::workflow::interpret_workflow_durable(__vox_hir, \"{name}\", &mut __vox_tracker).await?;\n"
    ));
    if let Some(ret) = &func.return_type {
        out.push_str(&format!(
            "    ::vox_workflow_runtime::workflow::extract_terminal_return::<{ty}>(&__vox_journal).map_err(|e| anyhow::anyhow!(e))\n",
            ty = emit_type(ret),
        ));
    } else {
        out.push_str("    Ok(())\n");
    }
    out
}

fn emit_activity_body(func: &HirFn, inferred_types: Option<&HashMap<Span, HirType>>, usage: Option<&super::usage::UsageTracker>) -> String {
    let activity_id = func
        .generated_hash
        .clone()
        .unwrap_or_else(|| func.name.clone());
    let mut out = String::new();
    out.push_str("    // P2-T7: activity body lowered to journal::execute\n");
    out.push_str(&format!(
        "    ::vox_workflow_runtime::journal::execute(\"{activity_id}\", async move {{\n"
    ));
    for stmt in &func.body {
        let inner = emit_stmt(stmt, 2, false, false, false, inferred_types, usage, None);
        out.push_str(&inner);
    }
    out.push_str("    }).await\n");
    out
}

fn emit_actor_body(
    func: &HirFn,
    _inferred_types: Option<&HashMap<Span, HirType>>,
    _usage: Option<&super::usage::UsageTracker>,
    handlers: &[&HirFn],
) -> String {
    let actor_name = &func.name;
    let state_struct = format!("{}State", actor_name);
    let mut out = String::new();
    out.push_str("    // P2-T7: actor handler lowered to spawn_process\n");
    out.push_str(&format!(
        "    let mut state = {}::default();\n",
        state_struct
    ));
    out.push_str("    ::vox_actor_runtime::spawn_process(move |mut ctx| async move {\n");
    out.push_str("        while let Some(envelope) = ctx.receive().await {\n");
    out.push_str("            match envelope.payload.as_str() {\n");
    for h in handlers {
        let event_name = h.name.split("::").last().unwrap();
        let sanitized_name = h.name.replace("::", "_");
        out.push_str(&format!("                {:?} => {{\n", event_name));
        out.push_str(&format!(
            "                    let args = serde_json::from_value(envelope.args).unwrap_or_default();\n"
        ));
        out.push_str(&format!(
            "                    {}(&mut state, args).await;\n",
            sanitized_name
        ));
        out.push_str("                }\n");
    }
    out.push_str("                _ => {}\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("    })\n");
    out
}

pub(super) fn emit_plain_body(
    func: &HirFn,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> String {
    let mut out = String::new();
    for stmt in &func.body {
        out.push_str(&emit_stmt(
            stmt,
            1,
            false,
            false,
            false,
            inferred_types,
            usage,
            None,
        ));
    }
    out
}
