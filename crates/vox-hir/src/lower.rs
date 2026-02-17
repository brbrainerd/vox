use vox_ast::decl::*;
use vox_ast::expr::{self, Expr, BinOp, UnOp};
use vox_ast::stmt::Stmt;
use vox_ast::types::TypeExpr;
use vox_ast::pattern::Pattern;
use crate::hir::*;
use crate::def_map::DefMap;

/// Lower an AST Module to a HirModule.
pub fn lower_module(module: &Module) -> HirModule {
    let mut ctx = LowerCtx::new();
    ctx.lower(module)
}

struct LowerCtx {
    def_map: DefMap,
}

impl LowerCtx {
    fn new() -> Self {
        Self { def_map: DefMap::new() }
    }

    fn lower(&mut self, module: &Module) -> HirModule {
        let mut hir = HirModule {
            imports: Vec::new(),
            functions: Vec::new(),
            types: Vec::new(),
            routes: Vec::new(),
            actors: Vec::new(),
            workflows: Vec::new(),
            activities: Vec::new(),
            tests: Vec::new(),
            server_fns: Vec::new(),
            tables: Vec::new(),
            indexes: Vec::new(),
            mcp_tools: Vec::new(),
        };

        for decl in &module.declarations {
            match decl {
                Decl::Import(imp) => {
                    for path in &imp.paths {
                        let (mod_path, item) = if path.segments.len() > 1 {
                            let item = path.segments.last().unwrap().clone();
                            let mod_path = path.segments[..path.segments.len()-1].to_vec();
                            (mod_path, item)
                        } else {
                            (vec![], path.segments[0].clone())
                        };
                        hir.imports.push(HirImport {
                            module_path: mod_path,
                            item,
                            span: path.span,
                        });
                    }
                }
                Decl::Function(f) => {
                    hir.functions.push(self.lower_fn(f, false));
                }
                Decl::Component(c) => {
                    hir.functions.push(self.lower_fn(&c.func, true));
                }
                Decl::TypeDef(t) => {
                    hir.types.push(self.lower_typedef(t));
                }
                Decl::HttpRoute(r) => {
                    hir.routes.push(self.lower_route(r));
                }
                Decl::Actor(a) => {
                    hir.actors.push(self.lower_actor(a));
                }
                Decl::Workflow(w) => {
                    hir.workflows.push(self.lower_workflow(w));
                }
                Decl::Activity(a) => {
                    hir.activities.push(self.lower_activity(a));
                }
                Decl::McpTool(m) => {
                    let func = self.lower_fn(&m.func, false);
                    hir.mcp_tools.push(HirMcpTool {
                        description: m.description.clone(),
                        func,
                    });
                }
                Decl::Test(t) => {
                    hir.tests.push(self.lower_fn(&t.func, false));
                }
                Decl::ServerFn(s) => {
                    let lowered = self.lower_fn(&s.func, false);
                    let route_path = format!("/api/{}", lowered.name);
                    hir.server_fns.push(HirServerFn {
                        id: lowered.id,
                        name: lowered.name.clone(),
                        params: lowered.params.clone(),
                        return_type: lowered.return_type.clone(),
                        body: lowered.body.clone(),
                        route_path,
                        span: lowered.span,
                    });
                }
                Decl::Table(t) => {
                    hir.tables.push(self.lower_table(t));
                }
                Decl::Index(idx) => {
                    hir.indexes.push(HirIndex {
                        table_name: idx.table_name.clone(),
                        index_name: idx.index_name.clone(),
                        columns: idx.columns.clone(),
                        span: idx.span,
                    });
                }
                Decl::V0Component(_) => {
                    // V0 design-from-image components have no body to lower;
                    // they are processed externally (e.g. by an AI design pipeline).
                }
                Decl::Routes(_) => {
                    // Client-side routing is handled by the TS codegen directly from AST.
                }
            }
        }

        // Analyze async logic
        for f in &mut hir.functions {
            if has_async_stmts(&f.body) { f.is_async = true; }
        }
        for t in &mut hir.tests {
             if has_async_stmts(&t.body) { t.is_async = true; }
        }

        hir
    }

    fn lower_fn(&mut self, f: &FnDecl, is_component: bool) -> HirFn {
        let id = self.def_map.define(f.name.clone());
        self.def_map.push_scope();
        let params = f.params.iter().map(|p| self.lower_param(p)).collect();
        let body = f.body.iter().map(|s| self.lower_stmt(s)).collect();
        self.def_map.pop_scope();

        HirFn {
            id,
            name: f.name.clone(),
            generics: f.generics.clone(),
            params,
            return_type: f.return_type.as_ref().map(|t| self.lower_type(t)),
            body,
            is_component,
            is_async: false,
            is_pub: f.is_pub,
            span: f.span,
        }
    }

