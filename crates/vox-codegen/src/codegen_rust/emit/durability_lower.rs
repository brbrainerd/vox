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

fn emit_activity_body(
    func: &HirFn,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> String {
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

/// Emit the per-handler dispatch arm for one `on <event>(params)` handler.
///
/// Decodes the positional `args` array into each typed param, then calls the
/// emitted `Actor_event(&mut _state, …)` function. On the request path
/// (`is_request`) the return value is serialized back through
/// `ProcessContext::reply`; on the message path the return (if any) is dropped.
/// A malformed/short args array drops the envelope (`continue`s the loop).
fn emit_actor_dispatch_arm(handler: &HirFn, is_request: bool) -> String {
    let event = handler.name.split("::").nth(1).unwrap_or(&handler.name);
    let call_name = handler.name.replace("::", "_");
    let mut arm = String::new();
    arm.push_str(&format!("                    {event:?} => {{\n"));
    let mut call_args = String::from("&mut _state");
    for (i, param) in handler.params.iter().enumerate() {
        let ty = emit_type(
            param
                .type_ann
                .as_ref()
                .unwrap_or(&HirType::Named("serde_json::Value".into())),
        );
        arm.push_str(&format!(
            "                        let {name}: {ty} = match __vox_arg(&__args, {i}usize) {{ \
             Some(__a) => __a, None => continue }};\n",
            name = param.name,
        ));
        call_args.push_str(&format!(", {}", param.name));
    }
    if is_request {
        if handler.return_type.is_some() {
            arm.push_str(&format!(
                "                        let __ret = {call_name}({call_args});\n"
            ));
            arm.push_str(
                "                        ::vox_actor_runtime::ProcessContext::reply(__req, \
                 ::serde_json::to_vec(&__ret).unwrap_or_default());\n",
            );
        } else {
            arm.push_str(&format!(
                "                        {call_name}({call_args});\n"
            ));
            arm.push_str(
                "                        ::vox_actor_runtime::ProcessContext::reply(__req, \
                 ::std::vec::Vec::new());\n",
            );
        }
    } else {
        arm.push_str(&format!(
            "                        let _ = {call_name}({call_args});\n"
        ));
    }
    arm.push_str("                    }\n");
    arm
}

fn emit_actor_body(
    func: &HirFn,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    handlers: &[&HirFn],
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

    // Actor SHELL: spawn the process loop and dispatch envelopes to handlers.
    // State struct is emitted as a sibling by `emit_actor_state_structs` in
    // workflow.rs; here we instantiate it and route each `Envelope` to the
    // matching `on <event>` handler via the `{"event","args"}` JSON wire format
    // (docs/superpowers/specs/2026-06-03-actor-message-wire-format-design.md):
    // `Message` is fire-and-forget; `Request` runs the handler and replies with
    // the serialized return value; `Signal` is lifecycle (dropped for now).
    let actor_name = &func.name;
    let state_struct = format!("{}State", actor_name);

    let message_arms: String = handlers
        .iter()
        .map(|h| emit_actor_dispatch_arm(h, false))
        .collect();
    let request_arms: String = handlers
        .iter()
        .map(|h| emit_actor_dispatch_arm(h, true))
        .collect();

    let mut out = String::new();
    out.push_str("    // actor shell — spawn the mailbox loop and dispatch envelopes.\n");
    out.push_str(&format!(
        "    #[allow(unused_mut, unused_variables)]\n    let mut _state = {state_struct}::default();\n"
    ));
    out.push_str(
        "    let _handle = ::vox_actor_runtime::spawn_process(move |mut ctx| async move {\n",
    );
    // Local helper: decode positional arg `i` from the `args` JSON array.
    out.push_str(
        "        fn __vox_arg<__T: ::serde::de::DeserializeOwned>(args: &::serde_json::Value, i: usize) -> ::std::option::Option<__T> {\n",
    );
    out.push_str(
        "            args.get(i).and_then(|__a| ::serde_json::from_value(__a.clone()).ok())\n",
    );
    out.push_str("        }\n");
    out.push_str("        while let Some(envelope) = ctx.receive().await {\n");
    out.push_str("            match envelope {\n");
    // Fire-and-forget messages.
    out.push_str("                ::vox_actor_runtime::Envelope::Message(__m) => {\n");
    out.push_str("                    let __v: ::serde_json::Value = match __m.payload.deserialize_json() { Ok(__v) => __v, Err(_) => continue };\n");
    out.push_str("                    let __ev = __v.get(\"event\").and_then(|__e| __e.as_str()).unwrap_or(\"\");\n");
    out.push_str("                    let __args = __v.get(\"args\").cloned().unwrap_or(::serde_json::Value::Null);\n");
    out.push_str("                    match __ev {\n");
    out.push_str(&message_arms);
    out.push_str("                    _ => {}\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    // Request/reply.
    out.push_str("                ::vox_actor_runtime::Envelope::Request(__req) => {\n");
    out.push_str("                    let __v: ::serde_json::Value = match __req.payload.deserialize_json() { Ok(__v) => __v, Err(_) => continue };\n");
    out.push_str("                    let __ev = __v.get(\"event\").and_then(|__e| __e.as_str()).unwrap_or(\"\").to_string();\n");
    out.push_str("                    let __args = __v.get(\"args\").cloned().unwrap_or(::serde_json::Value::Null);\n");
    out.push_str("                    match __ev.as_str() {\n");
    out.push_str(&request_arms);
    out.push_str("                    _ => {}\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                ::vox_actor_runtime::Envelope::Signal(_) => {}\n");
    out.push_str("            }\n");
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
