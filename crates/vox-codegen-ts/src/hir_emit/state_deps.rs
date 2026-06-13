//! Reactive `useMemo` / dependency lists: walks HIR for identifiers that reference `state` names.
//!
//! **Support-only API (OP-0134):** used from [`super::super::reactive`]; not part of the Web IR
//! surface. Prefer keeping logic here instead of duplicating walks in preview emit.
//!
//! ## Cross-call dep tracking (Phase E of the Svelte-mineable features plan)
//!
//! By default the walker stops at function-call boundaries: a `derived label = format(count)`
//! where `format` is a free function does not see the read of `count` through the call.
//! [`extract_state_deps_with_callees`] takes a map of `@reactive`-annotated free functions
//! (their `name -> body`) and recurses into them when called from the analyzed expression.
//!
//! Bounding (per the user's "K-complexity" guidance):
//! - Single-level recursion only — a `@reactive` function body is analyzed for reactive reads,
//!   but calls *out* of it are not followed further. (Eliminates pathological recursion blow-up
//!   without paying for whole-program escape analysis.)
//! - A `visited` set prevents infinite recursion on direct or mutual recursion.
//! - Functions without `@reactive` are not descended into; the call site contributes no deps
//!   from inside the callee. (Conservative under-tracking, opt-in extension.)

use std::collections::{HashMap, HashSet};
use vox_compiler::hir::*;

/// Backward-compatible entry point: walk `expr` collecting reads of `state_names` without
/// following any function calls. Retained for callers that don't need the cross-call
/// analysis or diagnostic surface; the active codegen path uses
/// [`extract_state_deps_with_diagnostics`] directly.
#[must_use]
#[allow(dead_code)] // back-compat shim; tests below exercise it
pub fn extract_state_deps(expr: &HirExpr, state_names: &HashSet<String>) -> Vec<String> {
    extract_state_deps_with_callees(expr, state_names, &HashMap::new())
}

/// Walk `expr` collecting reads of `state_names`, recursing one level into the bodies of
/// any function calls that resolve to a name in `reactive_callees` (the
/// `@reactive`-annotated free functions visible to the caller). See module docs for the
/// bounding policy.
#[must_use]
#[allow(dead_code)] // back-compat shim; tests below exercise it
pub fn extract_state_deps_with_callees(
    expr: &HirExpr,
    state_names: &HashSet<String>,
    reactive_callees: &HashMap<String, Vec<HirStmt>>,
) -> Vec<String> {
    extract_state_deps_with_diagnostics(expr, state_names, reactive_callees, &HashSet::new()).deps
}

/// Result of [`extract_state_deps_with_diagnostics`]: collected deps plus the names of
/// **unannotated** in-module functions called from inside `expr`. Each entry signals a
/// `dep_inference.over_track` situation (Phase E tier-2): the analyzer cannot recurse into
/// the callee's body and any reactive-state read inside it is invisible to the dep array.
/// Authors silence the hint by adding `@reactive` to the callee.
#[derive(Debug, Default)]
pub struct DepAnalysis {
    /// Sorted deduplicated list of reactive-binding names referenced by `expr` (or by
    /// `@reactive` callees recursed into).
    pub deps: Vec<String>,
    /// Sorted deduplicated list of in-module free-function names called from `expr` that
    /// (a) exist in `visible_fn_names`, (b) are not in `reactive_callees`. Empty when
    /// every visible call site is opt-in or the body has no calls to in-module functions.
    pub unannotated_calls: Vec<String>,
}

/// Like [`extract_state_deps_with_callees`] but also reports unannotated in-module calls
/// for the Phase E `dep_inference.over_track` hint surface. `visible_fn_names` should be
/// the set of all `fn` declarations in the enclosing module so cross-call analysis can
/// distinguish "in-module free function not annotated `@reactive`" (worth flagging) from
/// "method call / stdlib call / unknown identifier" (silent).
#[must_use]
pub fn extract_state_deps_with_diagnostics(
    expr: &HirExpr,
    state_names: &HashSet<String>,
    reactive_callees: &HashMap<String, Vec<HirStmt>>,
    visible_fn_names: &HashSet<String>,
) -> DepAnalysis {
    let mut deps = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut unannotated: HashSet<String> = HashSet::new();
    collect_deps_and_calls(
        expr,
        state_names,
        reactive_callees,
        visible_fn_names,
        &mut visited,
        &mut deps,
        &mut unannotated,
    );
    let mut sorted_deps: Vec<String> = deps.into_iter().collect();
    sorted_deps.sort();
    let mut sorted_calls: Vec<String> = unannotated.into_iter().collect();
    sorted_calls.sort();
    DepAnalysis {
        deps: sorted_deps,
        unannotated_calls: sorted_calls,
    }
}

