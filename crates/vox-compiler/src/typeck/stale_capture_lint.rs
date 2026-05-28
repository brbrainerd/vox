//! Lint: stale closure captures in lifecycle bodies.
//!
//! Fires `lint.closure.stale_capture` (Warning) when a Lambda inside an
//! `on_mount:` or an `effect:` without an explicit `depends_on` clause captures
//! a component state variable by name. The closure binds at mount/effect-run
//! time and will silently read the stale value from that snapshot rather than
//! the live reactive value.
//!
//! Suppressed by adding `effect depends_on (state_name, …):` to any `effect:`.
//! (`on_mount:` has no suppress mechanism — the lint always fires there.)

use crate::hir::{HirArg, HirExpr, HirModule, HirStmt};
use crate::hir::{HirReactiveComponent, HirReactiveMember};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use std::collections::HashSet;

/// Run the stale-capture lint across all reactive components in the module.
pub fn check_stale_captures(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for c in &hir.components {
        check_component(c, &mut diags);
    }
    diags
}

fn check_component(c: &HirReactiveComponent, diags: &mut Vec<Diagnostic>) {
    // Collect all state variable names declared in this component.
    let state_names: HashSet<String> = c
        .members
        .iter()
        .filter_map(|m| {
            if let HirReactiveMember::State(s) = m {
                Some(s.name.clone())
            } else {
                None
            }
        })
        .collect();

    if state_names.is_empty() {
        return;
    }

    for m in &c.members {
        match m {
            HirReactiveMember::OnMount(om) => {
                let mut captures: Vec<String> = Vec::new();
                find_stale_captures_in_expr(&om.body, &state_names, false, &mut captures);
                if !captures.is_empty() {
                    captures.sort();
                    captures.dedup();
                    diags.push(make_diag(&captures, om.span));
                }
            }
            HirReactiveMember::Effect(e) if e.explicit_deps.is_none() => {
                let mut captures: Vec<String> = Vec::new();
                find_stale_captures_in_expr(&e.body, &state_names, false, &mut captures);
                if !captures.is_empty() {
                    captures.sort();
                    captures.dedup();
                    diags.push(make_diag(&captures, e.span));
                }
            }
            _ => {}
        }
    }
}

fn make_diag(captures: &[String], span: crate::ast::span::Span) -> Diagnostic {
    Diagnostic {
        severity: TypeckSeverity::Warning,
        message: format!(
            "closure inside lifecycle block captures state {:?} by value at mount/run time — \
             reads will be stale after state updates. \
             Use `effect depends_on ({names}):` to re-run on changes, or access state \
             via a reactive binding outside the closure.",
            captures,
            names = captures.join(", "),
        ),
        span,
        code: Some("lint.closure.stale_capture".into()),
        category: DiagnosticCategory::Lint,
        expected_type: None,
        found_type: None,
        context: None,
        suggestions: vec![format!(
            "Add `effect depends_on ({}):` to re-run the effect when state changes.",
            captures.join(", ")
        )],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        ast_node_kind: Some("on_mount_or_effect".into()),
    }
}

