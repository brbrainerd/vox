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
use crate::hir::{HirArg, HirExpr, HirFn, HirModule, HirStmt};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use std::collections::{HashMap, HashSet};

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
/// plain fns are not themselves entry points (see module docs for
/// rationale). However, plain helper fns called *from* a workflow are
/// walked transitively, because a workflow that calls
/// `fn helper() { std.time.now_ms() }` is just as non-deterministic as
/// one that inlines the call. Activities encountered during traversal
/// are NOT recursed into — their result is journalled, so internal
/// non-determinism is replay-safe.
///
/// Cycles in the call graph are broken by a `visited` set keyed on
/// function name.
#[must_use]
pub fn check_workflow_determinism(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let by_name: HashMap<&str, &HirFn> =
        hir.functions.iter().map(|f| (f.name.as_str(), f)).collect();
    let mut diags = Vec::new();
    for f in &hir.functions {
        if f.durability != Some(DurabilityKind::Workflow) {
            continue;
        }
        let mut visited: HashSet<&str> = HashSet::new();
        visited.insert(f.name.as_str());
        for stmt in &f.body {
            walk_stmt(stmt, &by_name, &mut visited, &mut diags);
        }
    }
    diags
}

/// If `callee` is a callable expression we can resolve to a dotted source
/// path, return it. Otherwise `None`. Handles three HIR shapes:
/// - `MethodCall(receiver, method, ...)` — e.g. `std.time.now_ms()`,
///   `std.random()`, `std.uuid()`.
/// - `Call(FieldAccess(... , method), ...)` — fall-through form when the
///   call wasn't lowered as a method call.
/// - `Call(Ident("helper"), ...)` — bare-name calls to user-defined
///   functions. Required by the M-6 transitive walk so we can resolve
///   the callee against `by_name` and recurse into its body.
fn callee_path(expr: &HirExpr) -> Option<String> {
    match expr {
        HirExpr::MethodCall(receiver, method, _args, _plan, _span) => {
            let mut path = ident_path(receiver)?;
            path.push('.');
            path.push_str(method);
            Some(path)
        }
        HirExpr::Call(callee, _args, _tail, _span) => match callee.as_ref() {
            HirExpr::FieldAccess(inner, field, _) => {
                let mut path = ident_path(inner)?;
                path.push('.');
                path.push_str(field);
                Some(path)
            }
            HirExpr::Ident(name, _) => Some(name.clone()),
            _ => None,
        },
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
            p.push('.');
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

fn walk_expr<'a>(
    expr: &'a HirExpr,
    by_name: &HashMap<&'a str, &'a HirFn>,
    visited: &mut HashSet<&'a str>,
    diags: &mut Vec<Diagnostic>,
) {
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
        } else if let Some(callee) = by_name.get(path.as_str()).copied() {
            // M-6 transitive walk: a workflow that calls a plain helper
            // fn which itself does `std.time.now_ms()` is just as
            // non-deterministic as one that inlines the call. Recurse
            // into the callee's body. Activities are exempt — their
            // result is journalled, so internal non-determinism is
            // replay-safe (see module docs). `visited` breaks cycles in
            // the call graph by name.
            if callee.durability != Some(DurabilityKind::Activity)
                && visited.insert(callee.name.as_str())
            {
                for s in &callee.body {
                    walk_stmt(s, by_name, visited, diags);
                }
            }
        }
    }

    match expr {
        HirExpr::Call(callee, args, _tail, _span) => {
            walk_expr(callee, by_name, visited, diags);
            for a in args {
                walk_arg(a, by_name, visited, diags);
            }
        }
        HirExpr::MethodCall(receiver, _method, args, _plan, _span) => {
            walk_expr(receiver, by_name, visited, diags);
            for a in args {
                walk_arg(a, by_name, visited, diags);
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                walk_stmt(s, by_name, visited, diags);
            }
        }
        HirExpr::Binary(_, lhs, rhs, _) => {
            walk_expr(lhs, by_name, visited, diags);
            walk_expr(rhs, by_name, visited, diags);
        }
        HirExpr::Unary(_, inner, _) => walk_expr(inner, by_name, visited, diags),
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            walk_expr(cond, by_name, visited, diags);
            for s in then_stmts {
                walk_stmt(s, by_name, visited, diags);
            }
            if let Some(else_body) = else_stmts {
                for s in else_body {
                    walk_stmt(s, by_name, visited, diags);
                }
            }
        }
        HirExpr::FieldAccess(inner, _, _) => walk_expr(inner, by_name, visited, diags),
        HirExpr::Index(obj, idx, _) => {
            walk_expr(obj, by_name, visited, diags);
            walk_expr(idx, by_name, visited, diags);
        }
        HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
            for it in items {
                walk_expr(it, by_name, visited, diags);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_k, v) in fields {
                walk_expr(v, by_name, visited, diags);
            }
        }
        HirExpr::Lambda(_params, _ret, body, _cancel, _) => {
            walk_expr(body, by_name, visited, diags)
        }
        HirExpr::Match(scrutinee, arms, _) => {
            walk_expr(scrutinee, by_name, visited, diags);
            for arm in arms {
                walk_expr(&arm.body, by_name, visited, diags);
            }
        }
        HirExpr::Try(t) => walk_expr(&t.target, by_name, visited, diags),
        HirExpr::Spawn(inner, _) => walk_expr(inner, by_name, visited, diags),
        HirExpr::With(a, b, _) => {
            walk_expr(a, by_name, visited, diags);
            walk_expr(b, by_name, visited, diags);
        }
        HirExpr::AsyncView(v) => {
            if let Some(a) = &v.fetching_arm {
                walk_expr(a, by_name, visited, diags);
            }
            if let Some(a) = &v.empty_arm {
                walk_expr(a, by_name, visited, diags);
            }
            if let Some(a) = &v.error_arm {
                walk_expr(a, by_name, visited, diags);
            }
            if let Some(a) = &v.ok_arm {
                walk_expr(a, by_name, visited, diags);
            }
        }
        HirExpr::For(_var, _idx, iter, body, _key, _) => {
            walk_expr(iter, by_name, visited, diags);
            walk_expr(body, by_name, visited, diags);
        }
        // Note: HIR's lowering collapses `Expr::StringInterp` into a chain of
        // `HirExpr::Binary(Add, ...)` nodes (see `hir/lower/expr.rs:298`),
        // so there is no `HirExpr::StringInterp` variant to walk here — the
        // recursive Binary walk below covers interpolation expressions.
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

fn walk_arg<'a>(
    arg: &'a HirArg,
    by_name: &HashMap<&'a str, &'a HirFn>,
    visited: &mut HashSet<&'a str>,
    diags: &mut Vec<Diagnostic>,
) {
    walk_expr(&arg.value, by_name, visited, diags);
}

fn walk_stmt<'a>(
    stmt: &'a HirStmt,
    by_name: &HashMap<&'a str, &'a HirFn>,
    visited: &mut HashSet<&'a str>,
    diags: &mut Vec<Diagnostic>,
) {
    match stmt {
        HirStmt::Expr { expr, .. } => walk_expr(expr, by_name, visited, diags),
        HirStmt::Let { value, .. } => walk_expr(value, by_name, visited, diags),
        HirStmt::Return { value: Some(e), .. } => walk_expr(e, by_name, visited, diags),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Assign { value, .. } => walk_expr(value, by_name, visited, diags),
        HirStmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, by_name, visited, diags);
            for s in body {
                walk_stmt(s, by_name, visited, diags);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                walk_stmt(s, by_name, visited, diags);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}
