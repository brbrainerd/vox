use crate::ast::span::Span;
use crate::hir::*;
use crate::typeck::diagnostics::{Diagnostic, TypeckSeverity};
use crate::typeck::env::{Binding, BindingKind};
use crate::typeck::ty::Ty;

use super::{Checker, hir_expr_span};

impl<'a> Checker<'a> {
    pub(super) fn extract_iterable_element(&mut self, ty: &Ty) -> Ty {
        let resolved = self.uf.resolve(ty);
        match resolved {
            Ty::List(inner) => inner.as_ref().clone(),
            Ty::Set(inner) => inner.as_ref().clone(),
            Ty::Map(k, v) => Ty::Tuple(vec![k.as_ref().clone(), v.as_ref().clone()]),
            Ty::Stream(inner) => inner.as_ref().clone(),
            Ty::Str => Ty::Char,
            // `any` is an escape hatch — iterating over `any` yields `any` elements.
            Ty::Named(ref n) if n == "any" => Ty::Named("any".to_string()),
            _ => self.uf.fresh_var(),
        }
    }

    pub(super) fn check_unary_op(&mut self, op: HirUnOp, ty: Ty) -> Ty {
        let ty = self.uf.resolve(&ty);
        match op {
            HirUnOp::Not => {
                let _ = self.uf.unify(&ty, &Ty::Bool);
                Ty::Bool
            }
            HirUnOp::Neg => {
                if ty == Ty::Int {
                    Ty::Int
                } else if ty == Ty::Float || ty == Ty::Decimal {
                    ty
                } else {
                    Ty::Error
                }
            }
        }
    }

    pub(super) fn check_binary_op(&mut self, op: HirBinOp, l: Ty, r: Ty, span: Span) -> Ty {
        let l = self.uf.resolve(&l);
        let r = self.uf.resolve(&r);
        match op {
            HirBinOp::Add => {
                if l == Ty::Str || r == Ty::Str {
                    Ty::Str
                } else if l == Ty::Int && r == Ty::Int {
                    Ty::Int
                } else if l == Ty::Decimal && r == Ty::Decimal {
                    Ty::Decimal
                } else if (l == Ty::Float || l == Ty::Int) && (r == Ty::Float || r == Ty::Int) {
                    Ty::Float
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Invalid operands for +: {l:?} and {r:?}"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }
            }
            HirBinOp::Sub | HirBinOp::Mul | HirBinOp::Div | HirBinOp::Mod => {
                if l == Ty::Int && r == Ty::Int {
                    Ty::Int
                } else if l == Ty::Decimal && r == Ty::Decimal {
                    Ty::Decimal
                } else if (l == Ty::Float || l == Ty::Int) && (r == Ty::Float || r == Ty::Int) {
                    Ty::Float
                } else {
                    Ty::Error
                }
            }
            HirBinOp::Lt | HirBinOp::Gt | HirBinOp::Lte | HirBinOp::Gte => Ty::Bool,
            HirBinOp::Is | HirBinOp::Isnt => Ty::Bool,
            HirBinOp::And | HirBinOp::Or => {
                let _ = self.uf.unify(&l, &Ty::Bool);
                let _ = self.uf.unify(&r, &Ty::Bool);
                Ty::Bool
            }
            HirBinOp::Pipe => match r {
                Ty::Fn(params, ret) => {
                    let instantiated = self.uf.instantiate(&Ty::Fn(params, ret));
                    if let Ty::Fn(p, rt) = instantiated {
                        if !p.is_empty() {
                            let _ = self.uf.unify(&l, &p[0]);
                        }
                        rt.as_ref().clone()
                    } else {
                        Ty::Error
                    }
                }
                _ => l,
            },
        }
    }

    /// Validate `with { ... }` option bags (workflow / activity calls).
    pub(super) fn check_with_options(&mut self, options: &HirExpr, _with_span: Span) {
        match options {
            HirExpr::ObjectLit(fields, _) => {
                for (name, expr) in fields {
                    let v_ty = self.check_expr(expr, None);
                    let v_res = self.uf.resolve(&v_ty);
                    match name.as_str() {
                        "retries" => {
                            if self.uf.unify(&Ty::Int, &v_res).is_err() {
                                self.diags.push(Diagnostic::warning(
                                    format!(
                                        "'with' option 'retries' expects Int; got incompatible type ({v_res:?})"
                                    ),
                                    hir_expr_span(expr),
                                    self.source,
                                ));
                            }
                        }
                        "timeout" | "activity_id" | "id" | "meta" | "mens" | "initial_backoff" => {
                            if self.uf.unify(&Ty::Str, &v_res).is_err() {
                                self.diags.push(Diagnostic::warning(
                                    format!(
                                        "'with' option '{name}' expects Str; got incompatible type ({v_res:?})"
                                    ),
                                    hir_expr_span(expr),
                                    self.source,
                                ));
                            }
                        }
                        other => {
                            self.diags.push(Diagnostic::warning(
                                format!("Unknown 'with' option '{other}'"),
                                hir_expr_span(expr),
                                self.source,
                            ));
                        }
                    }
                }
            }
            _ => {
                let _ = self.check_expr(options, None);
                self.diags.push(Diagnostic::error(
                    "'with' options must be a record literal".into(),
                    hir_expr_span(options),
                    self.source,
                ));
            }
        }
    }

    pub(super) fn check_arguments(&mut self, expected: &[Ty], actual: &[HirArg], span: Span) {
        if expected.len() != actual.len() {
            self.diags.push(Diagnostic::error(
                crate::typeck::diagnostics::msg_arg_count_mismatch(expected.len(), actual.len()),
                span,
                self.source,
            ));
            for arg in actual {
                let _ = self.check_expr(&arg.value, None);
            }
            return;
        }
        for (e, a) in expected.iter().zip(actual.iter()) {
            let a_ty = self.check_expr(&a.value, Some(e));
            if let Err(msg) = self.uf.unify(e, &a_ty) {
                let arg_span = hir_expr_span(&a.value);
                self.diags.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!("Argument type mismatch: {msg}"),
                    span: arg_span,
                    expected_type: Some(format!("{e:?}")),
                    found_type: Some(format!("{a_ty:?}")),
                    context: Some(Diagnostic::capture_context(self.source, arg_span)),
                    suggestions: vec![],
                    category: crate::typeck::diagnostics::DiagnosticCategory::Typecheck,
                    code: Some("typecheck.arg_mismatch".into()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                });
            }
        }
    }

