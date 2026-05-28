//! Effect propagation check: `caller.capabilities ⊇ callee.capabilities`.
//!
//! Only enforced when the caller has an explicit `uses` clause (or `@pure`).
//! Stdlib intrinsic capabilities are checked by module name on MethodCall nodes:
//! `http.*` → Net, `db.*` → Db, `fs.*` → Fs, `env.*` → Env,
//! `time.*` / `clock.*` → Clock, `process.*` → Spawn.

use std::collections::{HashMap, HashSet};

use crate::hir::nodes::effect::HirEffectKind;
use crate::hir::nodes::{HirEndpointFn, HirFn};
use crate::hir::{HirArg, HirCapability, HirExpr, HirModule, HirStmt};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// Run effect propagation checks across all annotated functions in the module.
pub fn check_effect_compliance(module: &HirModule, source: &str) -> Vec<Diagnostic> {
    // Build name → effective capability set for every annotated function.
    let mut cap_map: HashMap<String, Vec<HirCapability>> = HashMap::new();
    for f in &module.functions {
        if is_annotated(f) {
            cap_map.insert(f.name.clone(), effective_caps(f));
        }
    }
    // Endpoint functions with explicit effects also participate in the cap map.
    for f in &module.endpoint_fns {
        if f.is_pure || !f.effects.is_empty() {
            let caps: Vec<HirCapability> = if f.is_pure {
                vec![]
            } else {
                f.effects.iter().map(effect_kind_to_cap).collect()
            };
            cap_map.insert(f.name.clone(), caps);
        }
    }

    // Bottom-up effect inference (P1-T6): for every unannotated function, infer
    // its direct stdlib capabilities from its body and add them to cap_map.
    // This lets annotated callers be checked against a callee's inferred effects
    // even when the callee carries no explicit `uses` clause.
    for f in &module.functions {
        if !cap_map.contains_key(&f.name) {
            let inferred = infer_body_effects(&f.body);
            if !inferred.is_empty() {
                cap_map.insert(f.name.clone(), inferred);
            }
        }
    }

    let mut diags = Vec::new();
    for f in &module.functions {
        check_fn(f, &cap_map, source, &mut diags);
    }
    // Walk endpoint bodies through the same propagation checker.
    for f in &module.endpoint_fns {
        check_endpoint_fn_body(f, &cap_map, source, &mut diags);
    }
    diags.extend(check_endpoint_fn_effects(&module.endpoint_fns));
    diags
}

/// Run structural effect checks over a slice of endpoint functions.
///
/// Endpoint functions carry the same `is_pure`/`effects` contract as regular
/// functions; this enforces the same `E_EFFECT_PURE_CONFLICT` and
/// `E_EFFECT_DUPLICATE` rules across the endpoint surface.
pub fn check_endpoint_fn_effects(fns: &[HirEndpointFn]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for f in fns {
        check_one_endpoint_fn(f, &mut diags);
    }
    diags
}

/// Convert an `HirEffectKind` (endpoint annotation) to its `HirCapability` equivalent.
/// Used to build the cap_map entry for endpoints and their body propagation check.
fn effect_kind_to_cap(eff: &HirEffectKind) -> HirCapability {
    match eff {
        HirEffectKind::Net => HirCapability::Net,
        HirEffectKind::Db => HirCapability::Db,
        HirEffectKind::Fs => HirCapability::Fs,
        HirEffectKind::Env => HirCapability::Env,
        HirEffectKind::Clock => HirCapability::Clock,
        HirEffectKind::Random => HirCapability::Random,
        HirEffectKind::Spawn => HirCapability::Spawn,
        HirEffectKind::GpuCompute => HirCapability::GpuCompute,
        HirEffectKind::Mutate => HirCapability::Mutate,
        HirEffectKind::Mcp(s) => HirCapability::Mcp(s.clone()),
    }
}

