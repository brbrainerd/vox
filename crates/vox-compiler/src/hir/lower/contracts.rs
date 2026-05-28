use super::LowerCtx;
use crate::ast::decl::fundecl::{FnDecl, VerifyMode};
use crate::hir::{HirArg, HirExpr, HirMatchArm, HirStmt, HirUnOp};

impl LowerCtx {
    /// Lowers contracts into the function body based on VerifyMode.
    pub(crate) fn inject_contracts(&mut self, f: &FnDecl, body: Vec<HirStmt>) -> Vec<HirStmt> {
        match f.verify_mode {
            VerifyMode::Off => body, // Strip all contracts
            VerifyMode::RequireOnly => {
                let mut new_body = Vec::new();
                for pre in &f.preconditions {
                    let expr = self.lower_expr(pre);
                    new_body.push(HirStmt::Expr {
                        expr,
                        span: crate::ast::span::Span::new(0, 0),
                    });
                }
                new_body.extend(body);
                new_body
            }
            VerifyMode::Full => {
                let pre_stmts: Vec<HirStmt> = f
                    .preconditions
                    .iter()
                    .map(|p| {
                        let expr = self.lower_expr(p);
                        HirStmt::Expr {
                            expr,
                            span: crate::ast::span::Span::new(0, 0),
                        }
                    })
                    .collect();

                let post_stmts: Vec<HirStmt> = f
                    .postconditions
                    .iter()
                    .map(|p| {
                        let cond_expr = self.lower_expr(&p.condition);
                        if let Some(fb) = &p.fallback {
                            // if !cond { return fallback() }
                            let span = crate::ast::span::Span::new(0, 0);
                            HirStmt::Expr {
                                expr: HirExpr::If(
                                    Box::new(HirExpr::Unary(
                                        HirUnOp::Not,
                                        Box::new(cond_expr),
                                        span,
                                    )),
                                    vec![HirStmt::Return {
                                        value: Some(HirExpr::Call(
                                            Box::new(HirExpr::Ident(fb.clone(), span)),
                                            vec![], // TODO: arguments?
                                            false,
                                            span,
                                        )),
                                        span,
                                    }],
                                    None,
                                    span,
                                ),
                                span,
                            }
                        } else {
                            HirStmt::Expr {
                                expr: cond_expr,
                                span: crate::ast::span::Span::new(0, 0),
                            }
                        }
                    })
                    .collect();

                let mut new_body = pre_stmts;
                let mut body_to_process = body;

                // Inject postconditions before every explicit return statement
                self.inject_postconditions_before_returns(&mut body_to_process, &post_stmts);

                // Check if the body ends in a return statement to avoid double-injecting at the end
                let ends_in_return = matches!(body_to_process.last(), Some(HirStmt::Return { .. }));

                new_body.extend(body_to_process);

                if !ends_in_return {
                    new_body.extend(post_stmts);
                }

                new_body
            }
        }
    }

    fn inject_postconditions_before_returns(
        &self,
        body: &mut Vec<HirStmt>,
        post_stmts: &[HirStmt],
    ) {
        let mut i = 0;
        while i < body.len() {
            match &mut body[i] {
                HirStmt::Return { value, span } => {
                    let s = *span;
                    if let Some(v) = value.take() {
                        // Replace Return { value: Some(v) } with:
                        // let __vox_result__ = v
                        // <postconditions>
                        // return __vox_result__
                        body[i] = HirStmt::Let {
                            pattern: crate::hir::HirPattern::Ident("__vox_result__".into(), s),
                            type_ann: None,
                            value: v,
                            mutable: false,
                            span: s,
                        };
                        i += 1;
                        for post in post_stmts {
                            body.insert(i, post.clone());
                            i += 1;
                        }
                        body.insert(
                            i,
                            HirStmt::Return {
                                value: Some(crate::hir::HirExpr::Ident("__vox_result__".into(), s)),
                                span: s,
                            },
                        );
                        i += 1;
                    } else {
                        // Replace Return { value: None } with:
                        // <postconditions>
                        // return
                        for post in post_stmts {
                            body.insert(i, post.clone());
                            i += 1;
                        }
                        i += 1; // Skip the original return (now at i)
                    }
                }
                HirStmt::While { body: b, .. } | HirStmt::Loop { body: b, .. } => {
                    self.inject_postconditions_before_returns(b, post_stmts);
                    i += 1;
                }
                HirStmt::Expr { expr, .. } => {
                    self.inject_postconditions_into_expr(expr, post_stmts);
                    i += 1;
                }
                HirStmt::Let { value, .. } => {
                    self.inject_postconditions_into_expr(value, post_stmts);
                    i += 1;
                }
                HirStmt::Assign { value, target, .. } => {
                    self.inject_postconditions_into_expr(value, post_stmts);
                    self.inject_postconditions_into_expr(target, post_stmts);
                    i += 1;
                }
                _ => i += 1,
            }
        }
    }

