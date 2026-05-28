use crate::ast::expr::{BinOp, Expr};
use crate::ast::stmt::Stmt;
use crate::typeck::builtins::BuiltinTypes;
use crate::typeck::ty::Ty;
use crate::typeck::unify::InferenceContext;

/// Lower an AST type to an internal Ty.
fn lower_ast_ty(type_expr: &crate::ast::types::TypeExpr, ctx: &mut InferenceContext) -> Ty {
    use crate::ast::types::TypeExpr;
    match type_expr {
        TypeExpr::Named { name, .. } => match name.as_str() {
            "int" => Ty::Int,
            "float" => Ty::Float,
            "str" => Ty::Str,
            "bool" => Ty::Bool,
            "char" => Ty::Char,
            "never" => Ty::Never,
            "Element" => Ty::Element,
            _ => Ty::Named(name.clone()),
        },
        TypeExpr::Generic { name, args, .. } => {
            if (name == "List" || name == "list") && args.len() == 1 {
                Ty::List(Box::new(lower_ast_ty(&args[0], ctx)))
            } else if name == "Option" && args.len() == 1 {
                Ty::Option(Box::new(lower_ast_ty(&args[0], ctx)))
            } else if name == "Result" && args.len() == 1 {
                Ty::Result(Box::new(lower_ast_ty(&args[0], ctx)))
            } else {
                Ty::Named(name.clone())
            }
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let pts = params.iter().map(|p| lower_ast_ty(p, ctx)).collect();
            let ret = lower_ast_ty(return_type, ctx);
            Ty::Fn(pts, Box::new(ret))
        }
        TypeExpr::Tuple { elements, .. } => {
            Ty::Tuple(elements.iter().map(|e| lower_ast_ty(e, ctx)).collect())
        }
        TypeExpr::Unit { .. } => Ty::Unit,
        TypeExpr::Decimal { .. } => Ty::Decimal,
        TypeExpr::Infer { .. } => ctx.fresh_var(),
    }
}

/// Contextual bidirectional type checking for expressions.
pub fn check_expr(
    expr: &Expr,
    expected: &Ty,
    ctx: &mut InferenceContext,
    builtins: &BuiltinTypes,
) -> Ty {
    let resolved_expected = ctx.resolve(expected);
    match expr {
        Expr::Lambda { params, body, .. } => {
            if let Ty::Fn(exp_params, exp_ret) = resolved_expected {
                let mut param_tys = Vec::new();
                for (i, _param) in params.iter().enumerate() {
                    let param_ty = if i < exp_params.len() {
                        exp_params[i].clone()
                    } else {
                        ctx.fresh_var()
                    };
                    param_tys.push(param_ty);
                }
                let ret_ty = check_expr(body, &exp_ret, ctx, builtins);
                return Ty::Fn(param_tys, Box::new(ret_ty));
            }
        }
        Expr::ListLit { elements, .. } => {
            if let Ty::List(inner_ty) = resolved_expected {
                if elements.is_empty() {
                    return Ty::List(inner_ty);
                }
                let mut elem_ty = *inner_ty.clone();
                for elem in elements {
                    elem_ty = check_expr(elem, &elem_ty, ctx, builtins);
                }
                return Ty::List(Box::new(elem_ty));
            }
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            check_expr(condition, &Ty::Bool, ctx, builtins);
            for stmt in then_body {
                check_stmt(stmt, expected, ctx, builtins);
            }
            if let Some(eb) = else_body {
                for stmt in eb {
                    check_stmt(stmt, expected, ctx, builtins);
                }
            }
            return expected.clone();
        }
        Expr::Match { subject, arms, .. } => {
            let _subj_ty = infer_expr(subject, ctx, builtins);
            for arm in arms {
                check_expr(&arm.body, expected, ctx, builtins);
            }
            return expected.clone();
        }
        Expr::Block { stmts, .. } => {
            let mut last_ty = Ty::Unit;
            if stmts.is_empty() {
                let _ = ctx.unify(&Ty::Unit, expected);
                return Ty::Unit;
            }
            for (i, stmt) in stmts.iter().enumerate() {
                if i == stmts.len() - 1 {
                    last_ty = check_stmt(stmt, expected, ctx, builtins);
                } else {
                    infer_stmt(stmt, ctx, builtins);
                }
            }
            return last_ty;
        }
        Expr::TupleLit { elements, .. } => {
            if let Ty::Tuple(exp_elems) = resolved_expected {
                if elements.len() == exp_elems.len() {
                    let mut tys = Vec::new();
                    for (elem, exp) in elements.iter().zip(exp_elems.iter()) {
                        tys.push(check_expr(elem, exp, ctx, builtins));
                    }
                    return Ty::Tuple(tys);
                }
            }
        }
        _ => {}
    }

    // Fallback to infer
    let ty = infer_expr(expr, ctx, builtins);
    let _ = ctx.unify(&ty, expected);
    ty
}

