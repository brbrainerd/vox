//! Lint: every `effect:` block must have either resolvable automatic deps or an
//! explicit `depends_on (…)` clause. Fires `lint.effect.unresolvable_deps` if
//! the body calls a function that is not a known builtin and not a declared
//! component state name, making dep tracking ambiguous.
//!
//! Silenced by adding `effect depends_on (state_name, …):` to declare deps
//! explicitly.

use crate::hir::{HirArg, HirExpr, HirModule, HirReactiveComponent, HirReactiveMember, HirStmt};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use std::collections::HashSet;

/// Builtins that are always safe to call inside an effect (dep-tracking not needed).
const KNOWN_BUILTINS: &[&str] = &[
    "log",
    "print",
    "assert",
    "str",
    "int",
    "float",
    "bool",
    "len",
    "range",
    "type_of",
    "Ok",
    "Err",
    "Some",
    "None",
    "push",
    "pop",
    "set",
    "get",
    "env",
    "console",
    "Math",
    "JSON",
    "String",
    "Number",
    "Array",
    "Object",
    "Promise",
    "setTimeout",
    "clearTimeout",
    "setInterval",
    "clearInterval",
];

/// Run the effect dependency completeness lint across all reactive components.
pub fn check_effect_deps(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for c in &hir.components {
        check_component(c, &mut diags);
    }
    diags
}

fn check_component(c: &HirReactiveComponent, diags: &mut Vec<Diagnostic>) {
    // Collect all state names declared in this component.
    let mut state_names: HashSet<String> = HashSet::new();
    for m in &c.members {
        if let HirReactiveMember::State(s) = m {
            state_names.insert(s.name.clone());
        }
    }

    // Build the set of known-safe call targets: builtins + state names themselves
    // (reading a state value is always safe and trackable).
    let mut known: HashSet<String> = HashSet::new();
    for b in KNOWN_BUILTINS {
        known.insert(b.to_string());
    }
    for name in &state_names {
        known.insert(name.clone());
    }

    for m in &c.members {
        if let HirReactiveMember::Effect(e) = m {
            // If the user provided an explicit `depends_on (…)` clause, skip the lint.
            if e.explicit_deps.is_some() {
                continue;
            }

            let mut unknown_calls: Vec<String> = Vec::new();
            collect_unknown_calls(&e.body, &known, &mut unknown_calls);

            if !unknown_calls.is_empty() {
                unknown_calls.sort();
                unknown_calls.dedup();
                diags.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "effect calls {:?} which cannot be automatically tracked as a dependency. \
                         Dep tracking is ambiguous. Add `effect depends_on (…):` to declare deps \
                         explicitly, or annotate the called function as `@reactive`.",
                        unknown_calls
                    ),
                    span: e.span,
                    code: Some("lint.effect.unresolvable_deps".into()),
                    category: DiagnosticCategory::Lint,
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec![
                        "Add `effect depends_on (state_name):` to suppress this lint.".into(),
                    ],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: Some("effect".into()),
                });
            }
        }
    }
}

