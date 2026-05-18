use std::collections::HashSet;

use crate::ast::span::Span;
use crate::hir::*;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use crate::typeck::env::{Binding, BindingKind};
use crate::typeck::registration::resolve_hir_type;
use crate::typeck::ty::Ty;

use super::Checker;
use super::match_exhaust::check_hir_match_exhaustiveness;

impl<'a> Checker<'a> {
    #[allow(dead_code)]
    fn check_db_select_projection(
        &mut self,
        fields: &[(String, Ty)],
        cols: &[String],
        span: Span,
    ) -> bool {
        let mut ok = true;
        if cols.is_empty() {
            self.diags.push(Diagnostic::error(
                "db select(...) requires at least one column name".into(),
                span,
                self.source,
            ));
            return false;
        }
        let mut seen = HashSet::new();
        for c in cols {
            if !seen.insert(c.as_str()) {
                self.diags.push(Diagnostic::error(
                    format!("duplicate column '{c}' in select(...)"),
                    span,
                    self.source,
                ));
                ok = false;
            }
            if fields.iter().all(|(n, _)| n != c) {
                self.diags.push(Diagnostic::error(
                    format!("Unknown field '{c}' in select(...)"),
                    span,
                    self.source,
                ));
                ok = false;
            }
        }
        if !ok {
            return false;
        }
        ok
    }