/// Walk `expr` recursively.
///
/// `inside_lambda` is `true` once we have entered a `Lambda` node.  When it is
/// `true` any `Ident` whose name is in `states` is a stale capture.
fn find_stale_captures_in_expr(
    expr: &HirExpr,
    states: &HashSet<String>,
    inside_lambda: bool,
    out: &mut Vec<String>,
) {
    match expr {
        // ── Leaf that may be a stale capture ─────────────────────────────────
        HirExpr::Ident(name, _) => {
            if inside_lambda && states.contains(name.as_str()) {
                out.push(name.clone());
            }
        }

        // ── Lambda — flip the flag for the body ──────────────────────────────
        HirExpr::Lambda(_params, _ret, body, _cancellable, _span) => {
            find_stale_captures_in_expr(body, states, true, out);
        }

        // ── Composite expressions — recurse, preserving the flag ─────────────
        HirExpr::Call(callee, args, _tail, _span) => {
            find_stale_captures_in_expr(callee, states, inside_lambda, out);
            for arg in args {
                find_stale_captures_in_arg(arg, states, inside_lambda, out);
            }
        }
        HirExpr::Block(stmts, _span) => {
            for s in stmts {
                find_stale_captures_in_stmt(s, states, inside_lambda, out);
            }
        }
        HirExpr::Binary(_op, lhs, rhs, _span) => {
            find_stale_captures_in_expr(lhs, states, inside_lambda, out);
            find_stale_captures_in_expr(rhs, states, inside_lambda, out);
        }
        HirExpr::Unary(_op, inner, _span) => {
            find_stale_captures_in_expr(inner, states, inside_lambda, out);
        }
        HirExpr::If(cond, then_stmts, else_stmts, _span) => {
            find_stale_captures_in_expr(cond, states, inside_lambda, out);
            for s in then_stmts {
                find_stale_captures_in_stmt(s, states, inside_lambda, out);
            }
            if let Some(else_body) = else_stmts {
                for s in else_body {
                    find_stale_captures_in_stmt(s, states, inside_lambda, out);
                }
            }
        }
        HirExpr::MethodCall(receiver, _method, args, _plan, _span) => {
            find_stale_captures_in_expr(receiver, states, inside_lambda, out);
            for arg in args {
                find_stale_captures_in_arg(arg, states, inside_lambda, out);
            }
        }
        HirExpr::FieldAccess(inner, _field, _span) => {
            find_stale_captures_in_expr(inner, states, inside_lambda, out);
        }
        HirExpr::Index(obj, idx, _span) => {
            find_stale_captures_in_expr(obj, states, inside_lambda, out);
            find_stale_captures_in_expr(idx, states, inside_lambda, out);
        }
        HirExpr::ListLit(items, _span) => {
            for item in items {
                find_stale_captures_in_expr(item, states, inside_lambda, out);
            }
        }
        HirExpr::TupleLit(items, _span) => {
            for item in items {
                find_stale_captures_in_expr(item, states, inside_lambda, out);
            }
        }
        HirExpr::ObjectLit(fields, _span) => {
            for (_key, val) in fields {
                find_stale_captures_in_expr(val, states, inside_lambda, out);
            }
        }
        HirExpr::Match(scrutinee, arms, _span) => {
            find_stale_captures_in_expr(scrutinee, states, inside_lambda, out);
            for arm in arms {
                find_stale_captures_in_expr(&arm.body, states, inside_lambda, out);
            }
        }
        HirExpr::Try(t) => {
            find_stale_captures_in_expr(&t.target, states, inside_lambda, out);
        }
        HirExpr::Spawn(inner, _span) => {
            find_stale_captures_in_expr(inner, states, inside_lambda, out);
        }
        HirExpr::With(a, b, _span) => {
            find_stale_captures_in_expr(a, states, inside_lambda, out);
            find_stale_captures_in_expr(b, states, inside_lambda, out);
        }
        HirExpr::For(_var, _idx, iter, body, _key, _span) => {
            find_stale_captures_in_expr(iter, states, inside_lambda, out);
            find_stale_captures_in_expr(body, states, inside_lambda, out);
        }

        HirExpr::AsyncView(v) => {
            if let Some(a) = &v.fetching_arm {
                find_stale_captures_in_expr(a, states, inside_lambda, out);
            }
            if let Some(a) = &v.empty_arm {
                find_stale_captures_in_expr(a, states, inside_lambda, out);
            }
            if let Some(a) = &v.error_arm {
                find_stale_captures_in_expr(a, states, inside_lambda, out);
            }
            if let Some(a) = &v.ok_arm {
                find_stale_captures_in_expr(a, states, inside_lambda, out);
            }
        }

        // ── Leaf literals — nothing to capture ───────────────────────────────
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::DecimalLit(..) => {}

        // ── JSX — not expected in lifecycle bodies; ignore ────────────────────
        HirExpr::Jsx(_) | HirExpr::JsxSelfClosing(_) | HirExpr::JsxFragment(..) => {}

        // workflow.version is a leaf marker; no captures possible.
        HirExpr::WorkflowVersion(_) => {}
    }
}

fn find_stale_captures_in_arg(
    arg: &HirArg,
    states: &HashSet<String>,
    inside_lambda: bool,
    out: &mut Vec<String>,
) {
    find_stale_captures_in_expr(&arg.value, states, inside_lambda, out);
}

fn find_stale_captures_in_stmt(
    stmt: &HirStmt,
    states: &HashSet<String>,
    inside_lambda: bool,
    out: &mut Vec<String>,
) {
    match stmt {
        HirStmt::Expr { expr, .. } => {
            find_stale_captures_in_expr(expr, states, inside_lambda, out);
        }
        HirStmt::Let { value, .. } => {
            find_stale_captures_in_expr(value, states, inside_lambda, out);
        }
        HirStmt::Return { value: Some(e), .. } => {
            find_stale_captures_in_expr(e, states, inside_lambda, out);
        }
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Assign { value, .. } => {
            find_stale_captures_in_expr(value, states, inside_lambda, out);
        }
        HirStmt::While {
            condition, body, ..
        } => {
            find_stale_captures_in_expr(condition, states, inside_lambda, out);
            for s in body {
                find_stale_captures_in_stmt(s, states, inside_lambda, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                find_stale_captures_in_stmt(s, states, inside_lambda, out);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}
