//! HIR type Checker aligned with `crate::hir` HIR nodes (lowered AST).

use crate::ast::span::Span;
use crate::hir::*;
use crate::typeck::builtins::BuiltinTypes;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use crate::typeck::env::{Binding, BindingKind, TypeEnv};
use crate::typeck::registration::{register_hir_module, resolve_hir_type};
use crate::typeck::ty::Ty;
use crate::typeck::unify::InferenceContext;

pub struct Checker<'a> {
    pub env: &'a mut TypeEnv,
    pub builtins: &'a BuiltinTypes,
    pub uf: &'a mut InferenceContext,
    pub diags: &'a mut Vec<Diagnostic>,
    pub inferred_types: &'a mut std::collections::HashMap<Span, HirType>,
    pub source: &'a str,
}

pub(crate) fn hir_expr_span(expr: &HirExpr) -> Span {
    match expr {
        HirExpr::IntLit(_, s)
        | HirExpr::FloatLit(_, s)
        | HirExpr::StringLit(_, s)
        | HirExpr::BoolLit(_, s)
        | HirExpr::Ident(_, s)
        | HirExpr::ObjectLit(_, s)
        | HirExpr::ListLit(_, s)
        | HirExpr::TupleLit(_, s)
        | HirExpr::Binary(_, _, _, s)
        | HirExpr::Unary(_, _, s)
        | HirExpr::Call(_, _, _, s)
        | HirExpr::MethodCall(_, _, _, _, s)
        | HirExpr::FieldAccess(_, _, s)
        | HirExpr::Match(_, _, s)
        | HirExpr::If(_, _, _, s)
        | HirExpr::For(_, _, _, _, _, s)
        | HirExpr::Lambda(_, _, _, _, s)
        | HirExpr::Spawn(_, s)
        | HirExpr::With(_, _, s)
        | HirExpr::Block(_, s)
        | HirExpr::Index(_, _, s) => *s,
        HirExpr::JsxSelfClosing(el) => el.span,
        HirExpr::Jsx(el) => el.span,
        HirExpr::JsxFragment(_, s) => *s,
        HirExpr::Try(t) => t.span,
        HirExpr::DecimalLit(_, s) => *s,
        HirExpr::AsyncView(v) => v.span,
        HirExpr::WorkflowVersion(v) => v.span,
    }
}

mod expr;
mod expr_field;
mod expr_ops;
mod match_exhaust;

impl<'a> Checker<'a> {
    pub fn new(
        env: &'a mut TypeEnv,
        builtins: &'a BuiltinTypes,
        uf: &'a mut InferenceContext,
        diags: &'a mut Vec<Diagnostic>,
        inferred_types: &'a mut std::collections::HashMap<Span, HirType>,
        source: &'a str,
    ) -> Self {
        Self {
            env,
            builtins,
            uf,
            diags,
            inferred_types,
            source,
        }
    }

    pub fn check_module(&mut self, module: &mut HirModule) {
        self.diags
            .extend(register_hir_module(self.env, module, Some(self.uf)));

        for f in &mut module.functions {
            self.check_function(f);
        }
        for sf in &mut module.endpoint_fns {
            self.check_endpoint_fn(sf);
            if sf.kind == crate::hir::HirEndpointKind::Query {
                self.enforce_query_read_only(sf);
            }
        }
        for t in &mut module.tests {
            self.check_function(t);
        }
        for t in &mut module.mcp_tools {
            self.check_function(&mut t.func);
        }
        for r in &mut module.mcp_resources {
            self.check_function(&mut r.func);
        }

        for c in &module.components {
            self.check_reactive_component(c);
        }

        for a in &module.agents {
            self.check_agent(a);
        }
        for e in &module.environments {
            self.check_environment(e);
        }
    }

    fn check_function(&mut self, f: &mut HirFn) {
        // Extern (TS-source FFI) functions have no Vox body to check — the body
        // lives in a sibling .ts file. Skip body unification but still consume
        // params/return type so call sites resolve correctly.
        if f.ts_extern_module.is_some() {
            return;
        }
        let was_inferred = f.return_type.is_none();
        let mut ret_ty = f
            .return_type
            .as_ref()
            .map_or(Ty::Infer, |t| resolve_hir_type(t, self.env));
        if matches!(ret_ty, Ty::Infer) {
            ret_ty = self.uf.fresh_var();
        }
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());

