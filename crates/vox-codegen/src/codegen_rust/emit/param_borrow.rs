//! Conservative parameter borrow inference (Workstream B, escape analysis).
//!
//! Decides which user-function parameters MAY be emitted as a borrow
//! (`&str` / `&[T]`) instead of an owned `String` / `Vec<T>`, eliminating
//! argument clones in generated Rust.
//!
//! ## Soundness rule (intentionally conservative)
//!
//! A parameter is **borrowable** iff *every* occurrence of it in the function
//! body is a **direct identifier argument of a call** — `f(p)` / `obj.m(p)`.
//! That is the only position where we can hand out a shared reference without
//! the body needing to own the value. ANY other use disqualifies it:
//! returning it, using it in an operator (`p + x`), binding it (`let q = p`),
//! indexing through it, putting it in a list/tuple, mutating it, or using it as
//! a method *receiver* (`p.foo()`, which may consume `p`).
//!
//! This rejects many legitimately-borrowable params, but it is **sound**: it
//! never marks a parameter borrowable when the body needs ownership. The
//! emission step layers an additional gate (the callee must actually borrow the
//! argument) on top of this set, so a `true` here is a *necessary*, not
//! *sufficient*, condition for emitting a borrow.
//!
//! NOTE: consumed only by tests for now; the signature/call-site emission step
//! (next slice-2 increment) wires it into codegen. The module-level
//! `allow(dead_code)` is removed at that point.
#![allow(dead_code)]

use std::collections::HashSet;
use vox_compiler::hir::{HirExpr, HirParam, HirStmt};

/// Returns the subset of `params` whose every body use is a direct call-arg
/// identifier (see module docs). A parameter that is never used at all is
/// included (borrowing or owning it is equivalent — no clone either way).
pub fn borrowable_params(params: &[HirParam], body: &[HirStmt]) -> HashSet<String> {
    let names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    if names.is_empty() {
        return names;
    }
    let mut w = Walker {
        params: &names,
        disqualified: HashSet::new(),
    };
    for s in body {
        w.stmt(s);
    }
    // Destructure to release the `&names` borrow held by `w.params` before we
    // consume `names`.
    let Walker { disqualified, .. } = w;
    names
        .into_iter()
        .filter(|n| !disqualified.contains(n))
        .collect()
}

struct Walker<'a> {
    params: &'a HashSet<String>,
    disqualified: HashSet<String>,
}

