use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirExpr, HirStmt};

/// Tracks the last span an identifier is used at within a block or function.
#[derive(Debug, Default)]
pub struct UsageTracker {
    /// Maps identifier name to its last known usage span.
    pub last_use: HashMap<String, Span>,
}

impl UsageTracker {
    pub fn build(body: &[HirStmt]) -> Self {
        let mut tracker = Self::default();
        for stmt in body {
            tracker.walk_stmt(stmt);
        }
        tracker
    }

    fn walk_stmt(&mut self, stmt: &HirStmt) {
        match stmt {
            HirStmt::Let { value, .. } => self.walk_expr(value),
            HirStmt::Assign { target, value, .. } => {
                self.walk_expr(target);
                self.walk_expr(value);
            }
            HirStmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.walk_expr(v);
                }
            }
            HirStmt::Expr { expr, .. } => self.walk_expr(expr),
            HirStmt::While { condition, body, .. } => {
                self.walk_expr(condition);
                for s in body { self.walk_stmt(s); }
            }
            HirStmt::Loop { body, .. } => {
                for s in body { self.walk_stmt(s); }
            }
            _ => {}
        }
    }

    fn walk_expr(&mut self, expr: &HirExpr) {
        match expr {
            HirExpr::Ident(name, span) => {
                self.last_use.insert(name.clone(), *span);
            }
            HirExpr::Binary(_, l, r, _) => {
                self.walk_expr(l);
                self.walk_expr(r);
            }
            HirExpr::Unary(_, e, _) => self.walk_expr(e),
            HirExpr::Call(callee, args, _, _) => {
                self.walk_expr(callee);
                for arg in args { self.walk_expr(&arg.value); }
            }
            HirExpr::MethodCall(obj, _, args, _, _) => {
                self.walk_expr(obj);
                for arg in args { self.walk_expr(&arg.value); }
            }
            HirExpr::FieldAccess(obj, _, _) => self.walk_expr(obj),
            HirExpr::Match(obj, arms, _) => {
                self.walk_expr(obj);
                for arm in arms { self.walk_expr(&arm.body); }
            }
            HirExpr::If(cond, then_b, else_b, _) => {
                self.walk_expr(cond);
                for s in then_b { self.walk_stmt(s); }
                if let Some(eb) = else_b {
                    for s in eb { self.walk_stmt(s); }
                }
            }
            HirExpr::ListLit(elts, _) => {
                for e in elts { self.walk_expr(e); }
            }
            HirExpr::TupleLit(elts, _) => {
                for e in elts { self.walk_expr(e); }
            }
            HirExpr::Block(body, _) => {
                for s in body { self.walk_stmt(s); }
            }
            HirExpr::Index(obj, idx, _) => {
                self.walk_expr(obj);
                self.walk_expr(idx);
            }
            _ => {}
        }
    }

    pub fn is_last_use(&self, name: &str, span: Span) -> bool {
        self.last_use.get(name).map_or(false, |s| *s == span)
    }
}