        self.env.define(
            "db".into(),
            Binding::new(Ty::Database, false, BindingKind::Variable),
        );

        for p in &f.params {
            let p_ty = p
                .type_ann
                .as_ref()
                .map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(
                p.name.clone(),
                Binding::new(p_ty, false, BindingKind::Parameter),
            );
        }

        let mut last_ty = Ty::Unit;
        for stmt in &f.body {
            last_ty = self.check_stmt(stmt);
        }
        let _ = self.uf.unify(&last_ty, &ret_ty);

        if was_inferred {
            let resolved = self.uf.resolve(&ret_ty);
            if !matches!(resolved, Ty::TypeVar(_)) {
                f.return_type = Some(resolved.to_hir_type());
            }
        }

        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_endpoint_fn(&mut self, sf: &mut HirEndpointFn) {
        let was_inferred = sf.return_type.is_none();
        let mut ret_ty = sf
            .return_type
            .as_ref()
            .map_or(Ty::Infer, |t| resolve_hir_type(t, self.env));
        if matches!(ret_ty, Ty::Infer) {
            ret_ty = self.uf.fresh_var();
        }
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());

        self.env.define(
            "db".into(),
            Binding::new(Ty::Database, false, BindingKind::Variable),
        );

        for p in &sf.params {
            let p_ty = p
                .type_ann
                .as_ref()
                .map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(
                p.name.clone(),
                Binding::new(p_ty, false, BindingKind::Parameter),
            );
        }

        let mut last_ty = Ty::Unit;
        for stmt in &sf.body {
            last_ty = self.check_stmt(stmt);
        }
        let _ = self.uf.unify(&last_ty, &ret_ty);

        if was_inferred {
            let resolved = self.uf.resolve(&ret_ty);
            if !matches!(resolved, Ty::TypeVar(_)) {
                sf.return_type = Some(resolved.to_hir_type());
            }
        }

        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_agent(&mut self, a: &HirAgent) {
        for h in &a.handlers {
            self.check_agent_handler(h);
        }
        for m in &a.migrations {
            self.check_migration_rule(m);
        }
    }

    fn check_agent_handler(&mut self, h: &HirAgentHandler) {
        self.check_db_scoped_handler(&h.return_type, &h.params, &h.body);
    }

