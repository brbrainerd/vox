use std::rc::Rc;
use crate::diagnostics::{Diagnostic, Severity};
use crate::env::TypeEnv;
use crate::ty::Ty;
use crate::unify::InferenceContext;
use vox_ast::types::TypeExpr;

pub fn resolve_type(te: &TypeExpr, env: &TypeEnv) -> Ty {
    match te {
        TypeExpr::Named { name, .. } => {
            if let Some(ty) = env.lookup_type(name) {
                return ty;
            }
            match name.as_str() {
                "int" => Ty::Int,
                "float" | "float64" => Ty::Float,
                "str" => Ty::Str,
                "bool" => Ty::Bool,
                "char" => Ty::Char,
                "bytes" => Ty::Bytes,
                "never" => Ty::Never,
                "Unit" => Ty::Unit,
                "Element" => Ty::Element,
                other => Ty::Named(other.to_string()),
            }
        }

        TypeExpr::Generic { name, args, .. } => {
            let inner_args: Vec<Ty> = args.iter().map(|a| resolve_type(a, env)).collect();
            match name.as_str() {
                "list" | "List" => Ty::List(Rc::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Option" => Ty::Option(Rc::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Result" => Ty::Result(Rc::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Stream" => Ty::Stream(Rc::new(
                    inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)),
                )),
                "Id" => {
                    let table_name =
                        if let Some(vox_ast::types::TypeExpr::Named { name, .. }) = args.first() {
                            name.clone()
                        } else {
                            "Unknown".to_string()
                        };
                    Ty::Id(table_name)
                }
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
            Ty::Fn(param_tys, Rc::new(ret_ty))
        }
        TypeExpr::Tuple { elements, .. } => {
            Ty::Tuple(elements.iter().map(|e| resolve_type(e, env)).collect())
        }
        TypeExpr::Unit { .. } => Ty::Unit,
        TypeExpr::Union { variants, .. } => {
            Ty::Union(variants.iter().map(|v| resolve_type(v, env)).collect())
        }
        TypeExpr::Map { key, value, .. } => Ty::Map(
            Rc::new(resolve_type(key, env)),
            Rc::new(resolve_type(value, env)),
        ),
        TypeExpr::Set { element, .. } => Ty::Set(Rc::new(resolve_type(element, env))),
        TypeExpr::Intersection { left, right, .. } => Ty::Intersection(
            Rc::new(resolve_type(left, env)),
            Rc::new(resolve_type(right, env)),
        ),
    }
}

pub fn instantiate_inner(
    ty: Ty,
    uf: &mut InferenceContext,
    map: &mut std::collections::HashMap<u32, Ty>,
) -> Ty {
    match ty {
        Ty::GenericParam(id) => map.entry(id).or_insert_with(|| uf.fresh_var()).clone(),
        Ty::List(inner) => Ty::List(Rc::new(instantiate_inner((*inner).clone(), uf, map))),
        Ty::Option(inner) => Ty::Option(Rc::new(instantiate_inner((*inner).clone(), uf, map))),
        Ty::Result(inner) => Ty::Result(Rc::new(instantiate_inner((*inner).clone(), uf, map))),
        Ty::Stream(inner) => Ty::Stream(Rc::new(instantiate_inner((*inner).clone(), uf, map))),
        Ty::Map(k, v) => Ty::Map(
            Rc::new(instantiate_inner((*k).clone(), uf, map)),
            Rc::new(instantiate_inner((*v).clone(), uf, map)),
        ),
        Ty::Set(inner) => Ty::Set(Rc::new(instantiate_inner((*inner).clone(), uf, map))),
        Ty::Fn(params, ret) => Ty::Fn(
            params
                .into_iter()
                .map(|p| instantiate_inner(p, uf, map))
                .collect(),
            Rc::new(instantiate_inner((*ret).clone(), uf, map)),
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
        Ty::Union(variants) => Ty::Union(
            variants
                .into_iter()
                .map(|v| instantiate_inner(v, uf, map))
                .collect(),
        ),
        Ty::Intersection(l, r) => Ty::Intersection(
            Rc::new(instantiate_inner((*l).clone(), uf, map)),
            Rc::new(instantiate_inner((*r).clone(), uf, map)),
        ),
        _ => ty,
    }
}

pub fn instantiate(ty: Ty, uf: &mut InferenceContext) -> Ty {
    let mut map = std::collections::HashMap::new();
    instantiate_inner(ty, uf, &mut map)
}

pub fn validate_type_expr(
    te: &TypeExpr,
    env: &TypeEnv,
    diagnostics: &mut Vec<Diagnostic>,
    source: &str,
) {
    match te {
        TypeExpr::Named { .. } => {}
        TypeExpr::Generic { name, args, span } => {
            if name == "Id" {
                if let Some(vox_ast::types::TypeExpr::Named {
                    name: table_name, ..
                }) = args.first()
                {
                    let binding = env.lookup(table_name);
                    let is_table = match binding {
                        Some(b) => matches!(b.ty, Ty::Table(..) | Ty::Collection(..)),
                        None => false,
                    };
                    if !is_table {
                        diagnostics.push(Diagnostic {
                            message: format!("'Id' references unknown table '{}'", table_name),
                            span: *span,
                            severity: Severity::Error,
                            expected_type: None,
                            found_type: None,
                            context: Some(Diagnostic::capture_context(source, *span)),
                            suggestions: vec![],
                        });
                    }
                }
            }
            for a in args {
                validate_type_expr(a, env, diagnostics, source);
            }
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            for p in params {
                validate_type_expr(p, env, diagnostics, source);
            }
            validate_type_expr(return_type, env, diagnostics, source);
        }
        TypeExpr::Tuple { elements, .. } => {
            for e in elements {
                validate_type_expr(e, env, diagnostics, source);
            }
        }
        TypeExpr::Union { variants, .. } => {
            for v in variants {
                validate_type_expr(v, env, diagnostics, source);
            }
        }
        TypeExpr::Map { key, value, .. } => {
            validate_type_expr(key, env, diagnostics, source);
            validate_type_expr(value, env, diagnostics, source);
        }
        TypeExpr::Set { element, .. } => {
            validate_type_expr(element, env, diagnostics, source);
        }
        TypeExpr::Intersection { left, right, .. } => {
            validate_type_expr(left, env, diagnostics, source);
            validate_type_expr(right, env, diagnostics, source);
        }
        TypeExpr::Unit { .. } => {}
    }
}
