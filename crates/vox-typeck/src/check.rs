use crate::builtins::BuiltinTypes;
use crate::diagnostics::{Diagnostic, Severity};
use crate::env::{ActorHandlerSig, AdtDef, Binding, BindingKind, TypeEnv, VariantDef, WorkflowSig};
use crate::ty::Ty;
use crate::unify::InferenceContext;
use vox_ast::decl::{
    ActivityDecl, ActorDecl, Decl, FnDecl, Module, TableDecl, TypeDefDecl, WorkflowDecl,
};
use vox_ast::expr::{BinOp, Expr};
use vox_ast::pattern::Pattern;
use vox_ast::stmt::Stmt;
use vox_ast::types::TypeExpr;

/// Type-check a complete Vox module, returning diagnostics.
///
/// This performs a two-pass analysis:
/// 1. **Registration pass**: Register all top-level declarations (types, functions,
///    actors, workflows) into the type environment so forward references work.
/// 2. **Checking pass**: Type-check each function/handler body using the populated
///    environment, checking return types, mutability, and match exhaustiveness.
pub fn typecheck_module(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let mut uf = InferenceContext::new();

    // ── Pass 1: Register all top-level declarations ───────────
    for decl in &module.declarations {
        match decl {
            Decl::TypeDef(td) => register_typedef(&mut env, td),
            Decl::Function(f) => register_function(&mut env, f),
            Decl::Component(c) => register_function(&mut env, &c.func),
            Decl::McpTool(m) => register_function(&mut env, &m.func),
            Decl::Test(t) => register_function(&mut env, &t.func),
            Decl::ServerFn(sf) => register_function(&mut env, &sf.func),
            Decl::Actor(a) => register_actor(&mut env, a),
            Decl::Workflow(w) => register_workflow(&mut env, w),
            Decl::Activity(a) => register_activity(&mut env, a),
            Decl::HttpRoute(_) | Decl::Import(_) | Decl::V0Component(_) | Decl::Routes(_) => {
                // HTTP routes checked in pass 2.
            }
            Decl::Table(t) => register_table(&mut env, t),
            Decl::Index(idx) => {
                // Validate that the referenced table exists
                if env.lookup(&idx.table_name).is_none() {
                    diagnostics.push(Diagnostic {
                        message: format!("@index references unknown table '{}'", idx.table_name),
                        span: idx.span,
                        severity: Severity::Error,
                        expected_type: None,
                        found_type: None,
                        suggestions: vec![],
                    });
                }
            }
        }
    }

    // ── Pass 2: Check each declaration body ───────────────────
    for decl in &module.declarations {
        match decl {
            Decl::Function(f)
            | Decl::McpTool(vox_ast::decl::McpToolDecl { func: f, .. })
            | Decl::Test(vox_ast::decl::TestDecl { func: f })
            | Decl::ServerFn(vox_ast::decl::ServerFnDecl { func: f }) => {
                env.push_scope();
                env.define(
                    "db".into(),
                    Binding {
                        ty: Ty::Database,
                        mutable: false,
                        kind: BindingKind::Variable,
                    },
                );
                check_function_body(&mut env, &builtins, &mut uf, &mut diagnostics, f, false);
                env.pop_scope();
            }
            Decl::Component(c) => {
                check_function_body(
                    &mut env,
                    &builtins,
                    &mut uf,
                    &mut diagnostics,
                    &c.func,
                    true,
                );
            }
            Decl::HttpRoute(r) => {
                env.push_scope();
                // Inject 'request' variable into the handler scope
                env.define(
                    "request".into(),
                    Binding {
                        ty: Ty::Named("Request".into()),
                        mutable: false,
                        kind: BindingKind::Variable,
                    },
                );
                // Inject 'db' variable
                env.define(
                    "db".into(),
                    Binding {
                        ty: Ty::Database,
                        mutable: false,
                        kind: BindingKind::Variable,
                    },
                );
                check_body(
                    &mut env,
                    &builtins,
                    &mut uf,
                    &mut diagnostics,
                    &r.body,
                    false,
                );
                env.pop_scope();
            }
            Decl::Actor(a) => {
                check_actor(&mut env, &builtins, &mut uf, &mut diagnostics, a);
            }
            Decl::Workflow(w) => {
                check_workflow(&mut env, &builtins, &mut uf, &mut diagnostics, w);
            }
            Decl::Activity(a) => {
                check_activity(&mut env, &builtins, &mut uf, &mut diagnostics, a);
            }
            Decl::TypeDef(_)
            | Decl::Import(_)
            | Decl::Table(_)
            | Decl::Index(_)
            | Decl::V0Component(_)
            | Decl::Routes(_) => {}
        }
    }

    diagnostics
}

