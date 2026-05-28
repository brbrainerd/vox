//! Vox `use_*` hook surface ↔ React runtime: naming registry, AST walks, lint toggles.

use crate::ast::expr::{Expr, StringPart};
use crate::ast::span::Span;
use crate::ast::stmt::Stmt;

/// Vox sources use snake_case `use_*`; stable built-ins map to React camelCase exports.
pub const VOX_TO_REACT_HOOK: &[(&str, &str)] = &[
    ("use_state", "useState"),
    ("use_effect", "useEffect"),
    ("use_memo", "useMemo"),
    ("use_ref", "useRef"),
    ("use_callback", "useCallback"),
    ("use_context", "useContext"),
    ("use_reducer", "useReducer"),
    ("use_id", "useId"),
    ("use_deferred_value", "useDeferredValue"),
    ("use_transition", "useTransition"),
    ("use_sync_external_store", "useSyncExternalStore"),
    ("use_layout_effect", "useLayoutEffect"),
    ("use_imperative_handle", "useImperativeHandle"),
    ("use_debug_value", "useDebugValue"),
    ("use_action_state", "useActionState"),
    ("use_optimistic", "useOptimistic"),
    ("use_form_status", "useFormStatus"),
];

/// Prefix for Vox-style hook identifiers (`use_effect`, custom `use_store`, …).
pub const VOX_HOOK_IDENT_PREFIX: &str = "use_";

/// React named exports used by Path C reactive codegen — keep in sync with [`VOX_TO_REACT_HOOK`].
pub mod react_exports {
    pub const USE_STATE: &str = "useState";
    pub const USE_EFFECT: &str = "useEffect";
    pub const USE_MEMO: &str = "useMemo";
    pub const USE_REF: &str = "useRef";
    pub const USE_CALLBACK: &str = "useCallback";
}

#[must_use]
pub fn react_hook_export_for_vox_ident(vox_name: &str) -> Option<&'static str> {
    VOX_TO_REACT_HOOK
        .iter()
        .find(|(vox, _)| *vox == vox_name)
        .map(|(_, react)| *react)
}

/// Opt-out for CI/fixtures: when **`VOX_SUPPRESS_LEGACY_HOOK_LINTS`** is `1` or `true`, skips
/// warnings for direct `use_*` calls inside `@component fn` bodies (see `ast_decl_lints`).
#[must_use]
pub fn legacy_hook_lint_suppressed() -> bool {
    std::env::var("VOX_SUPPRESS_LEGACY_HOOK_LINTS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Every direct `use_*` call (identifier callee) under `stmt`, including nested expressions.
pub fn for_each_vox_hook_call_in_stmt(stmt: &Stmt, f: &mut impl FnMut(&str, Span)) {
    match stmt {
        Stmt::Let { value, .. } => for_each_vox_hook_call_in_expr(value, f),
        Stmt::Assign { target, value, .. } => {
            for_each_vox_hook_call_in_expr(target, f);
            for_each_vox_hook_call_in_expr(value, f);
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                for_each_vox_hook_call_in_expr(v, f);
            }
        }
        Stmt::Expr { expr, .. } => for_each_vox_hook_call_in_expr(expr, f),
        Stmt::While {
            condition, body, ..
        } => {
            for_each_vox_hook_call_in_expr(condition, f);
            for s in body {
                for_each_vox_hook_call_in_stmt(s, f);
            }
        }
        Stmt::Loop { body, .. } => {
            for s in body {
                for_each_vox_hook_call_in_stmt(s, f);
            }
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

/// Walk `expr` and invoke `f(name, span)` for each `use_*`(`…`) with an identifier callee.
pub fn for_each_vox_hook_call_in_expr(expr: &Expr, f: &mut impl FnMut(&str, Span)) {
    match expr {
        Expr::Call {
            callee, args, span, ..
        } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                if name.starts_with(VOX_HOOK_IDENT_PREFIX) {
                    f(name, *span);
                }
            } else {
                for_each_vox_hook_call_in_expr(callee, f);
            }
            for a in args {
                for_each_vox_hook_call_in_expr(&a.value, f);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            for_each_vox_hook_call_in_expr(object, f);
            for a in args {
                for_each_vox_hook_call_in_expr(&a.value, f);
            }
        }
        Expr::Binary { left, right, .. } => {
            for_each_vox_hook_call_in_expr(left, f);
            for_each_vox_hook_call_in_expr(right, f);
        }
        Expr::Unary { operand, .. } => for_each_vox_hook_call_in_expr(operand, f),
        Expr::FieldAccess { object, .. } => for_each_vox_hook_call_in_expr(object, f),
        Expr::Lambda { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    for_each_vox_hook_call_in_expr(d, f);
                }
            }
            for_each_vox_hook_call_in_expr(body, f);
        }
        Expr::Pipe { left, right, .. } => {
            for_each_vox_hook_call_in_expr(left, f);
            for_each_vox_hook_call_in_expr(right, f);
        }
        Expr::Try { target, .. } => for_each_vox_hook_call_in_expr(target, f),
        Expr::Spawn { target, .. } => for_each_vox_hook_call_in_expr(target, f),
        Expr::With {
            operand, options, ..
        } => {
            for_each_vox_hook_call_in_expr(operand, f);
            for_each_vox_hook_call_in_expr(options, f);
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            for_each_vox_hook_call_in_expr(condition, f);
            for s in then_body {
                for_each_vox_hook_call_in_stmt(s, f);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    for_each_vox_hook_call_in_stmt(s, f);
                }
            }
        }
        Expr::For { iterable, body, .. } => {
            for_each_vox_hook_call_in_expr(iterable, f);
            for_each_vox_hook_call_in_expr(body, f);
        }
        Expr::Match { subject, arms, .. } => {
            for_each_vox_hook_call_in_expr(subject, f);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    for_each_vox_hook_call_in_expr(g, f);
                }
                for_each_vox_hook_call_in_expr(&arm.body, f);
            }
        }
        Expr::Block { stmts, .. } => {
            for s in stmts {
                for_each_vox_hook_call_in_stmt(s, f);
            }
        }
        Expr::ListLit { elements, .. } | Expr::TupleLit { elements, .. } => {
            for e in elements {
                for_each_vox_hook_call_in_expr(e, f);
            }
        }
        Expr::ObjectLit { fields, .. } => {
            for (_, v) in fields {
                for_each_vox_hook_call_in_expr(v, f);
            }
        }
        Expr::StringInterp { parts, .. } => {
            for p in parts {
                if let StringPart::Interpolation(e) = p {
                    for_each_vox_hook_call_in_expr(e, f);
                }
            }
        }
        Expr::Jsx(el) => {
            for ch in &el.children {
                for_each_vox_hook_call_in_expr(ch, f);
            }
            for attr in &el.attributes {
                for_each_vox_hook_call_in_expr(&attr.value, f);
            }
        }
        Expr::JsxSelfClosing(el) => {
            for attr in &el.attributes {
                for_each_vox_hook_call_in_expr(&attr.value, f);
            }
        }
        Expr::JsxFragment { children, .. } => {
            for ch in children {
                for_each_vox_hook_call_in_expr(ch, f);
            }
        }
        Expr::Index { object, index, .. } => {
            for_each_vox_hook_call_in_expr(object, f);
            for_each_vox_hook_call_in_expr(index, f);
        }
        Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::BoolLit { .. }
        | Expr::StringLit { .. }
        | Expr::Ident { .. }
        | Expr::DecimalLit { .. }
        | Expr::SideEffect { .. }
        | Expr::WorkflowVersion(_) => {}
    }
}
