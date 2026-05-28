//! Path C reactive components → React TSX via `hir_emit`.
//!
//! **Web IR (OP-0193+):** the `view:` body is taken from
//! [`crate::web_ir::emit_tsx::emit_component_view_tsx`] after lowering the module Web IR graph.
//! When [`validate_web_ir`](crate::web_ir::validate::validate_web_ir) reports **blocking** diagnostics
//! (non-advisory), or when no Web IR view root exists for the component, emit **fails fast**: the
//! return uses a small placeholder fragment and the diagnostics are appended to
//! `ReactiveViewBridgeStats::reactive_view_emit_failures`.
//!
//! Legacy [`emit_hir_expr`](crate::codegen_ts::hir_emit::emit_hir_expr) is computed **only** to
//! classify `ReactiveViewEmitPathway::WebIrViewEmittedParityMismatch` vs `WebIrViewEmitted`
//! (whitespace-normalized compare); it is never selected as the emitted view body.
//!
//! **Diagnostics:** `VOX_WEBIR_REACTIVE_TRACE=1` logs one stderr line per reactive view decision
//! (component name + pathway). Aggregate counts: `ReactiveViewBridgeStats`.
//!
//! **Behavior adapter (OP-S037):** non-`view:` reactive members still flow through `emit_block_stmts` /
//! `emit_hir_expr`; `behavior_nodes` / preview emit from [`crate::web_ir`] is the structural mirror—keep
//! pathway counters and parity guards updated together.
//!
//! **Route→behavior map (OP-S073) + notes B/C (OP-S163 / S195):** reactive `view:` bodies are keyed by component
//! name for [`crate::web_ir::emit_tsx::emit_component_view_tsx`] selection; do not rename without updating
//! [`crate::web_ir::WebIrModule::view_roots`] lowering.

use crate::codegen_ts::hir_emit::{
    EmitCtx, emit_block_stmts, emit_hir_expr, emit_hir_stmt, extract_state_deps_with_diagnostics,
    map_hir_type_to_ts,
};
use crate::web_ir::{WebIrDiagnostic, WebIrModule};
use std::collections::HashSet;
use vox_compiler::hir::*;
use vox_compiler::react_bridge::react_exports::{
    USE_CALLBACK, USE_EFFECT, USE_MEMO, USE_REF, USE_STATE,
};

fn web_ir_reactive_trace_enabled() -> bool {
    std::env::var("VOX_WEBIR_REACTIVE_TRACE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Which codegen path classified the reactive `view:` body after Web IR preview emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactiveViewEmitPathway {
    /// Clean Web IR TSX emitted; whitespace-normalized string ≠ legacy `emit_hir_expr` (stats-only).
    WebIrViewEmittedParityMismatch,
    /// Web IR preview TSX used for the view body (parity match).
    WebIrViewEmitted,
}

#[derive(Debug, Clone, Default)]
pub struct ReactiveViewBridgeStats {
    pub web_ir_view_emitted_parity_mismatch: u64,
    pub web_ir_view_emitted: u64,
    /// Blocking Web IR validate failures and missing view roots for this codegen run.
    pub reactive_view_emit_failures: Vec<WebIrDiagnostic>,
}

impl ReactiveViewBridgeStats {
    pub fn record_pathway(&mut self, p: ReactiveViewEmitPathway) {
        match p {
            ReactiveViewEmitPathway::WebIrViewEmittedParityMismatch => {
                self.web_ir_view_emitted_parity_mismatch += 1
            }
            ReactiveViewEmitPathway::WebIrViewEmitted => self.web_ir_view_emitted += 1,
        }
    }
}

/// Whitespace normalization for the reactive view parity guard (OP-0261 / OP-0179).
#[doc(hidden)]
pub fn normalize_reactive_view_jsx_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join("")
}

