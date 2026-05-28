//! AST-only declaration checks not represented in HIR (`@search_index`, `@index`).

use crate::ast::decl::{Decl, FnDecl, Module, SearchIndexDecl, TableDecl};
use crate::ast::expr::{Expr, StringPart};
use crate::ast::stmt::Stmt;
use crate::ast::types::TypeExpr;
use crate::typeck::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticFix, TypeckSeverity, codes,
};
use crate::typeck::env::{Binding, BindingKind, TypeEnv};
use crate::typeck::ty::Ty;

fn relative_luminance(hex: &str) -> Option<f32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;

    let f = |c: f32| -> f32 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };

    Some(0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b))
}

fn contrast_ratio(l1: f32, l2: f32) -> f32 {
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

fn check_theme_decl(t: &crate::ast::decl::ui::ThemeDecl, diags: &mut Vec<Diagnostic>) {
    for (variant_name, variant_props) in [("light", &t.light), ("dark", &t.dark)] {
        let get_color = |name: &str| -> Option<f32> {
            variant_props
                .iter()
                .find(|(k, _)| k == name)
                .and_then(|(_, v)| relative_luminance(v))
        };

        let check_contrast = |bg_key: &str,
                              fg_key: &str,
                              alt_fg_key: &str,
                              diags: &mut Vec<Diagnostic>| {
            let bg = get_color(bg_key);
            let fg = get_color(fg_key).or_else(|| get_color(alt_fg_key));
            if let (Some(b), Some(f)) = (bg, fg) {
                let ratio = contrast_ratio(b, f);
                if ratio < 4.5 {
                    diags.push(Diagnostic {
                        message: format!("@theme '{}' ({}) has poor contrast ({:.2}:1) between {} and {}. WCAG minimum is 4.5:1.", t.name, variant_name, ratio, bg_key, fg_key),
                        span: t.span,
                        severity: TypeckSeverity::Warning,
                        expected_type: None, found_type: None, context: None,
                        suggestions: vec![format!("Adjust {} or {} to improve readability.", bg_key, fg_key)],
                        category: DiagnosticCategory::Lint,
                        code: Some("lint.theme_contrast".into()),
                        fixes: vec![], line_col: None, missing_cases: vec![], ast_node_kind: None,
                    });
                }
            }
        };

        check_contrast("--color-bg", "--color-text", "--color-fg", diags);
        check_contrast(
            "--color-btn-bg",
            "--color-btn-text",
            "--color-btn-fg",
            diags,
        );
    }
}
fn resolve_type(te: &TypeExpr, env: &TypeEnv) -> Ty {
    match te {
        TypeExpr::Named { name, .. } => {
            if let Some(ty) = env.lookup_type(name) {
                return ty;
            }
            match name.as_str() {
                "int" => Ty::Int,
                "float" => Ty::Float,
                "str" => Ty::Str,
                "bool" => Ty::Bool,
                "Unit" => Ty::Unit,
                "Element" => Ty::Element,
                other => Ty::Named(other.to_string()),
            }
        }
        TypeExpr::Generic { name, args, .. } => {
            let inner_args: Vec<Ty> = args.iter().map(|a| resolve_type(a, env)).collect();
            match name.as_str() {
                "list" | "List" => Ty::List(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Option" => Ty::Option(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Result" => Ty::Result(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                _ => Ty::Named(name.clone()),
            }
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let param_tys: Vec<Ty> = params.iter().map(|p| resolve_type(p, env)).collect();
            let ret_ty = resolve_type(return_type, env);
            Ty::Fn(param_tys, Box::new(ret_ty))
        }
        TypeExpr::Tuple { elements, .. } => {
            Ty::Tuple(elements.iter().map(|e| resolve_type(e, env)).collect())
        }
        TypeExpr::Unit { .. } => Ty::Unit,
        TypeExpr::Infer { .. } => Ty::Error,
        TypeExpr::Decimal { .. } => Ty::Decimal,
    }
}

fn register_table(env: &mut TypeEnv, t: &TableDecl) {
    let field_types: Vec<(String, Ty)> = t
        .fields
        .iter()
        .map(|f| (f.name.clone(), resolve_type(&f.type_ann, env)))
        .collect();

    env.define(
        t.name.clone(),
        Binding {
            ty: Ty::Table(t.name.clone(), field_types),
            mutable: false,
            kind: BindingKind::Table,
            is_deprecated: t.is_deprecated,
        },
    );
}

fn check_search_index_decl(env: &TypeEnv, si: &SearchIndexDecl, diags: &mut Vec<Diagnostic>) {
    let Some(binding) = env.lookup(&si.table_name) else {
        diags.push(Diagnostic {
            message: format!(
                "search_index '{}' references unknown table '{}'",
                si.index_name, si.table_name
            ),
            span: si.span,
            severity: TypeckSeverity::Error,
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![],
            category: DiagnosticCategory::Lint,
            code: Some("lint.search_index_unknown_table".into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
        return;
    };

    let Ty::Table(_table_name, fields) = &binding.ty else {
        diags.push(Diagnostic {
            message: format!(
                "search_index '{}' target '{}' is not a table",
                si.index_name, si.table_name
            ),
            span: si.span,
            severity: TypeckSeverity::Error,
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![],
            category: DiagnosticCategory::Lint,
            code: Some("lint.search_index_not_table".into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
        return;
    };

    let Some((_, field_ty)) = fields.iter().find(|(n, _)| n == &si.search_field) else {
        diags.push(Diagnostic {
            message: format!(
                "search_index '{}': table '{}' has no field '{}'",
                si.index_name, si.table_name, si.search_field
            ),
            span: si.span,
            severity: TypeckSeverity::Error,
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![],
            category: DiagnosticCategory::Lint,
            code: Some("lint.search_index_unknown_field".into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
        return;
    };

    if *field_ty != Ty::Str {
        diags.push(Diagnostic {
            message: format!(
                "search_index '{}': field '{}' must be type 'str', found {:?}",
                si.index_name, si.search_field, field_ty
            ),
            span: si.span,
            severity: TypeckSeverity::Error,
            expected_type: Some("str".into()),
            found_type: Some(format!("{field_ty:?}")),
            context: None,
            suggestions: vec![],
            category: DiagnosticCategory::Lint,
            code: Some("lint.search_index_field_type".into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
    }
}

fn visit_fn_decl_in_decl(decl: &Decl, visit: &mut impl FnMut(&FnDecl)) {
    match decl {
        Decl::Function(f) => visit(f),
        Decl::McpTool(m) => visit(&m.func),
        Decl::Test(t) => visit(&t.func),
        Decl::Forall(f) => visit(&f.func),
        Decl::Endpoint(e) => visit(&e.func),
        Decl::Skill(s) => visit(&s.func),
        Decl::AgentDef(ad) => visit(&ad.func),
        Decl::Scheduled(s) => visit(&s.func),
        Decl::Loading(l) => visit(&l.func),
        Decl::McpResource(m) => visit(&m.func),
        _ => {}
    }
}

/// Shallow `@pure` hint: obvious I/O-ish calls (`print`, `sleep`, `spawn`) are rejected.
fn first_shallow_pure_violation_in_expr(e: &Expr) -> Option<crate::ast::span::Span> {
    match e {
        Expr::Call { callee, args, span } => {
            if matches!(
                callee.as_ref(),
                Expr::Ident { name, .. } if name == "print" || name == "sleep"
            ) {
                return Some(*span);
            }
            first_shallow_pure_violation_in_expr(callee).or_else(|| {
                args.iter()
                    .find_map(|a| first_shallow_pure_violation_in_expr(&a.value))
            })
        }
        Expr::Spawn { span, .. } => Some(*span),
        Expr::Unary { operand, .. } => first_shallow_pure_violation_in_expr(operand),
        Expr::Binary { left, right, .. } => first_shallow_pure_violation_in_expr(left)
            .or_else(|| first_shallow_pure_violation_in_expr(right)),
        Expr::MethodCall { object, args, .. } => first_shallow_pure_violation_in_expr(object)
            .or_else(|| {
                args.iter()
                    .find_map(|a| first_shallow_pure_violation_in_expr(&a.value))
            }),
        Expr::FieldAccess { object, .. } => first_shallow_pure_violation_in_expr(object),
        Expr::Match { subject, arms, .. } => {
            first_shallow_pure_violation_in_expr(subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(|g| first_shallow_pure_violation_in_expr(g))
                        .or_else(|| first_shallow_pure_violation_in_expr(arm.body.as_ref()))
                })
            })
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => first_shallow_pure_violation_in_expr(condition)
            .or_else(|| {
                then_body
                    .iter()
                    .find_map(first_shallow_pure_violation_in_stmt)
            })
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| b.iter().find_map(first_shallow_pure_violation_in_stmt))
            }),
        Expr::For { iterable, body, .. } => first_shallow_pure_violation_in_expr(iterable)
            .or_else(|| first_shallow_pure_violation_in_expr(body)),
        Expr::Lambda { body, .. } => first_shallow_pure_violation_in_expr(body),
        Expr::Pipe { left, right, .. } => first_shallow_pure_violation_in_expr(left)
            .or_else(|| first_shallow_pure_violation_in_expr(right)),
        Expr::Try { target, .. } => first_shallow_pure_violation_in_expr(target),
        Expr::With {
            operand, options, ..
        } => first_shallow_pure_violation_in_expr(operand)
            .or_else(|| first_shallow_pure_violation_in_expr(options)),
        Expr::Block { stmts, .. } => stmts.iter().find_map(first_shallow_pure_violation_in_stmt),
        Expr::ObjectLit { fields, .. } => fields
            .iter()
            .find_map(|(_, v)| first_shallow_pure_violation_in_expr(v)),
        Expr::ListLit { elements, .. } | Expr::TupleLit { elements, .. } => elements
            .iter()
            .find_map(first_shallow_pure_violation_in_expr),
        Expr::StringInterp { parts, .. } => parts.iter().find_map(|p| match p {
            StringPart::Literal(_) => None,
            StringPart::Interpolation(inner) => first_shallow_pure_violation_in_expr(inner),
        }),
        Expr::Jsx(el) => el
            .attributes
            .iter()
            .find_map(|a| first_shallow_pure_violation_in_expr(&a.value))
            .or_else(|| {
                el.children
                    .iter()
                    .find_map(first_shallow_pure_violation_in_expr)
            }),
        Expr::JsxSelfClosing(el) => el
            .attributes
            .iter()
            .find_map(|a| first_shallow_pure_violation_in_expr(&a.value)),
        Expr::JsxFragment { children, .. } => children
            .iter()
            .find_map(first_shallow_pure_violation_in_expr),
        Expr::Index { object, index, .. } => first_shallow_pure_violation_in_expr(object)
            .or_else(|| first_shallow_pure_violation_in_expr(index)),
        Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::StringLit { .. }
        | Expr::BoolLit { .. }
        | Expr::DecimalLit { .. }
        | Expr::Ident { .. }
        // side_effect blocks are sanctioned non-determinism; skip them in the pure-fn check.
        | Expr::SideEffect { .. }
        // workflow.version is a metadata marker; not a pure violation.
        | Expr::WorkflowVersion(_) => None,
    }
}

fn first_shallow_pure_violation_in_stmt(s: &Stmt) -> Option<crate::ast::span::Span> {
    match s {
        Stmt::Let { value, .. } => first_shallow_pure_violation_in_expr(value),
        Stmt::Assign { target, value, .. } => first_shallow_pure_violation_in_expr(target)
            .or_else(|| first_shallow_pure_violation_in_expr(value)),
        Stmt::Return { value, .. } => value
            .as_ref()
            .and_then(first_shallow_pure_violation_in_expr),
        Stmt::Expr { expr, .. } => first_shallow_pure_violation_in_expr(expr),
        Stmt::While {
            condition, body, ..
        } => first_shallow_pure_violation_in_expr(condition)
            .or_else(|| body.iter().find_map(first_shallow_pure_violation_in_stmt)),
        Stmt::Loop { body, .. } => body.iter().find_map(first_shallow_pure_violation_in_stmt),
        Stmt::Break { .. } | Stmt::Continue { .. } => None,
    }
}

/// Run `@table` / `@index` / `@search_index` validation that stays on the AST surface.
#[must_use]
pub fn lint_ast_declarations(module: &Module, _source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut env = TypeEnv::new();

    for decl in &module.declarations {
        if let Decl::Table(t) = decl {
            for field in &t.fields {
                if field.name == "id" {
                    diags.push(Diagnostic {
                        message: format!(
                            "Table '{}' declares a column `id`; Vox `@table` already adds a surrogate `_id` primary key. \
                             Rename this field (e.g. `user_key`, `external_id`) to avoid LLM confusion with `_id`.",
                            t.name
                        ),
                        span: field.span,
                        severity: TypeckSeverity::Warning,
                        expected_type: None,
                        found_type: None,
                        context: None,
                        suggestions: vec![
                            "Omit a separate `id` and use `_id` in generated Rust/Turso rows.".into(),
                            "Or rename to a non-reserved column name that is not `id`.".into(),
                        ],
                        category: DiagnosticCategory::Lint,
                        code: Some("lint.table_id_column".into()),
                        fixes: vec![],
                    line_col: None, missing_cases: vec![], ast_node_kind: None,
});
                }
            }
            register_table(&mut env, t);
        }
    }

    for decl in &module.declarations {
        match decl {
            Decl::Index(idx) if env.lookup(&idx.table_name).is_none() => {
                diags.push(Diagnostic {
                    message: format!("@index references unknown table '{}'", idx.table_name),
                    span: idx.span,
                    severity: TypeckSeverity::Error,
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec![],
                    category: DiagnosticCategory::Lint,
                    code: Some("lint.index_unknown_table".into()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                });
            }
            Decl::SearchIndex(si) => check_search_index_decl(&env, si, &mut diags),
            Decl::Theme(t) => check_theme_decl(t, &mut diags),
            _ => {}
        }
    }

    for decl in &module.declarations {
        visit_fn_decl_in_decl(decl, &mut |f: &FnDecl| {
            if !f.is_pure {
                return;
            }
            let span = f
                .params
                .iter()
                .find_map(|p| {
                    p.default
                        .as_ref()
                        .and_then(first_shallow_pure_violation_in_expr)
                })
                .or_else(|| f.body.iter().find_map(first_shallow_pure_violation_in_stmt));
            if let Some(span) = span {
                diags.push(Diagnostic {
                    message: "@pure function may not call print/sleep/spawn (shallow purity lint)."
                        .to_string(),
                    span,
                    severity: TypeckSeverity::Warning,
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec![
                        "Remove the call or drop @pure if side effects are intentional.".into(),
                    ],
                    category: DiagnosticCategory::Lint,
                    code: Some("lint.pure_shallow_violation".into()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                });
            }
        });
    }

    // P1-T1: DurablePromise arity check — bare `DurablePromise` without a type argument.
    for decl in &module.declarations {
        visit_fn_decl_in_decl(decl, &mut |f: &FnDecl| {
            // Check return type.
            if let Some(ret) = &f.return_type {
                check_durable_promise_arity(ret, &mut diags);
            }
            // Check param type annotations.
            for p in &f.params {
                if let Some(ann) = &p.type_ann {
                    check_durable_promise_arity(ann, &mut diags);
                }
            }
        });
    }

    // P1-T2: Deprecate Future[T] / Promise[T] — warn with DurablePromise[T] rewrite fix.
    for decl in &module.declarations {
        visit_fn_decl_in_decl(decl, &mut |f: &FnDecl| {
            if let Some(ret) = &f.return_type {
                check_deprecated_type_aliases(ret, &mut diags);
            }
            for p in &f.params {
                if let Some(ann) = &p.type_ann {
                    check_deprecated_type_aliases(ann, &mut diags);
                }
            }
        });
    }

    // P1-T5: Workflow determinism check — forbid time.now and random.* in workflow bodies.
    for decl in &module.declarations {
        if let Decl::Workflow(w) = decl {
            for stmt in &w.body {
                check_workflow_stmt_determinism(stmt, &w.name, &mut diags);
            }
        }
    }

    // P1-T7: Flag side_effect { } blocks used outside a workflow body (no journal to bind to).
    for decl in &module.declarations {
        visit_fn_decl_in_decl(decl, &mut |f: &FnDecl| {
            for stmt in &f.body {
                check_side_effect_outside_workflow_stmt(stmt, &f.name, &mut diags);
            }
        });
    }

    // P1-T3: @remote param serializability check.
    for decl in &module.declarations {
        visit_fn_decl_in_decl(decl, &mut |f: &FnDecl| {
            if !f.is_remote {
                return;
            }
            for p in &f.params {
                if let Some(ann) = &p.type_ann {
                    check_remote_param_serializable(ann, &p.name, &f.name, &mut diags);
                }
            }
        });
    }

    diags
}

/// Emit `vox/remote/param-not-serializable` when an `@remote fn` has a non-serializable param type.
///
/// Function-typed parameters cannot cross a serialization boundary.
fn check_remote_param_serializable(
    te: &TypeExpr,
    param_name: &str,
    fn_name: &str,
    diags: &mut Vec<Diagnostic>,
) {
    if matches!(te, TypeExpr::Function { .. }) {
        diags.push(Diagnostic {
            message: format!(
                "@remote fn `{fn_name}`: parameter `{param_name}` has a function type, \
                 which cannot be serialized for cross-node dispatch."
            ),
            span: te.span(),
            severity: TypeckSeverity::Error,
            expected_type: Some("a serializable type (int, str, bool, a struct, …)".into()),
            found_type: Some("function type".into()),
            context: None,
            suggestions: vec![
                "Replace the function parameter with a serializable value or use a local closure instead.".into(),
            ],
            category: DiagnosticCategory::Typecheck,
            code: Some(codes::REMOTE_NON_SERIALIZABLE_PARAM.into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
    }
}

/// Emit `vox/types/durable-promise-arity` when `DurablePromise` appears without a type argument.
fn check_durable_promise_arity(te: &TypeExpr, diags: &mut Vec<Diagnostic>) {
    match te {
        TypeExpr::Named { name, span } if name == "DurablePromise" => {
            diags.push(Diagnostic {
                message: "type `DurablePromise` expects 1 type argument, found 0.\n\
                          help: write `DurablePromise[T]` where T is the awaited value type."
                    .to_string(),
                span: *span,
                severity: TypeckSeverity::Error,
                expected_type: Some("DurablePromise[T]".into()),
                found_type: Some("DurablePromise".into()),
                context: None,
                suggestions: vec![
                    "Add a type argument: `DurablePromise[int]`, `DurablePromise[str]`, etc."
                        .into(),
                ],
                category: DiagnosticCategory::Typecheck,
                code: Some(codes::TYPES_DURABLE_PROMISE_ARITY.into()),
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: None,
            });
        }
        TypeExpr::Generic { name, args, span } if name == "DurablePromise" && args.len() != 1 => {
            diags.push(Diagnostic {
                message: format!(
                    "type `DurablePromise` expects 1 type argument, found {}.",
                    args.len()
                ),
                span: *span,
                severity: TypeckSeverity::Error,
                expected_type: Some("DurablePromise[T]".into()),
                found_type: Some(format!("DurablePromise[{} args]", args.len())),
                context: None,
                suggestions: vec![
                    "Write `DurablePromise[T]` with exactly one type argument.".into(),
                ],
                category: DiagnosticCategory::Typecheck,
                code: Some(codes::TYPES_DURABLE_PROMISE_ARITY.into()),
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: None,
            });
        }
        _ => {}
    }
}

/// Render a `TypeExpr` as a Vox type string (for constructing fix replacements).
fn type_expr_to_str(te: &TypeExpr) -> String {
    match te {
        TypeExpr::Named { name, .. } => name.clone(),
        TypeExpr::Generic { name, args, .. } => {
            let args_str: Vec<_> = args.iter().map(type_expr_to_str).collect();
            format!("{}[{}]", name, args_str.join(", "))
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let ps: Vec<_> = params.iter().map(type_expr_to_str).collect();
            format!("fn({}) to {}", ps.join(", "), type_expr_to_str(return_type))
        }
        TypeExpr::Tuple { elements, .. } => {
            let els: Vec<_> = elements.iter().map(type_expr_to_str).collect();
            format!("({})", els.join(", "))
        }
        TypeExpr::Unit { .. } => "Unit".to_string(),
        TypeExpr::Infer { .. } => "_".to_string(),
        TypeExpr::Decimal { .. } => "decimal".to_string(),
    }
}

/// Emit a deprecation warning when `Future[T]` or `Promise[T]` appears in a type position.
///
/// Both types are aliases for `DurablePromise[T]` during the v0.6 deprecation window.
/// They will be removed in v0.7 (SSOT release-contract table).
fn check_deprecated_type_aliases(te: &TypeExpr, diags: &mut Vec<Diagnostic>) {
    match te {
        TypeExpr::Named { name, span } if name == "Future" || name == "Promise" => {
            let (code, old) = if name == "Future" {
                (codes::TYPES_FUTURE_DEPRECATED, "Future")
            } else {
                (codes::TYPES_PROMISE_DEPRECATED, "Promise")
            };
            diags.push(Diagnostic {
                message: format!(
                    "type `{old}` is deprecated and will be removed in v0.7; \
                     use `DurablePromise[T]` instead."
                ),
                span: *span,
                severity: TypeckSeverity::Warning,
                expected_type: Some("DurablePromise[T]".into()),
                found_type: Some(old.into()),
                context: None,
                suggestions: vec![format!("Replace `{old}` with `DurablePromise[T]`.")],
                category: DiagnosticCategory::Typecheck,
                code: Some(code.into()),
                fixes: vec![DiagnosticFix {
                    label: format!("Replace `{old}` with `DurablePromise`"),
                    span: *span,
                    replacement: "DurablePromise".into(),
                }],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: None,
            });
        }
        TypeExpr::Generic { name, args, span } if name == "Future" || name == "Promise" => {
            let (code, old) = if name == "Future" {
                (codes::TYPES_FUTURE_DEPRECATED, "Future")
            } else {
                (codes::TYPES_PROMISE_DEPRECATED, "Promise")
            };
            let arg_str = args
                .first()
                .map(type_expr_to_str)
                .unwrap_or_else(|| "T".to_string());
            let replacement = format!("DurablePromise[{}]", arg_str);
            diags.push(Diagnostic {
                message: format!(
                    "type `{old}[T]` is deprecated and will be removed in v0.7; \
                     use `DurablePromise[T]` instead."
                ),
                span: *span,
                severity: TypeckSeverity::Warning,
                expected_type: Some("DurablePromise[T]".into()),
                found_type: Some(format!("{old}[T]")),
                context: None,
                suggestions: vec![format!(
                    "Replace `{old}[{arg_str}]` with `DurablePromise[{arg_str}]`."
                )],
                category: DiagnosticCategory::Typecheck,
                code: Some(code.into()),
                fixes: vec![DiagnosticFix {
                    label: format!("Replace `{old}[{arg_str}]` with `DurablePromise[{arg_str}]`"),
                    span: *span,
                    replacement,
                }],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: None,
            });
        }
        _ => {}
    }
}

/// Walk a workflow statement and flag non-deterministic calls.
fn check_workflow_stmt_determinism(stmt: &Stmt, wf_name: &str, diags: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Let { value, .. } => {
            check_workflow_expr_determinism(value, wf_name, diags);
        }
        Stmt::Return { value: Some(v), .. } => {
            check_workflow_expr_determinism(v, wf_name, diags);
        }
        Stmt::Return { value: None, .. } => {}
        Stmt::Assign { value, .. } => {
            check_workflow_expr_determinism(value, wf_name, diags);
        }
        Stmt::Expr { expr, .. } => {
            check_workflow_expr_determinism(expr, wf_name, diags);
        }
        Stmt::While {
            condition, body, ..
        } => {
            check_workflow_expr_determinism(condition, wf_name, diags);
            for s in body {
                check_workflow_stmt_determinism(s, wf_name, diags);
            }
        }
        Stmt::Loop { body, .. } => {
            for s in body {
                check_workflow_stmt_determinism(s, wf_name, diags);
            }
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

/// The set of forbidden non-deterministic call patterns in workflow bodies.
/// Each entry is `(receiver_name, method_name_or_wildcard)`.
const NON_DETERMINISTIC_CALLS: &[(&str, Option<&str>)] = &[
    ("time", Some("now")),
    ("random", None), // any method on `random`
];

fn is_non_deterministic_call(object_name: &str, method_name: &str) -> bool {
    for &(receiver, method) in NON_DETERMINISTIC_CALLS {
        if object_name == receiver {
            match method {
                Some(m) if method_name == m => return true,
                None => return true,
                _ => {}
            }
        }
    }
    false
}

fn check_workflow_expr_determinism(expr: &Expr, wf_name: &str, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::MethodCall {
            object,
            method,
            args,
            span,
        } => {
            if let Expr::Ident { name: obj_name, .. } = object.as_ref() {
                if is_non_deterministic_call(obj_name, method) {
                    diags.push(Diagnostic {
                        message: format!(
                            "workflow `{wf_name}`: call to `{obj_name}.{method}()` is \
                             non-deterministic and will break replay. Use `side_effect {{ }}` \
                             or move the call into an `activity`."
                        ),
                        span: *span,
                        severity: TypeckSeverity::Error,
                        expected_type: None,
                        found_type: None,
                        context: None,
                        suggestions: vec![
                            "Wrap the call in `activity { … }` or `side_effect { … }`.".into(),
                        ],
                        category: DiagnosticCategory::Typecheck,
                        code: Some(codes::WORKFLOW_NON_DETERMINISTIC_CALL.into()),
                        fixes: vec![],
                        line_col: None,
                        missing_cases: vec![],
                        ast_node_kind: None,
                    });
                }
            }
            // Recurse into the receiver and args regardless.
            check_workflow_expr_determinism(object, wf_name, diags);
            for arg in args {
                check_workflow_expr_determinism(&arg.value, wf_name, diags);
            }
        }
        Expr::Call { callee, args, .. } => {
            check_workflow_expr_determinism(callee, wf_name, diags);
            for arg in args {
                check_workflow_expr_determinism(&arg.value, wf_name, diags);
            }
        }
        Expr::Block { stmts, .. } => {
            for s in stmts {
                check_workflow_stmt_determinism(s, wf_name, diags);
            }
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            check_workflow_expr_determinism(condition, wf_name, diags);
            for s in then_body {
                check_workflow_stmt_determinism(s, wf_name, diags);
            }
            if let Some(body) = else_body {
                for s in body {
                    check_workflow_stmt_determinism(s, wf_name, diags);
                }
            }
        }
        Expr::Binary { left, right, .. } => {
            check_workflow_expr_determinism(left, wf_name, diags);
            check_workflow_expr_determinism(right, wf_name, diags);
        }
        Expr::Unary { operand, .. } => {
            check_workflow_expr_determinism(operand, wf_name, diags);
        }
        Expr::FieldAccess { object, .. } => {
            check_workflow_expr_determinism(object, wf_name, diags);
        }
        Expr::Match { subject, arms, .. } => {
            check_workflow_expr_determinism(subject, wf_name, diags);
            for arm in arms {
                check_workflow_expr_determinism(&arm.body, wf_name, diags);
            }
        }
        Expr::For { iterable, body, .. } => {
            check_workflow_expr_determinism(iterable, wf_name, diags);
            check_workflow_expr_determinism(body, wf_name, diags);
        }
        Expr::Lambda { body, .. } => {
            check_workflow_expr_determinism(body, wf_name, diags);
        }
        Expr::Pipe { left, right, .. } => {
            check_workflow_expr_determinism(left, wf_name, diags);
            check_workflow_expr_determinism(right, wf_name, diags);
        }
        Expr::ListLit { elements, .. } => {
            for item in elements {
                check_workflow_expr_determinism(item, wf_name, diags);
            }
        }
        Expr::TupleLit { elements, .. } => {
            for item in elements {
                check_workflow_expr_determinism(item, wf_name, diags);
            }
        }
        Expr::ObjectLit { fields, .. } => {
            for (_, v) in fields {
                check_workflow_expr_determinism(v, wf_name, diags);
            }
        }
        // Terminal / irreducible — nothing to recurse into.
        Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::StringLit { .. }
        | Expr::BoolLit { .. }
        | Expr::DecimalLit { .. }
        | Expr::Ident { .. } => {}
        // Remaining variants — don't recurse (safe to skip for MVP).
        _ => {}
    }
}

/// Walk a non-workflow function body and flag any `side_effect { }` blocks (P1-T7).
/// Outside a workflow body there is no replay journal, so `side_effect` has no meaning.
fn check_side_effect_outside_workflow_stmt(
    stmt: &Stmt,
    fn_name: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Let { value, .. } => check_side_effect_outside_workflow_expr(value, fn_name, diags),
        Stmt::Return { value: Some(v), .. } => {
            check_side_effect_outside_workflow_expr(v, fn_name, diags)
        }
        Stmt::Return { value: None, .. } => {}
        Stmt::Expr { expr: e, .. } => check_side_effect_outside_workflow_expr(e, fn_name, diags),
        Stmt::Assign { value, .. } => {
            check_side_effect_outside_workflow_expr(value, fn_name, diags)
        }
        _ => {}
    }
}

fn check_side_effect_outside_workflow_expr(
    expr: &Expr,
    fn_name: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::SideEffect { span, .. } => {
            diags.push(Diagnostic {
                message: format!(
                    "function `{fn_name}`: `side_effect {{ }}` can only appear inside a \
                     `workflow` body — there is no replay journal here."
                ),
                span: *span,
                severity: TypeckSeverity::Error,
                expected_type: None,
                found_type: None,
                context: None,
                suggestions: vec!["Move the `side_effect` block into a `workflow`.".into()],
                category: DiagnosticCategory::Typecheck,
                code: Some(codes::WORKFLOW_SIDE_EFFECT_OUTSIDE_WORKFLOW.into()),
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: None,
            });
        }
        // Recurse into nested expressions but stop at SideEffect boundaries.
        Expr::Call { callee, args, .. } => {
            check_side_effect_outside_workflow_expr(callee, fn_name, diags);
            for arg in args {
                check_side_effect_outside_workflow_expr(&arg.value, fn_name, diags);
            }
        }
        Expr::Block { stmts, .. } => {
            for s in stmts {
                check_side_effect_outside_workflow_stmt(s, fn_name, diags);
            }
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            check_side_effect_outside_workflow_expr(condition, fn_name, diags);
            for s in then_body {
                check_side_effect_outside_workflow_stmt(s, fn_name, diags);
            }
            if let Some(body) = else_body {
                for s in body {
                    check_side_effect_outside_workflow_stmt(s, fn_name, diags);
                }
            }
        }
        _ => {}
    }
}
