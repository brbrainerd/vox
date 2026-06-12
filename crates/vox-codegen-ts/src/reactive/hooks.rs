use std::collections::HashSet;
use vox_compiler::hir::*;
use vox_compiler::react_bridge::react_exports::{
    USE_CALLBACK, USE_EFFECT, USE_MEMO, USE_REF, USE_STATE,
};

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
pub(crate) fn collect_reactive_binding_names(members: &[HirReactiveMember]) -> HashSet<String> {
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

pub(crate) fn react_import_line(members: &[HirReactiveMember]) -> String {
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