    /// Shared body for actor/agent handlers: `db` binding plus param scope and statement typecheck.
    fn check_db_scoped_handler(
        &mut self,
        return_type: &Option<HirType>,
        params: &[HirParam],
        body: &[HirStmt],
    ) {
        let mut ret_ty = return_type
            .as_ref()
            .map_or(Ty::Infer, |t| resolve_hir_type(t, self.env));
        if matches!(ret_ty, Ty::Infer) {
            ret_ty = self.uf.fresh_var();
        }
        self.env.push_scope();
        self.env.push_return_type(ret_ty.clone());

        self.env.define(
            "db".into(),
            Binding::new(Ty::Database, false, BindingKind::Variable),
        );

        for p in params {
            let p_ty = p
                .type_ann
                .as_ref()
                .map_or(self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(
                p.name.clone(),
                Binding::new(p_ty, false, BindingKind::Parameter),
            );
        }
        for stmt in body {
            let _ = self.check_stmt(stmt);
        }
        self.env.pop_return_type();
        self.env.pop_scope();
    }

    fn check_migration_rule(&mut self, m: &HirMigrationRule) {
        self.env.push_scope();
        self.env.define(
            "db".into(),
            Binding::new(Ty::Database, false, BindingKind::Variable),
        );
        for stmt in &m.body {
            let _ = self.check_stmt(stmt);
        }
        self.env.pop_scope();
    }

    fn check_environment(&mut self, e: &HirEnvironment) {
        if e.name.is_empty() {
            self.diags.push(Diagnostic::error(
                "Environment name cannot be empty".into(),
                e.span,
                self.source,
            ));
        }
    }

    /// Typecheck a reactive component (Path C).
    ///
    /// Sets up a component-local scope so member expressions resolve names
    /// correctly:
    /// - `db` is bound (components may issue queries)
    /// - parameters are bound from `c.params`
    /// - every `state`/`derived` name is forward-declared with a fresh
    ///   inference variable (or its declared type if annotated) so members
    ///   can reference each other regardless of source order
    ///
    /// Then each member is checked: `state.init` and `derived.expr` are
    /// unified back into the forward-declared binding when annotated;
    /// effects/lifecycle bodies and the view go through `check_expr`;
    /// `Stmt` prelude members go through `check_stmt`.
    fn check_reactive_component(&mut self, c: &crate::hir::HirReactiveComponent) {
        if c.name.is_empty() {
            self.diags.push(Diagnostic::error(
                "Reactive component name cannot be empty".into(),
                c.span,
                self.source,
            ));
        }

        self.env.push_scope();

        self.env.define(
            "db".into(),
            Binding::new(Ty::Database, false, BindingKind::Variable),
        );

        for p in &c.params {
            let p_ty = p
                .type_ann
                .as_ref()
                .map_or_else(|| self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
            self.env.define(
                p.name.clone(),
                Binding::new(p_ty, false, BindingKind::Parameter),
            );
        }

        // Forward-declare state and derived bindings so members may reference
        // one another in any order (e.g. a `derived` reading a later `state`).
        // Track each binding's type variable so we can unify the inferred
        // init/expr type back into it during the check pass.
        let mut state_vars: Vec<(String, Ty)> = Vec::new();
        let mut derived_vars: Vec<(String, Ty)> = Vec::new();
        for m in &c.members {
            match m {
                crate::hir::HirReactiveMember::State(s) => {
                    let ty =
                        s.ty.as_ref()
                            .map_or_else(|| self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
                    // State is mutable: reactive components reassign it from
                    // event handlers (e.g. `on:click={count = count + 1}`).
                    // Derived stays immutable — it's a computed read.
                    self.env.define(
                        s.name.clone(),
                        Binding::new(ty.clone(), true, BindingKind::Variable),
                    );
                    state_vars.push((s.name.clone(), ty));
                }
                crate::hir::HirReactiveMember::Derived(d) => {
                    let ty =
                        d.ty.as_ref()
                            .map_or_else(|| self.uf.fresh_var(), |t| resolve_hir_type(t, self.env));
                    self.env.define(
                        d.name.clone(),
                        Binding::new(ty.clone(), false, BindingKind::Variable),
                    );
                    derived_vars.push((d.name.clone(), ty));
                }
                _ => {}
            }
        }

        let mut state_idx = 0usize;
        let mut derived_idx = 0usize;
        for m in &c.members {
            match m {
                crate::hir::HirReactiveMember::State(s) => {
                    let init_ty = self.check_expr(&s.init, None);
                    if let Some((_, decl_ty)) = state_vars.get(state_idx) {
                        // `any` is an escape hatch — skip unification and accept
                        // any initializer value (mirrors TypeScript `any` semantics).
                        let resolved_decl = self.uf.resolve(decl_ty);
                        let is_any = matches!(&resolved_decl, Ty::Named(n) if n == "any");
                        if !is_any {
                            if let Err(msg) = self.uf.unify(&init_ty, decl_ty) {
                                self.diags.push(
                                    Diagnostic::error(
                                        format!(
                                            "Type mismatch in `state {}` initializer: {msg}",
                                            s.name
                                        ),
                                        s.span,
                                        self.source,
                                    )
                                    .with_code(
                                        crate::typeck::diagnostics::codes::TYPES_TYPE_MISMATCH,
                                    ),
                                );
                            }
                        }
                    }
                    state_idx += 1;
                }
                crate::hir::HirReactiveMember::Derived(d) => {
                    let expr_ty = self.check_expr(&d.expr, None);
                    if let Some((_, decl_ty)) = derived_vars.get(derived_idx) {
                        if let Err(msg) = self.uf.unify(&expr_ty, decl_ty) {
                            self.diags.push(
                                Diagnostic::error(
                                    format!(
                                        "Type mismatch in `derived {}` expression: {msg}",
                                        d.name
                                    ),
                                    d.span,
                                    self.source,
                                )
                                .with_code(crate::typeck::diagnostics::codes::TYPES_TYPE_MISMATCH),
                            );
                        }
                    }
                    derived_idx += 1;
                }
                crate::hir::HirReactiveMember::Effect(e) => {
                    let _ = self.check_expr(&e.body, None);
                }
                crate::hir::HirReactiveMember::OnMount(om) => {
                    let _ = self.check_expr(&om.body, None);
                }
                crate::hir::HirReactiveMember::OnCleanup(oc) => {
                    let _ = self.check_expr(&oc.body, None);
                }
                crate::hir::HirReactiveMember::Stmt(stmt) => {
                    let _ = self.check_stmt(stmt);
                }
            }
        }

        if let Some(view) = &c.view {
            let _ = self.check_expr(view, None);
        }

        self.env.pop_scope();
    }

    fn enforce_query_read_only(&mut self, sf: &HirEndpointFn) {
        if Self::contains_db_write_or_unsafe_in_stmts(&sf.body) {
            self.diags.push(Diagnostic {
                severity: TypeckSeverity::Error,
                message: format!(
                    "@query '{}' must be read-only; use @mutation for db.insert/db.delete or raw `.query(...)`",
                    sf.name
                ),
                span: sf.span,
                expected_type: None,
                found_type: None,
                context: Some(Diagnostic::capture_context(self.source, sf.span)),
                suggestions: vec![
                    "Move write operations into an @mutation function.".into(),
                    "Replace `.query(clause)` with typed table operations.".into(),
                ],
                category: DiagnosticCategory::Lint,
                code: Some("lint.query_not_readonly".into()),
                fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
});
        }
    }

    fn contains_db_write_or_unsafe_in_stmts(stmts: &[HirStmt]) -> bool {
        stmts.iter().any(Self::contains_db_write_or_unsafe_in_stmt)
    }

    fn contains_db_write_or_unsafe_in_stmt(stmt: &HirStmt) -> bool {
        match stmt {
            HirStmt::Let { value, .. } => Self::contains_db_write_or_unsafe_in_expr(value),
            HirStmt::Assign { target, value, .. } => {
                Self::contains_db_write_or_unsafe_in_expr(target)
                    || Self::contains_db_write_or_unsafe_in_expr(value)
            }
            HirStmt::Return { value, .. } => value
                .as_ref()
                .is_some_and(Self::contains_db_write_or_unsafe_in_expr),
            HirStmt::While {
                condition, body, ..
            } => {
                Self::contains_db_write_or_unsafe_in_expr(condition)
                    || Self::contains_db_write_or_unsafe_in_stmts(body)
            }
            HirStmt::Loop { body, .. } => Self::contains_db_write_or_unsafe_in_stmts(body),
            HirStmt::Break { .. } | HirStmt::Continue { .. } => false,
            HirStmt::Expr { expr, .. } => Self::contains_db_write_or_unsafe_in_expr(expr),
        }
    }

    fn contains_db_write_or_unsafe_in_expr(expr: &HirExpr) -> bool {
        match expr {
            HirExpr::ObjectLit(fields, _) => fields
                .iter()
                .any(|(_, v)| Self::contains_db_write_or_unsafe_in_expr(v)),
            HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
                items.iter().any(Self::contains_db_write_or_unsafe_in_expr)
            }
            HirExpr::Binary(_, l, r, _) => {
                Self::contains_db_write_or_unsafe_in_expr(l)
                    || Self::contains_db_write_or_unsafe_in_expr(r)
            }
            HirExpr::Unary(_, e, _) => Self::contains_db_write_or_unsafe_in_expr(e),
            HirExpr::Call(callee, args, _, _) | HirExpr::MethodCall(callee, _, args, None, _) => {
                Self::contains_db_write_or_unsafe_in_expr(callee)
                    || args
                        .iter()
                        .any(|a| Self::contains_db_write_or_unsafe_in_expr(&a.value))
            }
            HirExpr::MethodCall(callee, _, args, Some(plan), _) => {
                matches!(
                    plan.op,
                    HirDbTableOp::Insert
                        | HirDbTableOp::Delete
                        | HirDbTableOp::UnsafeQueryRawClause
                ) || Self::contains_db_write_or_unsafe_in_expr(callee)
                    || args
                        .iter()
                        .any(|a| Self::contains_db_write_or_unsafe_in_expr(&a.value))
            }
            HirExpr::FieldAccess(obj, _, _) => Self::contains_db_write_or_unsafe_in_expr(obj),
            HirExpr::If(cond, then_body, else_body, _) => {
                Self::contains_db_write_or_unsafe_in_expr(cond)
                    || Self::contains_db_write_or_unsafe_in_stmts(then_body)
                    || else_body
                        .as_ref()
                        .is_some_and(|body| Self::contains_db_write_or_unsafe_in_stmts(body))
            }
            HirExpr::For(_, _, iter, body, _, _) => {
                Self::contains_db_write_or_unsafe_in_expr(iter)
                    || Self::contains_db_write_or_unsafe_in_expr(body)
            }
            HirExpr::Lambda(_, _, body, _, _) => Self::contains_db_write_or_unsafe_in_expr(body),
            HirExpr::Block(body, _) => Self::contains_db_write_or_unsafe_in_stmts(body),

            HirExpr::Spawn(e, _) => Self::contains_db_write_or_unsafe_in_expr(e),
            HirExpr::With(base, opts, _) => {
                Self::contains_db_write_or_unsafe_in_expr(base)
                    || Self::contains_db_write_or_unsafe_in_expr(opts)
            }
            HirExpr::Match(scrutinee, arms, _) => {
                Self::contains_db_write_or_unsafe_in_expr(scrutinee)
                    || arms
                        .iter()
                        .any(|a| Self::contains_db_write_or_unsafe_in_expr(&a.body))
            }
            HirExpr::Jsx(node) => {
                node.attributes
                    .iter()
                    .any(|a| Self::contains_db_write_or_unsafe_in_expr(&a.value))
                    || node
                        .children
                        .iter()
                        .any(Self::contains_db_write_or_unsafe_in_expr)
            }
            HirExpr::JsxSelfClosing(node) => node
                .attributes
                .iter()
                .any(|a| Self::contains_db_write_or_unsafe_in_expr(&a.value)),
            HirExpr::JsxFragment(children, _) => children
                .iter()
                .any(Self::contains_db_write_or_unsafe_in_expr),
            HirExpr::Try(t) => Self::contains_db_write_or_unsafe_in_expr(t.target.as_ref()),
            HirExpr::Index(obj, idx, _) => {
                Self::contains_db_write_or_unsafe_in_expr(obj)
                    || Self::contains_db_write_or_unsafe_in_expr(idx)
            }
            HirExpr::AsyncView(v) => [&v.fetching_arm, &v.empty_arm, &v.error_arm, &v.ok_arm]
                .iter()
                .filter_map(|a| a.as_deref())
                .any(Self::contains_db_write_or_unsafe_in_expr),
            HirExpr::IntLit(_, _)
            | HirExpr::FloatLit(_, _)
            | HirExpr::BoolLit(_, _)
            | HirExpr::StringLit(_, _)
            | HirExpr::Ident(_, _)
            | HirExpr::DecimalLit(_, _)
            | HirExpr::WorkflowVersion(_) => false,
        }
    }

    pub fn check_stmt(&mut self, stmt: &HirStmt) -> Ty {
        match stmt {
            HirStmt::Let {
                pattern,
                type_ann,
                value,
                mutable,
                ..
            } => {
                let val_ty = self.check_expr(value, None);
                let target_ty = if let Some(ann) = type_ann {
                    let ann_ty = resolve_hir_type(ann, self.env);
                    if let Err(msg) = self.uf.unify(&val_ty, &ann_ty) {
                        self.diags.push(
                            Diagnostic::error(
                                format!("Type mismatch in `let`: {msg}"),
                                hir_expr_span(value),
                                self.source,
                            )
                            .with_code(crate::typeck::diagnostics::codes::TYPES_TYPE_MISMATCH),
                        );
                    }
                    ann_ty
                } else {
                    val_ty
                };
                self.bind_pattern(pattern, &target_ty, *mutable);
                Ty::Unit
            }
            HirStmt::Assign { target, value, .. } => {
                if let HirExpr::Ident(name, name_span) = target {
                    if let Some(binding) = self.env.lookup(name) {
                        if !binding.mutable {
                            self.diags.push(Diagnostic::error(
                                format!("Cannot assign to immutable variable '{name}'"),
                                *name_span,
                                self.source,
                            ));
                        }
                    }
                }
                let target_ty = self.check_expr(target, None);
                let value_ty = self.check_expr(value, None);
                let _ = self.uf.unify(&target_ty, &value_ty);
                Ty::Unit
            }
            HirStmt::Return { value, span } => {
                // Pass the function's declared return type as an expected hint so that
                // anonymous record literals (`return { x: 0, y: 0 }`) can be typed against
                // the declared struct shape and resolve to `Ty::Named(StructName)` rather
                // than the looser `Ty::Record`, which won't unify with the declared type.
                let expected_ret = self.env.current_return_type().cloned();
                let val_ty = value
                    .as_ref()
                    .map_or(Ty::Unit, |v| self.check_expr(v, expected_ret.as_ref()));
                if let Some(expected) = self.env.current_return_type() {
                    if let Err(msg) = self.uf.unify(&val_ty, expected) {
                        self.diags.push(
                            Diagnostic::error(
                                format!("Return type mismatch: {msg}"),
                                *span,
                                self.source,
                            )
                            .with_code(crate::typeck::diagnostics::codes::TYPES_TYPE_MISMATCH),
                        );
                    }
                }
                Ty::Never
            }
            HirStmt::While {
                condition, body, ..
            } => {
                let cond_ty = self.check_expr(condition, None);
                let _ = self.uf.unify(&cond_ty, &Ty::Bool);
                for stmt in body {
                    self.check_stmt(stmt);
                }
                Ty::Unit
            }
            HirStmt::Loop { body, .. } => {
                for stmt in body {
                    self.check_stmt(stmt);
                }
                Ty::Never
            }
            HirStmt::Break { .. } | HirStmt::Continue { .. } => Ty::Never,
            HirStmt::Expr { expr, .. } => self.check_expr(expr, None),
        }
    }

    pub(crate) fn solve_constraints(&mut self) {
        let mut queue = std::mem::take(&mut self.uf.pending_constraints);
        let mut progress = true;
        while progress && !queue.is_empty() {
            progress = false;
            let mut next_queue = Vec::new();

            for constraint in queue {
                match constraint {
                    crate::typeck::unify::PendingConstraint::HasField {
                        target,
                        field,
                        result,
                        span,
                    } => {
                        let obj_ty = self.uf.resolve(&target);
                        match &obj_ty {
                            Ty::TypeVar(_) => {
                                next_queue.push(
                                    crate::typeck::unify::PendingConstraint::HasField {
                                        target: obj_ty,
                                        field,
                                        result,
                                        span,
                                    },
                                );
                            }
                            Ty::Record(fields)
                            | Ty::Table(_, fields)
                            | Ty::Collection(_, fields) => {
                                if let Some((_, f_ty)) = fields.iter().find(|(n, _)| n == &field) {
                                    let _ = self.uf.unify(&result, f_ty);
                                    progress = true;
                                } else {
                                    self.diags.push(
                                        Diagnostic::error(
                                            format!("Field '{field}' not found on {obj_ty:?}"),
                                            span,
                                            self.source,
                                        )
                                        .with_code(
                                            crate::typeck::diagnostics::codes::TYPES_FIELD_NOT_FOUND,
                                        ),
                                    );
                                }
                            }
                            Ty::Error | Ty::Never => {
                                progress = true;
                            } // suppress cascades
                            other => {
                                self.diags.push(
                                    Diagnostic::error(
                                        format!("Cannot access field '{field}' on {other:?}"),
                                        span,
                                        self.source,
                                    )
                                    .with_code(
                                        crate::typeck::diagnostics::codes::TYPES_FIELD_NOT_FOUND,
                                    ),
                                );
                            }
                        }
                    }
                    crate::typeck::unify::PendingConstraint::HasMethod {
                        target,
                        method,
                        result,
                        span,
                        ..
                    } => {
                        let obj_ty = self.uf.resolve(&target);
                        match &obj_ty {
                            Ty::TypeVar(_) => {
                                next_queue.push(
                                    crate::typeck::unify::PendingConstraint::HasMethod {
                                        target: obj_ty,
                                        method,
                                        result,
                                        args: vec![],
                                        span,
                                    },
                                );
                            }
                            Ty::Error | Ty::Never => {
                                progress = true;
                            }
                            other => {
                                if let Some(method_ty) = self.builtins.lookup_method(other, &method)
                                {
                                    let method_instantiated = self.uf.instantiate(&method_ty);
                                    if let Ty::Fn(_params, ret) = &method_instantiated {
                                        let _ = self.uf.unify(&result, ret.as_ref());
                                    }
                                    progress = true;
                                } else {
                                    // Use the human-readable type renderer
                                    // instead of the `{other:?}` Debug repr so
                                    // error messages don't leak internal
                                    // TypeVar IDs to script authors. See
                                    // closures-rfc-2026-05-23.md §11 Q4.
                                    let pretty = pretty_print_unresolved_type(other);
                                    self.diags.push(
                                        Diagnostic::error(
                                            format!(
                                                "Method `{method}` not found on {pretty}.\n\
                                                 If `{pretty}` looks unexpected, the receiver may need an \
                                                 explicit type annotation upstream."
                                            ),
                                            span,
                                            self.source,
                                        )
                                        .with_code(
                                            crate::typeck::diagnostics::codes::TYPES_METHOD_NOT_FOUND,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            queue = next_queue;
        }

        for constraint in queue {
            let (span, msg, code) = match &constraint {
                crate::typeck::unify::PendingConstraint::HasField {
                    span,
                    target,
                    field,
                    ..
                } => {
                    let target_str = pretty_print_unresolved_type(target);
                    (
                        *span,
                        format!(
                            "Cannot infer the type of this value to access field `{field}`.\n\
                         Inferred so far: {target_str}.\n\
                         Add an explicit type annotation to the variable that produced this value \
                         (e.g. `let x: <Type> = ...`)."
                        ),
                        crate::typeck::diagnostics::codes::TYPES_UNRESOLVED_TYPE,
                    )
                }
                crate::typeck::unify::PendingConstraint::HasMethod {
                    span,
                    target,
                    method,
                    ..
                } => {
                    // Per closures-rfc-2026-05-23.md §11 Q4: when a method call
                    // can't be resolved because the receiver type is still a
                    // TypeVar, the diagnostic MUST name the method, show
                    // what's been inferred about the receiver, and suggest
                    // an annotation. The prior message dumped the raw
                    // `{constraint:?}` Debug repr, which was unactionable.
                    let target_str = pretty_print_unresolved_type(target);
                    // Heuristic: if this looks like a closure param's type
                    // (TypeVar with no constraints), tilt the suggestion
                    // toward the closure-annotation form RFC §4 specifies.
                    let suggestion = if matches!(target, Ty::TypeVar(_)) {
                        format!(
                            "Add an explicit type to the binding that produced this value. \
                             For closures, that's `fn(x: <Type>) {{ ... }}` per the closures RFC §11."
                        )
                    } else {
                        format!(
                            "Add an explicit type annotation upstream so the receiver's type can be resolved."
                        )
                    };
                    (
                        *span,
                        format!(
                            "Cannot infer the type of this value to call `.{method}()`.\n\
                         Inferred so far: {target_str}.\n\
                         {suggestion}"
                        ),
                        crate::typeck::diagnostics::codes::TYPES_UNRESOLVED_TYPE,
                    )
                }
            };
            self.diags
                .push(Diagnostic::error(msg, span, self.source).with_code(code));
        }
    }
}

/// Render an unresolved typeck type for error messages. Plain `Ty::TypeVar(N)`
/// shows as `<unknown>` so users don't see the internal-id-of-the-day; named
/// types pretty-print themselves; everything else falls back to Debug.
///
/// Exposed crate-wide so per-construct diagnostics in `checker/expr.rs`
/// and `checker/expr_field.rs` can use it for consistent formatting.
pub(crate) fn pretty_print_unresolved_type(ty: &Ty) -> String {
    match ty {
        Ty::TypeVar(_) => "<unknown>".to_string(),
        Ty::Named(n) => n.clone(),
        Ty::Int => "Int".to_string(),
        Ty::Float => "Float".to_string(),
        Ty::Str => "Str".to_string(),
        Ty::Bool => "Bool".to_string(),
        Ty::Unit => "Unit".to_string(),
        Ty::Never => "Never".to_string(),
        Ty::List(inner) => format!("List[{}]", pretty_print_unresolved_type(inner)),
        Ty::Option(inner) => format!("Option[{}]", pretty_print_unresolved_type(inner)),
        Ty::Result(ok, _err) => format!("Result[{}]", pretty_print_unresolved_type(ok)),
        Ty::Fn(_, _) => "fn(...)".to_string(),
        _ => format!("{ty:?}"),
    }
}

pub fn typecheck_hir(
    module: &mut HirModule,
    env: &mut TypeEnv,
    builtins: &BuiltinTypes,
    source: &str,
) -> Vec<Diagnostic> {
    let mut uf = InferenceContext::new();
    let mut diags = Vec::new();
    let mut inferred_types = std::mem::take(&mut module.inferred_types);
    {
        let mut checker = Checker::new(
            env,
            builtins,
            &mut uf,
            &mut diags,
            &mut inferred_types,
            source,
        );
        checker.check_module(module);
        checker.solve_constraints();
    }
    module.inferred_types = inferred_types;
    diags
}
