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
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    _handlers: &[&HirFn],
) -> String {
    // An actor "handler" function has a name like "ChatRoom::join" and
    // carries the executable handler body lowered from the `on event(...)`
    // declaration. It is NOT a process-spawn site — it's a plain function
    // that the runtime dispatcher calls when it receives a matching
    // envelope. So emit it like any other function. Per the 2026-05-23
    // slot-3 chat bring-up: the prior implementation re-emitted the
    // spawn_process loop inside every handler body, referencing
    // bogus types like `ChatRoom::joinState`.
    if func.name.contains("::") {
        return emit_plain_body(func, inferred_types, usage);
    }

    // Actor SHELL: spawn the process loop. State struct is emitted as
    // a sibling by `emit_actor_state_structs` in workflow.rs; here we
    // just instantiate it. The Envelope dispatch is left as a no-op
    // for now — vox_actor_runtime::Envelope is a tagged enum
    // (`Message(Message) | Request(Request) | Signal(Signal)`), not a
    // struct with a `.payload` field, so a real dispatch table needs
    // a `match envelope { Envelope::Message(m) => ..., ... }` over
    // the inner MessagePayload. Until that wire shape is settled,
    // consume the envelope without routing — the binary compiles
    // and serves /health, which is what CR-P1 measures.
    let actor_name = &func.name;
    let state_struct = format!("{}State", actor_name);
    let mut out = String::new();
    out.push_str("    // actor shell — spawn the mailbox loop. Envelope\n");
    out.push_str("    // dispatch is a no-op pending the wire-shape decision\n");
    out.push_str("    // (see emit_actor_body comment in durability_lower.rs).\n");
    out.push_str(&format!(
        "    let _state = {state_struct}::default();\n"
    ));
    out.push_str(
        "    let _handle = ::vox_actor_runtime::spawn_process(move |mut ctx| async move {\n",
    );
    out.push_str("        while let Some(envelope) = ctx.receive().await {\n");
    out.push_str("            let _ = envelope; // dispatch not yet wired\n");
    out.push_str("        }\n");
    out.push_str("    });\n");
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
