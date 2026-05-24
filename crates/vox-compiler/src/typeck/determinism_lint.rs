//! Lint: non-deterministic stdlib calls inside `workflow` bodies (Task 6.1).
//!
//! Workflow bodies must be replay-safe per ADR-019 §5 ("constrained
//! deterministic control-flow subset"). At replay time the durability
//! runtime re-runs the workflow from the journal — any call that produces
//! a different value on a second run (system time, RNG, UUIDs, process
//! spawn, …) breaks determinism and corrupts the replay.
//!
//! This pass walks every [`HirFn`] with `durability == Some(Workflow)` and
//! emits `lint.workflow.non_deterministic` for any call whose resolved
//! callee path matches a fixed blocklist. Activities and plain `fn`s are
//! exempt:
//! - **activities** record their result in the journal — replay returns the
//!   recorded value instead of re-running the body, so non-determinism is
//!   contained.
//! - **plain `fn`s** are not on the durability replay path at all.
//!
//! See `docs/superpowers/plans/2026-05-23-durable-functions-completion.md`
//! Task 6.1 for context.

use crate::hir::nodes::durability::DurabilityKind;
use crate::hir::{HirArg, HirExpr, HirModule, HirStmt, HirStringPart};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// Stdlib call paths that are non-deterministic and therefore forbidden
/// inside a workflow body. Path is the source-level dotted form (e.g.
/// `std.time.now_ms`). The blocklist is intentionally small and explicit;
/// expand it deliberately as new non-deterministic surfaces appear.
const NON_DETERMINISTIC_CALLS: &[&str] = &[
    "std.time.now_ms",
    "std.time.now_seconds",
    "std.random",
    "std.uuid",
    "std.process.spawn",
];

/// Run the workflow-determinism lint across all functions in `hir`.
///
/// Only `DurabilityKind::Workflow` bodies are inspected; activities and
/// plain fns are skipped (see module docs for rationale).
#[must_use]
pub fn check_workflow_determinism(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for f in &hir.functions {
        if f.durability != Some(DurabilityKind::Workflow) {
            continue;
        }
        for stmt in &f.body {
            walk_stmt(stmt, &mut diags);
        }
    }
    diags
}