    fn lower_param(&mut self, p: &expr::Param) -> HirParam {
        let id = self.def_map.define(p.name.clone());
        HirParam {
            id,
            name: p.name.clone(),
            type_ann: p.type_ann.as_ref().map(|t| self.lower_type(t)),
            default: p.default.as_ref().map(|e| self.lower_expr(e)),
            span: p.span,
        }
    }

    fn lower_type(&self, t: &TypeExpr) -> HirType {
        match t {
            TypeExpr::Named { name, .. } => {
                if name == "Unit" { HirType::Unit }
                else { HirType::Named(name.clone()) }
            }
            TypeExpr::Generic { name, args, .. } => {
                HirType::Generic(name.clone(), args.iter().map(|a| self.lower_type(a)).collect())
            }
            TypeExpr::Function { params, return_type, .. } => {
                HirType::Function(
                    params.iter().map(|p| self.lower_type(p)).collect(),
                    Box::new(self.lower_type(return_type)),
                )
            }
            TypeExpr::Tuple { elements, .. } => {
                HirType::Tuple(elements.iter().map(|e| self.lower_type(e)).collect())
            }
            TypeExpr::Unit { .. } => HirType::Unit,
        }
    }

    fn lower_expr(&mut self, e: &Expr) -> HirExpr {
        match e {
            Expr::IntLit { value, span } => HirExpr::IntLit(*value, *span),
            Expr::FloatLit { value, span } => HirExpr::FloatLit(*value, *span),
            Expr::StringLit { value, span } => HirExpr::StringLit(value.clone(), *span),
            Expr::BoolLit { value, span } => HirExpr::BoolLit(*value, *span),
            Expr::Ident { name, span } => HirExpr::Ident(name.clone(), *span),
            Expr::ObjectLit { fields, span } => {
                HirExpr::ObjectLit(
                    fields.iter().map(|(k, v)| (k.clone(), self.lower_expr(v))).collect(),
                    *span,
                )
            }
            Expr::ListLit { elements, span } => {
                HirExpr::ListLit(elements.iter().map(|e| self.lower_expr(e)).collect(), *span)
            }
            Expr::TupleLit { elements, span } => {
                HirExpr::ListLit(elements.iter().map(|e| self.lower_expr(e)).collect(), *span)
            }
            Expr::Binary { op, left, right, span } => {
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
                    BinOp::Pipe => HirBinOp::Pipe,
                };
                HirExpr::Binary(hir_op, Box::new(self.lower_expr(left)), Box::new(self.lower_expr(right)), *span)
            }
            Expr::Unary { op, operand, span } => {
                let hir_op = match op {
                    UnOp::Not => HirUnOp::Not,
                    UnOp::Neg => HirUnOp::Neg,
                };
                HirExpr::Unary(hir_op, Box::new(self.lower_expr(operand)), *span)
            }
            Expr::Call { callee, args, span } => {
                HirExpr::Call(
                    Box::new(self.lower_expr(callee)),
                    args.iter().map(|a| HirArg { name: a.name.clone(), value: self.lower_expr(&a.value) }).collect(),
                    false,
                    *span,
                )
            }
            Expr::MethodCall { object, method, args, span } => {
                HirExpr::MethodCall(
                    Box::new(self.lower_expr(object)),
                    method.clone(),
                    args.iter().map(|a| HirArg { name: a.name.clone(), value: self.lower_expr(&a.value) }).collect(),
                    *span,
                )
            }
            Expr::FieldAccess { object, field, span } => {
                HirExpr::FieldAccess(Box::new(self.lower_expr(object)), field.clone(), *span)
            }
            Expr::Match { subject, arms, span } => {
                HirExpr::Match(
                    Box::new(self.lower_expr(subject)),
                    arms.iter().map(|a| HirMatchArm {
                        pattern: self.lower_pattern(&a.pattern),
                        guard: a.guard.as_ref().map(|g| Box::new(self.lower_expr(g))),
                        body: Box::new(self.lower_expr(&a.body)),
                        span: a.span,
                    }).collect(),
                    *span,
                )
            }
            Expr::If { condition, then_body, else_body, span } => {
                HirExpr::If(
                    Box::new(self.lower_expr(condition)),
                    then_body.iter().map(|s| self.lower_stmt(s)).collect(),
                    else_body.as_ref().map(|stmts| stmts.iter().map(|s| self.lower_stmt(s)).collect()),
                    *span,
                )
            }
            Expr::For { binding, iterable, body, span } => {
                HirExpr::For(binding.clone(), Box::new(self.lower_expr(iterable)), Box::new(self.lower_expr(body)), *span)
            }
            Expr::Lambda { params, return_type, body, span } => {
                self.def_map.push_scope();
                let hir_params = params.iter().map(|p| self.lower_param(p)).collect();
                let hir_body = self.lower_expr(body);
                self.def_map.pop_scope();
                HirExpr::Lambda(hir_params, return_type.as_ref().map(|t| self.lower_type(t)), Box::new(hir_body), *span)
            }
            Expr::Pipe { left, right, span } => {
                HirExpr::Pipe(Box::new(self.lower_expr(left)), Box::new(self.lower_expr(right)), *span)
            }
            Expr::Spawn { target, span } => {
                HirExpr::Spawn(Box::new(self.lower_expr(target)), *span)
            }
            Expr::With { operand, options, span } => {
                HirExpr::With(Box::new(self.lower_expr(operand)), Box::new(self.lower_expr(options)), *span)
            }
            Expr::Jsx(el) => {
                HirExpr::Jsx(HirJsxElement {
                    tag: el.tag.clone(),
                    attributes: el.attributes.iter().map(|a| HirJsxAttr {
                        name: a.name.clone(),
                        value: self.lower_expr(&a.value),
                    }).collect(),
                    children: el.children.iter().map(|c| self.lower_expr(c)).collect(),
                    span: el.span,
                })
            }
            Expr::JsxSelfClosing(el) => {
                HirExpr::JsxSelfClosing(HirJsxSelfClosing {
                    tag: el.tag.clone(),
                    attributes: el.attributes.iter().map(|a| HirJsxAttr {
                        name: a.name.clone(),
                        value: self.lower_expr(&a.value),
                    }).collect(),
                    span: el.span,
                })
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
        }
    }