// ── Registration helpers ──────────────────────────────────────

/// Convert an AST TypeExpr to an internal Ty.
/// Convert an AST TypeExpr to an internal Ty.
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
                _ => {
                    // Also check if name is generic param/alias
                    // But Generic with args implies type constructor.
                    // If env has it, use it? Ty::Recursive?
                    // For now fall back to Named
                    Ty::Named(name.clone())
                }
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
    }
}

fn register_typedef(env: &mut TypeEnv, td: &TypeDefDecl) {
    let variants: Vec<VariantDef> = td
        .variants
        .iter()
        .map(|v| VariantDef {
            name: v.name.clone(),
            fields: v
                .fields
                .iter()
                .map(|f| (f.name.clone(), resolve_type(&f.type_ann, env)))
                .collect(),
        })
        .collect();

    env.register_type(AdtDef {
        name: td.name.clone(),
        variants,
    });
}

fn register_function(env: &mut TypeEnv, f: &FnDecl) {
    env.push_scope();
    for (i, name) in f.generics.iter().enumerate() {
        env.define_type(name.clone(), Ty::GenericParam(i as u32));
    }

    let param_tys: Vec<Ty> = f
        .params
        .iter()
        .map(|p| {
            p.type_ann
                .as_ref()
                .map_or(Ty::TypeVar(0), |t| resolve_type(t, env))
        })
        .collect();
    let ret_ty = f
        .return_type
        .as_ref()
        .map_or(Ty::Unit, |t| resolve_type(t, env));

    env.pop_scope();

    env.define(
        f.name.clone(),
        Binding {
            ty: Ty::Fn(param_tys, Box::new(ret_ty)),
            mutable: false,
            kind: BindingKind::Function,
        },
    );
}

fn register_actor(env: &mut TypeEnv, a: &ActorDecl) {
    let handler_sigs: Vec<ActorHandlerSig> = a
        .handlers
        .iter()
        .map(|h| ActorHandlerSig {
            event_name: h.event_name.clone(),
            params: h
                .params
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        p.type_ann
                            .as_ref()
                            .map_or(Ty::TypeVar(0), |t| resolve_type(t, env)),
                    )
                })
                .collect(),
            return_type: h
                .return_type
                .as_ref()
                .map_or(Ty::Unit, |t| resolve_type(t, env)),
        })
        .collect();

    env.register_actor(a.name.clone(), handler_sigs);
}

fn register_workflow(env: &mut TypeEnv, w: &WorkflowDecl) {
    let params: Vec<(String, Ty)> = w
        .params
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                p.type_ann
                    .as_ref()
                    .map_or(Ty::TypeVar(0), |t| resolve_type(t, env)),
            )
        })
        .collect();
    let ret_ty = w
        .return_type
        .as_ref()
        .map_or(Ty::Unit, |t| resolve_type(t, env));

    env.register_workflow(WorkflowSig {
        name: w.name.clone(),
        params,
        return_type: ret_ty,
    });
}

fn register_activity(env: &mut TypeEnv, a: &ActivityDecl) {
    let param_tys: Vec<Ty> = a
        .params
        .iter()
        .map(|p| {
            p.type_ann
                .as_ref()
                .map_or(Ty::TypeVar(0), |t| resolve_type(t, env))
        })
        .collect();
    let ret_ty = a
        .return_type
        .as_ref()
        .map_or(Ty::Unit, |t| resolve_type(t, env));

    env.define(
        a.name.clone(),
        Binding {
            ty: Ty::Fn(param_tys, Box::new(ret_ty)),
            mutable: false,
            kind: BindingKind::Activity,
        },
    );
}

/// Register a table declaration as a named record type.
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
        },
    );
}

// ── Checking pass ─────────────────────────────────────────────

