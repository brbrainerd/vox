//! Recursive async-call detection for HIR event-handler emission.
//!
//! `stmt_has_async_call` and `expr_has_async_call` walk the HIR tree to determine whether
//! a statement or expression transitively calls any function in `async_fn_names`, without
//! crossing Lambda boundaries (lambdas have their own async scope).

use std::collections::HashSet;
use vox_compiler::hir::{HirExpr, HirStmt};

/// Returns `true` if `stmt` (or any expression recursively nested in it, excluding lambdas)
/// contains a call to one of `async_fn_names`.
pub fn stmt_has_async_call(stmt: &HirStmt, async_fn_names: &HashSet<String>) -> bool {
    match stmt {
        HirStmt::Let { value, .. } => expr_has_async_call(value, async_fn_names),
        HirStmt::Assign { value, .. } => expr_has_async_call(value, async_fn_names),
        HirStmt::Expr { expr, .. } => expr_has_async_call(expr, async_fn_names),
        HirStmt::Return { value, .. } => value
            .as_ref()
            .is_some_and(|v| expr_has_async_call(v, async_fn_names)),
        HirStmt::While {
            condition, body, ..
        } => {
            expr_has_async_call(condition, async_fn_names)
                || body.iter().any(|s| stmt_has_async_call(s, async_fn_names))
        }
        HirStmt::Loop { body, .. } => body.iter().any(|s| stmt_has_async_call(s, async_fn_names)),
        HirStmt::Break { .. } | HirStmt::Continue { .. } => false,
    }
}

/// Returns `true` if `expr` (or any sub-expression, excluding lambdas) contains a call to one
/// of `async_fn_names`.
pub fn expr_has_async_call(expr: &HirExpr, async_fn_names: &HashSet<String>) -> bool {
    match expr {
        // Direct call to an async fn: `parse_voice(...)` or `save(...)`
        HirExpr::Call(callee, args, _, _) => {
            // Check if this is a direct call to an async fn
            let is_async_call = if let HirExpr::Ident(name, _) = callee.as_ref() {
                async_fn_names.contains(name.as_str())
            } else {
                false
            };
            is_async_call
                || expr_has_async_call(callee, async_fn_names)
                || args
                    .iter()
                    .any(|a| expr_has_async_call(&a.value, async_fn_names))
        }
        // Method chain: `fetch_user().trim()` — walk receiver and args, but not across lambdas
        HirExpr::MethodCall(obj, _, args, _, _) => {
            expr_has_async_call(obj, async_fn_names)
                || args
                    .iter()
                    .any(|a| expr_has_async_call(&a.value, async_fn_names))
        }
        // Field access: `obj.field` — walk receiver
        HirExpr::FieldAccess(obj, _, _) => expr_has_async_call(obj, async_fn_names),
        // Index: `arr[i]`
        HirExpr::Index(obj, idx, _) => {
            expr_has_async_call(obj, async_fn_names) || expr_has_async_call(idx, async_fn_names)
        }
        // Binary/Unary operators
        HirExpr::Binary(_, lhs, rhs, _) => {
            expr_has_async_call(lhs, async_fn_names) || expr_has_async_call(rhs, async_fn_names)
        }
        HirExpr::Unary(_, inner, _) => expr_has_async_call(inner, async_fn_names),
        // Block: walk all stmts
        HirExpr::Block(stmts, _) => stmts.iter().any(|s| stmt_has_async_call(s, async_fn_names)),
        // If expression: walk condition + both branches
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            expr_has_async_call(cond, async_fn_names)
                || then_stmts
                    .iter()
                    .any(|s| stmt_has_async_call(s, async_fn_names))
                || else_stmts.as_ref().is_some_and(|stmts| {
                    stmts.iter().any(|s| stmt_has_async_call(s, async_fn_names))
                })
        }
        // Match expression: walk scrutinee and arm bodies
        HirExpr::Match(scrutinee, arms, _) => {
            expr_has_async_call(scrutinee, async_fn_names)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|g| expr_has_async_call(g, async_fn_names))
                        || expr_has_async_call(&arm.body, async_fn_names)
                })
        }
        // Try: walk the inner expression
        HirExpr::Try(t) => expr_has_async_call(&t.target, async_fn_names),
        // For loop: walk iterable and body
        HirExpr::For(_, _, iterable, body, _, _) => {
            expr_has_async_call(iterable, async_fn_names)
                || expr_has_async_call(body, async_fn_names)
        }
        // Object literal: walk values
        HirExpr::ObjectLit(fields, _) => fields
            .iter()
            .any(|(_, v)| expr_has_async_call(v, async_fn_names)),
        // List/Tuple literals: walk elements
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            elems.iter().any(|e| expr_has_async_call(e, async_fn_names))
        }
        // Spawn/With wrappers
        HirExpr::Spawn(inner, _) => expr_has_async_call(inner, async_fn_names),
        HirExpr::With(a, b, _) => {
            expr_has_async_call(a, async_fn_names) || expr_has_async_call(b, async_fn_names)
        }
        // Lambda: do NOT cross the lambda boundary — it has its own async scope
        HirExpr::Lambda(_, _, _, _, _) => false,
        // AsyncView: walk source and all optional arms
        HirExpr::AsyncView(v) => {
            expr_has_async_call(&v.source, async_fn_names)
                || v.fetching_arm
                    .as_ref()
                    .is_some_and(|e| expr_has_async_call(e, async_fn_names))
                || v.empty_arm
                    .as_ref()
                    .is_some_and(|e| expr_has_async_call(e, async_fn_names))
                || v.error_arm
                    .as_ref()
                    .is_some_and(|e| expr_has_async_call(e, async_fn_names))
                || v.ok_arm
                    .as_ref()
                    .is_some_and(|e| expr_has_async_call(e, async_fn_names))
        }
        // Leaf nodes: no sub-expressions to walk
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::Ident(..) => false,
        // JSX nodes: not in handler bodies, but walk for completeness
        HirExpr::Jsx(_) | HirExpr::JsxSelfClosing(_) | HirExpr::JsxFragment(_, _) => false,
        // WorkflowVersion is a leaf — no sub-expressions to walk
        HirExpr::WorkflowVersion(_) => false,
    }
}