    fn lower_stmt(&mut self, s: &Stmt) -> HirStmt {
        match s {
            Stmt::Let { pattern, type_ann, value, mutable, span } => {
                HirStmt::Let {
                    pattern: self.lower_pattern(pattern),
                    type_ann: type_ann.as_ref().map(|t| self.lower_type(t)),
                    value: self.lower_expr(value),
                    mutable: *mutable,
                    span: *span,
                }
            }
            Stmt::Assign { target, value, span } => {
                HirStmt::Assign {
                    target: self.lower_expr(target),
                    value: self.lower_expr(value),
                    span: *span,
                }
            }
            Stmt::Return { value, span } => {
                HirStmt::Return {
                    value: value.as_ref().map(|v| self.lower_expr(v)),
                    span: *span,
                }
            }
            Stmt::Expr { expr, span } => {
                HirStmt::Expr {
                    expr: self.lower_expr(expr),
                    span: *span,
                }
            }
        }
    }

    fn lower_pattern(&mut self, p: &Pattern) -> HirPattern {
        match p {
            Pattern::Ident { name, span } => {
                self.def_map.define(name.clone());
                HirPattern::Ident(name.clone(), *span)
            }
            Pattern::Tuple { elements, span } => {
                HirPattern::Tuple(elements.iter().map(|e| self.lower_pattern(e)).collect(), *span)
            }
            Pattern::Constructor { name, fields, span } => {
                HirPattern::Constructor(name.clone(), fields.iter().map(|f| self.lower_pattern(f)).collect(), *span)
            }
            Pattern::Wildcard { span } => HirPattern::Wildcard(*span),
            Pattern::Literal { value, span } => {
                HirPattern::Literal(Box::new(self.lower_expr(value)), *span)
            }
        }
    }

    fn lower_typedef(&mut self, t: &TypeDefDecl) -> HirTypeDef {
        let id = self.def_map.define(t.name.clone());
        HirTypeDef {
            id,
            name: t.name.clone(),
            variants: t.variants.iter().map(|v| HirVariant {
                name: v.name.clone(),
                fields: v.fields.iter().map(|f| (f.name.clone(), self.lower_type(&f.type_ann))).collect(),
                span: v.span,
            }).collect(),
            is_pub: t.is_pub,
            span: t.span,
        }
    }

    fn lower_route(&mut self, r: &HttpRouteDecl) -> HirRoute {
        let method = match r.method {
            HttpMethod::Get => HirHttpMethod::Get,
            HttpMethod::Post => HirHttpMethod::Post,
            HttpMethod::Put => HirHttpMethod::Put,
            HttpMethod::Delete => HirHttpMethod::Delete,
        };
        self.def_map.push_scope();
        let body = r.body.iter().map(|s| self.lower_stmt(s)).collect();
        self.def_map.pop_scope();

        HirRoute { method, path: r.path.clone(), return_type: r.return_type.as_ref().map(|t| self.lower_type(t)), body, span: r.span }
    }

