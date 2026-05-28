//! Lint: async event handler must declare `@cancellable`.
//!
//! Fires `lint.handler.uncancellable_async` (Warning) when a Lambda used as an
//! event-handler argument inside a component `view` expression:
//!   1. is **not** annotated `@cancellable`, AND
//!   2. calls at least one `@endpoint` function (async), AND
//!   3. also calls `set` (state setter) — meaning a state mutation can fire
//!      after the component has unmounted.
//!
//! Silence the lint by writing `@cancellable fn(_e) { … }` at the call site.

use crate::hir::{HirExpr, HirModule, HirReactiveMember, HirStmt};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use std::collections::HashSet;

/// Entry point: run the lint for all components in the module.
pub fn check_async_handlers(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Collect all endpoint function names (these are the "async" calls).
    let async_fn_names: HashSet<String> = hir.endpoint_fns.iter().map(|e| e.name.clone()).collect();

    if async_fn_names.is_empty() {
        return diags;
    }

    for c in &hir.components {
        if let Some(ref view) = c.view {
            scan_expr_for_handlers(view, &async_fn_names, &mut diags);
        }
        // Also scan members (on_mount, effect, etc.) in case lambdas appear there.
        for m in &c.members {
            match m {
                HirReactiveMember::OnMount(om) => {
                    scan_expr_for_handlers(&om.body, &async_fn_names, &mut diags);
                }
                HirReactiveMember::Effect(e) => {
                    scan_expr_for_handlers(&e.body, &async_fn_names, &mut diags);
                }
                HirReactiveMember::Stmt(s) => {
                    scan_stmt_for_handlers(s, &async_fn_names, &mut diags);
                }
                _ => {}
            }
        }
    }

    diags
}