fn collect_deps_and_calls(
    expr: &HirExpr,
    state_names: &HashSet<String>,
    reactive_callees: &HashMap<String, Vec<HirStmt>>,
    visible_fn_names: &HashSet<String>,
    visited: &mut HashSet<String>,
    deps: &mut HashSet<String>,
    unannotated: &mut HashSet<String>,
) {
    match expr {
        HirExpr::Ident(name, _) if state_names.contains(name) => {
            deps.insert(name.clone());
        }
        HirExpr::Binary(_, left, right, _) => {
            collect_deps_and_calls(
                left,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            collect_deps_and_calls(
                right,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::Unary(_, e, _) => {
            collect_deps_and_calls(
                e,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::Block(stmts, _) => {
            for stmt in stmts {
                collect_deps_and_calls_stmt(
                    stmt,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::Jsx(el) => {
            for attr in &el.attributes {
                collect_deps_and_calls(
                    &attr.value,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
            for child in &el.children {
                collect_deps_and_calls(
                    child,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for attr in &el.attributes {
                collect_deps_and_calls(
                    &attr.value,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::JsxFragment(children, _) => {
            for child in children {
                collect_deps_and_calls(
                    child,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, val) in fields {
                collect_deps_and_calls(
                    val,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            for e in elems {
                collect_deps_and_calls(
                    e,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::Call(callee, args, _, _) => {
            collect_deps_and_calls(
                callee,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            for arg in args {
                collect_deps_and_calls(
                    &arg.value,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
            // If the callee resolves to a known in-module free function, classify it.
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                if !visited.contains(name) {
                    if let Some(body) = reactive_callees.get(name) {
                        // `@reactive`: descend one level. Visited set bounds recursion depth
                        // and prevents infinite loops from direct or mutual recursion.
                        visited.insert(name.clone());
                        for stmt in body {
                            collect_deps_and_calls_stmt(
                                stmt,
                                state_names,
                                reactive_callees,
                                visible_fn_names,
                                visited,
                                deps,
                                unannotated,
                            );
                        }
                    } else if visible_fn_names.contains(name) {
                        // Known in-module fn without `@reactive`: flag for the
                        // dep_inference.over_track hint surface (Phase E tier-2). Method
                        // calls / stdlib calls / unknown identifiers stay silent.
                        unannotated.insert(name.clone());
                    }
                }
            }
        }
        HirExpr::MethodCall(_, _, args, Some(_), _) => {
            for arg in args {
                collect_deps_and_calls(
                    &arg.value,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::MethodCall(obj, _, args, _, _) => {
            collect_deps_and_calls(
                obj,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            for arg in args {
                collect_deps_and_calls(
                    &arg.value,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::FieldAccess(obj, _, _) => {
            collect_deps_and_calls(
                obj,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::If(cond, then_body, else_body, _) => {
            collect_deps_and_calls(
                cond,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            for stmt in then_body {
                collect_deps_and_calls_stmt(
                    stmt,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
            if let Some(estmts) = else_body {
                for stmt in estmts {
                    collect_deps_and_calls_stmt(
                        stmt,
                        state_names,
                        reactive_callees,
                        visible_fn_names,
                        visited,
                        deps,
                        unannotated,
                    );
                }
            }
        }
        HirExpr::For(_, _, iterable, body, _, _) => {
            collect_deps_and_calls(
                iterable,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            collect_deps_and_calls(
                body,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::Lambda(_, _, body, _, _) => {
            collect_deps_and_calls(
                body,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::Match(subject, arms, _) => {
            collect_deps_and_calls(
                subject,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            for arm in arms {
                collect_deps_and_calls(
                    &arm.body,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirExpr::With(left, right, _) => {
            collect_deps_and_calls(
                left,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            collect_deps_and_calls(
                right,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::Spawn(e, _) => {
            collect_deps_and_calls(
                e,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirExpr::Index(obj, idx, _) => {
            collect_deps_and_calls(
                obj,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            collect_deps_and_calls(
                idx,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        _ => {}
    }
}

fn collect_deps_and_calls_stmt(
    stmt: &HirStmt,
    state_names: &HashSet<String>,
    reactive_callees: &HashMap<String, Vec<HirStmt>>,
    visible_fn_names: &HashSet<String>,
    visited: &mut HashSet<String>,
    deps: &mut HashSet<String>,
    unannotated: &mut HashSet<String>,
) {
    match stmt {
        HirStmt::Let { value, .. } => {
            collect_deps_and_calls(
                value,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirStmt::Assign { target, value, .. } => {
            collect_deps_and_calls(
                target,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            collect_deps_and_calls(
                value,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirStmt::Expr { expr, .. } => {
            collect_deps_and_calls(
                expr,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
        }
        HirStmt::Return { value, .. } => {
            if let Some(v) = value {
                collect_deps_and_calls(
                    v,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirStmt::While {
            condition, body, ..
        } => {
            collect_deps_and_calls(
                condition,
                state_names,
                reactive_callees,
                visible_fn_names,
                visited,
                deps,
                unannotated,
            );
            for s in body {
                collect_deps_and_calls_stmt(
                    s,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                collect_deps_and_calls_stmt(
                    s,
                    state_names,
                    reactive_callees,
                    visible_fn_names,
                    visited,
                    deps,
                    unannotated,
                );
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_state_deps, extract_state_deps_with_callees};
    use std::collections::{HashMap, HashSet};
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{HirArg, HirBinOp, HirExpr, HirStmt};

    fn sp() -> Span {
        Span::new(0, 0)
    }

    #[test]
    fn extract_state_deps_finds_state_in_binary() {
        let state: HashSet<String> = HashSet::from(["count".into()]);
        let expr = HirExpr::Binary(
            HirBinOp::Add,
            Box::new(HirExpr::Ident("count".into(), sp())),
            Box::new(HirExpr::Ident("n".into(), sp())),
            sp(),
        );
        let deps = extract_state_deps(&expr, &state);
        assert_eq!(deps, vec!["count".to_string()]);
    }

    #[test]
    fn cross_call_recursion_into_reactive_callee_finds_state() {
        // derived label = format(count) where format is `@reactive fn format(c) { c + 1 }`
        // — without the callee descent the dep set is empty (caller doesn't read count
        // directly); with the descent we should see `count`.
        let state: HashSet<String> = HashSet::from(["count".into()]);
        let call = HirExpr::Call(
            Box::new(HirExpr::Ident("format".into(), sp())),
            vec![HirArg {
                name: None,
                value: HirExpr::Ident("count".into(), sp()),
            }],
            false,
            sp(),
        );
        // Body: just an expression statement that reads `count`.
        let format_body = vec![HirStmt::Expr {
            expr: HirExpr::Ident("count".into(), sp()),
            span: sp(),
        }];
        let mut callees: HashMap<String, Vec<HirStmt>> = HashMap::new();
        callees.insert("format".into(), format_body);

        let with = extract_state_deps_with_callees(&call, &state, &callees);
        assert_eq!(with, vec!["count".to_string()]);

        // Sanity: without the callees map, the same expression already finds `count` because
        // it appears as a direct call argument — but the body-internal read would be invisible
        // if the arg didn't reference state. Verify the body-only path:
        let opaque_arg_call = HirExpr::Call(
            Box::new(HirExpr::Ident("format".into(), sp())),
            vec![], // no args mentioning state
            false,
            sp(),
        );
        let body_only = extract_state_deps_with_callees(&opaque_arg_call, &state, &callees);
        assert_eq!(body_only, vec!["count".to_string()]);
        // Without the callees map the body-only path returns nothing.
        let no_callees = extract_state_deps(&opaque_arg_call, &state);
        assert!(no_callees.is_empty());
    }

    #[test]
    fn cross_call_recursion_does_not_loop_on_self_reference() {
        // A `@reactive` function that calls itself shouldn't blow the stack.
        let state: HashSet<String> = HashSet::from(["x".into()]);
        let call = HirExpr::Call(
            Box::new(HirExpr::Ident("recurse".into(), sp())),
            vec![],
            false,
            sp(),
        );
        let recurse_body = vec![HirStmt::Expr {
            expr: HirExpr::Call(
                Box::new(HirExpr::Ident("recurse".into(), sp())),
                vec![HirArg {
                    name: None,
                    value: HirExpr::Ident("x".into(), sp()),
                }],
                false,
                sp(),
            ),
            span: sp(),
        }];
        let mut callees: HashMap<String, Vec<HirStmt>> = HashMap::new();
        callees.insert("recurse".into(), recurse_body);

        let deps = extract_state_deps_with_callees(&call, &state, &callees);
        assert_eq!(deps, vec!["x".to_string()]);
    }

    #[test]
    fn extract_state_deps_sorts_and_dedupes() {
        let state: HashSet<String> = HashSet::from(["a".into(), "b".into()]);
        let inner = HirExpr::Binary(
            HirBinOp::Add,
            Box::new(HirExpr::Ident("a".into(), sp())),
            Box::new(HirExpr::Ident("b".into(), sp())),
            sp(),
        );
        let expr = HirExpr::Binary(
            HirBinOp::Add,
            Box::new(inner),
            Box::new(HirExpr::Ident("a".into(), sp())),
            sp(),
        );
        let deps = extract_state_deps(&expr, &state);
        assert_eq!(deps, vec!["a".to_string(), "b".to_string()]);
    }
}