fn check_function_body(
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    f: &FnDecl,
    is_component: bool,
) {
    env.push_scope();

    // Define generics
    for (i, g) in f.generics.iter().enumerate() {
        env.define_type(g.clone(), Ty::GenericParam(i as u32));
    }

    // Resolve return type (generics must be in scope)
    let expected_ty = f
        .return_type
        .as_ref()
        .map_or(Ty::Unit, |t| resolve_type(t, env));

    // Push expected return type for deep checking of return statements
    env.push_return_type(expected_ty.clone());

    // Bind parameters
    for param in &f.params {
        let ty = param
            .type_ann
            .as_ref()
            .map_or_else(|| uf.fresh_var(), |t| resolve_type(t, env));
        env.define(
            param.name.clone(),
            Binding {
                ty,
                mutable: false,
                kind: BindingKind::Parameter,
            },
        );
    }

    // Check body statements
    let body_ty = check_body(env, builtins, uf, diags, &f.body, false);

    env.pop_return_type();
    env.pop_scope();

    // Check component return type
    if is_component {
        // Must be Element.
        // We can check resolved type or AST. Inspecting resolved is better.
        if expected_ty != Ty::Element {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: format!("Component '{}' should return Element", f.name),
                span: f.span,
                expected_type: Some("Element".into()),
                found_type: Some(format!("{expected_ty:?}")),
                suggestions: vec!["Add 'to Element' to the function signature".into()],
            });
        }
    }

    // Check implicit return
    // Note: If body ends in explicit return, body_ty is Unit. This might cause false error if expected is Int.
    // We should only check if body_ty is NOT Unit? Or if it's implicitly returned.
    // Vox: block evaluates to last expr.
    // If last expr is specific type, check it.
    // If last expr is Semicolon/Unit, check it.

    let has_explicit_return = f
        .body
        .last()
        .is_some_and(|s| matches!(s, Stmt::Return { .. }));
    if !has_explicit_return {
        if let Err(msg) = uf.unify(&expected_ty, &body_ty) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!("Implicit return type mismatch in '{}': {}", f.name, msg),
                span: f.span,
                expected_type: Some(format!("{expected_ty:?}")),
                found_type: Some(format!("{body_ty:?}")),
                suggestions: vec![],
            });
        }
    }
}

fn check_actor(
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    actor: &ActorDecl,
) {
    for handler in &actor.handlers {
        env.push_scope();

        // Inject 'db' variable for actor handlers (runs on server)
        env.define(
            "db".into(),
            Binding {
                ty: Ty::Database,
                mutable: false,
                kind: BindingKind::Variable,
            },
        );

        // Resolve expected return type
        let expected_ty = handler
            .return_type
            .as_ref()
            .map_or(Ty::Unit, |t| resolve_type(t, env));
        env.push_return_type(expected_ty.clone());

        // Bind handler parameters
        for param in &handler.params {
            let ty = param
                .type_ann
                .as_ref()
                .map_or_else(|| uf.fresh_var(), |t| resolve_type(t, env));
            env.define(
                param.name.clone(),
                Binding {
                    ty,
                    mutable: false,
                    kind: BindingKind::Parameter,
                },
            );
        }

        let body_ty = check_body(env, builtins, uf, diags, &handler.body, false);
        env.pop_return_type();

        let has_explicit_return = handler
            .body
            .last()
            .is_some_and(|s| matches!(s, Stmt::Return { .. }));
        if !has_explicit_return {
            if let Err(msg) = uf.unify(&expected_ty, &body_ty) {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "Implicit return type mismatch in handler '{}': {}",
                        handler.event_name, msg
                    ),
                    span: handler.span,
                    expected_type: Some(format!("{expected_ty:?}")),
                    found_type: Some(format!("{body_ty:?}")),
                    suggestions: vec![],
                });
            }
        }
        env.pop_scope();
    }
}

fn check_workflow(
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    workflow: &WorkflowDecl,
) {
    env.push_scope();

    for param in &workflow.params {
        let ty = param
            .type_ann
            .as_ref()
            .map_or_else(|| uf.fresh_var(), |t| resolve_type(t, env));
        env.define(
            param.name.clone(),
            Binding {
                ty,
                mutable: false,
                kind: BindingKind::Parameter,
            },
        );
    }

    let expected_ty = workflow
        .return_type
        .as_ref()
        .map_or(Ty::Unit, |t| resolve_type(t, env));
    env.push_return_type(expected_ty.clone());

    let body_ty = check_body(env, builtins, uf, diags, &workflow.body, false);
    env.pop_return_type();

    let has_explicit_return = workflow
        .body
        .last()
        .is_some_and(|s| matches!(s, Stmt::Return { .. }));
    if !has_explicit_return {
        if let Err(msg) = uf.unify(&expected_ty, &body_ty) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "Implicit return type mismatch in '{}': {}",
                    workflow.name, msg
                ),
                span: workflow.span,
                expected_type: Some(format!("{expected_ty:?}")),
                found_type: Some(format!("{body_ty:?}")),
                suggestions: vec![],
            });
        }
    }

    env.pop_scope();
}