/// Contextual bidirectional type checking for statements.
pub fn check_stmt(
    stmt: &Stmt,
    expected: &Ty,
    ctx: &mut InferenceContext,
    builtins: &BuiltinTypes,
) -> Ty {
    match stmt {
        Stmt::Expr { expr, .. } => check_expr(expr, expected, ctx, builtins),
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                let target = ctx
                    .expected_return_ty
                    .clone()
                    .unwrap_or_else(|| ctx.fresh_var());
                check_expr(v, &target, ctx, builtins);
            } else {
                let target = ctx
                    .expected_return_ty
                    .clone()
                    .unwrap_or_else(|| ctx.fresh_var());
                let _ = ctx.unify(&Ty::Unit, &target);
            }
            Ty::Never
        }
        Stmt::Let {
            value, type_ann, ..
        } => {
            if let Some(ann) = type_ann {
                let exp = lower_ast_ty(ann, ctx);
                check_expr(value, &exp, ctx, builtins);
            } else {
                infer_expr(value, ctx, builtins);
            }
            let _ = ctx.unify(&Ty::Unit, expected);
            Ty::Unit
        }
        _ => {
            let ty = infer_stmt(stmt, ctx, builtins);
            let _ = ctx.unify(&ty, expected);
            ty
        }
    }
}

