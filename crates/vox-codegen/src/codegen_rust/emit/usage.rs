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
            HirStmt::Return { value: Some(v), .. } => self.walk_expr(v),
            HirStmt::Expr { expr, .. } => self.walk_expr(expr),
            HirStmt::While {
                condition, body, ..
            } => {
                self.walk_expr(condition);
                for s in body {
                    self.walk_stmt(s);
                }
            }
            HirStmt::Loop { body, .. } => {
                for s in body {
                    self.walk_stmt(s);
                }
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
                for arg in args {
                    self.walk_expr(&arg.value);
                }
            }
            HirExpr::MethodCall(obj, _, args, _, _) => {
                self.walk_expr(obj);
                for arg in args {
                    self.walk_expr(&arg.value);
                }
            }
            HirExpr::FieldAccess(obj, _, _) => self.walk_expr(obj),
            HirExpr::Match(obj, arms, _) => {
                self.walk_expr(obj);
                for arm in arms {
                    self.walk_expr(&arm.body);
                }
            }
            HirExpr::If(cond, then_b, else_b, _) => {
                self.walk_expr(cond);
                for s in then_b {
                    self.walk_stmt(s);
                }
                if let Some(eb) = else_b {
                    for s in eb {
                        self.walk_stmt(s);
                    }
                }
            }
            HirExpr::ListLit(elts, _) => {
                for e in elts {
                    self.walk_expr(e);
                }
            }
            HirExpr::TupleLit(elts, _) => {
                for e in elts {
                    self.walk_expr(e);
                }
            }
            HirExpr::Block(body, _) => {
                for s in body {
                    self.walk_stmt(s);
                }
            }
            HirExpr::Index(obj, idx, _) => {
                self.walk_expr(obj);
                self.walk_expr(idx);
            }
            _ => {}
        }
    }

    pub fn is_last_use(&self, name: &str, span: Span) -> bool {
        self.last_use.get(name).is_some_and(|s| *s == span)
    }
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;
    use vox_compiler::hir::lower_module;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse_script;

    fn first_fn_body(src: &str) -> Vec<HirStmt> {
        let module = parse_script(lex(src)).expect("parse");
        let hir = lower_module(&module);
        hir.functions.first().expect("one function").body.clone()
    }

    #[test]
    fn is_last_use_true_for_recorded_span_false_for_other() {
        // Catches: is_last_use() comparing the wrong field or always true/false.
        let body = first_fn_body("fn f(s: str) to Unit { std.print(s)\n std.log(s) }");
        let tracker = UsageTracker::build(&body);
        let recorded = *tracker.last_use.get("s").expect("s tracked");
        assert!(tracker.is_last_use("s", recorded));
        let bogus = Span::new(recorded.start + 1, recorded.end + 1);
        assert!(!tracker.is_last_use("s", bogus));
        assert!(!tracker.is_last_use("nonexistent", recorded));
    }
}
