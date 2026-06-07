use crate::hir::{HirAgent, HirFn, HirModule, HirTable, HirType, HirTypeDef, HirUrlDecl};
use crate::rust_interop_support::{
    RustInteropSemanticsState, RustInteropSupportClass, classify_rust_crate,
    is_template_managed_app_dependency, semantics_state_for_rust_crate,
};
use crate::typeck::diagnostics::Diagnostic;
use crate::typeck::env::{AdtDef, AgentHandlerSig, Binding, BindingKind, TypeEnv, VariantDef};
use crate::typeck::ty::Ty;
use crate::typeck::unify::InferenceContext;
use std::collections::HashMap;

/// Register all top-level declarations from an HIR module into the type environment.
///
/// This is the "Pass 1" of type checking: it makes every name visible so that
/// forward references work when bodies are checked in Pass 2.
pub fn register_hir_module(
    env: &mut TypeEnv,
    module: &HirModule,
    mut uf: Option<&mut InferenceContext>,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for td in &module.types {
        register_hir_typedef(env, td);
    }
    for imp in &module.imports {
        if env.lookup(&imp.item).is_none() {
            env.define(
                imp.item.clone(),
                Binding::new(
                    Ty::ImportPlaceholder(imp.item.clone()),
                    false,
                    BindingKind::Import,
                ),
            );
        }
    }
    let mut rust_crate_dep_specs: HashMap<
        String,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    > = HashMap::new();

    for r in &module.rust_imports {
        if env.lookup(&r.alias).is_some() {
            diags.push(Diagnostic::error(
                format!(
                    "Import alias conflict: '{}' is already defined in this module",
                    r.alias
                ),
                r.span,
                "",
            ));
            continue;
        }
        if r.path.is_some() && r.git.is_some() {
            diags.push(Diagnostic::lowering(
                format!(
                    "Rust import '{}' has conflicting sources: both `path` and `git` were specified",
                    r.crate_name
                ),
                r.span,
                "",
            ));
            continue;
        }
        if r.crate_name.trim().is_empty() {
            diags.push(Diagnostic::runtime_contract(
                "Rust import references an empty crate name".into(),
                r.span,
                "",
            ));
            continue;
        }

        let support_class = classify_rust_crate(&r.crate_name);
        if matches!(support_class, RustInteropSupportClass::InternalRuntimeOnly) {
            diags.push(Diagnostic::warning(
                format!(
                    "Rust import '{}' is classified as '{}' and is primarily a runtime implementation crate; prefer Vox contracts/wrappers instead of direct crate access.",
                    r.crate_name,
                    support_class.as_label()
                ),
                r.span,
                "",
            ));
        } else if matches!(support_class, RustInteropSupportClass::Deferred) {
            diags.push(Diagnostic::warning(
                format!(
                    "Rust import '{}' is classified as '{}' (high-debt/deferred for first-class support); expect escape-hatch semantics.",
                    r.crate_name,
                    support_class.as_label()
                ),
                r.span,
                "",
            ));
        } else if matches!(support_class, RustInteropSupportClass::EscapeHatchOnly) {
            diags.push(Diagnostic::warning(
                format!(
                    "Rust import '{}' is classified as '{}' and remains an explicit escape-hatch lane.",
                    r.crate_name,
                    support_class.as_label()
                ),
                r.span,
                "",
            ));
        }

        let semantics_state = semantics_state_for_rust_crate(&r.crate_name);
        if matches!(semantics_state, RustInteropSemanticsState::Planned) {
            diags.push(Diagnostic::warning(
                format!(
                    "Rust import '{}' currently has '{}' semantics in the support registry; prefer stable wrappers/contracts where available.",
                    r.crate_name,
                    semantics_state.as_label()
                ),
                r.span,
                "",
            ));
        }

        if r.version.is_none() && r.path.is_none() && r.git.is_none() {
            diags.push(Diagnostic::warning(
                format!(
                    "Rust import '{}' has no version/path/git pin and falls back to Cargo '*'; pin dependency metadata for reproducible builds.",
                    r.crate_name
                ),
                r.span,
                "",
            ));
        }

        if is_template_managed_app_dependency(&r.crate_name)
            && (r.version.is_some() || r.path.is_some() || r.git.is_some() || r.rev.is_some())
        {
            diags.push(Diagnostic::warning(
                format!(
                    "Rust import '{}' includes explicit dependency metadata, but full app codegen templates may own this crate version/path and ignore import-level pins.",
                    r.crate_name
                ),
                r.span,
                "",
            ));
        }

        let spec = (
            r.version.clone(),
            r.path.clone(),
            r.git.clone(),
            r.rev.clone(),
        );
        if let Some(prev) = rust_crate_dep_specs.get(&r.crate_name) {
            if prev != &spec {
                diags.push(Diagnostic::lowering(
                    format!(
                        "Conflicting dependency specifications for rust crate '{}': versions/path/git metadata must match across imports for the same crate name",
                        r.crate_name
                    ),
                    r.span,
                    "",
                ));
                continue;
            }
        } else {
            rust_crate_dep_specs.insert(r.crate_name.clone(), spec);
        }

        env.define(
            r.alias.clone(),
            Binding::new(
                Ty::Named(format!("RustCrate::{}", r.crate_name)),
                false,
                BindingKind::Import,
            ),
        );
    }
    // Register tables BEFORE functions so a function signature that names a
    // table as a (possibly nested) type — e.g. `fn f(rows: List[HealthEventLog])`
    // — resolves the table name to its field-bearing `Ty::Table(...)` via the
    // `env.define_type` alias rather than falling through to a bare
    // `Ty::Named`. `db.Table.all()/get()/query()/find()` yield structural
    // `Ty::Record`s of the same fields; the unifier (see `unify.rs`) treats a
    // `Ty::Table`/`Ty::Collection` as structurally compatible with such a
    // record, so the entity name is usable as the row type. Table registration
    // depends only on imports (already registered above), so this reordering is
    // side-effect-free.
    for t in &module.tables {
        register_hir_table(env, t);
    }
    for f in &module.functions {
        register_hir_function(env, f, uf.as_deref_mut());
    }
    for sf in &module.endpoint_fns {
        register_fn_like(
            env,
            &sf.params,
            sf.return_type.as_ref(),
            &sf.name,
            uf.as_deref_mut(),
        );
    }
    for t in &module.tests {
        register_hir_function(env, t, uf.as_deref_mut());
    }
    for t in &module.mcp_tools {
        register_hir_function(env, &t.func, uf.as_deref_mut());
    }
    for r in &module.mcp_resources {
        register_hir_function(env, &r.func, uf.as_deref_mut());
    }
    for a in &module.agents {
        register_hir_agent(env, a, uf.as_deref_mut());
    }
    for u in &module.url_decls {
        register_hir_url_decl(env, u);
    }
    // Register actor handler signatures so call sites can look them up.
    // Actors are stored in `module.functions` with `durability = Actor`;
    // their handlers are sibling `HirFn`s with name `ActorName::handler`.
    register_hir_actors(env, module);
    diags
}

