use crate::diagnostics::{Diagnostic, Severity};
use crate::env::{BindingKind, TypeEnv, Binding};
use crate::ty::Ty;
use crate::unify::InferenceContext;
use vox_ast::pattern::Pattern;

pub fn bind_pattern(
    env: &mut TypeEnv,
    uf: &mut InferenceContext,
    _diags: &mut Vec<Diagnostic>,
    pattern: &Pattern,
    ty: &Ty,
    mutable: bool,
) {
    let ty = uf.resolve(ty);
    match pattern {
        Pattern::Ident { name, .. } => {
            env.define(
                name.clone(),
                Binding::new(ty.clone(), mutable, BindingKind::Variable),
            );
        }
        Pattern::Tuple { elements, .. } => {
            if let Ty::Tuple(elem_tys) = ty {
                for (pat, elem_ty) in elements.iter().zip(elem_tys.iter()) {
                    bind_pattern(env, uf, _diags, pat, elem_ty, mutable);
                }
            } else {
                for pat in elements {
                    let fv = uf.fresh_var();
                    bind_pattern(env, uf, _diags, pat, &fv, mutable);
                }
            }
        }
        Pattern::Constructor { name, fields, .. } => {
            let field_tys = match &ty {
                Ty::Option(inner) if name == "Some" => vec![inner.as_ref().clone()],
                Ty::Option(_) if name == "None" => vec![],
                Ty::Result(inner) if name == "Ok" => vec![inner.as_ref().clone()],
                Ty::Result(_) if name == "Error" => vec![Ty::Str],
                Ty::Named(adt_name) => {
                    if let Some(adt) = env.lookup_adt(adt_name) {
                        if let Some(variant) = adt.variants.iter().find(|v| &v.name == name) {
                            variant.fields.iter().map(|f| f.1.clone()).collect::<Vec<Ty>>()
                        } else {
                            fields.iter().map(|_| uf.fresh_var()).collect()
                        }
                    } else {
                        fields.iter().map(|_| uf.fresh_var()).collect()
                    }
                }
                _ => {
                    fields.iter().map(|_| uf.fresh_var()).collect()
                }
            };

            for (pat, fty) in fields.iter().zip(field_tys.iter()) {
                bind_pattern(env, uf, _diags, pat, fty, mutable);
            }
        }
        Pattern::Wildcard { .. } => {}
        Pattern::Literal { .. } => {}
        Pattern::Or { patterns, .. } => {
            for p in patterns {
                bind_pattern(env, uf, _diags, p, &ty, mutable);
            }
        }
        Pattern::Binding { name, pattern, .. } => {
            env.define(
                name.clone(),
                Binding::new(ty.clone(), mutable, BindingKind::Variable),
            );
            bind_pattern(env, uf, _diags, pattern, &ty, mutable);
        }
        Pattern::Rest { name, .. } => {
            if let Some(n) = name {
                env.define(
                    n.clone(),
                    Binding::new(ty.clone(), mutable, BindingKind::Variable),
                );
            }
        }
        Pattern::Range { .. } => {}
        Pattern::Record { fields, .. } => {
            for (fname, pat_opt) in fields {
                if let Some(p) = pat_opt {
                    let field_ty = if let Ty::Record(record_fields) = &ty {
                        record_fields
                            .iter()
                            .find(|(n, _)| n == fname)
                            .map(|(_, t)| t.clone())
                            .unwrap_or_else(|| uf.fresh_var())
                    } else {
                        uf.fresh_var()
                    };
                    bind_pattern(env, uf, _diags, p, &field_ty, mutable);
                }
            }
        }
    }
}

pub fn check_match_exhaustiveness(
    env: &TypeEnv,
    diags: &mut Vec<Diagnostic>,
    subject_ty: &Ty,
    arms: &[vox_ast::expr::MatchArm],
    span: vox_ast::span::Span,
    source: &str,
) {
    let type_name = match subject_ty {
        Ty::Named(name) => name.as_str(),
        _ => return,
    };

    let adt = match env.lookup_adt(type_name) {
        Some(adt) => adt,
        None => return,
    };

    let mut covered_variants: Vec<String> = Vec::new();
    let mut has_wildcard = false;

    for arm in arms {
        match &arm.pattern {
            Pattern::Wildcard { .. } => {
                has_wildcard = true;
            }
            Pattern::Ident { name, .. } => {
                if adt.variants.iter().any(|v| v.name == *name) {
                    covered_variants.push(name.clone());
                } else {
                    has_wildcard = true;
                }
            }
            Pattern::Constructor { name, .. } => {
                covered_variants.push(name.clone());
            }
            Pattern::Literal { .. }
            | Pattern::Tuple { .. }
            | Pattern::Or { .. }
            | Pattern::Binding { .. }
            | Pattern::Rest { .. }
            | Pattern::Range { .. }
            | Pattern::Record { .. } => {}
        }
    }

    if has_wildcard {
        return;
    }

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
            context: Some(Diagnostic::capture_context(source, span)),
            suggestions: missing
                .iter()
                .map(|m| format!("Add arm: {m} -> ..."))
                .collect(),
        });
    }
}