impl Walker<'_> {
    /// Mark a param name as needing ownership (not borrowable).
    fn disqualify_if_param(&mut self, name: &str) {
        if self.params.contains(name) {
            self.disqualified.insert(name.to_string());
        }
    }

    /// Conservatively disqualify every parameter. Used for expression shapes we
    /// don't traverse precisely: if the body contains one, we cannot prove a
    /// param isn't used in an owning position inside it, so none are borrowable.
    /// Over-conservative but never unsound.
    fn disqualify_all(&mut self) {
        for n in self.params.iter() {
            self.disqualified.insert(n.clone());
        }
    }

    fn stmt(&mut self, s: &HirStmt) {
        match s {
            // Every statement context is an "owning" context for a bare ident —
            // only call-arg positions (handled in `arg`) are borrow-safe.
            HirStmt::Let { value, .. } => self.expr_owning(value),
            HirStmt::Assign { target, value, .. } => {
                self.expr_owning(target);
                self.expr_owning(value);
            }
            HirStmt::Return { value: Some(v), .. } => self.expr_owning(v),
            HirStmt::Expr { expr, .. } => self.expr_owning(expr),
            HirStmt::While {
                condition, body, ..
            } => {
                self.expr_owning(condition);
                for s in body {
                    self.stmt(s);
                }
            }
            HirStmt::Loop { body, .. } => {
                for s in body {
                    self.stmt(s);
                }
            }
            _ => {}
        }
    }

    /// Walk an expression in an *owning* context: a bare param identifier here
    /// disqualifies the param. Call arguments are the one borrow-safe exception.
    fn expr_owning(&mut self, e: &HirExpr) {
        match e {
            HirExpr::Ident(name, _) => self.disqualify_if_param(name),
            HirExpr::Call(callee, args, _, _) => {
                self.expr_owning(callee);
                for a in args {
                    self.arg(&a.value);
                }
            }
            HirExpr::MethodCall(obj, _, args, _, _) => {
                // The receiver may be consumed → owning context.
                self.expr_owning(obj);
                for a in args {
                    self.arg(&a.value);
                }
            }
            HirExpr::Binary(_, l, r, _) => {
                self.expr_owning(l);
                self.expr_owning(r);
            }
            HirExpr::Unary(_, e, _) => self.expr_owning(e),
            HirExpr::FieldAccess(obj, _, _) => self.expr_owning(obj),
            HirExpr::Match(obj, arms, _) => {
                self.expr_owning(obj);
                for arm in arms {
                    self.expr_owning(&arm.body);
                }
            }
            HirExpr::If(cond, then_b, else_b, _) => {
                self.expr_owning(cond);
                for s in then_b {
                    self.stmt(s);
                }
                if let Some(eb) = else_b {
                    for s in eb {
                        self.stmt(s);
                    }
                }
            }
            HirExpr::ListLit(elts, _) | HirExpr::TupleLit(elts, _) => {
                for e in elts {
                    self.expr_owning(e);
                }
            }
            HirExpr::ObjectLit(fields, _) => {
                for (_, v) in fields {
                    self.expr_owning(v);
                }
            }
            HirExpr::Block(body, _) => {
                for s in body {
                    self.stmt(s);
                }
            }
            HirExpr::Index(obj, idx, _) => {
                self.expr_owning(obj);
                self.expr_owning(idx);
            }
            HirExpr::Lambda(_, _, body, _, _) => self.expr_owning(body),
            HirExpr::For(_, _, iter, body, key, _) => {
                self.expr_owning(iter);
                self.expr_owning(body);
                if let Some(k) = key {
                    self.expr_owning(k);
                }
            }
            // Literals contain no identifiers — safe no-ops.
            HirExpr::IntLit(..)
            | HirExpr::FloatLit(..)
            | HirExpr::StringLit(..)
            | HirExpr::BoolLit(..)
            | HirExpr::DecimalLit(..) => {}
            // Any other shape (Try `?`, Spawn, With, Jsx*, AsyncView,
            // WorkflowVersion, …) is not traversed precisely → conservatively
            // disqualify so a hidden owning use can never be missed.
            _ => self.disqualify_all(),
        }
    }

    /// Walk a call argument. A bare identifier argument is borrow-safe (does NOT
    /// disqualify); any compound argument is walked in an owning context.
    fn arg(&mut self, e: &HirExpr) {
        match e {
            HirExpr::Ident(_, _) => { /* direct call-arg ident: borrow-safe */ }
            other => self.expr_owning(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::borrowable_params;
    use vox_compiler::hir::lower_module;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse_script;

    /// Parse + lower a single-function program and run inference on its first
    /// function (HIR).
    fn borrowable(src: &str) -> Vec<String> {
        let module = parse_script(lex(src)).expect("parse");
        let hir = lower_module(&module);
        let f = hir.functions.first().expect("one function");
        let mut v: Vec<String> = borrowable_params(&f.params, &f.body).into_iter().collect();
        v.sort();
        v
    }

    #[test]
    fn param_only_passed_to_call_is_borrowable() {
        // `s` is only a direct call argument → borrowable.
        assert_eq!(
            borrowable("fn f(s: str) to Unit { std.print(s) }"),
            vec!["s"]
        );
    }

    #[test]
    fn returned_param_is_not_borrowable() {
        assert!(borrowable("fn f(s: str) to str { return s }").is_empty());
    }

    #[test]
    fn param_inside_object_literal_is_not_borrowable() {
        // `{ k: s }` is a compound call-arg → walked as owning → `s` disqualified.
        assert!(borrowable("fn f(s: str) to Unit { std.print({ k: s }) }").is_empty());
    }

    #[test]
    fn param_in_operator_is_not_borrowable() {
        assert!(borrowable("fn f(s: str) to str { return s + \"!\" }").is_empty());
    }

    #[test]
    fn param_bound_in_let_is_not_borrowable() {
        assert!(borrowable("fn f(s: str) to Unit { let q = s\n std.print(q) }").is_empty());
    }

    #[test]
    fn param_used_in_compound_call_arg_is_not_borrowable() {
        // `s + "x"` is a compound arg → owning context → not borrowable.
        assert!(borrowable("fn f(s: str) to Unit { std.print(s + \"x\") }").is_empty());
    }

    #[test]
    fn unused_param_is_borrowable() {
        assert_eq!(borrowable("fn f(s: str) to int { return 1 }"), vec!["s"]);
    }

    #[test]
    fn mixed_params_classified_independently() {
        // `a` only printed (borrowable); `b` returned (owned).
        let r = borrowable("fn f(a: str, b: str) to str { std.print(a)\n return b }");
        assert_eq!(r, vec!["a"]);
    }
}