/// The effective capability set: `@pure` or `uses nothing` → empty (no permitted effects).
fn effective_caps(f: &HirFn) -> Vec<HirCapability> {
    if f.is_pure {
        return vec![];
    }
    f.capabilities
        .iter()
        .filter(|c| !matches!(c, HirCapability::Nothing))
        .cloned()
        .collect()
}

fn is_annotated(f: &HirFn) -> bool {
    !f.capabilities.is_empty() || f.is_pure
}

fn check_fn(
    caller: &HirFn,
    cap_map: &HashMap<String, Vec<HirCapability>>,
    source: &str,
    diags: &mut Vec<Diagnostic>,
) {
    if !is_annotated(caller) {
        return;
    }
    let caller_set: HashSet<HirCapability> = effective_caps(caller).into_iter().collect();
    for stmt in &caller.body {
        check_stmt(stmt, &caller.name, &caller_set, cap_map, source, diags);
    }
}

/// Walk the body of an endpoint function through the propagation checker.
///
/// Endpoint functions annotated with `uses` clauses or `@pure` participate in
/// the same effect-propagation rules as regular functions: if an endpoint
/// declares `uses db`, it must not call functions that require `net` without
/// also declaring `uses net`.
fn check_endpoint_fn_body(
    f: &HirEndpointFn,
    cap_map: &HashMap<String, Vec<HirCapability>>,
    source: &str,
    diags: &mut Vec<Diagnostic>,
) {
    // Unannotated endpoints are open-world — skip.
    if !f.is_pure && f.effects.is_empty() {
        return;
    }
    let caller_set: HashSet<HirCapability> = if f.is_pure {
        HashSet::new()
    } else {
        f.effects.iter().map(effect_kind_to_cap).collect()
    };
    for stmt in &f.body {
        check_stmt(stmt, &f.name, &caller_set, cap_map, source, diags);
    }
}