/// Recursively walk an expression, firing the lint on every Lambda that is used
/// as an argument (event-handler position) and meets the three conditions.
fn scan_expr_for_handlers(
    expr: &HirExpr,
    async_fns: &HashSet<String>,
    diags: &mut Vec<Diagnostic>,
) {
    match expr {
        // When we see a Call, inspect each argument that is a Lambda.
        HirExpr::Call(callee, args, _, _span) => {
            scan_expr_for_handlers(callee, async_fns, diags);
            for arg in args {
                if let HirExpr::Lambda(_, _, body, cancellable, span) = &arg.value {
                    if !cancellable {
                        let calls_async = expr_calls_any_of(body, async_fns);
                        let has_assign = expr_has_state_assign(body);
                        if calls_async && has_assign {
                            diags.push(make_diag(*span));
                        }
                    }
                    // Always recurse into the lambda body for nested lambdas.
                    scan_expr_for_handlers(body, async_fns, diags);
                } else {
                    scan_expr_for_handlers(&arg.value, async_fns, diags);
                }
            }
        }
        // JSX attribute values may hold lambdas.
        HirExpr::Jsx(el) => {
            for attr in &el.attributes {
                if let HirExpr::Lambda(_, _, body, cancellable, span) = &attr.value {
                    if !cancellable {
                        let calls_async = expr_calls_any_of(body, async_fns);
                        let has_assign = expr_has_state_assign(body);
                        if calls_async && has_assign {
                            diags.push(make_diag(*span));
                        }
                    }
                    scan_expr_for_handlers(body, async_fns, diags);
                } else {
                    scan_expr_for_handlers(&attr.value, async_fns, diags);
                }
            }
            for child in &el.children {
                scan_expr_for_handlers(child, async_fns, diags);
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for attr in &el.attributes {
                if let HirExpr::Lambda(_, _, body, cancellable, span) = &attr.value {
                    if !cancellable {
                        let calls_async = expr_calls_any_of(body, async_fns);
                        let has_assign = expr_has_state_assign(body);
                        if calls_async && has_assign {
                            diags.push(make_diag(*span));
                        }
                    }
                    scan_expr_for_handlers(body, async_fns, diags);
                } else {
                    scan_expr_for_handlers(&attr.value, async_fns, diags);
                }
            }
        }
        HirExpr::JsxFragment(children, _) => {
            for child in children {
                scan_expr_for_handlers(child, async_fns, diags);
            }
        }

        // Recurse into other composite expressions.
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                scan_stmt_for_handlers(s, async_fns, diags);
            }
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            scan_expr_for_handlers(cond, async_fns, diags);
            for s in then_stmts {
                scan_stmt_for_handlers(s, async_fns, diags);
            }
            if let Some(else_body) = else_stmts {
                for s in else_body {
                    scan_stmt_for_handlers(s, async_fns, diags);
                }
            }
        }
        HirExpr::For(_, _, iter, body, key, _) => {
            scan_expr_for_handlers(iter, async_fns, diags);
            scan_expr_for_handlers(body, async_fns, diags);
            if let Some(k) = key {
                scan_expr_for_handlers(k, async_fns, diags);
            }
        }
        HirExpr::Lambda(_, _, body, _, _) => {
            scan_expr_for_handlers(body, async_fns, diags);
        }
        HirExpr::Binary(_, l, r, _) => {
            scan_expr_for_handlers(l, async_fns, diags);
            scan_expr_for_handlers(r, async_fns, diags);
        }
        HirExpr::Unary(_, inner, _) => {
            scan_expr_for_handlers(inner, async_fns, diags);
        }
        HirExpr::MethodCall(recv, _, args, _, _) => {
            scan_expr_for_handlers(recv, async_fns, diags);
            for arg in args {
                scan_expr_for_handlers(&arg.value, async_fns, diags);
            }
        }
        HirExpr::FieldAccess(inner, _, _) => {
            scan_expr_for_handlers(inner, async_fns, diags);
        }
        HirExpr::Index(obj, idx, _) => {
            scan_expr_for_handlers(obj, async_fns, diags);
            scan_expr_for_handlers(idx, async_fns, diags);
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                scan_expr_for_handlers(v, async_fns, diags);
            }
        }
        HirExpr::ListLit(items, _) => {
            for item in items {
                scan_expr_for_handlers(item, async_fns, diags);
            }
        }
        HirExpr::TupleLit(items, _) => {
            for item in items {
                scan_expr_for_handlers(item, async_fns, diags);
            }
        }
        HirExpr::Match(scrutinee, arms, _) => {
            scan_expr_for_handlers(scrutinee, async_fns, diags);
            for arm in arms {
                scan_expr_for_handlers(&arm.body, async_fns, diags);
            }
        }
        HirExpr::Try(t) => {
            scan_expr_for_handlers(&t.target, async_fns, diags);
        }
        HirExpr::Spawn(inner, _) => {
            scan_expr_for_handlers(inner, async_fns, diags);
        }
        HirExpr::With(a, b, _) => {
            scan_expr_for_handlers(a, async_fns, diags);
            scan_expr_for_handlers(b, async_fns, diags);
        }
        HirExpr::AsyncView(v) => {
            if let Some(a) = &v.fetching_arm {
                scan_expr_for_handlers(a, async_fns, diags);
            }
            if let Some(a) = &v.empty_arm {
                scan_expr_for_handlers(a, async_fns, diags);
            }
            if let Some(a) = &v.error_arm {
                scan_expr_for_handlers(a, async_fns, diags);
            }
            if let Some(a) = &v.ok_arm {
                scan_expr_for_handlers(a, async_fns, diags);
            }
        }
        // Leaf literals — nothing to recurse into.
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::Ident(..)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn scan_stmt_for_handlers(
    stmt: &HirStmt,
    async_fns: &HashSet<String>,
    diags: &mut Vec<Diagnostic>,
) {
    match stmt {
        HirStmt::Expr { expr, .. } => scan_expr_for_handlers(expr, async_fns, diags),
        HirStmt::Let { value, .. } => scan_expr_for_handlers(value, async_fns, diags),
        HirStmt::Return { value: Some(e), .. } => scan_expr_for_handlers(e, async_fns, diags),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Assign { value, .. } => scan_expr_for_handlers(value, async_fns, diags),
        HirStmt::While {
            condition, body, ..
        } => {
            scan_expr_for_handlers(condition, async_fns, diags);
            for s in body {
                scan_stmt_for_handlers(s, async_fns, diags);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                scan_stmt_for_handlers(s, async_fns, diags);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

// ── Helpers: detect calls inside a lambda body ───────────────────────────────

/// Returns true if `expr` (or any sub-expression) contains a Call to any name in `names`.
fn expr_calls_any_of(expr: &HirExpr, names: &HashSet<String>) -> bool {
    match expr {
        HirExpr::Call(callee, args, _, _) => {
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                if names.contains(name.as_str()) {
                    return true;
                }
            }
            if expr_calls_any_of(callee, names) {
                return true;
            }
            args.iter().any(|a| expr_calls_any_of(&a.value, names))
        }
        HirExpr::Block(stmts, _) => stmts.iter().any(|s| stmt_calls_any_of(s, names)),
        HirExpr::If(cond, then_s, else_s, _) => {
            expr_calls_any_of(cond, names)
                || then_s.iter().any(|s| stmt_calls_any_of(s, names))
                || else_s
                    .as_ref()
                    .is_some_and(|es| es.iter().any(|s| stmt_calls_any_of(s, names)))
        }
        HirExpr::Lambda(_, _, body, _, _) => expr_calls_any_of(body, names),
        HirExpr::Binary(_, l, r, _) => expr_calls_any_of(l, names) || expr_calls_any_of(r, names),
        HirExpr::Unary(_, inner, _) => expr_calls_any_of(inner, names),
        HirExpr::MethodCall(recv, _, args, _, _) => {
            expr_calls_any_of(recv, names)
                || args.iter().any(|a| expr_calls_any_of(&a.value, names))
        }
        HirExpr::FieldAccess(inner, _, _) => expr_calls_any_of(inner, names),
        HirExpr::Index(o, i, _) => expr_calls_any_of(o, names) || expr_calls_any_of(i, names),
        HirExpr::ListLit(items, _) => items.iter().any(|i| expr_calls_any_of(i, names)),
        HirExpr::TupleLit(items, _) => items.iter().any(|i| expr_calls_any_of(i, names)),
        HirExpr::ObjectLit(fields, _) => fields.iter().any(|(_, v)| expr_calls_any_of(v, names)),
        HirExpr::Match(s, arms, _) => {
            expr_calls_any_of(s, names) || arms.iter().any(|a| expr_calls_any_of(&a.body, names))
        }
        HirExpr::Try(t) => expr_calls_any_of(&t.target, names),
        HirExpr::Spawn(inner, _) => expr_calls_any_of(inner, names),
        HirExpr::With(a, b, _) => expr_calls_any_of(a, names) || expr_calls_any_of(b, names),
        HirExpr::For(_, _, iter, body, key, _) => {
            expr_calls_any_of(iter, names)
                || expr_calls_any_of(body, names)
                || key.as_ref().is_some_and(|k| expr_calls_any_of(k, names))
        }
        _ => false,
    }
}

fn stmt_calls_any_of(stmt: &HirStmt, names: &HashSet<String>) -> bool {
    match stmt {
        HirStmt::Expr { expr, .. } => expr_calls_any_of(expr, names),
        HirStmt::Let { value, .. } => expr_calls_any_of(value, names),
        HirStmt::Return { value: Some(e), .. } => expr_calls_any_of(e, names),
        HirStmt::Return { value: None, .. } => false,
        HirStmt::Assign { value, .. } => expr_calls_any_of(value, names),
        HirStmt::While {
            condition, body, ..
        } => {
            expr_calls_any_of(condition, names) || body.iter().any(|s| stmt_calls_any_of(s, names))
        }
        HirStmt::Loop { body, .. } => body.iter().any(|s| stmt_calls_any_of(s, names)),
        HirStmt::Break { .. } | HirStmt::Continue { .. } => false,
    }
}

/// Returns true if `expr` (or any sub-expression) contains a state-mutating `Assign` statement.
///
/// In Vox reactive components, `n = x` lowers to `HirStmt::Assign` where the target is an
/// `HirExpr::Ident` bound to the state variable. We conservatively flag any `Assign` inside
/// the lambda body as a potential post-unmount state write.
fn expr_has_state_assign(expr: &HirExpr) -> bool {
    match expr {
        HirExpr::Block(stmts, _) => stmts.iter().any(stmt_has_state_assign),
        HirExpr::If(cond, then_s, else_s, _) => {
            expr_has_state_assign(cond)
                || then_s.iter().any(stmt_has_state_assign)
                || else_s
                    .as_ref()
                    .is_some_and(|es| es.iter().any(stmt_has_state_assign))
        }
        HirExpr::Lambda(_, _, body, _, _) => expr_has_state_assign(body),
        HirExpr::Binary(_, l, r, _) => expr_has_state_assign(l) || expr_has_state_assign(r),
        HirExpr::Unary(_, inner, _) => expr_has_state_assign(inner),
        HirExpr::Call(callee, args, _, _) => {
            expr_has_state_assign(callee) || args.iter().any(|a| expr_has_state_assign(&a.value))
        }
        HirExpr::MethodCall(recv, _, args, _, _) => {
            expr_has_state_assign(recv) || args.iter().any(|a| expr_has_state_assign(&a.value))
        }
        HirExpr::FieldAccess(inner, _, _) => expr_has_state_assign(inner),
        HirExpr::Index(o, i, _) => expr_has_state_assign(o) || expr_has_state_assign(i),
        HirExpr::Match(s, arms, _) => {
            expr_has_state_assign(s) || arms.iter().any(|a| expr_has_state_assign(&a.body))
        }
        HirExpr::Try(t) => expr_has_state_assign(&t.target),
        HirExpr::With(a, b, _) => expr_has_state_assign(a) || expr_has_state_assign(b),
        HirExpr::Spawn(inner, _) => expr_has_state_assign(inner),
        _ => false,
    }
}

fn stmt_has_state_assign(stmt: &HirStmt) -> bool {
    match stmt {
        HirStmt::Assign { .. } => true,
        HirStmt::Expr { expr, .. } => expr_has_state_assign(expr),
        HirStmt::Let { value, .. } => expr_has_state_assign(value),
        HirStmt::Return { value: Some(e), .. } => expr_has_state_assign(e),
        HirStmt::Return { value: None, .. } => false,
        HirStmt::While {
            condition, body, ..
        } => expr_has_state_assign(condition) || body.iter().any(stmt_has_state_assign),
        HirStmt::Loop { body, .. } => body.iter().any(stmt_has_state_assign),
        HirStmt::Break { .. } | HirStmt::Continue { .. } => false,
    }
}

fn make_diag(span: crate::ast::span::Span) -> Diagnostic {
    Diagnostic {
        severity: TypeckSeverity::Warning,
        message: "async event handler calls an endpoint and then sets state without \
                  `@cancellable` — the state update may fire after the component unmounts. \
                  Annotate the handler `@cancellable fn(…) { … }` to suppress this warning."
            .to_string(),
        span,
        code: Some("lint.handler.uncancellable_async".into()),
        category: DiagnosticCategory::Lint,
        expected_type: None,
        found_type: None,
        context: None,
        suggestions: vec![
            "Add `@cancellable` before `fn` in the event handler lambda.".to_string(),
        ],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        ast_node_kind: Some("lambda".into()),
    }
}