fn check_activity(
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    activity: &ActivityDecl,
) {
    env.push_scope();

    for param in &activity.params {
        let ty = param
            .type_ann
            .as_ref()
            .map_or_else(|| uf.fresh_var(), |t| resolve_type(t, env));
        env.define(
            param.name.clone(),
            Binding {
                ty,
                mutable: false,
                kind: BindingKind::Parameter,
            },
        );
    }

    // Enforce that activity returns Result
    let expected_ty = if let Some(ref ret_type_expr) = activity.return_type {
        let ty = resolve_type(ret_type_expr, env);
        if !matches!(ty, Ty::Result(_)) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "Activity '{}' must return a Result[...] type",
                    activity.name
                ),
                span: activity.span,
                expected_type: Some("Result[...]".into()),
                found_type: Some(format!("{ty:?}")),
                suggestions: vec![],
            });
        }
        ty
    } else {
        diags.push(Diagnostic {
            severity: Severity::Warning,
            message: format!(
                "Activity '{}' should have an explicit return type (e.g. 'to Result[Unit]')",
                activity.name
            ),
            span: activity.span,
            expected_type: Some("Result[...]".into()),
            found_type: None,
            suggestions: vec![],
        });
        Ty::Result(Box::new(Ty::Unit))
    };

    env.push_return_type(expected_ty.clone());
    let body_ty = check_body(env, builtins, uf, diags, &activity.body, false);
    env.pop_return_type();

    let has_explicit_return = activity
        .body
        .last()
        .is_some_and(|s| matches!(s, Stmt::Return { .. }));
    if !has_explicit_return {
        if let Err(msg) = uf.unify(&expected_ty, &body_ty) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "Implicit return type mismatch in '{}': {}",
                    activity.name, msg
                ),
                span: activity.span,
                expected_type: Some(format!("{expected_ty:?}")),
                found_type: Some(format!("{body_ty:?}")),
                suggestions: vec![],
            });
        }
    }
    env.pop_scope();
}

/// Check a block of statements, producing the type of the last expression.
fn check_body(
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    stmts: &[Stmt],
    _in_actor: bool,
) -> Ty {
    let mut last_ty = Ty::Unit;
    for stmt in stmts {
        last_ty = check_stmt(env, builtins, uf, diags, stmt);
    }
    last_ty
}

fn check_stmt(
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    stmt: &Stmt,
) -> Ty {
    match stmt {
        Stmt::Let {
            pattern,
            type_ann,
            value,
            mutable,
            span,
        } => {
            let value_ty = infer_expr(env, builtins, uf, diags, value);

            // Check annotated type against inferred type
            if let Some(ann) = type_ann {
                let expected = resolve_type(ann, env);
                if let Err(msg) = uf.unify(&expected, &value_ty) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("Type mismatch in let binding: {msg}"),
                        span: *span,
                        expected_type: Some(format!("{expected:?}")),
                        found_type: Some(format!("{value_ty:?}")),
                        suggestions: vec![],
                    });
                }
            }

            // Bind pattern names into scope
            bind_pattern(env, diags, pattern, &value_ty, *mutable);
            Ty::Unit
        }

        Stmt::Assign {
            target,
            value,
            span,
        } => {
            // Check that the target is mutable
            if let Expr::Ident { name, .. } = target {
                match env.lookup(name) {
                    Some(binding) if !binding.mutable => {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            message: format!(
                                "Cannot assign to immutable variable '{name}'. \
                                 Use 'let mut {name}' to make it mutable."
                            ),
                            span: *span,
                            expected_type: None,
                            found_type: None,
                            suggestions: vec![format!("Change to: let mut {name} = ...")],
                        });
                    }
                    None => {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            message: format!("Undefined variable '{name}'"),
                            span: *span,
                            expected_type: None,
                            found_type: None,
                            suggestions: vec![],
                        });
                    }
                    _ => {}
                }
            }

            let _target_ty = infer_expr(env, builtins, uf, diags, target);
            let _value_ty = infer_expr(env, builtins, uf, diags, value);
            Ty::Unit
        }

        Stmt::Return { value, span } => {
            let actual_ty = if let Some(v) = value {
                infer_expr(env, builtins, uf, diags, v)
            } else {
                Ty::Unit
            };

            if let Some(expected_ty) = env.current_return_type() {
                if let Err(msg) = uf.unify(expected_ty, &actual_ty) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("Return type mismatch: {}", msg),
                        span: *span,
                        expected_type: Some(format!("{expected_ty:?}")),
                        found_type: Some(format!("{actual_ty:?}")),
                        suggestions: vec![],
                    });
                }
            }
            Ty::Unit
        }

        Stmt::Expr { expr, .. } => infer_expr(env, builtins, uf, diags, expr),
    }
}