fn indent_view_for_return(view: &str) -> String {
    let pad = "    ";
    view.trim_end()
        .lines()
        .map(|line| {
            let t = line.trim_end();
            if t.is_empty() {
                String::new()
            } else {
                format!("{pad}{t}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit_reactive_view_body(
    component_name: &str,
    rc: &HirReactiveComponent,
    ctx: &EmitCtx<'_>,
    web: &WebIrModule,
    stats: &mut ReactiveViewBridgeStats,
) -> String {
    let Some(view) = &rc.view else {
        return String::new();
    };
    const FAIL_PLACEHOLDER: &str = "<>{/* webir reactive view emit failed */}</>";
    let diags = crate::web_ir::validate::validate_web_ir(web);
    let error_diags: Vec<WebIrDiagnostic> = diags
        .into_iter()
        .filter(|d| !crate::web_ir::validate::is_advisory_diagnostic(d))
        .collect();
    if !error_diags.is_empty() {
        stats.reactive_view_emit_failures.extend(error_diags);
        if web_ir_reactive_trace_enabled() {
            eprintln!(
                "[vox-webir-reactive] component={component_name} pathway=WebIrValidateFailedFailFast"
            );
        }
        return indent_view_for_return(FAIL_PLACEHOLDER);
    }
    let Some(tsx) = crate::web_ir::emit_tsx::emit_component_view_tsx(web, &rc.name) else {
        stats.reactive_view_emit_failures.push(WebIrDiagnostic {
            code: "codegen.reactive.no_web_ir_view_root".to_string(),
            message: format!("no Web IR view root for reactive component `{component_name}`"),
            span: None,
            category: Some("reactive".to_string()),
        });
        if web_ir_reactive_trace_enabled() {
            eprintln!(
                "[vox-webir-reactive] component={component_name} pathway=NoWebIrViewRootFailFast"
            );
        }
        return indent_view_for_return(FAIL_PLACEHOLDER);
    };
    let legacy = emit_hir_expr(view, ctx);
    let n_legacy = normalize_reactive_view_jsx_ws(&legacy);
    let n_tsx = normalize_reactive_view_jsx_ws(&tsx);
    let pathway = if n_legacy == n_tsx {
        ReactiveViewEmitPathway::WebIrViewEmitted
    } else {
        ReactiveViewEmitPathway::WebIrViewEmittedParityMismatch
    };
    stats.record_pathway(pathway);
    if web_ir_reactive_trace_enabled() {
        let label = if n_legacy == n_tsx {
            "WebIrViewEmitted"
        } else {
            "WebIrViewEmittedParityMismatch"
        };
        eprintln!("[vox-webir-reactive] component={component_name} pathway={label}");
    }
    // Convergence policy: Web IR output is canonical; parity is tracked for CI / migration only.
    indent_view_for_return(&tsx)
}

fn scan_hir_expr_for_react_imports(
    e: &HirExpr,
    need_state: &mut bool,
    need_effect: &mut bool,
    need_memo: &mut bool,
    need_ref: &mut bool,
    need_callback: &mut bool,
) {
    match e {
        HirExpr::Call(callee, args, _, _) => {
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                match name.as_str() {
                    "use_state" => *need_state = true,
                    "use_effect" | "use_layout_effect" => *need_effect = true,
                    "use_memo" => *need_memo = true,
                    "use_ref" => *need_ref = true,
                    "use_callback" => *need_callback = true,
                    _ => {}
                }
            }
            for a in args {
                scan_hir_expr_for_react_imports(
                    &a.value,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            scan_hir_expr_for_react_imports(
                l,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            scan_hir_expr_for_react_imports(
                r,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::Unary(_, x, _) => {
            scan_hir_expr_for_react_imports(
                x,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::MethodCall(recv, _, args, _, _) => {
            scan_hir_expr_for_react_imports(
                recv,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            for a in args {
                scan_hir_expr_for_react_imports(
                    &a.value,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::FieldAccess(b, _, _) => {
            scan_hir_expr_for_react_imports(
                b,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
            for x in items {
                scan_hir_expr_for_react_imports(
                    x,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, x) in fields {
                scan_hir_expr_for_react_imports(
                    x,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                scan_hir_stmt_for_react_imports(
                    s,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::Lambda(_, _, body, _, _) => {
            scan_hir_expr_for_react_imports(
                body,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            scan_hir_expr_for_react_imports(
                cond,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            for s in then_b {
                scan_hir_stmt_for_react_imports(
                    s,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
            if let Some(els) = else_b {
                for s in els {
                    scan_hir_stmt_for_react_imports(
                        s,
                        need_state,
                        need_effect,
                        need_memo,
                        need_ref,
                        need_callback,
                    );
                }
            }
        }
        HirExpr::Match(scr, arms, _) => {
            scan_hir_expr_for_react_imports(
                scr,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            for arm in arms {
                if let Some(g) = &arm.guard {
                    scan_hir_expr_for_react_imports(
                        g,
                        need_state,
                        need_effect,
                        need_memo,
                        need_ref,
                        need_callback,
                    );
                }
                scan_hir_expr_for_react_imports(
                    arm.body.as_ref(),
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            scan_hir_expr_for_react_imports(
                it,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            scan_hir_expr_for_react_imports(
                body,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::With(a, b, _) => {
            scan_hir_expr_for_react_imports(
                a,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            scan_hir_expr_for_react_imports(
                b,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::Spawn(x, _) => {
            scan_hir_expr_for_react_imports(
                x,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }

        HirExpr::Try(t) => {
            scan_hir_expr_for_react_imports(
                t.target.as_ref(),
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::JsxFragment(children, _) => {
            for child in children {
                scan_hir_expr_for_react_imports(
                    child,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::Index(obj, idx, _) => {
            scan_hir_expr_for_react_imports(
                obj,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            scan_hir_expr_for_react_imports(
                idx,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirExpr::AsyncView(v) => {
            scan_hir_expr_for_react_imports(
                &v.source,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            for arm in [
                v.fetching_arm.as_deref(),
                v.empty_arm.as_deref(),
                v.error_arm.as_deref(),
                v.ok_arm.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                scan_hir_expr_for_react_imports(
                    arm,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirExpr::JsxSelfClosing(_)
        | HirExpr::Jsx(_)
        | HirExpr::IntLit(_, _)
        | HirExpr::FloatLit(_, _)
        | HirExpr::StringLit(_, _)
        | HirExpr::BoolLit(_, _)
        | HirExpr::DecimalLit(_, _)
        | HirExpr::Ident(_, _)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn scan_hir_stmt_for_react_imports(
    s: &HirStmt,
    need_state: &mut bool,
    need_effect: &mut bool,
    need_memo: &mut bool,
    need_ref: &mut bool,
    need_callback: &mut bool,
) {
    match s {
        HirStmt::Let { value, .. } => {
            scan_hir_expr_for_react_imports(
                value,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirStmt::Assign { target, value, .. } => {
            scan_hir_expr_for_react_imports(
                target,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            scan_hir_expr_for_react_imports(
                value,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirStmt::Expr { expr, .. } => {
            scan_hir_expr_for_react_imports(
                expr,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
        }
        HirStmt::Return { value, .. } => {
            if let Some(v) = value {
                scan_hir_expr_for_react_imports(
                    v,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirStmt::While {
            condition, body, ..
        } => {
            scan_hir_expr_for_react_imports(
                condition,
                need_state,
                need_effect,
                need_memo,
                need_ref,
                need_callback,
            );
            for x in body {
                scan_hir_stmt_for_react_imports(
                    x,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirStmt::Loop { body, .. } => {
            for x in body {
                scan_hir_stmt_for_react_imports(
                    x,
                    need_state,
                    need_effect,
                    need_memo,
                    need_ref,
                    need_callback,
                );
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

/// Phase E tier-2: emit a `// dep_inference.over_track` hint comment above a `useMemo` /
/// `useEffect` line whenever its body calls visible in-module functions that aren't
/// `@reactive`-annotated. Surfaces the conservative under-tracking gap to humans and AI
/// readers of the generated TSX. Stripped by minifiers; harmless to runtime.
fn emit_dep_inference_hints(out: &mut String, owner: &str, unannotated: &[String]) {
    if unannotated.is_empty() {
        return;
    }
    out.push_str(&format!(
        "  // dep_inference.over_track: `{}` calls [{}] which lack `@reactive` — \
         reactive reads inside those bodies will not trigger re-runs. Add `@reactive` \
         to the callee(s) to opt in to cross-call dep tracking.\n",
        owner,
        unannotated
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
}

fn collect_reactive_binding_names(members: &[HirReactiveMember]) -> HashSet<String> {
    fn pat_names(pat: &HirPattern, out: &mut HashSet<String>) {
        match pat {
            HirPattern::Ident(n, _) => {
                out.insert(n.clone());
            }
            HirPattern::Tuple(items, _) => {
                for p in items {
                    pat_names(p, out);
                }
            }
            HirPattern::Constructor(_, items, _) => {
                for p in items {
                    pat_names(p, out);
                }
            }
            HirPattern::Wildcard(_) | HirPattern::Literal(_, _) => {}
        }
    }
    fn stmt_names(s: &HirStmt, out: &mut HashSet<String>) {
        match s {
            HirStmt::Let { pattern, .. } => pat_names(pattern, out),
            HirStmt::While { body, .. } | HirStmt::Loop { body, .. } => {
                for x in body {
                    stmt_names(x, out);
                }
            }
            _ => {}
        }
    }

    let mut names = HashSet::new();
    for m in members {
        match m {
            HirReactiveMember::State(s) => {
                names.insert(s.name.clone());
            }
            HirReactiveMember::Stmt(s) => stmt_names(s, &mut names),
            _ => {}
        }
    }
    names
}

fn react_import_line(members: &[HirReactiveMember]) -> String {
    let mut need_state = false;
    let mut need_effect = false;
    let mut need_memo = false;
    let mut need_ref = false;
    let mut need_callback = false;
    for m in members {
        match m {
            HirReactiveMember::State(_) => need_state = true,
            HirReactiveMember::Derived(_) => need_memo = true,
            HirReactiveMember::Effect(_)
            | HirReactiveMember::OnMount(_)
            | HirReactiveMember::OnCleanup(_) => need_effect = true,
            HirReactiveMember::Stmt(s) => {
                scan_hir_stmt_for_react_imports(
                    s,
                    &mut need_state,
                    &mut need_effect,
                    &mut need_memo,
                    &mut need_ref,
                    &mut need_callback,
                );
            }
        }
    }
    let mut hooks = Vec::new();
    if need_state {
        hooks.push(USE_STATE);
    }
    if need_effect {
        hooks.push(USE_EFFECT);
    }
    if need_memo {
        hooks.push(USE_MEMO);
    }
    if need_ref {
        hooks.push(USE_REF);
    }
    if need_callback {
        hooks.push(USE_CALLBACK);
    }
    if hooks.is_empty() {
        return "import React from \"react\";\n\n".to_string();
    }
    format!(
        "import React, {{ {} }} from \"react\";\n\n",
        hooks.join(", ")
    )
}

/// Walk an HIR expression tree and collect names of free-fn calls that match a known
/// set of identifiers (used for @endpoint imports — see [`generate_reactive_component`]).
fn collect_callee_refs(expr: &HirExpr, known: &HashSet<String>, out: &mut HashSet<String>) {
    match expr {
        HirExpr::Call(callee, args, _, _) => {
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                if known.contains(name) {
                    out.insert(name.clone());
                }
            }
            collect_callee_refs(callee, known, out);
            for arg in args {
                collect_callee_refs(&arg.value, known, out);
            }
        }
        HirExpr::MethodCall(obj, _, args, _, _) => {
            collect_callee_refs(obj, known, out);
            for arg in args {
                collect_callee_refs(&arg.value, known, out);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            collect_callee_refs(l, known, out);
            collect_callee_refs(r, known, out);
        }
        HirExpr::Unary(_, e, _) => collect_callee_refs(e, known, out),
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                collect_callee_refs_stmt(s, known, out);
            }
        }
        HirExpr::If(c, t, e, _) => {
            collect_callee_refs(c, known, out);
            for s in t {
                collect_callee_refs_stmt(s, known, out);
            }
            if let Some(stmts) = e {
                for s in stmts {
                    collect_callee_refs_stmt(s, known, out);
                }
            }
        }
        HirExpr::For(_, _, iter, body, _, _) => {
            collect_callee_refs(iter, known, out);
            collect_callee_refs(body, known, out);
        }
        HirExpr::Match(subj, arms, _) => {
            collect_callee_refs(subj, known, out);
            for arm in arms {
                collect_callee_refs(&arm.body, known, out);
            }
        }
        HirExpr::Lambda(_, _, body, _, _) => collect_callee_refs(body, known, out),
        HirExpr::Index(o, i, _) => {
            collect_callee_refs(o, known, out);
            collect_callee_refs(i, known, out);
        }
        HirExpr::FieldAccess(o, _, _) => collect_callee_refs(o, known, out),
        HirExpr::Jsx(el) => {
            for attr in &el.attributes {
                collect_callee_refs(&attr.value, known, out);
            }
            for child in &el.children {
                collect_callee_refs(child, known, out);
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for attr in &el.attributes {
                collect_callee_refs(&attr.value, known, out);
            }
        }
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                collect_callee_refs(c, known, out);
            }
        }
        _ => {}
    }
}

fn collect_callee_refs_stmt(stmt: &HirStmt, known: &HashSet<String>, out: &mut HashSet<String>) {
    match stmt {
        HirStmt::Expr { expr, .. }
        | HirStmt::Let { value: expr, .. }
        | HirStmt::Assign { value: expr, .. } => collect_callee_refs(expr, known, out),
        HirStmt::Return { value: Some(v), .. } => collect_callee_refs(v, known, out),
        HirStmt::While {
            condition, body, ..
        } => {
            collect_callee_refs(condition, known, out);
            for s in body {
                collect_callee_refs_stmt(s, known, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                collect_callee_refs_stmt(s, known, out);
            }
        }
        _ => {}
    }
}

/// Walk an HIR expression tree and collect uppercase JSX tag names that correspond
/// to known Vox components. Used to emit cross-component import statements.
fn collect_jsx_component_refs(expr: &HirExpr, known: &HashSet<String>, out: &mut HashSet<String>) {
    match expr {
        HirExpr::Jsx(el) => {
            if el.tag.starts_with(|c: char| c.is_uppercase()) && known.contains(&el.tag) {
                out.insert(el.tag.clone());
            }
            for child in &el.children {
                collect_jsx_component_refs(child, known, out);
            }
        }
        HirExpr::JsxSelfClosing(el)
            if el.tag.starts_with(|c: char| c.is_uppercase()) && known.contains(&el.tag) =>
        {
            out.insert(el.tag.clone());
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            collect_jsx_component_refs(cond, known, out);
            for s in then_stmts {
                collect_jsx_component_refs_stmt(s, known, out);
            }
            if let Some(stmts) = else_stmts {
                for s in stmts {
                    collect_jsx_component_refs_stmt(s, known, out);
                }
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                collect_jsx_component_refs_stmt(s, known, out);
            }
        }
        HirExpr::For(_, _, iter, body, _, _) => {
            collect_jsx_component_refs(iter, known, out);
            collect_jsx_component_refs(body, known, out);
        }
        HirExpr::JsxFragment(children, _) => {
            for child in children {
                collect_jsx_component_refs(child, known, out);
            }
        }
        _ => {}
    }
}

fn collect_jsx_component_refs_stmt(
    stmt: &HirStmt,
    known: &HashSet<String>,
    out: &mut HashSet<String>,
) {
    match stmt {
        HirStmt::Expr { expr, .. } => collect_jsx_component_refs(expr, known, out),
        HirStmt::Let { value, .. } => collect_jsx_component_refs(value, known, out),
        HirStmt::Assign { value, .. } => collect_jsx_component_refs(value, known, out),
        HirStmt::Return { value: Some(v), .. } => collect_jsx_component_refs(v, known, out),
        _ => {}
    }
}

/// `hir` must be the full module (imports, endpoints, `@reactive` callees, etc.).
///
/// `web_projection` must be the same Web IR graph as [`crate::projection_bundle::project_bundle_from_hir`]`(hir).web`
/// so reactive emit does not re-lower the module per component.
pub fn generate_reactive_component(
    hir: &HirModule,
    rc: &HirReactiveComponent,
    web_projection: &WebIrModule,
    stats: &mut ReactiveViewBridgeStats,
) -> (String, String) {
    let name = &rc.name;
    let filename = format!("{name}.tsx");
    let mut out = String::new();

    let state_names = collect_reactive_binding_names(&rc.members);

    // Phase E: collect `@reactive`-annotated free functions visible from this module so the
    // dep walker can recurse one level into their bodies when called from `derived` / `effect`.
    // Functions without `@reactive` are not indexed; their call sites contribute no deps from
    // inside the callee (conservative under-tracking, opt-in extension).
    let reactive_callees: std::collections::HashMap<String, Vec<HirStmt>> = hir
        .functions
        .iter()
        .filter(|f| f.is_reactive)
        .map(|f| (f.name.clone(), f.body.clone()))
        .collect();
    // Phase E tier-2: full set of in-module fn names — used by the dep walker to
    // distinguish "in-module fn missing @reactive" (worth a hint) from "method call /
    // stdlib call / unknown identifier" (silent).
    let visible_fn_names: HashSet<String> = hir.functions.iter().map(|f| f.name.clone()).collect();

    out.push_str(&react_import_line(&rc.members));

    // Phase 5: external React components (`import react Foo from "./Foo.tsx"` in Vox source).
    let mut react_es_imports: Vec<(&str, &str)> = hir
        .imports
        .iter()
        .filter_map(|imp| {
            imp.es_module_specifier
                .as_ref()
                .map(|spec| (imp.item.as_str(), spec.as_str()))
        })
        .collect();
    react_es_imports.sort_by_key(|(item, _)| *item);
    let has_react_es = !react_es_imports.is_empty();
    for (item, spec) in &react_es_imports {
        out.push_str(&format!("import {item} from \"{spec}\";\n"));
    }
    if has_react_es {
        out.push('\n');
    }

    // Emit import statements for other Vox components referenced in the view.
    let known_components: HashSet<String> = hir.components.iter().map(|c| c.name.clone()).collect();
    let mut comp_refs: HashSet<String> = HashSet::new();
    if let Some(view_expr) = &rc.view {
        collect_jsx_component_refs(view_expr, &known_components, &mut comp_refs);
    }
    // Also walk the view inside members (e.g. inline JSX in state initialisers).
    for m in &rc.members {
        if let HirReactiveMember::Stmt(s) = m {
            collect_jsx_component_refs_stmt(s, &known_components, &mut comp_refs);
        }
    }
    let mut sorted_refs: Vec<String> = comp_refs.into_iter().collect();
    sorted_refs.sort();
    for comp in &sorted_refs {
        out.push_str(&format!("import {{ {comp} }} from \"./{comp}\";\n"));
    }
    if !sorted_refs.is_empty() {
        out.push('\n');
    }

    // Bug D: emit import for every @endpoint fn referenced from this component.
    // Endpoint fns are exported from `vox-client.ts` (see [`crate::codegen_ts::vox_client`]).
    let endpoint_names: HashSet<String> = hir.endpoint_fns.iter().map(|e| e.name.clone()).collect();
    let mut endpoint_refs: HashSet<String> = HashSet::new();
    if let Some(view_expr) = &rc.view {
        collect_callee_refs(view_expr, &endpoint_names, &mut endpoint_refs);
    }
    for m in &rc.members {
        if let HirReactiveMember::Stmt(s) = m {
            collect_callee_refs_stmt(s, &endpoint_names, &mut endpoint_refs);
        }
    }
    if !endpoint_refs.is_empty() {
        let mut sorted: Vec<String> = endpoint_refs.into_iter().collect();
        sorted.sort();
        out.push_str(&format!(
            "import {{ {} }} from \"./vox-client\";\n\n",
            sorted.join(", ")
        ));
    }

    if !rc.styles.is_empty() {
        out.push_str(&format!("import \"./{name}.css\";\n\n"));
    }

    if !rc.params.is_empty() {
        out.push_str(&format!("export interface {name}Props {{\n"));
        for param in &rc.params {
            let ts_type = param
                .type_ann
                .as_ref()
                .map_or("any".to_string(), map_hir_type_to_ts);
            out.push_str(&format!("  {}: {};\n", param.name, ts_type));
        }
        out.push_str("}\n\n");
    }

    if rc.params.is_empty() {
        out.push_str(&format!(
            "export function {name}(): React.ReactElement {{\n"
        ));
    } else {
        let param_names: Vec<String> = rc.params.iter().map(|p| p.name.clone()).collect();
        out.push_str(&format!(
            "export function {name}({{ {} }}: {name}Props): React.ReactElement {{\n",
            param_names.join(", ")
        ));
    }

    // §1.A.2: build an emit context that threads endpoint (async) fn names into handler emission,
    // so calls to @endpoint fns inside onClick/onChange etc. receive `await`.
    let plain_ctx = EmitCtx::new(&state_names);
    let view_ctx = EmitCtx::with_async(&state_names, &endpoint_names);

    for member in &rc.members {
        match member {
            HirReactiveMember::State(s) => {
                let init = emit_hir_expr(&s.init, &plain_ctx);
                out.push_str(&format!(
                    "  const [{}, set_{}] = useState({});\n",
                    s.name, s.name, init
                ));
            }
            HirReactiveMember::Derived(d) => {
                let expr = emit_hir_expr(&d.expr, &plain_ctx);
                let analysis = extract_state_deps_with_diagnostics(
                    &d.expr,
                    &state_names,
                    &reactive_callees,
                    &visible_fn_names,
                );
                emit_dep_inference_hints(&mut out, &d.name, &analysis.unannotated_calls);
                let dep_str = analysis.deps.join(", ");
                out.push_str(&format!(
                    "  const {} = useMemo(() => {}, [{}]);\n",
                    d.name, expr, dep_str
                ));
            }
            HirReactiveMember::Effect(e) => {
                let stmts_str = emit_block_stmts(&e.body, &plain_ctx, 2);
                let analysis = extract_state_deps_with_diagnostics(
                    &e.body,
                    &state_names,
                    &reactive_callees,
                    &visible_fn_names,
                );
                emit_dep_inference_hints(&mut out, "effect", &analysis.unannotated_calls);
                let dep_str = analysis.deps.join(", ");
                out.push_str(&format!(
                    "  useEffect(() => {{\n{}  }}, [{}]);\n",
                    stmts_str, dep_str
                ));
            }
            HirReactiveMember::OnMount(m) => {
                let stmts_str = emit_block_stmts(&m.body, &plain_ctx, 2);
                out.push_str(&format!("  useEffect(() => {{\n{}  }}, []);\n", stmts_str));
            }
            HirReactiveMember::OnCleanup(c) => {
                let stmts_str = emit_block_stmts(&c.body, &plain_ctx, 2);
                out.push_str(&format!(
                    "  useEffect(() => () => {{\n{}  }}, []);\n",
                    stmts_str
                ));
            }
            HirReactiveMember::Stmt(s) => {
                out.push_str(&emit_hir_stmt(s, &plain_ctx, 2));
            }
        }
    }

    if rc.view.is_some() {
        let view_js = emit_reactive_view_body(name, rc, &view_ctx, web_projection, stats);
        out.push_str(&format!("  return (\n{}\n  );\n", view_js));
    }

    out.push_str("}\n");
    (filename, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::hir::lower::lower_module;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse;

    fn compile(src: &str) -> Vec<(String, String)> {
        let tokens = lex(src);
        let module = parse(tokens).expect("parse error");
        let hir = lower_module(&module);
        let bundle = crate::projection_bundle::project_bundle_from_hir(&hir);
        let mut stats = ReactiveViewBridgeStats::default();
        hir.components
            .iter()
            .map(|rc| generate_reactive_component(&hir, rc, &bundle.web, &mut stats))
            .collect()
    }

    fn get(files: &[(String, String)], name: &str) -> String {
        files
            .iter()
            .find(|(f, _)| f == name)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    #[test]
    fn test_cross_component_import_emitted() {
        let files = compile(
            "component Inner() { view: panel() { text() { \"hi\" } } }\n\
             component Outer() { view: column() { Inner() } }",
        );
        let outer = get(&files, "Outer.tsx");
        assert!(
            outer.contains("import { Inner } from \"./Inner\";"),
            "expected import for Inner in Outer.tsx, got:\n{outer}"
        );
    }

    #[test]
    fn test_no_import_for_html_primitives() {
        let files = compile("component Card() { view: panel() { text() { \"x\" } } }");
        let card = get(&files, "Card.tsx");
        // 'panel' and 'text' are primitives, must not generate import lines
        assert!(
            !card.contains("import { panel }"),
            "primitive 'panel' should not be imported"
        );
        assert!(
            !card.contains("import { text }"),
            "primitive 'text' should not be imported"
        );
    }

    #[test]
    fn derived_calling_reactive_callee_includes_state_in_dep_array() {
        // Phase E end-to-end: a `derived` that calls a `@reactive`-annotated free function
        // which reads a reactive `state` binding should include that binding in the
        // emitted React `useMemo` dep array. Without the wiring (or without `@reactive`)
        // the dep array would be empty, leaving the memo stale on state updates.
        let files = compile(
            "@reactive fn double_it(c: int) to int { c * 2 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = double_it(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        assert!(
            counter.contains("useMemo(() => double_it(count), [count])"),
            "expected useMemo dep array to include `count` traced through @reactive callee:\n{counter}"
        );
    }

    #[test]
    fn derived_calling_non_reactive_callee_emits_dep_inference_over_track_hint() {
        // Phase E tier-2: when `derived` calls a visible in-module fn that is not
        // `@reactive`, emit a `// dep_inference.over_track` hint comment above the
        // useMemo line so downstream readers (humans + AI) see why the dep array might
        // miss reactive reads through the call.
        let files = compile(
            "fn opaque(x: int) to int { x + 1 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = opaque(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        assert!(
            counter.contains("// dep_inference.over_track"),
            "expected over_track hint comment:\n{counter}"
        );
        assert!(
            counter.contains("`opaque`"),
            "expected hint to name the offending callee:\n{counter}"
        );
    }

    #[test]
    fn derived_with_only_reactive_callees_omits_dep_inference_hint() {
        // Counterpart: when every called in-module fn is `@reactive`, no hint comment
        // appears (the analyzer can fully descend, no over-tracking risk).
        let files = compile(
            "@reactive fn double_it(c: int) to int { c * 2 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = double_it(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        assert!(
            !counter.contains("dep_inference.over_track"),
            "did not expect over_track hint:\n{counter}"
        );
    }

    #[test]
    fn derived_calling_non_reactive_callee_omits_state_from_dep_array() {
        // Counterpart: without `@reactive`, the analyzer must NOT recurse into the callee
        // body (conservative under-tracking). The dep array is empty and the memo will be
        // stale — opt-in is the policy.
        let files = compile(
            "fn double_it(c: int) to int { c * 2 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = double_it(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        // The arg `count` still appears as a direct read, so dep array is `[count]`. To
        // truly demonstrate the under-tracking, use an arg that doesn't reference state:
        let files2 = compile(
            "fn opaque() to int { 42 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = opaque()\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter2 = get(&files2, "Counter.tsx");
        assert!(
            counter2.contains("useMemo(() => opaque(), [])"),
            "expected empty dep array (no @reactive on `opaque`):\n{counter2}"
        );
        // And the first compile should still find `count` via the direct argument read:
        assert!(
            counter.contains("[count]"),
            "expected `count` dep from the direct argument read:\n{counter}"
        );
    }

    #[test]
    fn test_import_inside_if_branch() {
        let files = compile(
            "component Badge() { view: text() { \"x\" } }\n\
             component Host(show: bool) {\n\
               state s: bool = false\n\
               view: if s { Badge() } else { text() { \"no\" } }\n\
             }",
        );
        let host = get(&files, "Host.tsx");
        assert!(
            host.contains("import { Badge } from \"./Badge\";"),
            "expected Badge import inside if branch:\n{host}"
        );
    }
}
