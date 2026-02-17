use crate::ty::Ty;
use crate::unify::InferenceContext;
use vox_ast::expr::{Expr, BinOp};
use vox_ast::stmt::Stmt;
use crate::builtins::BuiltinTypes;

/// Infer the type of an expression.
pub fn infer_expr(expr: &Expr, ctx: &mut InferenceContext, builtins: &BuiltinTypes) -> Ty {
    match expr {
        Expr::IntLit { .. } => Ty::Int,
        Expr::FloatLit { .. } => Ty::Float,
        Expr::StringLit { .. } => Ty::Str,
        Expr::BoolLit { .. } => Ty::Bool,
        Expr::Ident { name, .. } => {
            builtins.lookup_var(name).unwrap_or_else(|| ctx.fresh_var())
        }
        Expr::ListLit { elements, .. } => {
            if elements.is_empty() {
                Ty::List(Box::new(ctx.fresh_var()))
            } else {
                let elem_ty = infer_expr(&elements[0], ctx, builtins);
                Ty::List(Box::new(elem_ty))
            }
        }
        Expr::ObjectLit { fields, .. } => {
            let field_types: Vec<(String, Ty)> = fields
                .iter()
                .map(|(name, expr)| (name.clone(), infer_expr(expr, ctx, builtins)))
                .collect();
            Ty::Record(field_types)
        }
        Expr::Binary { op, left, right, .. } => {
            let _left_ty = infer_expr(left, ctx, builtins);
            let _right_ty = infer_expr(right, ctx, builtins);
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => _left_ty,
                BinOp::Lt | BinOp::Gt | BinOp::Lte | BinOp::Gte
                | BinOp::Is | BinOp::Isnt | BinOp::And | BinOp::Or => Ty::Bool,
                BinOp::Pipe => _right_ty,
            }
        }
        Expr::Unary { op: _, operand, .. } => infer_expr(operand, ctx, builtins),
        Expr::Call { callee, args, .. } => {
            let callee_ty = infer_expr(callee, ctx, builtins);
            // Infer args for side effects
            for arg in args {
                infer_expr(&arg.value, ctx, builtins);
            }
            match callee_ty {
                Ty::Fn(_, ret) => *ret,
                _ => ctx.fresh_var(),
            }
        }
        Expr::MethodCall { object, method, args, .. } => {
            let obj_ty = infer_expr(object, ctx, builtins);
            for arg in args {
                infer_expr(&arg.value, ctx, builtins);
            }
            builtins.lookup_method(&obj_ty, method).unwrap_or_else(|| ctx.fresh_var())
        }
        Expr::FieldAccess { object, .. } => {
            infer_expr(object, ctx, builtins);
            ctx.fresh_var()
        }
        Expr::Match { subject, arms, .. } => {
            infer_expr(subject, ctx, builtins);
            if let Some(first_arm) = arms.first() {
                infer_expr(&first_arm.body, ctx, builtins)
            } else {
                Ty::Unit
            }
        }
        Expr::If { condition, .. } => {
            infer_expr(condition, ctx, builtins);
            Ty::Unit
        }
        Expr::For { iterable, body, .. } => {
            infer_expr(iterable, ctx, builtins);
            infer_expr(body, ctx, builtins)
        }
        Expr::Lambda { params, .. } => {
            let param_tys: Vec<Ty> = params.iter().map(|_| ctx.fresh_var()).collect();
            let ret_ty = ctx.fresh_var();
            Ty::Fn(param_tys, Box::new(ret_ty))
        }
        Expr::Pipe { left, right, .. } => {
            infer_expr(left, ctx, builtins);
            infer_expr(right, ctx, builtins)
        }
        Expr::Spawn { .. } => ctx.fresh_var(),
        Expr::Jsx(_) | Expr::JsxSelfClosing(_) => Ty::Element,
        Expr::StringInterp { .. } => Ty::Str,
        Expr::Block { stmts, .. } => {
            let mut last_ty = Ty::Unit;
            for stmt in stmts {
                last_ty = infer_stmt(stmt, ctx, builtins);
            }
            last_ty
        }
        Expr::TupleLit { elements, .. } => {
            Ty::Tuple(elements.iter().map(|e| infer_expr(e, ctx, builtins)).collect())
        }
        Expr::With { operand, options, .. } => {
            let op_ty = infer_expr(operand, ctx, builtins);
            let _opt_ty = infer_expr(options, ctx, builtins);
            op_ty
        }
    }
}

/// Infer the type produced by a statement.
pub fn infer_stmt(stmt: &Stmt, ctx: &mut InferenceContext, builtins: &BuiltinTypes) -> Ty {
    match stmt {
        Stmt::Let { value, .. } => {
            infer_expr(value, ctx, builtins);
            Ty::Unit
        }
        Stmt::Assign { value, .. } => {
            infer_expr(value, ctx, builtins);
            Ty::Unit
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                infer_expr(v, ctx, builtins)
            } else {
                Ty::Unit
            }
        }
        Stmt::Expr { expr, .. } => infer_expr(expr, ctx, builtins),
    }
}