/// Bind names from a pattern into the environment with the given type.
fn bind_pattern(
    env: &mut TypeEnv,
    _diags: &mut Vec<Diagnostic>,
    pattern: &Pattern,
    ty: &Ty,
    mutable: bool,
) {
    match pattern {
        Pattern::Ident { name, .. } => {
            env.define(
                name.clone(),
                Binding {
                    ty: ty.clone(),
                    mutable,
                    kind: BindingKind::Variable,
                },
            );
        }
        Pattern::Tuple { elements, .. } => {
            if let Ty::Tuple(elem_tys) = ty {
                for (pat, elem_ty) in elements.iter().zip(elem_tys.iter()) {
                    bind_pattern(env, _diags, pat, elem_ty, mutable);
                }
            } else {
                // When destructuring a tuple but the type isn't resolved (e.g. use_state
                // with a generic return), bind each elem as a fresh type var.
                for pat in elements {
                    bind_pattern(env, _diags, pat, &Ty::TypeVar(0), mutable);
                }
            }
        }
        Pattern::Constructor { fields, .. } => {
            for pat in fields {
                bind_pattern(env, _diags, pat, &Ty::TypeVar(0), mutable);
            }
        }
        Pattern::Wildcard { .. } => {}
        Pattern::Literal { .. } => {}
    }
}

fn instantiate_inner(
    ty: Ty,
    uf: &mut InferenceContext,
    map: &mut std::collections::HashMap<u32, Ty>,
) -> Ty {
    match ty {
        Ty::GenericParam(id) => map.entry(id).or_insert_with(|| uf.fresh_var()).clone(),
        Ty::List(inner) => Ty::List(Box::new(instantiate_inner(*inner, uf, map))),
        Ty::Option(inner) => Ty::Option(Box::new(instantiate_inner(*inner, uf, map))),
        Ty::Result(inner) => Ty::Result(Box::new(instantiate_inner(*inner, uf, map))),
        Ty::Fn(params, ret) => Ty::Fn(
            params
                .into_iter()
                .map(|p| instantiate_inner(p, uf, map))
                .collect(),
            Box::new(instantiate_inner(*ret, uf, map)),
        ),
        Ty::Tuple(elems) => Ty::Tuple(
            elems
                .into_iter()
                .map(|e| instantiate_inner(e, uf, map))
                .collect(),
        ),
        Ty::Record(fields) => Ty::Record(
            fields
                .into_iter()
                .map(|(n, t)| (n, instantiate_inner(t, uf, map)))
                .collect(),
        ),
        _ => ty,
    }
}

fn instantiate(ty: Ty, uf: &mut InferenceContext) -> Ty {
    let mut map = std::collections::HashMap::new();
    instantiate_inner(ty, uf, &mut map)
}

fn check_arguments(
    env: &TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    expected_args: &[Ty],
    actual_args: &[vox_ast::expr::Arg],
    span: vox_ast::span::Span,
) {
    if expected_args.len() != actual_args.len() {
        diags.push(Diagnostic {
            severity: Severity::Error,
            message: format!(
                "Argument count mismatch: expected {}, found {}",
                expected_args.len(),
                actual_args.len()
            ),
            span,
            expected_type: None,
            found_type: None,
            suggestions: vec![],
        });
        // Still check args
        for arg in actual_args {
            infer_expr(env, builtins, uf, diags, &arg.value);
        }
        return;
    }

    for (expected, arg) in expected_args.iter().zip(actual_args.iter()) {
        let actual_ty = infer_expr(env, builtins, uf, diags, &arg.value);
        if let Err(msg) = uf.unify(expected, &actual_ty) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!("Argument type mismatch: {msg}"),
                span: arg.value.span(),
                expected_type: Some(format!("{expected:?}")),
                found_type: Some(format!("{actual_ty:?}")),
                suggestions: vec![],
            });
        }
    }
}