    #[allow(dead_code)]
    fn validate_db_predicate(
        &mut self,
        pred: &HirDbPredicate,
        fields: &[(String, Ty)],
        args: &[HirArg],
        arg_ix: &mut usize,
        table: &str,
        span: Span,
    ) -> bool {
        let expect_value = |field: &str| -> Option<Ty> {
            let (_, f_ty) = fields.iter().find(|(n, _)| n == field).cloned()?;
            Some(f_ty)
        };
        match pred {
            HirDbPredicate::Eq { field }
            | HirDbPredicate::Neq { field }
            | HirDbPredicate::Lt { field }
            | HirDbPredicate::Lte { field }
            | HirDbPredicate::Gt { field }
            | HirDbPredicate::Gte { field }
            | HirDbPredicate::Contains { field } => {
                let Some(f_ty) = expect_value(field) else {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown field '{field}' on table '{table}' for predicate"),
                        span,
                        self.source,
                    ));
                    return false;
                };
                if *arg_ix >= args.len() {
                    self.diags.push(Diagnostic::error(
                        "db predicate/value mismatch (internal arg ordering error)".into(),
                        span,
                        self.source,
                    ));
                    return false;
                }
                let v_ty = self.check_expr(&args[*arg_ix].value, None);
                *arg_ix += 1;
                let _ = self.uf.unify(&f_ty, &v_ty);
                true
            }
            HirDbPredicate::IsNull { field } => {
                if expect_value(field).is_none() {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown field '{field}' on table '{table}' for predicate"),
                        span,
                        self.source,
                    ));
                    return false;
                }
                true
            }
            HirDbPredicate::In { field, arity } => {
                let Some(f_ty) = expect_value(field) else {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown field '{field}' on table '{table}' for predicate"),
                        span,
                        self.source,
                    ));
                    return false;
                };
                if *arity == 0 {
                    self.diags.push(Diagnostic::error(
                        "db where(...): `in` must have at least one value".into(),
                        span,
                        self.source,
                    ));
                    return false;
                }
                for _ in 0..*arity {
                    if *arg_ix >= args.len() {
                        self.diags.push(Diagnostic::error(
                            "db predicate/value mismatch (internal arg ordering error)".into(),
                            span,
                            self.source,
                        ));
                        return false;
                    }
                    let v_ty = self.check_expr(&args[*arg_ix].value, None);
                    *arg_ix += 1;
                    let _ = self.uf.unify(&f_ty, &v_ty);
                }
                true
            }
            HirDbPredicate::And(parts) | HirDbPredicate::Or(parts) => parts
                .iter()
                .all(|p| self.validate_db_predicate(p, fields, args, arg_ix, table, span)),
            HirDbPredicate::Not(inner) => {
                self.validate_db_predicate(inner, fields, args, arg_ix, table, span)
            }
        }
    }
    pub fn check_expr(&mut self, expr: &HirExpr, expected: Option<&Ty>) -> Ty {
        let ty = match expr {
            HirExpr::IntLit(_, _) => Ty::Int,
            HirExpr::FloatLit(_, _) => Ty::Float,
            HirExpr::StringLit(_, _) => Ty::Str,
            HirExpr::BoolLit(_, _) => Ty::Bool,
            HirExpr::DecimalLit(_, _) => Ty::Decimal,
            HirExpr::TupleLit(exprs, _) => {
                let tys = exprs.iter().map(|e| self.check_expr(e, None)).collect();
                Ty::Tuple(tys)
            }

            HirExpr::Ident(name, span) => {
                if let Some(binding) = self.env.lookup(name) {
                    if binding.is_deprecated {
                        self.diags.push(Diagnostic {
                            severity: TypeckSeverity::Warning,
                            message: format!("'{name}' is deprecated"),
                            span: *span,
                            expected_type: None,
                            found_type: None,
                            context: Some(Diagnostic::capture_context(self.source, *span)),
                            suggestions: vec![],
                            category: DiagnosticCategory::Typecheck,
                            code: Some("typecheck.deprecated_ident".into()),
                            fixes: vec![],
                            line_col: None,
                            missing_cases: vec![],
                            ast_node_kind: None,
                        });
                    }
                    binding.ty.clone()
                } else if let Some(ty) = self.builtins.lookup_var(name) {
                    ty
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Undefined variable: {name}"),
                        *span,
                        self.source,
                    ));
                    Ty::Error
                }
            }

            HirExpr::ObjectLit(fields, _span) => {
                let mut expected_fields = std::collections::HashMap::new();
                let mut struct_name: Option<String> = None;
                let mut map_value_ty: Option<Ty> = None;
                if let Some(exp) = expected {
                    let resolved = self.uf.resolve(exp);
                    match &resolved {
                        Ty::Record(fds) | Ty::Table(_, fds) | Ty::Collection(_, fds) => {
                            for (n, t) in fds {
                                expected_fields.insert(n.clone(), t.clone());
                            }
                        }
                        Ty::Map(k_ty, v_ty) => {
                            // Object-literal-as-Map: `{a: 1, b: 2}` ascribed
                            // to `Map[K, V]` infers as Map, with every field
                            // value checked against V and the key shape
                            // unified to K (typically Str). Closes the
                            // `Record vs Map` class of typecheck failures
                            // surfaced in option_type / nested_types /
                            // ref_types — 2026-05-18 P2.3.
                            let _ = self.uf.unify(&Ty::Str, k_ty);
                            map_value_ty = Some(v_ty.as_ref().clone());
                        }
                        Ty::Named(n) => {
                            // Struct type: pull declared fields from the env so an anonymous
                            // record literal `{ f: e, ... }` ascribed to `Named(Foo)` checks
                            // against the struct's shape and unifies with `Named(Foo)`.
                            if let Some(adt) = self.env.lookup_adt(n) {
                                if !adt.fields.is_empty() {
                                    for (fname, fty) in &adt.fields {
                                        expected_fields.insert(fname.clone(), fty.clone());
                                    }
                                    struct_name = Some(n.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }

                let typed_fields: Vec<(String, Ty)> = fields
                    .iter()
                    .map(|(name, expr)| {
                        let field_expected =
                            map_value_ty.as_ref().or_else(|| expected_fields.get(name));
                        (name.clone(), self.check_expr(expr, field_expected))
                    })
                    .collect();
                if let Some(value_ty) = map_value_ty {
                    Ty::Map(Box::new(Ty::Str), Box::new(value_ty))
                } else if let Some(name) = struct_name {
                    Ty::Named(name)
                } else {
                    Ty::Record(typed_fields)
                }
            }
            HirExpr::ListLit(elements, _span) => {
                let mut expected_elem_ty = None;
                if let Some(exp) = expected {
                    if let Ty::List(inner) = self.uf.resolve(exp) {
                        expected_elem_ty = Some(inner.as_ref().clone())
                    }
                }

                let elem_ty = if elements.is_empty() {
                    expected_elem_ty.unwrap_or_else(|| self.uf.fresh_var())
                } else {
                    let mut unified_ty =
                        expected_elem_ty.unwrap_or_else(|| self.check_expr(&elements[0], None));
                    for e in elements.iter() {
                        let t = self.check_expr(e, Some(&unified_ty));
                        if let Ok(lub) = self.uf.least_upper_bound(unified_ty.clone(), t.clone()) {
                            unified_ty = lub;
                        } else {
                            let _ = self.uf.unify(&unified_ty, &t);
                        }
                    }
                    unified_ty
                };
                Ty::List(Box::new(elem_ty))
            }

            HirExpr::Binary(HirBinOp::Pipe, left, right, span) => {
                let l_ty = self.check_expr(left, None);
                let raw_right = self.check_expr(right, None);
                let resolved_right = self.uf.resolve(&raw_right);
                // Instantiate generics before inspecting — without this, the
                // RHS function's own type variables are reused across pipe
                // sites, leaking constraints (and effectively monomorphizing
                // the binding to whichever site checks first).
                let r_ty = self.uf.instantiate(&resolved_right);
                match r_ty {
                    Ty::Fn(params, ret) => {
                        if let Some(first) = params.first() {
                            let _ = self.uf.unify(&l_ty, first);
                        }
                        ret.as_ref().clone()
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diagnostic::error(
                            format!("Not a function: {r_ty:?}"),
                            *span,
                            self.source,
                        ));
                        Ty::Error
                    }
                }
            }
            HirExpr::Binary(op, left, right, span) => {
                let l_ty = self.check_expr(left, None);
                let r_ty = self.check_expr(right, None);
                self.check_binary_op(*op, l_ty, r_ty, *span)
            }
            HirExpr::Unary(op, operand, _span) => {
                let ty = self.check_expr(operand, None);
                self.check_unary_op(*op, ty)
            }

            HirExpr::Call(callee, args, _tail, span) => {
                let raw_callee = self.check_expr(callee, None);
                let callee_ty = self.uf.resolve(&raw_callee);
                let callee_ty = self.uf.instantiate(&callee_ty);
                match callee_ty {
                    Ty::Fn(params, ret) => {
                        self.check_arguments(&params, args, *span);
                        ret.as_ref().clone()
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diagnostic::error(
                            format!("Not a function: {callee_ty:?}"),
                            *span,
                            self.source,
                        ));
                        Ty::Error
                    }
                }
            }
            HirExpr::MethodCall(object, method, args, opt_plan, span) => {
                if opt_plan.is_some() && matches!(method.as_str(), "limit" | "order_by") {
                    self.diags.push(Diagnostic::error(
                        format!(
                            "db query chaining via '.{method}(...)' is not supported yet; use typed db.Table operations directly"
                        ),
                        *span,
                        self.source,
                    ));
                    return Ty::Error;
                }
                let obj_ty = self.check_expr(object, None);
                let obj_ty = self.uf.resolve(&obj_ty);
                if let Some(method_ty) = self.builtins.lookup_method(&obj_ty, method) {
                    let bindings = match &obj_ty {
                        Ty::List(inner)
                        | Ty::Option(inner)
                        | Ty::Stream(inner)
                        | Ty::Set(inner) => {
                            vec![inner.as_ref().clone()]
                        }
                        Ty::Result(ok, err) => vec![ok.as_ref().clone(), err.as_ref().clone()],
                        Ty::Map(k, v) => vec![k.as_ref().clone(), v.as_ref().clone()],
                        _ => vec![],
                    };
                    let method_ty = self.uf.instantiate_with(&method_ty, &bindings);

                    if let Ty::Fn(params, ret) = method_ty {
                        self.check_arguments(&params, args, *span);
                        ret.as_ref().clone()
                    } else {
                        Ty::Error
                    }
                } else if let Ty::ActorRef(actor_name) = &obj_ty {
                    if let Some(handlers) = self.env.lookup_actor(actor_name) {
                        if let Some(h) = handlers.iter().find(|h| h.event_name == *method) {
                            let params: Vec<Ty> = h.params.iter().map(|(_, t)| t.clone()).collect();
                            let method_ty = self
                                .uf
                                .instantiate(&Ty::Fn(params, Box::new(h.return_type.clone())));
                            if let Ty::Fn(params, ret) = method_ty {
                                self.check_arguments(&params, args, *span);
                                ret.as_ref().clone()
                            } else {
                                Ty::Error
                            }
                        } else {
                            self.diags.push(Diagnostic::error(
                                format!("Method '{method}' not found on actor '{actor_name}'"),
                                *span,
                                self.source,
                            ));
                            Ty::Error
                        }
                    } else {
                        self.diags.push(Diagnostic::error(
                            format!("Unknown actor '{actor_name}' for method call"),
                            *span,
                            self.source,
                        ));
                        Ty::Error
                    }
                } else if let Ty::Named(n) = &obj_ty {
                    let ns = match n.as_str() {
                        "StdHttpNs" => Some("http"),
                        "StdLogNs" => Some("log"),
                        "StdJsonNs" => Some("json"),
                        "StdFsNs" => Some("fs"),
                        "StdEnvNs" => Some("env"),
                        "StdProcessNs" => Some("process"),
                        "StdCryptoNs" => Some("crypto"),
                        "StdTimeNs" => Some("time"),
                        "StdMobileNs" => Some("mobile"),
                        "StdRegexNs" => Some("regex"),
                        "StdAgentosNs" => Some("agentos"),
                        "StdCsvNs" => Some("csv"),
                        "StdTomlNs" => Some("toml"),
                        "StdYamlNs" => Some("yaml"),
                        "StdIoNs" => Some("io"),
                        _ => None,
                    };
                    if let Some(ns) = ns {
                        if let Some(method_ty) =
                            crate::builtin_registry::std_namespace_method_ty(ns, method)
                        {
                            let method_ty = self.uf.instantiate(&method_ty);
                            if let Ty::Fn(params, ret) = method_ty {
                                self.check_arguments(&params, args, *span);
                                ret.as_ref().clone()
                            } else {
                                Ty::Error
                            }
                        } else {
                            self.diags.push(Diagnostic::error(
                                format!("Method '{method}' not found on {obj_ty:?}"),
                                *span,
                                self.source,
                            ));
                            Ty::Error
                        }
                    } else {
                        self.diags.push(Diagnostic::error(
                            format!("Method '{method}' not found on {obj_ty:?}"),
                            *span,
                            self.source,
                        ));
                        Ty::Error
                    }
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Method '{method}' not found on {obj_ty:?}"),
                        *span,
                        self.source,
                    ));
                    Ty::Error
                }
            }

            HirExpr::FieldAccess(object, field, span) => {
                self.check_expr_field_access(object, field.as_str(), *span)
            }

            HirExpr::Match(subject, arms, span) => {
                let sub_ty = self.check_expr(subject, None);
                let sub_resolved = self.uf.resolve(&sub_ty);
                check_hir_match_exhaustiveness(
                    self.env,
                    self.diags,
                    &sub_resolved,
                    arms,
                    *span,
                    self.source,
                );
                let ret_ty = expected.cloned().unwrap_or_else(|| self.uf.fresh_var());
                for arm in arms {
                    self.env.push_scope();
                    self.bind_pattern(&arm.pattern, &sub_ty, false);
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.check_expr(guard, None);
                        let _ = self.uf.unify(&guard_ty, &Ty::Bool);
                    }
                    let arm_ty = self.check_expr(arm.body.as_ref(), Some(&ret_ty));
                    let _ = self.uf.unify(&ret_ty, &arm_ty);
                    self.env.pop_scope();
                }
                ret_ty
            }

            HirExpr::If(cond, then_body, else_body, _span) => {
                let cond_ty = self.check_expr(cond, None);
                let _ = self.uf.unify(&cond_ty, &Ty::Bool);
                self.env.push_scope();
                let mut then_ty = Ty::Unit;
                for stmt in then_body {
                    then_ty = self.check_stmt(stmt);
                }
                self.env.pop_scope();
                if let Some(eb) = else_body {
                    self.env.push_scope();
                    let mut else_ty = Ty::Unit;
                    for stmt in eb {
                        else_ty = self.check_stmt(stmt);
                    }
                    self.env.pop_scope();
                    let _ = self.uf.unify(&then_ty, &else_ty);
                }
                then_ty
            }

            HirExpr::Block(stmts, _span) => {
                self.env.push_scope();
                let mut last_ty = Ty::Unit;
                for stmt in stmts {
                    last_ty = self.check_stmt(stmt);
                }
                self.env.pop_scope();
                last_ty
            }

            HirExpr::For(binding, index, iterable, body, _, _span) => {
                let iter_ty = self.check_expr(iterable, None);
                let element_ty = self.extract_iterable_element(&iter_ty);
                self.env.push_scope();
                self.bind_pattern(
                    &HirPattern::Ident(binding.clone(), Span::new(0, 0)),
                    &element_ty,
                    false,
                );
                if let Some(idx_name) = index {
                    // Bind the index variable to Int — same type as `len` and
                    // range returns in this checker.
                    self.bind_pattern(
                        &HirPattern::Ident(idx_name.clone(), Span::new(0, 0)),
                        &Ty::Int,
                        false,
                    );
                }
                let _ = self.check_expr(body, None);
                self.env.pop_scope();
                Ty::Unit
            }

            HirExpr::Lambda(params, ret_ann, body, _cancellable, _span) => {
                self.env.push_scope();

                // If expected is a specific function type, distribute param types to arguments.
                let mut expected_params = None;
                let mut expected_ret_ty = None;
                if let Some(exp) = expected {
                    let res_exp = self.uf.resolve(exp);
                    if let Ty::Fn(exp_p, exp_r) = res_exp {
                        if exp_p.len() == params.len() {
                            expected_params = Some(exp_p);
                            expected_ret_ty = Some(exp_r.as_ref().clone());
                        }
                    }
                }

                let param_tys: Vec<Ty> = params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let p_ty = p
                            .type_ann
                            .as_ref()
                            .map(|t| resolve_hir_type(t, self.env))
                            .unwrap_or_else(|| {
                                expected_params
                                    .as_ref()
                                    .and_then(|ep| ep.get(i))
                                    .cloned()
                                    .unwrap_or_else(|| self.uf.fresh_var())
                            });
                        self.env.define(
                            p.name.clone(),
                            Binding::new(p_ty.clone(), false, BindingKind::Parameter),
                        );
                        p_ty
                    })
                    .collect();

                let ret_ty = ret_ann
                    .as_ref()
                    .map(|t| resolve_hir_type(t, self.env))
                    .or(expected_ret_ty.clone())
                    .unwrap_or_else(|| self.uf.fresh_var());

                self.env.push_return_type(ret_ty.clone());
                let body_ty = self.check_expr(body.as_ref(), Some(&ret_ty));
                let _ = self.uf.unify(&body_ty, &ret_ty);
                self.env.pop_return_type();
                self.env.pop_scope();
                Ty::Fn(param_tys, Box::new(ret_ty))
            }

            HirExpr::Spawn(inner, span) => {
                let inner_ty = self.check_expr(inner, None);
                let actor_name = match inner.as_ref() {
                    HirExpr::Ident(name, _) => name.clone(),
                    _ => {
                        self.diags.push(Diagnostic::error(
                            "spawn target must be a bare actor name (identifier)".into(),
                            *span,
                            self.source,
                        ));
                        return Ty::Error;
                    }
                };
                if self.uf.resolve(&inner_ty) == Ty::Error {
                    return Ty::Error;
                }
                if !self
                    .env
                    .lookup(&actor_name)
                    .is_some_and(|b| b.kind == BindingKind::Actor)
                {
                    self.diags.push(Diagnostic::error(
                        format!("'{actor_name}' is not an actor"),
                        *span,
                        self.source,
                    ));
                    return Ty::Error;
                }
                if self.env.lookup_actor(&actor_name).is_none() {
                    self.diags.push(Diagnostic::error(
                        format!(
                            "actor '{actor_name}' has no handler signatures in this module (declare `actor {actor_name}:`)"
                        ),
                        *span,
                        self.source,
                    ));
                    return Ty::Error;
                }
                Ty::ActorRef(actor_name)
            }

            HirExpr::With(resource, options, span) => {
                let res_ty = self.check_expr(resource, None);
                self.check_with_options(options.as_ref(), *span);
                let expected = Ty::Result(Box::new(self.uf.fresh_var()), Box::new(Ty::Str));
                if let Err(msg) = self.uf.unify(&res_ty, &expected) {
                    self.diags.push(Diagnostic::error(
                        format!(
                            "'with' operand must have type Result[T]; found incompatible type ({:?}): {msg}",
                            self.uf.resolve(&res_ty)
                        ),
                        *span,
                        self.source,
                    ));
                    Ty::Error
                } else {
                    self.uf.resolve(&expected)
                }
            }

            HirExpr::Jsx(el) => {
                for attr in &el.attributes {
                    let _ = self.check_expr(&attr.value, None);
                }
                for child in &el.children {
                    let _ = self.check_expr(child, None);
                }
                Ty::Element
            }
            HirExpr::JsxSelfClosing(el) => {
                for attr in &el.attributes {
                    let _ = self.check_expr(&attr.value, None);
                }
                Ty::Element
            }
            HirExpr::JsxFragment(children, _) => {
                for child in children {
                    let _ = self.check_expr(child, None);
                }
                Ty::Element
            }

            HirExpr::Index(object, index, _) => {
                let _ = self.check_expr(object.as_ref(), None);
                let _ = self.check_expr(index.as_ref(), None);
                self.uf.fresh_var()
            }

            HirExpr::AsyncView(v) => {
                let _ = self.check_expr(&v.source, None);
                if let Some(a) = &v.fetching_arm {
                    let _ = self.check_expr(a, None);
                }
                if let Some(a) = &v.empty_arm {
                    let _ = self.check_expr(a, None);
                }
                if let Some(a) = &v.error_arm {
                    let _ = self.check_expr(a, None);
                }
                if let Some(a) = &v.ok_arm {
                    let _ = self.check_expr(a, None);
                }
                Ty::Unit
            }
            HirExpr::WorkflowVersion(_) => Ty::Unit,

            HirExpr::Try(hir_try) => {
                let inner_ty = self.check_expr(hir_try.target.as_ref(), None);
                let resolved = self.uf.resolve(&inner_ty);
                let resolved = self.uf.instantiate(&resolved);
                match resolved {
                    Ty::Result(ok_ty, _err_ty) => {
                        if let Some(expected_ret) = self.env.current_return_type() {
                            let expected_ret_inst = self.uf.instantiate(expected_ret);
                            match self.uf.resolve(&expected_ret_inst) {
                                Ty::Result(_, _) => {}
                                Ty::Error => {}
                                _ => {
                                    self.diags.push(Diagnostic::error(
                                        "Cannot use `?` operator on Result in a function that does not return a Result".into(),
                                        hir_try.span,
                                        self.source,
                                    ).with_suggestion("Change the function's return type to Result[T] or handle the error using pattern matching / `unwrap()`."));
                                }
                            }
                        }
                        (*ok_ty).clone()
                    }
                    Ty::Option(some_ty) => {
                        if let Some(expected_ret) = self.env.current_return_type() {
                            let expected_ret_inst = self.uf.instantiate(expected_ret);
                            match self.uf.resolve(&expected_ret_inst) {
                                Ty::Option(_) => {}
                                Ty::Error => {}
                                _ => {
                                    self.diags.push(Diagnostic::error(
                                        "Cannot use `?` operator on Option in a function that does not return an Option".into(),
                                        hir_try.span,
                                        self.source,
                                    ).with_suggestion("Change the function's return type to Option[T] or handle the None case using pattern matching / `unwrap()`."));
                                }
                            }
                        }
                        (*some_ty).clone()
                    }
                    Ty::Error => Ty::Error,
                    other => {
                        self.diags.push(Diagnostic::error(
                            format!(
                                "`?` can only be used on Result or Option types; found incompatible type ({other:?})"
                            ),
                            hir_try.span,
                            self.source,
                        ));
                        Ty::Error
                    }
                }
            }
        };
        let resolved = self.uf.resolve(&ty);
        let hir_ty = resolved.to_hir_type();
        self.inferred_types.insert(super::hir_expr_span(expr), hir_ty);
        ty
    }
}
