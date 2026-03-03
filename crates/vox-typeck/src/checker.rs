use crate::builtins::BuiltinTypes;
use crate::env::{Binding, BindingKind, TypeEnv};
use crate::registration::{register_hir_module, resolve_hir_type};
use crate::ty::Ty;
use crate::unify::InferenceContext;
use crate::diagnostics::Diagnostic;
use std::rc::Rc;
use vox_hir::hir::*;
use vox_ast::span::Span;

pub struct Checker<'a> {
    pub env: &'a mut TypeEnv,
    pub builtins: &'a BuiltinTypes,
    pub uf: &'a mut InferenceContext,
    pub diags: &'a mut Vec<Diagnostic>,
    pub source: &'a str,
}

impl<'a> Checker<'a> {
    pub fn new(
        env: &'a mut TypeEnv,
        builtins: &'a BuiltinTypes,
        uf: &'a mut InferenceContext,
        diags: &'a mut Vec<Diagnostic>,
        source: &'a str,
    ) -> Self {
        Self { env, builtins, uf, diags, source }
    }

    // ── Module-level checking ─────────────────────────────────────

    pub fn check_module(&mut self, module: &HirModule) {
        // Pass 1: register all top-level declarations
        register_hir_module(self.env, module);

        // Pass 2: check all bodies
        for f in &module.functions { self.check_function(f); }
        for a in &module.actors { self.check_actor(a); }
        for w in &module.workflows { self.check_workflow(w); }
        for act in &module.activities { self.check_activity(act); }
        for sf in &module.server_fns { self.check_server_fn(sf); }
        for t in &module.tests { self.check_function(t); }
        for f in &module.fixtures { self.check_function(f); }
        for h in &module.hooks { self.check_hook(h); }
        for p in &module.providers { self.check_function(&p.func); }
        for q in &module.queries { self.check_function(&q.func); }
        for m in &module.mutations { self.check_function(&m.func); }
        for a in &module.actions { self.check_function(&a.func); }
        for s in &module.skills { self.check_function(&s.func); }
        for a in &module.agents { self.check_function(&a.func); }
        for s in &module.scheduled { self.check_function(&s.func); }
        for t in &module.mcp_tools { self.check_function(&t.func); }
        for r in &module.mcp_resources { self.check_function(&r.func); }
        for m in &module.mocks { self.check_function(&m.func); }
        for r in &module.routes { self.check_route(r); }
        for na in &module.native_agents { self.check_native_agent(na); }
    }

    // ── Declaration body checking ─────────────────────────────────

    fn check_function(&mut self, f: &HirFn) {
        let ret_ty = f.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        self.env.push_pure_context(f.is_pure);

        for p in &f.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }

        let mut last_ty = Ty::Unit;
        for stmt in &f.body {
            last_ty = self.check_stmt(stmt);
        }
        let _ = self.uf.unify(&last_ty, &ret_ty);

        self.env.pop_pure_context();
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_actor(&mut self, a: &HirActor) {
        for h in &a.handlers {
            self.check_actor_handler(h);
        }
    }