/// If `callee` is a stdlib path matching the blocklist, return the matched
/// path. Otherwise `None`. Handles two HIR shapes:
/// - `MethodCall(receiver, method, ...)` — e.g. `std.time.now_ms()`,
///   `std.random()`, `std.uuid()`.
/// - `Call(FieldAccess(... , method), ...)` — fall-through form when the
///   call wasn't lowered as a method call.
fn callee_path(expr: &HirExpr) -> Option<String> {
    match expr {
        HirExpr::MethodCall(receiver, method, _args, _plan, _span) => {
            let mut path = ident_path(receiver)?;
            path.push_str(".");
            path.push_str(method);
            Some(path)
        }
        HirExpr::Call(callee, _args, _tail, _span) => {
            if let HirExpr::FieldAccess(inner, field, _) = callee.as_ref() {
                let mut path = ident_path(inner)?;
                path.push_str(".");
                path.push_str(field);
                Some(path)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Collapse a chain of `FieldAccess`/`Ident` into a dotted source path.
/// Returns `None` if the chain ever hits a non-ident, non-field node.
fn ident_path(expr: &HirExpr) -> Option<String> {
    match expr {
        HirExpr::Ident(name, _) => Some(name.clone()),
        HirExpr::FieldAccess(inner, field, _) => {
            let mut p = ident_path(inner)?;
            p.push_str(".");
            p.push_str(field);
            Some(p)
        }
        _ => None,
    }
}

fn emit_diag(path: &str, span: crate::ast::span::Span, diags: &mut Vec<Diagnostic>) {
    diags.push(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "workflow body calls non-deterministic stdlib function `{path}`. \
             Workflows must be replay-safe (ADR-019 §5); move this call into an \
             `activity` and invoke that activity from the workflow instead — the \
             journal will record its result so replay is deterministic."
        ),
        span,
        expected_type: None,
        found_type: None,
        context: None,
        suggestions: vec![format!(
            "Wrap the `{path}` call in an `activity` and call that activity from the workflow."
        )],
        category: DiagnosticCategory::Lint,
        code: Some("lint.workflow.non_deterministic".into()),
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        ast_node_kind: Some("workflow".into()),
    });
}

fn walk_expr(expr: &HirExpr, diags: &mut Vec<Diagnostic>) {
    // Check the call itself before recursing into its children — this
    // matches an offending call at the outermost layer it appears.
    if let Some(path) = callee_path(expr) {
        if NON_DETERMINISTIC_CALLS.contains(&path.as_str()) {
            let span = match expr {
                HirExpr::MethodCall(_, _, _, _, s) => *s,
                HirExpr::Call(_, _, _, s) => *s,
                _ => unreachable!(),
            };
            emit_diag(&path, span, diags);
        }
    }

    match expr {
        HirExpr::Call(callee, args, _tail, _span) => {
            walk_expr(callee, diags);
            for a in args {
                walk_arg(a, diags);
            }
        }
        HirExpr::MethodCall(receiver, _method, args, _plan, _span) => {
            walk_expr(receiver, diags);
            for a in args {
                walk_arg(a, diags);
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                walk_stmt(s, diags);
            }
        }
        HirExpr::Binary(_, lhs, rhs, _) => {
            walk_expr(lhs, diags);
            walk_expr(rhs, diags);
        }
        HirExpr::Unary(_, inner, _) => walk_expr(inner, diags),
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            walk_expr(cond, diags);
            for s in then_stmts {
                walk_stmt(s, diags);
            }
            if let Some(else_body) = else_stmts {
                for s in else_body {
                    walk_stmt(s, diags);
                }
            }
        }
        HirExpr::FieldAccess(inner, _, _) => walk_expr(inner, diags),
        HirExpr::Index(obj, idx, _) => {
            walk_expr(obj, diags);
            walk_expr(idx, diags);
        }
        HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
            for it in items {
                walk_expr(it, diags);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_k, v) in fields {
                walk_expr(v, diags);
            }
        }
        HirExpr::Lambda(_params, _ret, body, _cancel, _) => walk_expr(body, diags),
        HirExpr::Match(scrutinee, arms, _) => {
            walk_expr(scrutinee, diags);
            for arm in arms {
                walk_expr(&arm.body, diags);
            }
        }
        HirExpr::Try(t) => walk_expr(&t.target, diags),
        HirExpr::Spawn(inner, _) => walk_expr(inner, diags),
        HirExpr::With(a, b, _) => {
            walk_expr(a, diags);
            walk_expr(b, diags);
        }
        HirExpr::AsyncView(v) => {
            if let Some(a) = &v.fetching_arm {
                walk_expr(a, diags);
            }
            if let Some(a) = &v.empty_arm {
                walk_expr(a, diags);
            }
            if let Some(a) = &v.error_arm {
                walk_expr(a, diags);
            }
            if let Some(a) = &v.ok_arm {
                walk_expr(a, diags);
            }
        }
        HirExpr::For(_var, _idx, iter, body, _key, _) => {
            walk_expr(iter, diags);
            walk_expr(body, diags);
        }
        HirExpr::StringInterp { parts, .. } => {
            for p in parts {
                if let HirStringPart::Interpolation(e) = p {
                    walk_expr(e, diags);
                }
            }
        }
        // Leaves and JSX shapes — no sub-expressions worth walking for
        // determinism (JSX bodies are not durable workflow surfaces).
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::Ident(..)
        | HirExpr::Jsx(_)
        | HirExpr::JsxSelfClosing(_)
        | HirExpr::JsxFragment(..)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn walk_arg(arg: &HirArg, diags: &mut Vec<Diagnostic>) {
    walk_expr(&arg.value, diags);
}

fn walk_stmt(stmt: &HirStmt, diags: &mut Vec<Diagnostic>) {
    match stmt {
        HirStmt::Expr { expr, .. } => walk_expr(expr, diags),
        HirStmt::Let { value, .. } => walk_expr(value, diags),
        HirStmt::Return { value: Some(e), .. } => walk_expr(e, diags),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Assign { value, .. } => walk_expr(value, diags),
        HirStmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, diags);
            for s in body {
                walk_stmt(s, diags);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                walk_stmt(s, diags);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}