/// Convert an HIR type to an internal [`Ty`].
pub fn resolve_hir_type(te: &HirType, env: &TypeEnv) -> Ty {
    match te {
        HirType::Named(name) => {
            if let Some(ty) = env.lookup_type(name) {
                return ty;
            }
            match name.as_str() {
                "int" => Ty::Int,
                "float" | "float64" => Ty::Float,
                "str" => Ty::Str,
                "bool" => Ty::Bool,
                "char" => Ty::Char,
                "never" => Ty::Never,
                "Unit" => Ty::Unit,
                "dec" => Ty::Decimal,
                "Element" => Ty::Element,
                other => Ty::Named(other.to_string()),
            }
        }
        HirType::Generic(name, args) => {
            let inner_args: Vec<Ty> = args.iter().map(|a| resolve_hir_type(a, env)).collect();
            match name.as_str() {
                "list" | "List" => Ty::List(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Option" => Ty::Option(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Result" => {
                    // Result[T] => Result[T, str] (default err); Result[T, E] keeps E.
                    let mut result_args = inner_args.into_iter();
                    let ok = result_args.next().unwrap_or(Ty::TypeVar(0));
                    let err = result_args.next().unwrap_or(Ty::Str);
                    Ty::Result(Box::new(ok), Box::new(err))
                }
                "Stream" => Ty::Stream(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Map" => {
                    let mut it = inner_args.into_iter();
                    Ty::Map(
                        Box::new(it.next().unwrap_or(Ty::TypeVar(0))),
                        Box::new(it.next().unwrap_or(Ty::TypeVar(1))),
                    )
                }
                "Set" => Ty::Set(Box::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                // `Id[T]` is a surrogate-int newtype in v0.5 — see
                // ast_decl_lints::resolve_type and infer::lower_ast_ty
                // for the matching aliasing.
                "Id" => Ty::Int,
                _ => Ty::Named(name.clone()),
            }
        }
        HirType::Function(params, ret) => Ty::Fn(
            params.iter().map(|p| resolve_hir_type(p, env)).collect(),
            Box::new(resolve_hir_type(ret, env)),
        ),
        HirType::Tuple(elements) => {
            Ty::Tuple(elements.iter().map(|e| resolve_hir_type(e, env)).collect())
        }
        HirType::Unit => Ty::Unit,
        HirType::Decimal => Ty::Decimal,
    }
}

/// Render a deterministic function signature from HIR params + optional return annotation.
#[must_use]
pub fn type_signature_from_hir(
    params: &[crate::hir::HirParam],
    ret: Option<&HirType>,
    env: &TypeEnv,
) -> String {
    let params = params
        .iter()
        .map(|p| {
            let ty = p
                .type_ann
                .as_ref()
                .map(|t| resolve_hir_type(t, env).signature())
                .unwrap_or_else(|| "unknown".to_string());
            format!("{}: {ty}", p.name)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let ret = ret
        .map(|r| resolve_hir_type(r, env).signature())
        .unwrap_or_else(|| "Unit".to_string());
    format!("fn({params}) -> {ret}")
}

pub fn register_hir_typedef(env: &mut TypeEnv, td: &HirTypeDef) {
    let variants: Vec<VariantDef> = td
        .variants
        .iter()
        .map(|v| VariantDef {
            name: v.name.clone(),
            fields: v
                .fields
                .iter()
                .map(|f| (f.0.clone(), resolve_hir_type(&f.1, env)))
                .collect(),
        })
        .collect();
    let struct_fields: Vec<(String, Ty)> = td
        .fields
        .iter()
        .map(|f| (f.0.clone(), resolve_hir_type(&f.1, env)))
        .collect();
    env.register_type(AdtDef {
        name: td.name.clone(),
        variants,
        fields: struct_fields,
    });
}

/// Register a URL decl's name as an ADT type so variant names can be resolved.
pub fn register_hir_url_decl(env: &mut TypeEnv, u: &HirUrlDecl) {
    let variants: Vec<VariantDef> = u
        .variants
        .iter()
        .map(|v| VariantDef {
            name: v.name.clone(),
            fields: v
                .args
                .iter()
                .map(|a| (a.name.clone(), resolve_hir_type(&a.ty, env)))
                .collect(),
        })
        .collect();
    env.register_type(AdtDef {
        name: u.name.clone(),
        variants,
        fields: vec![],
    });
}

pub fn register_hir_function(env: &mut TypeEnv, f: &HirFn, mut uf: Option<&mut InferenceContext>) {
    env.push_scope();
    for (i, g) in f.generics.iter().enumerate() {
        env.define_type(g.clone(), Ty::GenericParam(i as u32));
    }
    let param_tys: Vec<Ty> = f
        .params
        .iter()
        .map(|p| {
            if let Some(t) = &p.type_ann {
                resolve_hir_type(t, env)
            } else if let Some(ctx) = uf.as_deref_mut() {
                ctx.fresh_var()
            } else {
                Ty::Infer
            }
        })
        .collect();
    let ret_ty = f
        .return_type
        .as_ref()
        .map(|t| resolve_hir_type(t, env))
        .unwrap_or_else(|| {
            if let Some(ctx) = uf {
                ctx.fresh_var()
            } else {
                Ty::Infer
            }
        });
    env.pop_scope();
    env.define(
        f.name.clone(),
        Binding {
            ty: Ty::Fn(param_tys, Box::new(ret_ty)),
            mutable: false,
            kind: BindingKind::Function,
            is_deprecated: false,
        },
    );
}

fn register_fn_like(
    env: &mut TypeEnv,
    params: &[crate::hir::HirParam],
    ret: Option<&HirType>,
    name: &str,
    mut uf: Option<&mut InferenceContext>,
) {
    let param_tys: Vec<Ty> = params
        .iter()
        .map(|p| {
            if let Some(t) = &p.type_ann {
                resolve_hir_type(t, env)
            } else if let Some(ctx) = uf.as_deref_mut() {
                ctx.fresh_var()
            } else {
                Ty::Infer
            }
        })
        .collect();
    let ret_ty = ret.map(|t| resolve_hir_type(t, env)).unwrap_or_else(|| {
        if let Some(ctx) = uf {
            ctx.fresh_var()
        } else {
            Ty::Infer
        }
    });
    env.define(
        name.to_string(),
        Binding::new(
            Ty::Fn(param_tys, Box::new(ret_ty)),
            false,
            BindingKind::Function,
        ),
    );
}

pub fn register_hir_table(env: &mut TypeEnv, t: &HirTable) {
    let fields: Vec<(String, Ty)> = t
        .fields
        .iter()
        .map(|f| (f.name.clone(), resolve_hir_type(&f.type_ann, env)))
        .collect();
    let table_ty = Ty::Table(t.name.clone(), fields);
    env.define(
        t.name.clone(),
        Binding {
            ty: table_ty.clone(),
            mutable: false,
            kind: BindingKind::Table,
            is_deprecated: t.is_deprecated,
        },
    );
    // Type alias so `fn f(row: UserProfile)` resolves to Ty::Table
    // (field-bearing) instead of bare Ty::Named — same fix as
    // ast_decl_lints::register_table.
    env.define_type(t.name.clone(), table_ty);
}

/// Register actor handler signatures from `module.functions`.
///
/// Actors live in `module.functions` as `HirFn`s with
/// `durability = DurabilityKind::Actor`. Their `on <event>(...)` handlers
/// are sibling functions named `ActorName::event`. We synthesize an
/// [`crate::typeck::env::ActorHandlerSig`] per handler and register them under the actor's
/// bare name, so call sites can `env.lookup_actor(name)` and walk
/// handler signatures.
pub fn register_hir_actors(env: &mut TypeEnv, module: &crate::hir::HirModule) {
    use crate::hir::nodes::DurabilityKind;
    use crate::typeck::env::ActorHandlerSig;

    let mut by_actor: std::collections::HashMap<String, Vec<ActorHandlerSig>> =
        std::collections::HashMap::new();
    // Each actor's main fn carries `durability = Actor`; its handlers are
    // separate `HirFn`s whose `name` is `ActorName::handlerName`.
    let actor_names: Vec<String> = module
        .functions
        .iter()
        .filter(|f| matches!(f.durability, Some(DurabilityKind::Actor)))
        .filter(|f| !f.name.contains("::"))
        .map(|f| f.name.clone())
        .collect();
    for actor_name in &actor_names {
        let prefix = format!("{actor_name}::");
        for h in module
            .functions
            .iter()
            .filter(|f| f.name.starts_with(&prefix))
        {
            let event_name = h.name[prefix.len()..].to_string();
            let params: Vec<(String, Ty)> = h
                .params
                .iter()
                .map(|p| {
                    let ty = p
                        .type_ann
                        .as_ref()
                        .map(|t| resolve_hir_type(t, env))
                        .unwrap_or(Ty::Infer);
                    (p.name.clone(), ty)
                })
                .collect();
            let return_type = h
                .return_type
                .as_ref()
                .map(|t| resolve_hir_type(t, env))
                .unwrap_or(Ty::Unit);
            by_actor
                .entry(actor_name.clone())
                .or_default()
                .push(ActorHandlerSig {
                    event_name,
                    params,
                    return_type,
                });
        }
        // Ensure even handler-less actors register so `lookup_actor`
        // returns `Some(empty_vec)` instead of `None` (the subscribe()
        // fallback to `Stream[str]` still works either way).
        by_actor.entry(actor_name.clone()).or_default();
    }
    for (name, handlers) in by_actor {
        env.register_actor(name, handlers);
    }
}

pub fn register_hir_agent(env: &mut TypeEnv, a: &HirAgent, mut uf: Option<&mut InferenceContext>) {
    let handlers: Vec<AgentHandlerSig> = a
        .handlers
        .iter()
        .map(|h| AgentHandlerSig {
            event_name: h.event_name.clone(),
            params: h
                .params
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        if let Some(t) = &p.type_ann {
                            resolve_hir_type(t, env)
                        } else if let Some(ctx) = uf.as_deref_mut() {
                            ctx.fresh_var()
                        } else {
                            Ty::Infer
                        },
                    )
                })
                .collect(),
            return_type: h
                .return_type
                .as_ref()
                .map(|t| resolve_hir_type(t, env))
                .unwrap_or_else(|| {
                    if let Some(ctx) = uf.as_deref_mut() {
                        ctx.fresh_var()
                    } else {
                        Ty::Infer
                    }
                }),
        })
        .collect();
    env.register_agent(a.name.clone(), handlers);
}
