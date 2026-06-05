use crate::ast::expr::{self, BinOp, Expr, UnOp};
use crate::hir::HirWorkflowVersion;
use crate::hir::nodes::durability::DurabilityKind;
use crate::hir::*;

use super::LowerCtx;

impl LowerCtx {
    pub(crate) fn lower_expr(&mut self, e: &Expr) -> HirExpr {
        match e {
            Expr::IntLit { value, span } => HirExpr::IntLit(*value, *span),
            Expr::FloatLit { value, span } => HirExpr::FloatLit(*value, *span),
            Expr::StringLit { value, span } => HirExpr::StringLit(value.clone(), *span),
            Expr::BoolLit { value, span } => HirExpr::BoolLit(*value, *span),
            Expr::DecimalLit { value, span } => HirExpr::DecimalLit(value.clone(), *span),
            Expr::Ident { name, span } => HirExpr::Ident(name.clone(), *span),
            Expr::ObjectLit { fields, span } => HirExpr::ObjectLit(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.lower_expr(v)))
                    .collect(),
                *span,
            ),
            Expr::ListLit { elements, span } => {
                HirExpr::ListLit(elements.iter().map(|e| self.lower_expr(e)).collect(), *span)
            }
            Expr::TupleLit { elements, span } => {
                HirExpr::TupleLit(elements.iter().map(|e| self.lower_expr(e)).collect(), *span)
            }
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let hir_op = match op {
                    BinOp::Add => HirBinOp::Add,
                    BinOp::Sub => HirBinOp::Sub,
                    BinOp::Mul => HirBinOp::Mul,
                    BinOp::Div => HirBinOp::Div,
                    BinOp::Lt => HirBinOp::Lt,
                    BinOp::Gt => HirBinOp::Gt,
                    BinOp::Lte => HirBinOp::Lte,
                    BinOp::Gte => HirBinOp::Gte,
                    BinOp::And => HirBinOp::And,
                    BinOp::Or => HirBinOp::Or,
                    BinOp::Is => HirBinOp::Is,
                    BinOp::Isnt => HirBinOp::Isnt,
                    BinOp::Mod => HirBinOp::Mod,
                    BinOp::Pipe => HirBinOp::Pipe,
                };
                HirExpr::Binary(
                    hir_op,
                    Box::new(self.lower_expr(left)),
                    Box::new(self.lower_expr(right)),
                    *span,
                )
            }
            Expr::Unary { op, operand, span } => {
                let hir_op = match op {
                    UnOp::Not => HirUnOp::Not,
                    UnOp::Neg => HirUnOp::Neg,
                };
                HirExpr::Unary(hir_op, Box::new(self.lower_expr(operand)), *span)
            }
            Expr::Call { callee, args, span } => HirExpr::Call(
                Box::new(self.lower_expr(callee)),
                args.iter()
                    .map(|a| HirArg {
                        name: a.name.clone(),
                        value: self.lower_expr(&a.value),
                    })
                    .collect(),
                false,
                *span,
            ),
            Expr::MethodCall {
                object,
                method,
                args,
                span,
            } => self.lower_method_call_expr(e, object, method, args, *span),
            Expr::FieldAccess {
                object,
                field,
                span,
            } => HirExpr::FieldAccess(Box::new(self.lower_expr(object)), field.clone(), *span),
            Expr::Match {
                subject,
                arms,
                span,
            } => HirExpr::Match(
                Box::new(self.lower_expr(subject)),
                arms.iter()
                    .map(|a| HirMatchArm {
                        pattern: self.lower_pattern(&a.pattern),
                        guard: a.guard.as_ref().map(|g| Box::new(self.lower_expr(g))),
                        body: Box::new(self.lower_expr(&a.body)),
                        span: a.span,
                    })
                    .collect(),
                *span,
            ),
            Expr::If {
                condition,
                then_body,
                else_body,
                span,
            } => HirExpr::If(
                Box::new(self.lower_expr(condition)),
                then_body.iter().map(|s| self.lower_stmt(s)).collect(),
                else_body
                    .as_ref()
                    .map(|stmts| stmts.iter().map(|s| self.lower_stmt(s)).collect()),
                *span,
            ),
            Expr::Try { target, span } => HirExpr::Try(HirTry {
                target: Box::new(self.lower_expr(target)),
                span: *span,
            }),
            Expr::For {
                binding,
                index,
                iterable,
                body,
                key,
                span,
            } => HirExpr::For(
                binding.clone(),
                index.clone(),
                Box::new(self.lower_expr(iterable)),
                Box::new(self.lower_expr(body)),
                key.as_ref().map(|k| Box::new(self.lower_expr(k))),
                *span,
            ),
            Expr::Lambda {
                params,
                return_type,
                body,
                cancellable,
                span,
            } => {
                self.def_map.push_scope();
                let hir_params = params.iter().map(|p| self.lower_param(p)).collect();
                let hir_body = self.lower_expr(body);
                self.def_map.pop_scope();
                HirExpr::Lambda(
                    hir_params,
                    return_type.as_ref().map(|t| self.lower_type(t)),
                    Box::new(hir_body),
                    *cancellable,
                    *span,
                )
            }
            Expr::Pipe { left, right, span } => HirExpr::Binary(
                HirBinOp::Pipe,
                Box::new(self.lower_expr(left)),
                Box::new(self.lower_expr(right)),
                *span,
            ),
            Expr::Spawn { target, span } => {
                HirExpr::Spawn(Box::new(self.lower_expr(target)), *span)
            }
            Expr::With {
                operand,
                options,
                span,
            } => HirExpr::With(
                Box::new(self.lower_expr(operand)),
                Box::new(self.lower_expr(options)),
                *span,
            ),
            Expr::Jsx(el) => HirExpr::Jsx(HirJsxElement {
                tag: el.tag.clone(),
                attributes: el
                    .attributes
                    .iter()
                    .map(|a| HirJsxAttr {
                        name: a.name.clone(),
                        value: self.lower_expr(&a.value),
                    })
                    .collect(),
                children: el.children.iter().map(|c| self.lower_expr(c)).collect(),
                span: el.span,
            }),
            Expr::JsxSelfClosing(el) => HirExpr::JsxSelfClosing(HirJsxSelfClosing {
                tag: el.tag.clone(),
                attributes: el
                    .attributes
                    .iter()
                    .map(|a| HirJsxAttr {
                        name: a.name.clone(),
                        value: self.lower_expr(&a.value),
                    })
                    .collect(),
                span: el.span,
            }),
            Expr::JsxFragment { children, span } => {
                HirExpr::JsxFragment(children.iter().map(|c| self.lower_expr(c)).collect(), *span)
            }
            Expr::StringInterp { parts, span } => {
                // Convert string interpolation to template literal-style
                // For now, represent as a string concat
                let mut result_parts = Vec::new();
                for part in parts {
                    match part {
                        expr::StringPart::Literal(s) => {
                            result_parts.push(HirExpr::StringLit(s.clone(), *span));
                        }
                        expr::StringPart::Interpolation(e) => {
                            result_parts.push(self.lower_expr(e));
                        }
                    }
                }
                if result_parts.len() == 1 {
                    result_parts.pop().unwrap()
                } else {
                    // Represent as a concat chain
                    let mut acc = result_parts.remove(0);
                    for part in result_parts {
                        acc = HirExpr::Binary(HirBinOp::Add, Box::new(acc), Box::new(part), *span);
                    }
                    acc
                }
            }
            Expr::Block { stmts, span } => {
                HirExpr::Block(stmts.iter().map(|s| self.lower_stmt(s)).collect(), *span)
            }
            // P1-T7: desugar `side_effect { … }` into a synthesised anonymous inline activity
            // and a zero-argument call to it.  The body stmts may use non-deterministic builtins
            // (time.now, random.*) — the workflow determinism check is exempt inside the block.
            Expr::SideEffect { stmts, span } => {
                let n = self.next_synthesis_counter();
                let name = format!("__side_effect_{n}");
                let body: Vec<HirStmt> = stmts.iter().map(|s| self.lower_stmt(s)).collect();
                let id = self.def_map.define(name.clone());
                self.synthesised_fns.push(HirFn {
                    id,
                    name: name.clone(),
                    generics: vec![],
                    params: vec![],
                    return_type: None,
                    body,
                    is_async: false,
                    is_pub: false,
                    is_mobile_native: false,
                    is_pure: false,
                    is_reactive: false,
                    capabilities: vec![],
                    is_remote: false,
                    is_llm: false,
                    llm_model: None,
                    ai_structured_output: None,
                    ai_fixture: None,
                    embed: None,
                    is_deprecated: false,
                    schedule_interval: None,
                    durability: Some(DurabilityKind::Activity),
                    actor_state_fields: vec![],
                    postconditions: vec![],
                    ts_extern_module: None,
                    generated_hash: None,
                    span: *span,
                    inference_model: None,
                    training_step: false,
                    distributed_train: None,
                });
                HirExpr::Call(Box::new(HirExpr::Ident(name, *span)), vec![], false, *span)
            }
            Expr::Index {
                object,
                index,
                span,
            } => HirExpr::Index(
                Box::new(self.lower_expr(object)),
                Box::new(self.lower_expr(index)),
                *span,
            ),
            Expr::WorkflowVersion(call) => HirExpr::WorkflowVersion(HirWorkflowVersion {
                change_id: call.change_id.clone(),
                min: call.min,
                max: call.max,
                span: call.span,
            }),
        }
    }

    /// Lower a `MethodCall` expression.  Extracted from `lower_expr` (CR-A1):
    /// the four DB-query-plan early-return branches contributed ~8 decision
    /// points inline; moving them here drops `lower_expr` to ~11 CC.
    pub(crate) fn lower_method_call_expr(
        &mut self,
        e: &crate::ast::expr::Expr,
        object: &crate::ast::expr::Expr,
        method: &str,
        args: &[crate::ast::expr::Arg],
        span: crate::ast::span::Span,
    ) -> HirExpr {
        if let Some(chain) = super::expr_db::extract_db_query_chain(self, e) {
            let plan = super::expr_db::make_db_plan_from_chain(&chain);
            // Lower the call-site args ORIGINALLY so typecheck matches the
            // surface `filter(record)` / `update(id, record)` etc. signatures
            // in typeck/builtins.rs.
            let surface_args: Vec<HirArg> = args
                .iter()
                .map(|a| HirArg {
                    name: a.name.clone(),
                    value: self.lower_expr(&a.value),
                })
                .collect();
            return HirExpr::MethodCall(
                Box::new(self.lower_expr(object)),
                method.to_string(),
                surface_args,
                Some(Box::new(plan)),
                span,
            );
        }
        let obj_hir = self.lower_expr(object);
        let hir_args: Vec<HirArg> = args
            .iter()
            .map(|a| HirArg {
                name: a.name.clone(),
                value: self.lower_expr(&a.value),
            })
            .collect();
        if method == "count"
            && hir_args.is_empty()
            && let Some((table, count_args)) =
                super::expr_db::extract_count_chain_args(self, object)
        {
            let plan = HirDbQueryPlan {
                table: table.clone(),
                op: HirDbTableOp::Count,
                predicate: None,
                projection: None,
                order_by: None,
                has_limit: false,
                capabilities: HirDbPlanCapabilities::default(),
                predicate_args: Vec::new(),
                limit_value: None,
            };
            return HirExpr::MethodCall(
                Box::new(obj_hir),
                method.to_string(),
                count_args,
                Some(Box::new(plan)),
                span,
            );
        }
        if method == "filter"
            && let Some(table) = super::expr_db::db_table_handle_name(&obj_hir)
            && let Some(filter_args) = super::expr_db::extract_filter_record_args(&hir_args)
            && !filter_args.is_empty()
        {
            let plan = HirDbQueryPlan {
                table: table.clone(),
                op: HirDbTableOp::FilterRecord,
                predicate: Some(HirDbPredicate::And(
                    filter_args
                        .iter()
                        .filter_map(|a| {
                            Some(HirDbPredicate::Eq {
                                field: a.name.clone()?,
                            })
                        })
                        .collect(),
                )),
                projection: None,
                order_by: None,
                has_limit: false,
                capabilities: HirDbPlanCapabilities::default(),
                // Predicate values are the filter record fields, in the same
                // order as the `And([Eq{field}..])` predicate above.
                predicate_args: filter_args.clone(),
                limit_value: None,
            };
            // Pass the ORIGINAL hir_args so typecheck matches the
            // `filter(record) -> Result[List[Row]]` signature.
            return HirExpr::MethodCall(
                Box::new(obj_hir),
                method.to_string(),
                hir_args,
                Some(Box::new(plan)),
                span,
            );
        }
        if let Some((table, op)) = super::expr_db::db_table_op_from_field(&obj_hir, method) {
            let mut cap = HirDbPlanCapabilities::default();
            if matches!(op, HirDbTableOp::UnsafeQueryRawClause) {
                cap.emits_change_log = true;
            }
            HirExpr::MethodCall(
                Box::new(obj_hir),
                method.to_string(),
                hir_args,
                Some(Box::new(HirDbQueryPlan {
                    table,
                    op,
                    predicate: None,
                    projection: None,
                    order_by: None,
                    has_limit: false,
                    capabilities: cap,
                    predicate_args: Vec::new(),
                    limit_value: None,
                })),
                span,
            )
        } else {
            HirExpr::MethodCall(Box::new(obj_hir), method.to_string(), hir_args, None, span)
        }
    }
}