    fn check_actor_handler(&mut self, h: &HirActorHandler) {
        let ret_ty = h.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        for p in &h.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }
        for stmt in &h.body { self.check_stmt(stmt); }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_workflow(&mut self, w: &HirWorkflow) {
        let ret_ty = w.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        for p in &w.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }
        for stmt in &w.body { self.check_stmt(stmt); }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_activity(&mut self, a: &HirActivity) {
        let ret_ty = a.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        for p in &a.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }
        for stmt in &a.body { self.check_stmt(stmt); }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_server_fn(&mut self, sf: &HirServerFn) {
        let ret_ty = sf.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        for p in &sf.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }
        for stmt in &sf.body { self.check_stmt(stmt); }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_hook(&mut self, h: &HirHook) {
        let ret_ty = h.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        for p in &h.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }
        for stmt in &h.body { self.check_stmt(stmt); }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_route(&mut self, r: &HirRoute) {
        let ret_ty = r.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());
        for p in &r.params {
            let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
        }
        for stmt in &r.body { self.check_stmt(stmt); }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_native_agent(&mut self, a: &HirAgent) {
        for h in &a.handlers {
            let ret_ty = h.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, self.env));
            self.env.push_scope();
            self.env.push_return_type(ret_ty.clone());
            for p in &h.params {
                let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
                self.env.define(p.name.clone(), Binding::new(p_ty, false, BindingKind::Parameter));
            }
            for stmt in &h.body { self.check_stmt(stmt); }
            self.env.pop_return_type();
            self.env.pop_scope();
        }
    }

    // ── Statement checking ────────────────────────────────────────

    pub fn check_stmt(&mut self, stmt: &HirStmt) -> Ty {
        match stmt {
            HirStmt::Let { pattern, type_ann, value, mutable, .. } => {
                let val_ty = self.check_expr(value);
                let target_ty = if let Some(ann) = type_ann {
                    let ann_ty = resolve_hir_type(ann, self.env);
                    let _ = self.uf.unify(&val_ty, &ann_ty);
                    ann_ty
                } else {
                    val_ty
                };
                self.bind_pattern(pattern, &target_ty, *mutable);
                Ty::Unit
            }
            HirStmt::Assign { target, value, .. } => {
                let target_ty = self.check_expr(target);
                let value_ty = self.check_expr(value);
                let _ = self.uf.unify(&target_ty, &value_ty);
                Ty::Unit
            }
            HirStmt::Return { value, .. } => {
                let val_ty = value.as_ref().map_or(Ty::Unit, |v| self.check_expr(v));
                if let Some(expected) = self.env.current_return_type() {
                    let _ = self.uf.unify(&val_ty, expected);
                }
                Ty::Never
            }
            HirStmt::Expr { expr, .. } => self.check_expr(expr),
            HirStmt::Break { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v);
                }
                Ty::Never
            }
            HirStmt::Continue { .. } => Ty::Never,
            HirStmt::Emit { value, .. } => {
                let val_ty = self.check_expr(value);
                if let Some(expected) = self.env.current_return_type() {
                    if let Ty::Stream(inner) = expected {
                        let _ = self.uf.unify(&val_ty, inner.as_ref());
                    }
                }
                Ty::Unit
            }
            HirStmt::Use { .. } => Ty::Unit,
            HirStmt::For { binding, index_binding, iterable, body, .. } => {
                let iter_ty = self.check_expr(iterable);
                let element_ty = self.extract_iterable_element(&iter_ty);
                self.env.push_scope();
                self.bind_pattern(binding, &element_ty, false);
                if let Some(idx_name) = index_binding {
                    self.env.define(idx_name.clone(), Binding::new(Ty::Int, false, BindingKind::Variable));
                }
                for stmt in body { self.check_stmt(stmt); }
                self.env.pop_scope();
                Ty::Unit
            }
        }
    }

    // ── Expression checking ───────────────────────────────────────

    pub fn check_expr(&mut self, expr: &HirExpr) -> Ty {
        match expr {
            // Literals
            HirExpr::IntLit(_, _) => Ty::Int,
            HirExpr::FloatLit(_, _) => Ty::Float,
            HirExpr::StringLit(_, _) => Ty::Str,
            HirExpr::BoolLit(_, _) => Ty::Bool,

            // Identifiers
            HirExpr::Ident(name, span) => {
                if let Some(binding) = self.env.lookup(name) {
                    binding.ty.clone()
                } else if let Some(ty) = self.builtins.lookup_var(name) {
                    ty
                } else {
                    self.diags.push(Diagnostic::error(format!("Undefined variable: {}", name), *span, self.source));
                    Ty::Error
                }
            }

            // Collection literals
            HirExpr::ObjectLit(fields, _span) => {
                let typed_fields: Vec<(String, Ty)> = fields
                    .iter()
                    .map(|(name, expr)| (name.clone(), self.check_expr(expr)))
                    .collect();
                Ty::Record(typed_fields)
            }
            HirExpr::ListLit(elements, _span) => {
                let elem_ty = if elements.is_empty() {
                    self.uf.fresh_var()
                } else {
                    let first = self.check_expr(&elements[0]);
                    for e in &elements[1..] {
                        let t = self.check_expr(e);
                        let _ = self.uf.unify(&first, &t);
                    }
                    first
                };
                Ty::List(Rc::new(elem_ty))
            }
            HirExpr::TupleLit(elements, _span) => {
                let tys: Vec<Ty> = elements.iter().map(|e| self.check_expr(e)).collect();
                Ty::Tuple(tys)
            }
            HirExpr::MapLit(pairs, _span) => {
                let (k_ty, v_ty) = if pairs.is_empty() {
                    (self.uf.fresh_var(), self.uf.fresh_var())
                } else {
                    let k = self.check_expr(&pairs[0].0);
                    let v = self.check_expr(&pairs[0].1);
                    for (ke, ve) in &pairs[1..] {
                        let kt = self.check_expr(ke);
                        let vt = self.check_expr(ve);
                        let _ = self.uf.unify(&k, &kt);
                        let _ = self.uf.unify(&v, &vt);
                    }
                    (k, v)
                };
                Ty::Map(Rc::new(k_ty), Rc::new(v_ty))
            }
            HirExpr::SetLit(elements, _span) => {
                let elem_ty = if elements.is_empty() {
                    self.uf.fresh_var()
                } else {
                    let first = self.check_expr(&elements[0]);
                    for e in &elements[1..] {
                        let t = self.check_expr(e);
                        let _ = self.uf.unify(&first, &t);
                    }
                    first
                };
                Ty::Set(Rc::new(elem_ty))
            }

            // Operators
            HirExpr::Binary(op, left, right, span) => {
                let l_ty = self.check_expr(left);
                let r_ty = self.check_expr(right);
                self.check_binary_op(*op, l_ty, r_ty, *span)
            }
            HirExpr::Unary(op, operand, _span) => {
                let ty = self.check_expr(operand);
                self.check_unary_op(*op, ty)
            }

            // Function calls
            HirExpr::Call(callee, args, is_async, span) => {
                let raw_callee = self.check_expr(callee);
                let callee_ty = self.uf.resolve(&raw_callee);
                match callee_ty {
                    Ty::Fn(params, ret) => {
                        self.check_arguments(&params, args, *span);
                        if *is_async { Ty::Stream(ret) } else { ret.as_ref().clone() }
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diagnostic::error(format!("Not a function: {:?}", callee_ty), *span, self.source));
                        Ty::Error
                    }
                }
            }
            HirExpr::MethodCall(object, method, args, span) => {
                let obj_ty = self.check_expr(object);
                if let Some(method_ty) = self.builtins.lookup_method(&obj_ty, method) {
                    if let Ty::Fn(params, ret) = method_ty {
                        self.check_arguments(&params, args, *span);
                        ret.as_ref().clone()
                    } else { Ty::Error }
                } else {
                    self.diags.push(Diagnostic::error(format!("Method '{}' not found on {:?}", method, obj_ty), *span, self.source));
                    Ty::Error
                }
            }

            // Field/index access
            HirExpr::FieldAccess(object, field, span) => {
                let raw_obj = self.check_expr(object);
                let obj_ty = self.uf.resolve(&raw_obj);
                match &obj_ty {
                    Ty::Record(fields) | Ty::Table(_, fields) | Ty::Collection(_, fields) => {
                        fields.iter().find(|(n, _)| n == field).map(|(_, t)| t.clone()).unwrap_or_else(|| {
                            self.diags.push(Diagnostic::error(format!("Field '{}' not found on {:?}", field, obj_ty), *span, self.source));
                            Ty::Error
                        })
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diagnostic::error(format!("Cannot access field '{}' on {:?}", field, obj_ty), *span, self.source));
                        Ty::Error
                    }
                }
            }
            HirExpr::IndexAccess(collection, index, span) => {
                let raw_coll = self.check_expr(collection);
                let coll_ty = self.uf.resolve(&raw_coll);
                let idx_ty = self.check_expr(index);
                match &coll_ty {
                    Ty::List(inner) => {
                        let _ = self.uf.unify(&idx_ty, &Ty::Int);
                        inner.as_ref().clone()
                    }
                    Ty::Map(k, v) => {
                        let _ = self.uf.unify(&idx_ty, k.as_ref());
                        v.as_ref().clone()
                    }
                    Ty::Str => {
                        let _ = self.uf.unify(&idx_ty, &Ty::Int);
                        Ty::Char
                    }
                    Ty::Tuple(tys) => {
                        // Static index — just return a fresh var since we can't know index at compile time
                        if tys.is_empty() { Ty::Error } else { tys[0].clone() }
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(format!("Cannot index {:?}", coll_ty), *span, self.source));
                        Ty::Error
                    }
                }
            }
            HirExpr::Slice(collection, _start, _end, _span) => {
                let coll_ty = self.check_expr(collection);
                coll_ty // slice of list/str returns same type
            }
            HirExpr::OptionalChain(object, field, span) => {
                let raw_opt = self.check_expr(object);
                let obj_ty = self.uf.resolve(&raw_opt);
                match &obj_ty {
                    Ty::Option(inner) => {
                        let inner = inner.as_ref().clone();
                        let inner_resolved = self.uf.resolve(&inner);
                        match &inner_resolved {
                            Ty::Record(fields) | Ty::Table(_, fields) | Ty::Collection(_, fields) => {
                                let field_ty = fields.iter().find(|(n, _)| n == field).map(|(_, t)| t.clone()).unwrap_or(Ty::Error);
                                Ty::Option(Rc::new(field_ty))
                            }
                            _ => Ty::Option(Rc::new(Ty::Error))
                        }
                    }
                    Ty::Record(fields) | Ty::Table(_, fields) | Ty::Collection(_, fields) => {
                        let field_ty = fields.iter().find(|(n, _)| n == field).map(|(_, t)| t.clone()).unwrap_or(Ty::Error);
                        Ty::Option(Rc::new(field_ty))
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(format!("Cannot optional-chain on {:?}", obj_ty), *span, self.source));
                        Ty::Error
                    }
                }
            }

            // Control flow
            HirExpr::If(cond, then_body, else_body, _span) => {
                let cond_ty = self.check_expr(cond);
                let _ = self.uf.unify(&cond_ty, &Ty::Bool);
                self.env.push_scope();
                let mut then_ty = Ty::Unit;
                for stmt in then_body { then_ty = self.check_stmt(stmt); }
                self.env.pop_scope();
                if let Some(eb) = else_body {
                    self.env.push_scope();
                    let mut else_ty = Ty::Unit;
                    for stmt in eb { else_ty = self.check_stmt(stmt); }
                    self.env.pop_scope();
                    let _ = self.uf.unify(&then_ty, &else_ty);
                }
                then_ty
            }
            HirExpr::Match(subject, arms, _span) => {
                let sub_ty = self.check_expr(subject);
                let ret_ty = self.uf.fresh_var();
                for arm in arms {
                    self.env.push_scope();
                    self.bind_pattern(&arm.pattern, &sub_ty, false);
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.check_expr(guard);
                        let _ = self.uf.unify(&guard_ty, &Ty::Bool);
                    }
                    let arm_ty = self.check_expr(&arm.body);
                    let _ = self.uf.unify(&ret_ty, &arm_ty);
                    self.env.pop_scope();
                }
                ret_ty
            }
            HirExpr::Block(stmts, _span) => {
                self.env.push_scope();
                let mut last_ty = Ty::Unit;
                for stmt in stmts { last_ty = self.check_stmt(stmt); }
                self.env.pop_scope();
                last_ty
            }
            HirExpr::While { condition, body, .. } => {
                let cond_ty = self.check_expr(condition);
                let _ = self.uf.unify(&cond_ty, &Ty::Bool);
                self.env.push_scope();
                for stmt in body { self.check_stmt(stmt); }
                self.env.pop_scope();
                Ty::Unit
            }
            HirExpr::Loop { body, .. } => {
                self.env.push_scope();
                for stmt in body { self.check_stmt(stmt); }
                self.env.pop_scope();
                Ty::Unit
            }
            HirExpr::For(pattern, iterable, body, _span) => {
                let iter_ty = self.check_expr(iterable);
                let element_ty = self.extract_iterable_element(&iter_ty);
                self.env.push_scope();
                self.bind_pattern(pattern, &element_ty, false);
                self.check_expr(body);
                self.env.pop_scope();
                Ty::Unit
            }

            // Functions and closures
            HirExpr::Lambda(params, ret_ann, body, _span) => {
                self.env.push_scope();
                let param_tys: Vec<Ty> = params.iter().map(|p| {
                    let p_ty = p.type_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
                    self.env.define(p.name.clone(), Binding::new(p_ty.clone(), false, BindingKind::Parameter));
                    p_ty
                }).collect();
                let ret_ty = ret_ann.as_ref().map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
                self.env.push_return_type(ret_ty.clone());
                let body_ty = self.check_expr(body);
                let _ = self.uf.unify(&body_ty, &ret_ty);
                self.env.pop_return_type();
                self.env.pop_scope();
                Ty::Fn(param_tys, Rc::new(ret_ty))
            }
            HirExpr::Pipe(left, right, _span) => {
                let l_ty = self.check_expr(left);
                let raw_right = self.check_expr(right);
                let r_ty = self.uf.resolve(&raw_right);
                match r_ty {
                    Ty::Fn(params, ret) => {
                        if let Some(first) = params.first() {
                            let _ = self.uf.unify(&l_ty, first);
                        }
                        ret.as_ref().clone()
                    }
                    _ => l_ty // fallback: pipe identity
                }
            }

            // Concurrency
            HirExpr::Spawn(inner, _span) => {
                self.check_expr(inner);
                Ty::Unit // spawn returns unit (fire and forget)
            }
            HirExpr::Await(inner, _span) => {
                let ty = self.check_expr(inner);
                match self.uf.resolve(&ty) {
                    Ty::Stream(inner) => inner.as_ref().clone(),
                    other => other // await on non-stream is identity
                }
            }
            HirExpr::StreamBlock(stmts, _span) => {
                self.env.push_scope();
                let emitted = self.uf.fresh_var();
                for stmt in stmts {
                    if let HirStmt::Emit { value, .. } = stmt {
                        let val_ty = self.check_expr(value);
                        let _ = self.uf.unify(&emitted, &val_ty);
                    } else {
                        self.check_stmt(stmt);
                    }
                }
                self.env.pop_scope();
                Ty::Stream(Rc::new(emitted))
            }

            // Error handling
            HirExpr::TryCatch { body, catch_binding, catch_body, .. } => {
                self.env.push_scope();
                let mut body_ty = Ty::Unit;
                for stmt in body { body_ty = self.check_stmt(stmt); }
                self.env.pop_scope();

                self.env.push_scope();
                self.env.define(catch_binding.clone(), Binding::new(Ty::Error, false, BindingKind::Variable));
                let mut catch_ty = Ty::Unit;
                for stmt in catch_body { catch_ty = self.check_stmt(stmt); }
                self.env.pop_scope();

                let _ = self.uf.unify(&body_ty, &catch_ty);
                body_ty
            }
            HirExpr::TryOp(inner, _span) => {
                let raw_inner = self.check_expr(inner);
                let ty = self.uf.resolve(&raw_inner);
                match ty {
                    Ty::Result(inner) => inner.as_ref().clone(),
                    Ty::Option(inner) => inner.as_ref().clone(),
                    other => other // lenient fallback
                }
            }

            // Comprehension
            HirExpr::ListComprehension { expr, binding, iterable, condition, .. } => {
                let iter_ty = self.check_expr(iterable);
                let element_ty = self.extract_iterable_element(&iter_ty);
                self.env.push_scope();
                self.bind_pattern(binding, &element_ty, false);
                if let Some(cond) = condition {
                    let cond_ty = self.check_expr(cond);
                    let _ = self.uf.unify(&cond_ty, &Ty::Bool);
                }
                let res_ty = self.check_expr(expr);
                self.env.pop_scope();
                Ty::List(Rc::new(res_ty))
            }

            // JSX
            HirExpr::Jsx(el) => {
                for attr in &el.attributes { self.check_expr(&attr.value); }
                for child in &el.children { self.check_expr(child); }
                Ty::Element
            }
            HirExpr::JsxSelfClosing(el) => {
                for attr in &el.attributes { self.check_expr(&attr.value); }
                Ty::Element
            }

            // String interpolation
            HirExpr::StringInterp { parts, .. } => {
                for part in parts {
                    if let HirStringPart::Interpolation(expr) = part {
                        self.check_expr(expr);
                    }
                }
                Ty::Str
            }

            // Type cast
            HirExpr::TypeCast(_expr, target_type, _span) => {
                // Check the expression but return the target type
                // (cast validity checked at runtime or by a separate pass)
                resolve_hir_type(target_type, self.env)
            }

            // With (resource management)
            HirExpr::With(resource, body, _span) => {
                self.check_expr(resource);
                self.check_expr(body)
            }
        }
    }

    // ── Helper methods ────────────────────────────────────────────

    fn extract_iterable_element(&mut self, ty: &Ty) -> Ty {
        let resolved = self.uf.resolve(ty);
        match resolved {
            Ty::List(inner) => inner.as_ref().clone(),
            Ty::Set(inner) => inner.as_ref().clone(),
            Ty::Map(k, v) => Ty::Tuple(vec![k.as_ref().clone(), v.as_ref().clone()]),
            Ty::Stream(inner) => inner.as_ref().clone(),
            Ty::Str => Ty::Char,
            _ => self.uf.fresh_var()
        }
    }

    fn check_unary_op(&mut self, op: HirUnOp, ty: Ty) -> Ty {
        let ty = self.uf.resolve(&ty);
        match op {
            HirUnOp::Not => {
                let _ = self.uf.unify(&ty, &Ty::Bool);
                Ty::Bool
            }
            HirUnOp::Neg => {
                if ty == Ty::Int { Ty::Int }
                else if ty == Ty::Float { Ty::Float }
                else { Ty::Error }
            }
            HirUnOp::BitNot => {
                let _ = self.uf.unify(&ty, &Ty::Int);
                Ty::Int
            }
        }
    }

    fn check_binary_op(&mut self, op: HirBinOp, l: Ty, r: Ty, _span: Span) -> Ty {
        let l = self.uf.resolve(&l);
        let r = self.uf.resolve(&r);
        match op {
            HirBinOp::Add => {
                if l == Ty::Str || r == Ty::Str { Ty::Str }
                else if l == Ty::Int && r == Ty::Int { Ty::Int }
                else if (l == Ty::Float || l == Ty::Int) && (r == Ty::Float || r == Ty::Int) { Ty::Float }
                else { Ty::Error }
            }
            HirBinOp::Sub | HirBinOp::Mul | HirBinOp::Div | HirBinOp::Mod => {
                if l == Ty::Int && r == Ty::Int { Ty::Int }
                else if (l == Ty::Float || l == Ty::Int) && (r == Ty::Float || r == Ty::Int) { Ty::Float }
                else { Ty::Error }
            }
            HirBinOp::Lt | HirBinOp::Gt | HirBinOp::Lte | HirBinOp::Gte => Ty::Bool,
            HirBinOp::Is | HirBinOp::Isnt => Ty::Bool,
            HirBinOp::And | HirBinOp::Or => {
                let _ = self.uf.unify(&l, &Ty::Bool);
                let _ = self.uf.unify(&r, &Ty::Bool);
                Ty::Bool
            }
            HirBinOp::Pipe => {
                // Pipe is handled at HirExpr::Pipe level, this is for BinOp::Pipe
                match r {
                    Ty::Fn(_, ret) => ret.as_ref().clone(),
                    _ => l
                }
            }
            HirBinOp::Range | HirBinOp::RangeInclusive => {
                let _ = self.uf.unify(&l, &Ty::Int);
                let _ = self.uf.unify(&r, &Ty::Int);
                Ty::List(Rc::new(Ty::Int))
            }
            HirBinOp::NullCoalesce => {
                // a ?? b: if a is Option<T>, result is T
                match l {
                    Ty::Option(inner) => {
                        let _ = self.uf.unify(inner.as_ref(), &r);
                        r
                    }
                    _ => l
                }
            }
            HirBinOp::BitAnd | HirBinOp::BitOr | HirBinOp::BitXor
            | HirBinOp::Shl | HirBinOp::Shr => {
                let _ = self.uf.unify(&l, &Ty::Int);
                let _ = self.uf.unify(&r, &Ty::Int);
                Ty::Int
            }
        }
    }

    fn check_arguments(&mut self, expected: &[Ty], actual: &[HirArg], span: Span) {
        if expected.len() != actual.len() {
            self.diags.push(Diagnostic::error(
                format!("Expected {} arguments, found {}", expected.len(), actual.len()),
                span,
                self.source,
            ));
            // Still check args for interior errors
            for arg in actual {
                self.check_expr(&arg.value);
            }
            return;
        }
        for (e, a) in expected.iter().zip(actual.iter()) {
            let a_ty = self.check_expr(&a.value);
            if let Err(msg) = self.uf.unify(e, &a_ty) {
                self.diags.push(Diagnostic {
                    severity: crate::diagnostics::Severity::Error,
                    message: format!("Argument type mismatch: {msg}"),
                    span: a.value.span(),
                    expected_type: Some(format!("{e:?}")),
                    found_type: Some(format!("{a_ty:?}")),
                    context: Some(Diagnostic::capture_context(self.source, a.value.span())),
                    suggestions: vec![],
                });
            }
        }
    }

    fn bind_pattern(&mut self, pattern: &HirPattern, ty: &Ty, mutable: bool) {
        let ty = self.uf.resolve(ty);
        match pattern {
            HirPattern::Ident(name, _) => {
                self.env.define(name.clone(), Binding::new(ty.clone(), mutable, BindingKind::Variable));
            }
            HirPattern::Tuple(patterns, span) => {
                if let Ty::Tuple(tys) = &ty {
                    if tys.len() != patterns.len() {
                        self.diags.push(Diagnostic::error(
                            format!("Tuple size mismatch: expected {}, found {}", tys.len(), patterns.len()),
                            *span, self.source,
                        ));
                    }
                    for (p, t) in patterns.iter().zip(tys.iter()) {
                        self.bind_pattern(p, t, mutable);
                    }
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Cannot destructure non-tuple type {:?}", ty), *span, self.source,
                    ));
                }
            }
            HirPattern::Wildcard(_) => {}
            HirPattern::Constructor(name, fields, _span) => {
                match &ty {
                    Ty::Option(inner) if name == "Some" && fields.len() == 1 => {
                        self.bind_pattern(&fields[0], inner.as_ref(), mutable);
                    }
                    Ty::Result(inner) if name == "Ok" && fields.len() == 1 => {
                        self.bind_pattern(&fields[0], inner.as_ref(), mutable);
                    }
                    Ty::Result(_) if name == "Err" && fields.len() == 1 => {
                        self.bind_pattern(&fields[0], &Ty::Error, mutable);
                    }
                    Ty::Option(_) if name == "None" => {}
                    _ => {
                        // ADT constructors — look up in env
                        if let Some(adt_ty) = self.env.lookup_adt_variant(name) {
                            for (i, p) in fields.iter().enumerate() {
                                let field_ty = adt_ty.get(i).map(|(_, t)| t.clone()).unwrap_or(Ty::Error);
                                self.bind_pattern(p, &field_ty, mutable);
                            }
                        }
                    }
                }
            }
            HirPattern::Literal(_, _) => {} // constrains type, no binding
            HirPattern::Or(branches, _span) => {
                // Each branch must bind the same names
                for branch in branches {
                    self.bind_pattern(branch, &ty, mutable);
                }
            }
            HirPattern::Binding(name, inner, _span) => {
                self.env.define(name.clone(), Binding::new(ty.clone(), mutable, BindingKind::Variable));
                self.bind_pattern(inner, &ty, mutable);
            }
            HirPattern::Rest(maybe_name, _span) => {
                if let Some(name) = maybe_name {
                    // Bind the rest as a list of the element type
                    self.env.define(name.clone(), Binding::new(
                        Ty::List(Rc::new(ty.clone())),
                        mutable,
                        BindingKind::Variable,
                    ));
                }
            }
            HirPattern::Record(fields, _span) => {
                if let Ty::Record(ty_fields) = &ty {
                    for (field_name, sub_pattern) in fields {
                        let field_ty = ty_fields.iter()
                            .find(|(n, _)| n == field_name)
                            .map(|(_, t)| t.clone())
                            .unwrap_or(Ty::Error);
                        if let Some(p) = sub_pattern {
                            self.bind_pattern(p, &field_ty, mutable);
                        } else {
                            // Shorthand: `{name}` binds `name` to the field type
                            self.env.define(field_name.clone(), Binding::new(field_ty, mutable, BindingKind::Variable));
                        }
                    }
                }
            }
        }
    }
}

// ── Public API ────────────────────────────────────────────────────

pub fn typecheck_hir(
    module: &HirModule,
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    source: &str,
) -> Vec<Diagnostic> {
    let mut uf = InferenceContext::new();
    let mut diags = Vec::new();
    let mut checker = Checker::new(env, builtins, &mut uf, &mut diags, source);
    checker.check_module(module);
    diags
}