/// Infer the type of an expression.
pub fn infer_expr(expr: &Expr, ctx: &mut InferenceContext, builtins: &BuiltinTypes) -> Ty {
    match expr {
        Expr::IntLit { .. } => Ty::Int,
        Expr::FloatLit { .. } => Ty::Float,
        Expr::StringLit { .. } => Ty::Str,
        Expr::BoolLit { .. } => Ty::Bool,
        Expr::Ident { name, .. } => builtins.lookup_var(name).unwrap_or_else(|| ctx.fresh_var()),
        Expr::ListLit { elements, .. } => {
            if elements.is_empty() {
                Ty::List(Box::new(ctx.fresh_var()))
            } else {
                let elem_ty = infer_expr(&elements[0], ctx, builtins);
                // Enforce coherence across list elements representing LUB
                for elem in elements.iter().skip(1) {
                    let next_ty = infer_expr(elem, ctx, builtins);
                    let _ = ctx.unify(&elem_ty, &next_ty);
                }
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
        Expr::Binary {
            op, left, right, ..
        } => {
            let _left_ty = infer_expr(left, ctx, builtins);
            let _right_ty = infer_expr(right, ctx, builtins);
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => _left_ty,
                BinOp::Lt
                | BinOp::Gt
                | BinOp::Lte
                | BinOp::Gte
                | BinOp::Is
                | BinOp::Isnt
                | BinOp::And
                | BinOp::Or => Ty::Bool,
                BinOp::Pipe => _right_ty,
            }
        }
        Expr::Unary { op: _, operand, .. } => infer_expr(operand, ctx, builtins),
        Expr::Call { callee, args, .. } => {
            let callee_ty = infer_expr(callee, ctx, builtins);
            let callee_resolved = ctx.resolve(&callee_ty);
            match callee_resolved {
                Ty::Fn(params, ret) => {
                    for (i, arg) in args.iter().enumerate() {
                        if i < params.len() {
                            check_expr(&arg.value, &params[i], ctx, builtins);
                        } else {
                            infer_expr(&arg.value, ctx, builtins);
                        }
                    }
                    *ret
                }
                _ => {
                    for arg in args {
                        infer_expr(&arg.value, ctx, builtins);
                    }
                    ctx.fresh_var()
                }
            }
        }
        Expr::MethodCall {
            object,
            method,
            args,
            span,
            ..
        } => {
            let obj_ty = infer_expr(object, ctx, builtins);
            let method_ty = builtins.lookup_method(&obj_ty, method).unwrap_or_else(|| {
                if let Ty::TypeVar(_) = ctx.resolve(&obj_ty) {
                    let ret_var = ctx.fresh_var();
                    ctx.pending_constraints.push(
                        crate::typeck::unify::PendingConstraint::HasMethod {
                            target: obj_ty.clone(),
                            method: method.clone(),
                            result: ret_var.clone(),
                            args: vec![], // we can refine to collect args types later if needed
                            span: *span,
                        },
                    );
                    ret_var
                } else {
                    ctx.fresh_var()
                }
            });

            let method_ty_instantiated = ctx.instantiate(&method_ty);
            match (ctx.resolve(&obj_ty), ctx.resolve(&method_ty_instantiated)) {
                (Ty::List(obj_inner), Ty::Fn(_params, ref ret)) => {
                    if let Ty::List(ret_inner) = &**ret {
                        let _ = ctx.unify(obj_inner.as_ref(), ret_inner.as_ref());
                    }
                }
                (Ty::Option(obj_inner), Ty::Fn(_params, ref ret)) => {
                    if let Ty::Option(ret_inner) = &**ret {
                        let _ = ctx.unify(obj_inner.as_ref(), ret_inner.as_ref());
                    }
                }
                (Ty::Result(obj_inner), Ty::Fn(_params, ref ret)) => {
                    if let Ty::Result(ret_inner) = &**ret {
                        let _ = ctx.unify(obj_inner.as_ref(), ret_inner.as_ref());
                    }
                }
                _ => {}
            }

            let method_resolved = ctx.resolve(&method_ty_instantiated);
            match method_resolved {
                Ty::Fn(params, ret) => {
                    for (i, arg) in args.iter().enumerate() {
                        if i < params.len() {
                            check_expr(&arg.value, &params[i], ctx, builtins);
                        } else {
                            infer_expr(&arg.value, ctx, builtins);
                        }
                    }
                    *ret
                }
                _ => {
                    for arg in args {
                        infer_expr(&arg.value, ctx, builtins);
                    }
                    method_ty_instantiated
                }
            }
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
        Expr::Try { target, .. } => {
            let operand_ty = infer_expr(target, ctx, builtins);
            match ctx.resolve(&operand_ty) {
                Ty::Result(inner) | Ty::Option(inner) => (*inner).clone(),
                _ => ctx.fresh_var(),
            }
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
        Expr::Jsx(_) | Expr::JsxSelfClosing(_) | Expr::JsxFragment { .. } => Ty::Element,
        Expr::StringInterp { .. } => Ty::Str,
        Expr::Block { stmts, .. } => {
            let mut last_ty = Ty::Unit;
            for stmt in stmts {
                last_ty = infer_stmt(stmt, ctx, builtins);
            }
            last_ty
        }
        Expr::TupleLit { elements, .. } => Ty::Tuple(
            elements
                .iter()
                .map(|e| infer_expr(e, ctx, builtins))
                .collect(),
        ),
        Expr::With {
            operand, options, ..
        } => {
            let op_ty = infer_expr(operand, ctx, builtins);
            let _opt_ty = infer_expr(options, ctx, builtins);
            op_ty
        }
        Expr::DecimalLit { .. } => Ty::Decimal,
        Expr::Index { object, index, .. } => {
            let _ = infer_expr(object, ctx, builtins);
            let _ = infer_expr(index, ctx, builtins);
            ctx.fresh_var()
        }
        // side_effect { … } desugars to a call at HIR time; infer as a fresh type var.
        Expr::SideEffect { stmts, .. } => {
            for s in stmts {
                infer_stmt(s, ctx, builtins);
            }
            ctx.fresh_var()
        }
        // workflow.version is a patch-marker primitive; infer as unit.
        Expr::WorkflowVersion(_) => Ty::Unit,
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
        Stmt::While {
            condition, body, ..
        } => {
            infer_expr(condition, ctx, builtins);
            for s in body {
                infer_stmt(s, ctx, builtins);
            }
            Ty::Unit
        }
        Stmt::Loop { body, .. } => {
            for s in body {
                infer_stmt(s, ctx, builtins);
            }
            Ty::Never
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => Ty::Never,
    }
}