fn check_stmt(
    stmt: &HirStmt,
    caller_name: &str,
    caller_set: &HashSet<HirCapability>,
    cap_map: &HashMap<String, Vec<HirCapability>>,
    source: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Expr { expr: value, .. } => {
            check_expr(value, caller_name, caller_set, cap_map, source, diags);
        }
        HirStmt::Assign { value, .. } => {
            check_expr(value, caller_name, caller_set, cap_map, source, diags);
        }
        HirStmt::Return { value: Some(v), .. } => {
            check_expr(v, caller_name, caller_set, cap_map, source, diags);
        }
        HirStmt::Return { value: None, .. } => {}
        HirStmt::While {
            condition, body, ..
        } => {
            check_expr(condition, caller_name, caller_set, cap_map, source, diags);
            for s in body {
                check_stmt(s, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                check_stmt(s, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn check_expr(
    expr: &HirExpr,
    caller_name: &str,
    caller_set: &HashSet<HirCapability>,
    cap_map: &HashMap<String, Vec<HirCapability>>,
    source: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match expr {
        HirExpr::Call(callee_expr, args, _, span) => {
            // Only enforce when callee is a direct identifier (not a method/closure).
            if let HirExpr::Ident(callee_name, _) = callee_expr.as_ref() {
                if let Some(callee_caps) = cap_map.get(callee_name) {
                    for cap in callee_caps {
                        if !caller_set.contains(cap) {
                            let mut d = Diagnostic::error(
                                format!(
                                    "Function `{}` calls `{}` which requires `{}`, but `{}` does not declare `uses {}`",
                                    caller_name, callee_name, cap, caller_name, cap
                                ),
                                *span,
                                source,
                            );
                            d.category = DiagnosticCategory::EffectViolation;
                            diags.push(d);
                        }
                    }
                }
            }
            // Recurse into callee expression and arguments.
            check_expr(callee_expr, caller_name, caller_set, cap_map, source, diags);
            for arg in args {
                check_arg(arg, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::MethodCall(obj, method_name, args, _, span) => {
            // Enforce stdlib intrinsic capabilities when the receiver is a known module.
            if let HirExpr::Ident(module_name, _) = obj.as_ref() {
                if let Some(required) = stdlib_module_capability(module_name) {
                    if !caller_set.contains(&required) {
                        let mut d = Diagnostic::error(
                            format!(
                                "Function `{}` calls `{}.{}()` which requires `{}`, \
                                 but `{}` does not declare `uses {}`",
                                caller_name,
                                module_name,
                                method_name,
                                required,
                                caller_name,
                                required
                            ),
                            *span,
                            source,
                        );
                        d.category = DiagnosticCategory::EffectViolation;
                        diags.push(d);
                    }
                }
            }
            check_expr(obj, caller_name, caller_set, cap_map, source, diags);
            for arg in args {
                check_arg(arg, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::Binary(_, left, right, _) => {
            check_expr(left, caller_name, caller_set, cap_map, source, diags);
            check_expr(right, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::Unary(_, operand, _) => {
            check_expr(operand, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            check_expr(cond, caller_name, caller_set, cap_map, source, diags);
            for s in then_stmts {
                check_stmt(s, caller_name, caller_set, cap_map, source, diags);
            }
            if let Some(els) = else_stmts {
                for s in els {
                    check_stmt(s, caller_name, caller_set, cap_map, source, diags);
                }
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                check_stmt(s, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::For(_, _, iterable, body, _, _) => {
            check_expr(iterable, caller_name, caller_set, cap_map, source, diags);
            check_expr(body, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::Lambda(_, _, body, _, _) => {
            check_expr(body, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::With(lhs, rhs, _) => {
            check_expr(lhs, caller_name, caller_set, cap_map, source, diags);
            check_expr(rhs, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::Match(subject, arms, _) => {
            check_expr(subject, caller_name, caller_set, cap_map, source, diags);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    check_expr(g, caller_name, caller_set, cap_map, source, diags);
                }
                check_expr(&arm.body, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::FieldAccess(obj, _, _) => {
            check_expr(obj, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::ListLit(elems, _) => {
            for e in elems {
                check_expr(e, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::TupleLit(elems, _) => {
            for e in elems {
                check_expr(e, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                check_expr(v, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::Spawn(inner, _) => {
            check_expr(inner, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::JsxFragment(children, _) => {
            for child in children {
                check_expr(child, caller_name, caller_set, cap_map, source, diags);
            }
        }
        HirExpr::Index(obj, idx, _) => {
            check_expr(obj, caller_name, caller_set, cap_map, source, diags);
            check_expr(idx, caller_name, caller_set, cap_map, source, diags);
        }
        HirExpr::AsyncView(v) => {
            check_expr(&v.source, caller_name, caller_set, cap_map, source, diags);
            if let Some(a) = &v.fetching_arm {
                check_expr(a, caller_name, caller_set, cap_map, source, diags);
            }
            if let Some(a) = &v.empty_arm {
                check_expr(a, caller_name, caller_set, cap_map, source, diags);
            }
            if let Some(a) = &v.error_arm {
                check_expr(a, caller_name, caller_set, cap_map, source, diags);
            }
            if let Some(a) = &v.ok_arm {
                check_expr(a, caller_name, caller_set, cap_map, source, diags);
            }
        }
        // Leaves.
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::Ident(..)
        | HirExpr::JsxSelfClosing(_)
        | HirExpr::Jsx(_)
        | HirExpr::Try(_)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

/// Infer the direct stdlib capabilities used in a function body by walking
/// method-call nodes whose receiver is a known stdlib module identifier.
fn infer_body_effects(body: &[HirStmt]) -> Vec<HirCapability> {
    let mut caps: HashSet<HirCapability> = HashSet::new();
    for stmt in body {
        infer_stmt_effects(stmt, &mut caps);
    }
    caps.into_iter().collect()
}

fn infer_stmt_effects(stmt: &HirStmt, caps: &mut HashSet<HirCapability>) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Expr { expr: value, .. } => {
            infer_expr_effects(value, caps);
        }
        HirStmt::Assign { value, .. } => {
            infer_expr_effects(value, caps);
        }
        HirStmt::Return { value: Some(v), .. } => {
            infer_expr_effects(v, caps);
        }
        HirStmt::Return { value: None, .. } => {}
        HirStmt::While {
            condition, body, ..
        } => {
            infer_expr_effects(condition, caps);
            for s in body {
                infer_stmt_effects(s, caps);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                infer_stmt_effects(s, caps);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn infer_expr_effects(expr: &HirExpr, caps: &mut HashSet<HirCapability>) {
    match expr {
        HirExpr::MethodCall(obj, _, args, _, _) => {
            if let HirExpr::Ident(module_name, _) = obj.as_ref() {
                if let Some(cap) = stdlib_module_capability(module_name) {
                    caps.insert(cap);
                }
            }
            infer_expr_effects(obj, caps);
            for arg in args {
                infer_expr_effects(&arg.value, caps);
            }
        }
        HirExpr::Call(callee, args, _, _) => {
            infer_expr_effects(callee, caps);
            for arg in args {
                infer_expr_effects(&arg.value, caps);
            }
        }
        HirExpr::Binary(_, left, right, _) => {
            infer_expr_effects(left, caps);
            infer_expr_effects(right, caps);
        }
        HirExpr::Unary(_, operand, _) => {
            infer_expr_effects(operand, caps);
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            infer_expr_effects(cond, caps);
            for s in then_stmts {
                infer_stmt_effects(s, caps);
            }
            if let Some(els) = else_stmts {
                for s in els {
                    infer_stmt_effects(s, caps);
                }
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                infer_stmt_effects(s, caps);
            }
        }
        HirExpr::For(_, _, iterable, body, _, _) => {
            infer_expr_effects(iterable, caps);
            infer_expr_effects(body, caps);
        }
        HirExpr::Lambda(_, _, body, _, _) => {
            infer_expr_effects(body, caps);
        }
        HirExpr::With(lhs, rhs, _) => {
            infer_expr_effects(lhs, caps);
            infer_expr_effects(rhs, caps);
        }
        HirExpr::Match(subject, arms, _) => {
            infer_expr_effects(subject, caps);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    infer_expr_effects(g, caps);
                }
                infer_expr_effects(&arm.body, caps);
            }
        }
        HirExpr::FieldAccess(obj, _, _) => {
            infer_expr_effects(obj, caps);
        }
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            for e in elems {
                infer_expr_effects(e, caps);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                infer_expr_effects(v, caps);
            }
        }
        HirExpr::Spawn(inner, _) => {
            infer_expr_effects(inner, caps);
        }
        HirExpr::JsxFragment(children, _) => {
            for child in children {
                infer_expr_effects(child, caps);
            }
        }
        HirExpr::Index(obj, idx, _) => {
            infer_expr_effects(obj, caps);
            infer_expr_effects(idx, caps);
        }
        HirExpr::AsyncView(v) => {
            infer_expr_effects(v.source.as_ref(), caps);
            if let Some(a) = &v.fetching_arm {
                infer_expr_effects(a, caps);
            }
            if let Some(a) = &v.empty_arm {
                infer_expr_effects(a, caps);
            }
            if let Some(a) = &v.error_arm {
                infer_expr_effects(a, caps);
            }
            if let Some(a) = &v.ok_arm {
                infer_expr_effects(a, caps);
            }
        }
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::Ident(..)
        | HirExpr::JsxSelfClosing(_)
        | HirExpr::Jsx(_)
        | HirExpr::Try(_)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

/// Map a stdlib module name to its required capability.
/// Any method call on the module inherits this capability.
fn stdlib_module_capability(module: &str) -> Option<HirCapability> {
    match module {
        "http" | "HTTP" => Some(HirCapability::Net),
        "db" | "DB" | "database" => Some(HirCapability::Db),
        "fs" | "filesystem" => Some(HirCapability::Fs),
        "env" | "environment" => Some(HirCapability::Env),
        "time" | "clock" | "Clock" => Some(HirCapability::Clock),
        "process" | "Process" => Some(HirCapability::Spawn),
        _ => None,
    }
}

fn check_arg(
    arg: &HirArg,
    caller_name: &str,
    caller_set: &HashSet<HirCapability>,
    cap_map: &HashMap<String, Vec<HirCapability>>,
    source: &str,
    diags: &mut Vec<Diagnostic>,
) {
    check_expr(&arg.value, caller_name, caller_set, cap_map, source, diags);
}

fn check_one_endpoint_fn(f: &HirEndpointFn, diags: &mut Vec<Diagnostic>) {
    // E_EFFECT_PURE_CONFLICT: @pure + uses clause is contradictory.
    if f.is_pure && !f.effects.is_empty() {
        let labels: Vec<String> = f
            .effects
            .iter()
            .map(|e: &HirEffectKind| e.label())
            .collect();
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "endpoint `{}` is marked `@pure` but also declares effects: {}. \
                 Remove `@pure` or remove the `uses` clause.",
                f.name,
                labels.join(", ")
            ),
            span: f.span,
            expected_type: None,
            found_type: None,
            context: Some("@pure means no side effects; `uses` declares side effects".to_string()),
            suggestions: vec![],
            category: DiagnosticCategory::Typecheck,
            code: Some("E_EFFECT_PURE_CONFLICT".to_string()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: Some("EndpointFnDecl".to_string()),
        });
    }

    // E_EFFECT_DUPLICATE: same effect listed more than once.
    let mut seen: Vec<&HirEffectKind> = Vec::new();
    for eff in &f.effects {
        if seen.contains(&eff) {
            diags.push(Diagnostic {
                severity: TypeckSeverity::Error,
                message: format!(
                    "endpoint `{}` declares effect `{}` more than once in its `uses` clause.",
                    f.name,
                    eff.label()
                ),
                span: f.span,
                expected_type: None,
                found_type: None,
                context: Some("each effect kind should appear at most once".to_string()),
                suggestions: vec![],
                category: DiagnosticCategory::Typecheck,
                code: Some("E_EFFECT_DUPLICATE".to_string()),
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: Some("EndpointFnDecl".to_string()),
            });
        } else {
            seen.push(eff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::DefId;
    use crate::hir::lower::lower_module;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn dummy_span() -> crate::ast::span::Span {
        crate::ast::span::Span::new(0, 0)
    }

    fn check(src: &str) -> Vec<Diagnostic> {
        let tokens = lex(src);
        let module = parse(tokens).expect("parse error");
        let hir = lower_module(&module);
        check_effect_compliance(&hir, src)
    }

    #[test]
    fn test_pure_fn_no_calls_ok() {
        let diags = check("fn add(a: int, b: int) uses nothing to int { a + b }");
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_unannotated_caller_not_enforced() {
        let diags = check(
            "fn fetch() uses net to str { \"ok\" }
fn caller() to str { fetch() }",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_annotated_caller_missing_capability_is_error() {
        let diags = check(
            "fn fetch() uses net to str { \"ok\" }
fn caller() uses nothing to str { fetch() }",
        );
        assert_eq!(diags.len(), 1, "expected one violation: {diags:?}");
        assert!(diags[0].message.contains("net"));
    }

    #[test]
    fn test_annotated_caller_with_superset_is_ok() {
        let diags = check(
            "fn fetch() uses net to str { \"ok\" }
fn caller() uses net, db to str { fetch() }",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_pure_decorator_treated_as_nothing() {
        let diags = check(
            "fn fetch() uses net to str { \"ok\" }
@pure
fn caller() to str { fetch() }",
        );
        assert_eq!(diags.len(), 1, "expected one violation: {diags:?}");
    }

    // ── Stdlib intrinsic capability tests ────────────────────────────────────

    #[test]
    fn test_http_method_call_requires_net() {
        let diags = check(r#"fn f() uses nothing to str { http.get("https://example.com") }"#);
        assert_eq!(diags.len(), 1, "expected net violation: {diags:?}");
        assert!(
            diags[0].message.contains("net"),
            "message: {}",
            diags[0].message
        );
        assert!(
            diags[0].message.contains("http.get"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn test_http_method_call_ok_with_net() {
        let diags = check(r#"fn f() uses net to str { http.get("https://example.com") }"#);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_db_method_call_requires_db() {
        let diags = check(r#"fn f() uses nothing to str { db.query("SELECT 1") }"#);
        assert_eq!(diags.len(), 1, "expected db violation: {diags:?}");
        assert!(
            diags[0].message.contains("db"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn test_db_method_call_ok_with_db() {
        let diags = check(r#"fn f() uses db to str { db.insert("users", {}) }"#);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn test_fs_method_call_requires_fs() {
        let diags = check(r#"fn f() uses nothing to str { fs.read("/etc/hosts") }"#);
        assert_eq!(diags.len(), 1, "expected fs violation: {diags:?}");
        assert!(
            diags[0].message.contains("fs"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn test_unannotated_caller_skips_stdlib_check() {
        // Unannotated callers must not be penalised — open world.
        let diags = check(r#"fn f() to str { http.get("https://example.com") }"#);
        assert!(
            diags.is_empty(),
            "unannotated caller must not be checked: {diags:?}"
        );
    }

    #[test]
    fn test_env_method_call_requires_env() {
        let diags = check(r#"fn f() uses nothing to str { env.get("HOME") }"#);
        assert_eq!(diags.len(), 1, "expected env violation: {diags:?}");
        assert!(
            diags[0].message.contains("env"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn test_multiple_stdlib_calls_all_checked() {
        // Caller declares net but not db — db call should error, http should not.
        let diags = check(r#"fn f() uses net to str { http.get("url"); db.query("SELECT 1") }"#);
        assert_eq!(diags.len(), 1, "expected exactly one violation: {diags:?}");
        assert!(
            diags[0].message.contains("db"),
            "expected db violation: {}",
            diags[0].message
        );
    }

    // ── endpoint fn checks ──────────────────────────────────────────────────

    fn make_endpoint_fn(name: &str, is_pure: bool, effects: Vec<HirEffectKind>) -> HirEndpointFn {
        use crate::hir::nodes::HirEndpointKind;
        HirEndpointFn {
            kind: HirEndpointKind::Query,
            id: DefId(0),
            name: name.to_string(),
            params: vec![],
            return_type: None,
            body: vec![],
            route_path: format!("/api/query/{name}"),
            is_pure,
            effects,
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            span: dummy_span(),
        }
    }

    #[test]
    fn endpoint_pure_conflict_is_caught() {
        let f = make_endpoint_fn("bad_endpoint", true, vec![HirEffectKind::Db]);
        let diags = check_endpoint_fn_effects(&[f]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, Some("E_EFFECT_PURE_CONFLICT".to_string()));
        assert_eq!(diags[0].severity, TypeckSeverity::Error);
        assert!(diags[0].ast_node_kind.as_deref() == Some("EndpointFnDecl"));
    }

    #[test]
    fn endpoint_duplicate_effect_is_caught() {
        let f = make_endpoint_fn(
            "dup_endpoint",
            false,
            vec![HirEffectKind::Net, HirEffectKind::Net],
        );
        let diags = check_endpoint_fn_effects(&[f]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, Some("E_EFFECT_DUPLICATE".to_string()));
    }

    #[test]
    fn endpoint_clean_effects_pass() {
        let f = make_endpoint_fn("list_tasks", false, vec![HirEffectKind::Db]);
        let diags = check_endpoint_fn_effects(&[f]);
        assert!(
            diags.is_empty(),
            "endpoint with single declared effect should pass"
        );
    }
}