    fn lower_actor(&mut self, a: &ActorDecl) -> HirActor {
        let id = self.def_map.define(a.name.clone());
        HirActor {
            id,
            name: a.name.clone(),
            handlers: a.handlers.iter().map(|h| {
                self.def_map.push_scope();
                let params = h.params.iter().map(|p| self.lower_param(p)).collect();
                let body = h.body.iter().map(|s| self.lower_stmt(s)).collect();
                self.def_map.pop_scope();
                HirActorHandler {
                    event_name: h.event_name.clone(),
                    params,
                    return_type: h.return_type.as_ref().map(|t| self.lower_type(t)),
                    body,
                    span: h.span,
                }
            }).collect(),
            span: a.span,
        }
    }

    fn lower_workflow(&mut self, w: &WorkflowDecl) -> HirWorkflow {
        let id = self.def_map.define(w.name.clone());
        self.def_map.push_scope();
        let params = w.params.iter().map(|p| self.lower_param(p)).collect();
        let body = w.body.iter().map(|s| self.lower_stmt(s)).collect();
        self.def_map.pop_scope();

        HirWorkflow { id, name: w.name.clone(), params, return_type: w.return_type.as_ref().map(|t| self.lower_type(t)), body, span: w.span }
    }

    fn lower_activity(&mut self, a: &ActivityDecl) -> HirActivity {
        let id = self.def_map.define(a.name.clone());
        self.def_map.push_scope();
        let params = a.params.iter().map(|p| self.lower_param(p)).collect();
        let body = a.body.iter().map(|s| self.lower_stmt(s)).collect();
        self.def_map.pop_scope();

        HirActivity { id, name: a.name.clone(), params, return_type: a.return_type.as_ref().map(|t| self.lower_type(t)), body, span: a.span }
    }

    fn lower_table(&mut self, t: &TableDecl) -> HirTable {
        let id = self.def_map.define(t.name.clone());
        HirTable {
            id,
            name: t.name.clone(),
            fields: t.fields.iter().map(|f| HirTableField {
                name: f.name.clone(),
                type_ann: self.lower_type(&f.type_ann),
                span: f.span,
            }).collect(),
            is_pub: t.is_pub,
            span: t.span,
        }
    }
}

fn has_async_stmts(stmts: &[HirStmt]) -> bool {
    stmts.iter().any(has_async_stmt)
}

fn has_async_stmt(s: &HirStmt) -> bool {
    match s {
        HirStmt::Let { value, .. } => has_async_expr(value),
        HirStmt::Assign { value, .. } => has_async_expr(value),
        HirStmt::Return { value, .. } => value.as_ref().map_or(false, |v| has_async_expr(v)),
        HirStmt::Expr { expr, .. } => has_async_expr(expr),
    }
}

fn has_async_expr(e: &HirExpr) -> bool {
    match e {
        HirExpr::IntLit(..) | HirExpr::FloatLit(..) | HirExpr::StringLit(..) | HirExpr::BoolLit(..) | HirExpr::Ident(..) | HirExpr::Spawn(..) | HirExpr::Jsx(..) | HirExpr::JsxSelfClosing(..) => false,
        HirExpr::ListLit(elements, _) => elements.iter().any(has_async_expr),
        HirExpr::ObjectLit(fields, _) => fields.iter().map(|(_, v)| v).any(has_async_expr),
        HirExpr::Binary(_, l, r, _) => has_async_expr(l) || has_async_expr(r),
        HirExpr::Unary(_, e, _) => has_async_expr(e),
        HirExpr::Call(callee, args, is_await, _) => *is_await || has_async_expr(callee) || args.iter().map(|a| &a.value).any(has_async_expr),
        HirExpr::MethodCall(obj, m, args, _) => {
            if m == "send" { return true; }
            has_async_expr(obj) || args.iter().map(|a| &a.value).any(has_async_expr)
        }
        HirExpr::FieldAccess(obj, _, _) => has_async_expr(obj),
        HirExpr::Match(subj, arms, _) => has_async_expr(subj) || arms.iter().any(|arm| has_async_expr(&arm.body) || arm.guard.as_ref().map_or(false, |g| has_async_expr(g))),
        HirExpr::If(cond, then_b, else_b, _) => has_async_expr(cond) || has_async_stmts(then_b) || else_b.as_ref().map_or(false, |b| has_async_stmts(b)),
        HirExpr::For(_, iter, body, _) => has_async_expr(iter) || has_async_expr(body),
        HirExpr::Lambda(..) => false,
        HirExpr::Pipe(l, r, _) => has_async_expr(l) || has_async_expr(r),
        HirExpr::With(l, r, _) => has_async_expr(l) || has_async_expr(r),
        HirExpr::Block(stmts, _) => has_async_stmts(stmts),
    }
}