/// Walk an expression looking for calls to unknown (non-builtin, non-state) functions.
fn collect_unknown_calls(expr: &HirExpr, known: &HashSet<String>, out: &mut Vec<String>) {
    match expr {
        HirExpr::Call(callee, args, _tail, _span) => {
            // Check if the callee is a simple identifier that we don't know.
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                if !known.contains(name.as_str()) {
                    out.push(name.clone());
                }
            }
            // Recurse into callee and all arguments.
            collect_unknown_calls(callee, known, out);
            for arg in args {
                collect_unknown_calls_arg(arg, known, out);
            }
        }
        HirExpr::Block(stmts, _span) => {
            for s in stmts {
                collect_unknown_calls_stmt(s, known, out);
            }
        }
        HirExpr::Binary(_op, lhs, rhs, _span) => {
            collect_unknown_calls(lhs, known, out);
            collect_unknown_calls(rhs, known, out);
        }
        HirExpr::Unary(_op, inner, _span) => {
            collect_unknown_calls(inner, known, out);
        }
        HirExpr::If(cond, then_stmts, else_stmts, _span) => {
            collect_unknown_calls(cond, known, out);
            for s in then_stmts {
                collect_unknown_calls_stmt(s, known, out);
            }
            if let Some(else_body) = else_stmts {
                for s in else_body {
                    collect_unknown_calls_stmt(s, known, out);
                }
            }
        }
        HirExpr::MethodCall(receiver, _method, args, _plan, _span) => {
            collect_unknown_calls(receiver, known, out);
            for arg in args {
                collect_unknown_calls_arg(arg, known, out);
            }
        }
        HirExpr::FieldAccess(inner, _field, _span) => {
            collect_unknown_calls(inner, known, out);
        }
        HirExpr::Index(obj, idx, _span) => {
            collect_unknown_calls(obj, known, out);
            collect_unknown_calls(idx, known, out);
        }
        HirExpr::ListLit(items, _span) => {
            for item in items {
                collect_unknown_calls(item, known, out);
            }
        }
        HirExpr::TupleLit(items, _span) => {
            for item in items {
                collect_unknown_calls(item, known, out);
            }
        }
        HirExpr::ObjectLit(fields, _span) => {
            for (_key, val) in fields {
                collect_unknown_calls(val, known, out);
            }
        }
        HirExpr::Lambda(_params, _ret, body, _cancellable, _span) => {
            collect_unknown_calls(body, known, out);
        }
        HirExpr::Match(scrutinee, arms, _span) => {
            collect_unknown_calls(scrutinee, known, out);
            for arm in arms {
                collect_unknown_calls(&arm.body, known, out);
            }
        }
        HirExpr::Try(t) => {
            collect_unknown_calls(&t.target, known, out);
        }
        HirExpr::Spawn(inner, _span) => {
            collect_unknown_calls(inner, known, out);
        }
        HirExpr::With(a, b, _span) => {
            collect_unknown_calls(a, known, out);
            collect_unknown_calls(b, known, out);
        }
        HirExpr::AsyncView(v) => {
            if let Some(a) = &v.fetching_arm {
                collect_unknown_calls(a, known, out);
            }
            if let Some(a) = &v.empty_arm {
                collect_unknown_calls(a, known, out);
            }
            if let Some(a) = &v.error_arm {
                collect_unknown_calls(a, known, out);
            }
            if let Some(a) = &v.ok_arm {
                collect_unknown_calls(a, known, out);
            }
        }
        // Leaf nodes — no sub-expressions to recurse.
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::Ident(..) => {}
        // JSX variants — not expected in effect bodies, ignore.
        HirExpr::Jsx(_) | HirExpr::JsxSelfClosing(_) | HirExpr::JsxFragment(..) => {}
        // For loops — recurse into body.
        HirExpr::For(_var, _idx, iter, body, _key, _span) => {
            collect_unknown_calls(iter, known, out);
            collect_unknown_calls(body, known, out);
        }
        // workflow.version is a patch-marker leaf; no calls to track.
        HirExpr::WorkflowVersion(_) => {}
    }
}

fn collect_unknown_calls_arg(arg: &HirArg, known: &HashSet<String>, out: &mut Vec<String>) {
    collect_unknown_calls(&arg.value, known, out);
}

fn collect_unknown_calls_stmt(stmt: &HirStmt, known: &HashSet<String>, out: &mut Vec<String>) {
    match stmt {
        HirStmt::Expr { expr, .. } => collect_unknown_calls(expr, known, out),
        HirStmt::Let { value, .. } => collect_unknown_calls(value, known, out),
        HirStmt::Return { value: Some(e), .. } => collect_unknown_calls(e, known, out),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Assign { value, .. } => collect_unknown_calls(value, known, out),
        HirStmt::While {
            condition, body, ..
        } => {
            collect_unknown_calls(condition, known, out);
            for s in body {
                collect_unknown_calls_stmt(s, known, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                collect_unknown_calls_stmt(s, known, out);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}