/// Infer the type of an expression, recording diagnostics for errors.
fn infer_expr(
    env: &TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    expr: &Expr,
) -> Ty {
    match expr {
        Expr::IntLit { .. } => Ty::Int,
        Expr::FloatLit { .. } => Ty::Float,
        Expr::StringLit { .. } => Ty::Str,
        Expr::BoolLit { .. } => Ty::Bool,

        Expr::Ident { name, span } => {
            if let Some(binding) = env.lookup(name) {
                instantiate(binding.ty.clone(), uf)
            } else {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Undefined variable '{name}'"),
                    span: *span,
                    expected_type: None,
                    found_type: None,
                    suggestions: vec![],
                });
                Ty::Error
            }
        }

        Expr::ListLit { elements, .. } => {
            if elements.is_empty() {
                Ty::List(Box::new(uf.fresh_var()))
            } else {
                let elem_ty = infer_expr(env, builtins, uf, diags, &elements[0]);
                Ty::List(Box::new(elem_ty))
            }
        }

        Expr::ObjectLit { fields, .. } => {
            let field_types: Vec<(String, Ty)> = fields
                .iter()
                .map(|(name, expr)| (name.clone(), infer_expr(env, builtins, uf, diags, expr)))
                .collect();
            Ty::Record(field_types)
        }

        Expr::TupleLit { elements, .. } => Ty::Tuple(
            elements
                .iter()
                .map(|e| infer_expr(env, builtins, uf, diags, e))
                .collect(),
        ),

        Expr::Binary {
            op, left, right, ..
        } => {
            let left_ty = infer_expr(env, builtins, uf, diags, left);
            let _right_ty = infer_expr(env, builtins, uf, diags, right);
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => left_ty,
                BinOp::Lt
                | BinOp::Gt
                | BinOp::Lte
                | BinOp::Gte
                | BinOp::Is
                | BinOp::Isnt
                | BinOp::And
                | BinOp::Or => Ty::Bool,
                BinOp::Pipe => _right_ty,
            }
        }

        Expr::Unary { operand, .. } => infer_expr(env, builtins, uf, diags, operand),

        Expr::Call { callee, args, span } => {
            let callee_ty = infer_expr(env, builtins, uf, diags, callee);
            let callee_ty = instantiate(callee_ty, uf);

            match callee_ty {
                Ty::Fn(param_tys, ret_ty) => {
                    check_arguments(env, builtins, uf, diags, &param_tys, args, *span);
                    *ret_ty
                }
                Ty::Error => Ty::Error,
                Ty::TypeVar(_) => {
                    for arg in args {
                        infer_expr(env, builtins, uf, diags, &arg.value);
                    }
                    uf.fresh_var()
                }
                _ => {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("Expression is not callable: {callee_ty:?}"),
                        span: callee.span(),
                        expected_type: Some("function".into()),
                        found_type: Some(format!("{callee_ty:?}")),
                        suggestions: vec![],
                    });
                    for arg in args {
                        infer_expr(env, builtins, uf, diags, &arg.value);
                    }
                    Ty::Error
                }
            }
        }

        Expr::MethodCall {
            object,
            method,
            args,
            span,
        } => {
            let obj_ty = infer_expr(env, builtins, uf, diags, object);

            // If obj_ty is a type variable, we can't reliably check the method yet.
            // Just assume it exists and returns a fresh var, and verify args.
            if let Ty::TypeVar(_) = obj_ty {
                for arg in args {
                    infer_expr(env, builtins, uf, diags, &arg.value);
                }
                return uf.fresh_var();
            }

            if let Some(mut method_ty) = builtins.lookup_method(&obj_ty, method) {
                method_ty = instantiate(method_ty, uf);

                if let Ty::Fn(param_tys, ret_ty) = method_ty {
                    check_arguments(env, builtins, uf, diags, &param_tys, args, *span);
                    *ret_ty
                } else {
                    // unexpected: method in builtins map is not a function?
                    Ty::Error
                }
            } else {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Method '{method}' not found on type {obj_ty:?}"),
                    span: *span,
                    expected_type: None,
                    found_type: None,
                    suggestions: vec![],
                });
                for arg in args {
                    infer_expr(env, builtins, uf, diags, &arg.value);
                }
                Ty::Error
            }
        }

        Expr::FieldAccess {
            object,
            field,
            span,
        } => {
            let obj_ty = infer_expr(env, builtins, uf, diags, object);
            // If we know it's a Record, look up the field
            if let Ty::Record(fields) = &obj_ty {
                if let Some((_, fty)) = fields.iter().find(|(n, _)| n == field) {
                    return fty.clone();
                }
            }
            // Check for db.Table
            if let Ty::Database = &obj_ty {
                if let Some(binding) = env.lookup(field) {
                    if binding.kind == BindingKind::Table {
                        return binding.ty.clone();
                    }
                }
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Unknown table '{}' in database", field),
                    span: *span,
                    expected_type: None,
                    found_type: None,
                    suggestions: vec![],
                });
                return Ty::Error;
            }
            uf.fresh_var()
        }

        Expr::Match {
            subject,
            arms,
            span,
        } => {
            let _subject_ty = infer_expr(env, builtins, uf, diags, subject);

            // Check match exhaustiveness for known ADTs
            check_match_exhaustiveness(env, diags, &_subject_ty, arms, *span);

            if let Some(first_arm) = arms.first() {
                infer_expr(env, builtins, uf, diags, &first_arm.body)
            } else {
                Ty::Unit
            }
        }

        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            let cond_ty = infer_expr(env, builtins, uf, diags, condition);
            if cond_ty != Ty::Bool && !matches!(cond_ty, Ty::TypeVar(_) | Ty::Error) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("If condition should be bool, found {:?}", cond_ty),
                    span: condition.span(),
                    expected_type: Some("bool".into()),
                    found_type: Some(format!("{cond_ty:?}")),
                    suggestions: vec![],
                });
            }
            // Note: we don't create new scopes for then/else here since
            // check_body handles that
            for stmt in then_body {
                check_stmt_immutable(env, builtins, uf, diags, stmt);
            }
            if let Some(else_stmts) = else_body {
                for stmt in else_stmts {
                    check_stmt_immutable(env, builtins, uf, diags, stmt);
                }
            }
            Ty::Unit
        }

        Expr::For {
            binding: _,
            iterable,
            body,
            ..
        } => {
            let iter_ty = infer_expr(env, builtins, uf, diags, iterable);
            let _elem_ty = match &iter_ty {
                Ty::List(inner) => *inner.clone(),
                _ => uf.fresh_var(),
            };
            // Note: we'd need a proper scope push/pop here for the binding.
            // For now, we just infer the body.
            infer_expr(env, builtins, uf, diags, body)
        }

        Expr::Lambda { params, .. } => {
            let param_tys: Vec<Ty> = params
                .iter()
                .map(|p| {
                    p.type_ann
                        .as_ref()
                        .map_or_else(|| uf.fresh_var(), |t| resolve_type(t, env))
                })
                .collect();
            let ret_ty = uf.fresh_var();
            Ty::Fn(param_tys, Box::new(ret_ty))
        }

        Expr::Pipe { left, right, .. } => {
            infer_expr(env, builtins, uf, diags, left);
            infer_expr(env, builtins, uf, diags, right)
        }

        Expr::Spawn { target, .. } => {
            infer_expr(env, builtins, uf, diags, target);
            uf.fresh_var() // spawn returns a PID/handle
        }

        Expr::With {
            operand, options, ..
        } => {
            let op_ty = infer_expr(env, builtins, uf, diags, operand);
            let opt_ty = infer_expr(env, builtins, uf, diags, options);

            // Ensure options is a Record (or can be one)
            match &opt_ty {
                Ty::Record(fields) => {
                    // Validate known option keys and their types
                    let known_options: &[(&str, &[&str])] = &[
                        ("retries", &["Int"]),
                        ("timeout", &["Str", "Int"]),
                        ("initial_backoff", &["Str"]),
                        ("max_backoff", &["Str"]),
                        ("backoff_multiplier", &["Float", "Int"]),
                        ("activity_id", &["Str"]),
                    ];

                    for (key, val_ty) in fields {
                        let known = known_options.iter().find(|(k, _)| *k == key.as_str());
                        match known {
                            Some((_, expected_types)) => {
                                let type_name = match val_ty {
                                    Ty::Int => "Int",
                                    Ty::Float => "Float",
                                    Ty::Str => "Str",
                                    Ty::Bool => "Bool",
                                    Ty::TypeVar(_) => continue, // defer to unification
                                    _ => "Other",
                                };
                                if !expected_types.contains(&type_name) && type_name != "Other" {
                                    diags.push(Diagnostic {
                                        severity: Severity::Warning,
                                        message: format!(
                                            "'with' option '{}' expects type {}, found {}",
                                            key,
                                            expected_types.join(" or "),
                                            type_name
                                        ),
                                        span: options.span(),
                                        expected_type: Some(expected_types.join(" | ")),
                                        found_type: Some(type_name.to_string()),
                                        suggestions: vec![],
                                    });
                                }
                            }
                            None => {
                                diags.push(Diagnostic {
                                    severity: Severity::Warning,
                                    message: format!(
                                        "Unknown 'with' option '{}'. Known options: retries, timeout, initial_backoff, max_backoff, backoff_multiplier, activity_id",
                                        key
                                    ),
                                    span: options.span(),
                                    expected_type: None,
                                    found_type: None,
                                    suggestions: vec!["retries".into(), "timeout".into()],
                                });
                            }
                        }
                    }
                }
                Ty::TypeVar(_) => {} // allowed, will unify later
                _ => {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: "'with' options must be a record/object literal".to_string(),
                        span: options.span(),
                        expected_type: Some("Record".into()),
                        found_type: Some(format!("{opt_ty:?}")),
                        suggestions: vec![],
                    });
                }
            }
            op_ty
        }

        Expr::Jsx(_) | Expr::JsxSelfClosing(_) => Ty::Element,

        Expr::StringInterp { parts, .. } => {
            for part in parts {
                if let vox_ast::expr::StringPart::Interpolation(expr) = part {
                    infer_expr(env, builtins, uf, diags, expr);
                }
            }
            Ty::Str
        }

        Expr::Block { stmts, .. } => {
            let mut last_ty = Ty::Unit;
            for stmt in stmts {
                last_ty = check_stmt_immutable(env, builtins, uf, diags, stmt);
            }
            last_ty
        }
    }
}

