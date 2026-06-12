use std::collections::HashSet;
use vox_compiler::hir::*;
/// Walk an HIR expression tree and collect names of free-fn calls that match a known
/// set of identifiers (used for @endpoint imports — see [`generate_reactive_component`]).
pub(crate) fn collect_callee_refs(
    expr: &HirExpr,
    known: &HashSet<String>,
    out: &mut HashSet<String>,
) {
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

pub(crate) fn collect_callee_refs_stmt(
    stmt: &HirStmt,
    known: &HashSet<String>,
    out: &mut HashSet<String>,
) {
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
pub(crate) fn collect_jsx_component_refs(
    expr: &HirExpr,
    known: &HashSet<String>,
    out: &mut HashSet<String>,
) {
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

pub(crate) fn collect_jsx_component_refs_stmt(
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

/// Single source of truth for a component's cross-file import sets, shared by
/// the web (reactive) and React-Native component emitters so they never drift.
///
/// Walks the `view:` expression AND every member body — state initialisers,
/// derived expressions, effects, `on mount` / `on cleanup`, and prelude
/// statements — collecting:
/// - sibling component refs (PascalCase JSX tags in `known_components`), and
/// - endpoint-fn refs (free-fn calls in `endpoint_names`).
///
/// Self-references are removed. Both vectors are returned sorted for stable,
/// deterministic emit. Walking member bodies (not just the view + prelude)
/// matters because the common case — loading data in `on mount:` via a
/// `@query` fn — would otherwise emit a call with no matching import.
pub(crate) fn collect_component_import_refs(
    rc: &HirReactiveComponent,
    known_components: &HashSet<String>,
    endpoint_names: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let mut comps: HashSet<String> = HashSet::new();
    let mut endpoints: HashSet<String> = HashSet::new();

    let visit = |e: &HirExpr, comps: &mut HashSet<String>, endpoints: &mut HashSet<String>| {
        collect_jsx_component_refs(e, known_components, comps);
        collect_callee_refs(e, endpoint_names, endpoints);
    };

    if let Some(view) = &rc.view {
        visit(view, &mut comps, &mut endpoints);
    }
    for m in &rc.members {
        match m {
            HirReactiveMember::State(s) => visit(&s.init, &mut comps, &mut endpoints),
            HirReactiveMember::Derived(d) => visit(&d.expr, &mut comps, &mut endpoints),
            HirReactiveMember::Effect(e) => visit(&e.body, &mut comps, &mut endpoints),
            HirReactiveMember::OnMount(o) => visit(&o.body, &mut comps, &mut endpoints),
            HirReactiveMember::OnCleanup(o) => visit(&o.body, &mut comps, &mut endpoints),
            HirReactiveMember::Stmt(s) => {
                collect_jsx_component_refs_stmt(s, known_components, &mut comps);
                collect_callee_refs_stmt(s, endpoint_names, &mut endpoints);
            }
        }
    }

    comps.remove(&rc.name);
    let mut comps_v: Vec<String> = comps.into_iter().collect();
    comps_v.sort();
    let mut endpoints_v: Vec<String> = endpoints.into_iter().collect();
    endpoints_v.sort();
    (comps_v, endpoints_v)