    fn inject_postconditions_into_expr(&self, expr: &mut HirExpr, post_stmts: &[HirStmt]) {
        match expr {
            HirExpr::Block(stmts, _) => {
                self.inject_postconditions_before_returns(stmts, post_stmts);
            }
            HirExpr::If(cond, then_body, else_body, _) => {
                self.inject_postconditions_into_expr(cond, post_stmts);
                self.inject_postconditions_before_returns(then_body, post_stmts);
                if let Some(eb) = else_body {
                    self.inject_postconditions_before_returns(eb, post_stmts);
                }
            }
            HirExpr::Match(subj, arms, _) => {
                self.inject_postconditions_into_expr(subj, post_stmts);
                for arm in arms {
                    if let Some(g) = &mut arm.guard {
                        self.inject_postconditions_into_expr(g, post_stmts);
                    }
                    self.inject_postconditions_into_expr(&mut arm.body, post_stmts);
                }
            }
            HirExpr::For(_, _, it, body, _, _) => {
                self.inject_postconditions_into_expr(it, post_stmts);
                self.inject_postconditions_into_expr(body, post_stmts);
            }
            HirExpr::Try(t) => {
                let span = t.span;
                self.inject_postconditions_into_expr(&mut t.target, post_stmts);

                // Transform 'expr?' into a match to capture early returns for postcondition injection.
                // Note: This assumes Result (Ok/Err) or Option (Some/None).
                // For Result: Match { Ok(v) => v, Err(e) => { post; return Err(e) } }
                let target = t.target.clone();

                let ok_arm = HirMatchArm {
                    pattern: crate::hir::HirPattern::Constructor(
                        "Ok".into(),
                        vec![crate::hir::HirPattern::Ident("__vox_ok_val__".into(), span)],
                        span,
                    ),
                    guard: None,
                    body: Box::new(HirExpr::Ident("__vox_ok_val__".into(), span)),
                    span,
                };

                let err_body = vec![HirStmt::Return {
                    value: Some(HirExpr::Call(
                        Box::new(HirExpr::Ident("Err".into(), span)),
                        vec![HirArg {
                            name: None,
                            value: HirExpr::Ident("__vox_err_val__".into(), span),
                        }],
                        false,
                        span,
                    )),
                    span,
                }];

                let err_arm = HirMatchArm {
                    pattern: crate::hir::HirPattern::Constructor(
                        "Err".into(),
                        vec![crate::hir::HirPattern::Ident(
                            "__vox_err_val__".into(),
                            span,
                        )],
                        span,
                    ),
                    guard: None,
                    body: Box::new(HirExpr::Block(err_body, span)),
                    span,
                };

                *expr = HirExpr::Match(target, vec![ok_arm, err_arm], span);
                // Recursively visit the new match to inject postconditions into the Err branch's return.
                self.inject_postconditions_into_expr(expr, post_stmts);
            }
            HirExpr::Call(callee, args, _, _) | HirExpr::MethodCall(callee, _, args, _, _) => {
                self.inject_postconditions_into_expr(callee, post_stmts);
                for arg in args {
                    self.inject_postconditions_into_expr(&mut arg.value, post_stmts);
                }
            }
            HirExpr::ObjectLit(fields, _) => {
                for (_, v) in fields {
                    self.inject_postconditions_into_expr(v, post_stmts);
                }
            }
            HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
                for it in items {
                    self.inject_postconditions_into_expr(it, post_stmts);
                }
            }
            HirExpr::Binary(_, l, r, _) | HirExpr::With(l, r, _) => {
                self.inject_postconditions_into_expr(l, post_stmts);
                self.inject_postconditions_into_expr(r, post_stmts);
            }
            HirExpr::Unary(_, o, _) | HirExpr::Spawn(o, _) | HirExpr::FieldAccess(o, _, _) => {
                self.inject_postconditions_into_expr(o, post_stmts);
            }
            HirExpr::Jsx(el) => {
                for attr in &mut el.attributes {
                    self.inject_postconditions_into_expr(&mut attr.value, post_stmts);
                }
                for child in &mut el.children {
                    self.inject_postconditions_into_expr(child, post_stmts);
                }
            }
            HirExpr::JsxSelfClosing(el) => {
                for attr in &mut el.attributes {
                    self.inject_postconditions_into_expr(&mut attr.value, post_stmts);
                }
            }
            HirExpr::JsxFragment(children, _) => {
                for child in children {
                    self.inject_postconditions_into_expr(child, post_stmts);
                }
            }

            HirExpr::Index(obj, idx, _) => {
                self.inject_postconditions_into_expr(obj, post_stmts);
                self.inject_postconditions_into_expr(idx, post_stmts);
            }
            // Lambdas create a new return context, so we do NOT inject postconditions into them.
            HirExpr::Lambda(..) => {}
            HirExpr::AsyncView(_) => {}
            HirExpr::IntLit(..)
            | HirExpr::FloatLit(..)
            | HirExpr::StringLit(..)
            | HirExpr::BoolLit(..)
            | HirExpr::DecimalLit(..)
            | HirExpr::Ident(..)
            | HirExpr::WorkflowVersion(_) => {}
        }
    }
}