/// Check a statement without being able to modify the env (for nested blocks
/// where we don't want to add bindings to the outer scope).
fn check_stmt_immutable(
    env: &TypeEnv,
    builtins: &BuiltinTypes,
    uf: &mut InferenceContext,
    diags: &mut Vec<Diagnostic>,
    stmt: &Stmt,
) -> Ty {
    println!("DEBUG: Checking stmt {:?}", stmt);
    match stmt {
        Stmt::Let { value, .. } => {
            infer_expr(env, builtins, uf, diags, value);
            Ty::Unit
        }
        Stmt::Assign { target, value, .. } => {
            infer_expr(env, builtins, uf, diags, target);
            infer_expr(env, builtins, uf, diags, value);
            Ty::Unit
        }
        Stmt::Return { value, span } => {
            let actual_ty = if let Some(v) = value {
                println!("DEBUG: Stmt::Return inferring value");
                let t = infer_expr(env, builtins, uf, diags, v);
                println!("DEBUG: Stmt::Return inferred type: {:?}", t);
                t
            } else {
                println!("DEBUG: Stmt::Return NO value (Unit)");
                Ty::Unit
            };

            if let Some(expected_ty) = env.current_return_type() {
                if let Err(msg) = uf.unify(expected_ty, &actual_ty) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("Return type mismatch: {}", msg),
                        span: *span,
                        expected_type: Some(format!("{expected_ty:?}")),
                        found_type: Some(format!("{actual_ty:?}")),
                        suggestions: vec![],
                    });
                    println!(
                        "DEBUG: Explicit mismatch actual {:?} vs expected {:?}",
                        actual_ty, expected_ty
                    );
                    expected_ty.clone()
                } else {
                    println!(
                        "DEBUG: Explicit match actual {:?} vs expected {:?}",
                        actual_ty, expected_ty
                    );
                    actual_ty
                }
            } else {
                println!("DEBUG: No expected type. Returning actual {:?}", actual_ty);
                actual_ty
            }
        }
        Stmt::Expr { expr, .. } => infer_expr(env, builtins, uf, diags, expr),
    }
}