    pub(super) fn bind_pattern(&mut self, pattern: &HirPattern, ty: &Ty, mutable: bool) {
        let ty = self.uf.resolve(ty);
        match pattern {
            HirPattern::Ident(name, _) => {
                self.env.define(
                    name.clone(),
                    Binding::new(ty.clone(), mutable, BindingKind::Variable),
                );
            }
            HirPattern::Tuple(patterns, span) => {
                if let Ty::Tuple(tys) = &ty {
                    if tys.len() != patterns.len() {
                        self.diags.push(Diagnostic::error(
                            crate::typeck::diagnostics::msg_tuple_size_mismatch(
                                tys.len(),
                                patterns.len(),
                            ),
                            *span,
                            self.source,
                        ));
                    }
                    for (p, t) in patterns.iter().zip(tys.iter()) {
                        self.bind_pattern(p, t, mutable);
                    }
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Cannot destructure non-tuple type {ty:?}"),
                        *span,
                        self.source,
                    ));
                }
            }
            HirPattern::Wildcard(_) => {}
            HirPattern::Literal(_, _) => {}
            HirPattern::Constructor(name, fields, span) => match &ty {
                Ty::Option(inner) if name == "Some" && fields.len() == 1 => {
                    self.bind_pattern(&fields[0], inner.as_ref(), mutable);
                }
                Ty::Result(ok_ty, _) if name == "Ok" && fields.len() == 1 => {
                    self.bind_pattern(&fields[0], ok_ty.as_ref(), mutable);
                }
                Ty::Result(_, err_ty)
                    if (name == "Error" || name == "Err") && fields.len() == 1 =>
                {
                    self.bind_pattern(&fields[0], err_ty.as_ref(), mutable);
                }
                Ty::Option(_) if name == "None" && fields.is_empty() => {}
                _ => {
                    let expected_adt_name = match &ty {
                        Ty::Named(n) => Some(n.as_str()),
                        _ => None,
                    };
                    if let Some(field_defs) = self.env.lookup_adt_variant(name, expected_adt_name) {
                        for (i, p) in fields.iter().enumerate() {
                            let ft = field_defs
                                .get(i)
                                .map(|(_, t)| t.clone())
                                .unwrap_or(Ty::Error);
                            self.bind_pattern(p, &ft, mutable);
                        }
                    } else {
                        self.diags.push(Diagnostic::error(
                            format!("Unknown constructor or pattern mismatch: {name} on {ty:?}"),
                            *span,
                            self.source,
                        ));
                    }
                }
            },
        }
    }
}