/// Check match exhaustiveness for ADT-typed subjects.
fn check_match_exhaustiveness(
    env: &TypeEnv,
    diags: &mut Vec<Diagnostic>,
    subject_ty: &Ty,
    arms: &[vox_ast::expr::MatchArm],
    span: vox_ast::span::Span,
) {
    // Only check exhaustiveness for named types (ADTs)
    let type_name = match subject_ty {
        Ty::Named(name) => name.as_str(),
        _ => return,
    };

    let adt = match env.lookup_adt(type_name) {
        Some(adt) => adt,
        None => return,
    };

    // Collect constructor names from match arms
    let mut covered_variants: Vec<String> = Vec::new();
    let mut has_wildcard = false;

    for arm in arms {
        match &arm.pattern {
            Pattern::Wildcard { .. } => {
                has_wildcard = true;
            }
            Pattern::Ident { name, .. } => {
                // Check if this ident is a nullary constructor
                if adt.variants.iter().any(|v| v.name == *name) {
                    covered_variants.push(name.clone());
                } else {
                    // It's a catch-all binding (like a variable name)
                    has_wildcard = true;
                }
            }
            Pattern::Constructor { name, .. } => {
                covered_variants.push(name.clone());
            }
            Pattern::Literal { .. } => {}
            Pattern::Tuple { .. } => {}
        }
    }

    if has_wildcard {
        return; // Wildcard/catch-all covers everything
    }

    // Find missing variants
    let missing: Vec<&str> = adt
        .variants
        .iter()
        .filter(|v| !covered_variants.contains(&v.name))
        .map(|v| v.name.as_str())
        .collect();

    if !missing.is_empty() {
        diags.push(Diagnostic {
            severity: Severity::Error,
            message: format!(
                "Non-exhaustive match on type '{}'. Missing variant(s): {}",
                type_name,
                missing.join(", ")
            ),
            span,
            expected_type: None,
            found_type: None,
            suggestions: missing
                .iter()
                .map(|m| format!("Add arm: {m} -> ..."))
                .collect(),
        });
    }
}
