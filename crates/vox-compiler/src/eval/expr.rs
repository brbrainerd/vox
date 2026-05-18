use super::value::VoxValue;
use super::{EvalError, Interpreter};
use crate::hir::nodes::{HirBinOp, HirExpr, HirUnOp};

pub fn eval_expr(interp: &mut Interpreter, expr: &HirExpr) -> Result<VoxValue, EvalError> {
    interp.track_step()?;
    match expr {
        HirExpr::IntLit(value, _) => Ok(VoxValue::Int(*value)),
        HirExpr::FloatLit(value, _) => Ok(VoxValue::Float(*value)),
        HirExpr::StringLit(value, _) => Ok(VoxValue::Str(value.clone())),
        HirExpr::BoolLit(value, _) => Ok(VoxValue::Bool(*value)),
        HirExpr::Ident(name, _) => {
            if let Some(val) = interp.scope.get(name) {
                Ok(val.clone())
            } else if matches!(
                name.as_str(),
                "print" | "range" | "str" | "int" | "float" | "len" | "assert"
            ) {
                // Return a placeholder function for builtins
                Ok(VoxValue::Fn {
                    params: vec!["args".into()],
                    body: vec![], // Not used for builtins
                    env: interp.scope.clone(),
                })
            } else if name == "None" {
                // Built-in Option::None — referenced as a bare ident, not called.
                Ok(VoxValue::Option(None))
            } else if name == "Unit" {
                // Built-in unit value, used as the payload of Ok(Unit) / Result[Unit].
                Ok(VoxValue::Null)
            } else if matches!(name.as_str(), "Some" | "Ok" | "Err" | "Error") {
                // Built-in Option/Result constructors. Returned as a Constructor
                // value so the Call arm below can specialize them into typed
                // Option/Result values.
                Ok(VoxValue::Constructor(name.clone()))
            } else {
                Err(EvalError::UndefinedVariable(name.clone()))
            }
        }
        HirExpr::ListLit(elems, _) => {
            let mut list = Vec::new();
            for e in elems {
                list.push(eval_expr(interp, e)?);
            }
            Ok(VoxValue::List(list))
        }
        HirExpr::ObjectLit(fields, _) => {
            let mut obj = Vec::new();
            for (k, v) in fields {
                obj.push((k.clone(), eval_expr(interp, v)?));
            }
            Ok(VoxValue::Object(obj))
        }
        HirExpr::Block(stmts, _) => {
            interp.scope.push_frame();
            let mut val = VoxValue::Null;
            for stmt in stmts {
                val = super::stmt::eval_stmt(interp, stmt)?;
                if matches!(
                    val,
                    VoxValue::_Return(_) | VoxValue::_Break | VoxValue::_Continue
                ) {
                    break;
                }
            }
            interp.scope.pop_frame();
            Ok(val)
        }
        HirExpr::Binary(op, left, right, _) => {
            let l = eval_expr(interp, left)?;
            if *op == HirBinOp::And {
                if let VoxValue::Bool(false) = l {
                    return Ok(VoxValue::Bool(false));
                }
                return eval_expr(interp, right);
            }
            if *op == HirBinOp::Or {
                if let VoxValue::Bool(true) = l {
                    return Ok(VoxValue::Bool(true));
                }
                return eval_expr(interp, right);
            }
            let r = eval_expr(interp, right)?;
            match (op, l, r) {
                (HirBinOp::Add, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Int(a + b)),
                (HirBinOp::Sub, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Int(a - b)),
                (HirBinOp::Mul, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Int(a * b)),
                (HirBinOp::Div, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Int(a / b)),
                (HirBinOp::Mod, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Int(a % b)),
                (HirBinOp::Is, a, b) => Ok(VoxValue::Bool(a == b)),
                (HirBinOp::Isnt, a, b) => Ok(VoxValue::Bool(a != b)),
                (HirBinOp::Lt, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Bool(a < b)),
                (HirBinOp::Gt, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Bool(a > b)),
                (HirBinOp::Lte, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Bool(a <= b)),
                (HirBinOp::Gte, VoxValue::Int(a), VoxValue::Int(b)) => Ok(VoxValue::Bool(a >= b)),
                (HirBinOp::Add, VoxValue::Str(a), other) => Ok(VoxValue::Str(format!(
                    "{}{}",
                    a,
                    super::builtins::vox_value_display(&other)
                ))),
                (HirBinOp::Add, other, VoxValue::Str(b)) => Ok(VoxValue::Str(format!(
                    "{}{}",
                    super::builtins::vox_value_display(&other),
                    b
                ))),
                (HirBinOp::Add, VoxValue::Float(a), VoxValue::Float(b)) => {
                    Ok(VoxValue::Float(a + b))
                }
                (HirBinOp::Sub, VoxValue::Float(a), VoxValue::Float(b)) => {
                    Ok(VoxValue::Float(a - b))
                }
                (HirBinOp::Mul, VoxValue::Float(a), VoxValue::Float(b)) => {
                    Ok(VoxValue::Float(a * b))
                }
                (HirBinOp::Div, VoxValue::Float(a), VoxValue::Float(b)) => {
                    Ok(VoxValue::Float(a / b))
                }
                (HirBinOp::Lt, VoxValue::Float(a), VoxValue::Float(b)) => Ok(VoxValue::Bool(a < b)),
                (HirBinOp::Gt, VoxValue::Float(a), VoxValue::Float(b)) => Ok(VoxValue::Bool(a > b)),
                (HirBinOp::Lte, VoxValue::Float(a), VoxValue::Float(b)) => {
                    Ok(VoxValue::Bool(a <= b))
                }
                (HirBinOp::Gte, VoxValue::Float(a), VoxValue::Float(b)) => {
                    Ok(VoxValue::Bool(a >= b))
                }
                _ => Ok(VoxValue::Null),
            }
        }
        HirExpr::Unary(op, inner, _) => {
            let v = eval_expr(interp, inner)?;
            match (op, v) {
                (HirUnOp::Not, VoxValue::Bool(b)) => Ok(VoxValue::Bool(!b)),
                (HirUnOp::Neg, VoxValue::Int(n)) => Ok(VoxValue::Int(-n)),
                (HirUnOp::Neg, VoxValue::Float(f)) => Ok(VoxValue::Float(-f)),
                _ => Ok(VoxValue::Null),
            }
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            let c = eval_expr(interp, cond)?;
            let b = match c {
                VoxValue::Bool(b) => b,
                _ => {
                    return Err(EvalError::TypeError {
                        expected: "bool",
                        found: "other".into(),
                    });
                }
            };
            if b {
                interp.scope.push_frame();
                let mut val = VoxValue::Null;
                for stmt in then_b {
                    val = super::stmt::eval_stmt(interp, stmt)?;
                    if matches!(
                        val,
                        VoxValue::_Return(_) | VoxValue::_Break | VoxValue::_Continue
                    ) {
                        break;
                    }
                }
                interp.scope.pop_frame();
                Ok(val)
            } else if let Some(el_b) = else_b {
                interp.scope.push_frame();
                let mut val = VoxValue::Null;
                for stmt in el_b {
                    val = super::stmt::eval_stmt(interp, stmt)?;
                    if matches!(
                        val,
                        VoxValue::_Return(_) | VoxValue::_Break | VoxValue::_Continue
                    ) {
                        break;
                    }
                }
                interp.scope.pop_frame();
                Ok(val)
            } else {
                Ok(VoxValue::Null)
            }
        }
        HirExpr::Lambda(params, _, body, _, _) => {
            let b = vec![crate::hir::nodes::HirStmt::Expr {
                expr: *body.clone(),
                span: crate::ast::span::Span::new(0, 0),
            }];
            Ok(VoxValue::Fn {
                params: params.iter().map(|p| p.name.clone()).collect(),
                body: b,
                env: interp.scope.clone(),
            })
        }
        HirExpr::Call(callee, args, _, _) => {
            let mut eval_args = Vec::new();
            for a in args {
                eval_args.push(eval_expr(interp, &a.value)?);
            }
            // Try global built-in first when callee is a bare identifier
            if let HirExpr::Ident(name, _) = callee.as_ref()
                && interp.scope.get(name).is_none()
            {
                if let Some(result) = super::builtins::call_global_builtin(name, eval_args.clone())
                {
                    return Ok(result);
                } else if matches!(name.as_str(), "assert") {
                    // assert returning None means failure
                    return Err(EvalError::AssertionFailed("assert failed".to_string()));
                }
            }
            let c = eval_expr(interp, callee)?;
            match c {
                VoxValue::Fn {
                    params,
                    body,
                    mut env,
                } => {
                    env.push_frame();
                    for (p, arg) in params.iter().zip(eval_args) {
                        env.set(p.clone(), arg);
                    }

                    let old_scope = interp.scope.clone();
                    interp.scope = env;

                    let mut val = VoxValue::Null;
                    for stmt in body {
                        val = super::stmt::eval_stmt(interp, &stmt)?;
                        if let VoxValue::_Return(v) = val {
                            val = *v;
                            break;
                        }
                        if matches!(val, VoxValue::_Break | VoxValue::_Continue) {
                            break;
                        }
                    }

                    interp.scope = old_scope;
                    Ok(val)
                }
                VoxValue::Constructor(name) => {
                    // Specialize built-in Option / Result constructors so the
                    // pattern-match path in stmt.rs (which already handles
                    // VoxValue::Option / VoxValue::Result) sees them. Other
                    // (user-declared) variants fall through to Tagged.
                    match (name.as_str(), eval_args.len()) {
                        ("Some", 1) => {
                            let v = eval_args.into_iter().next().unwrap();
                            Ok(VoxValue::Option(Some(Box::new(v))))
                        }
                        ("Ok", 1) => {
                            let v = eval_args.into_iter().next().unwrap();
                            Ok(VoxValue::Result(Ok(Box::new(v))))
                        }
                        ("Err", 1) | ("Error", 1) => {
                            let v = eval_args.into_iter().next().unwrap();
                            // Result's Err side stores a String per VoxValue;
                            // coerce strings directly, render anything else
                            // via the builtins display helper.
                            let msg = match v {
                                VoxValue::Str(s) => s,
                                other => super::builtins::vox_value_display(&other),
                            };
                            Ok(VoxValue::Result(Err(msg)))
                        }
                        _ => Ok(VoxValue::Tagged {
                            name,
                            fields: eval_args,
                        }),
                    }
                }
                _ => Err(EvalError::TypeError {
                    expected: "function",
                    found: "other".into(),
                }),
            }
        }
        HirExpr::MethodCall(obj, method, args, _, _) => {
            let o = eval_expr(interp, obj)?;
            let mut eval_args = Vec::new();
            for a in args {
                eval_args.push(eval_expr(interp, &a.value)?);
            }
            if let Some(r) =
                super::builtins::call_builtin_method(&o, method, eval_args, interp.caps.as_ref())
            {
                Ok(r)
            } else {
                Err(EvalError::AssertionFailed(format!(
                    "Method {} not found",
                    method
                )))
            }
        }
        HirExpr::Match(subject, arms, _) => {
            let s = eval_expr(interp, subject)?;
            for arm in arms {
                interp.scope.push_frame();
                if super::stmt::eval_pattern(interp, &arm.pattern, s.clone()).is_ok() {
                    let mut is_match = true;
                    if let Some(guard) = &arm.guard {
                        if let Ok(VoxValue::Bool(b)) = eval_expr(interp, guard) {
                            is_match = b;
                        } else {
                            is_match = false;
                        }
                    }
                    if is_match {
                        let res = eval_expr(interp, &arm.body);
                        interp.scope.pop_frame();
                        return res;
                    }
                }
                interp.scope.pop_frame();
            }
            Err(EvalError::AssertionFailed("No match arm found".into()))
        }
        HirExpr::For(binding, index, iterable, body, _, _) => {
            let c = eval_expr(interp, iterable)?;
            let mut results = Vec::new();
            if let VoxValue::List(ls) = c {
                interp.scope.push_frame();
                for (i, l) in ls.into_iter().enumerate() {
                    interp.scope.set(binding.clone(), l);
                    if let Some(idx_name) = index {
                        interp.scope.set(idx_name.clone(), VoxValue::Int(i as i64));
                    }
                    results.push(eval_expr(interp, body)?);
                }
                interp.scope.pop_frame();
                Ok(VoxValue::List(results))
            } else {
                Err(EvalError::TypeError {
                    expected: "List",
                    found: "other".into(),
                })
            }
        }
        HirExpr::FieldAccess(obj, field, _) => {
            let o = eval_expr(interp, obj)?;
            if let VoxValue::Object(fields) = o {
                fields
                    .iter()
                    .find(|(k, _)| k == field)
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| {
                        EvalError::AssertionFailed(format!("Field {} not found on object", field))
                    })
            } else {
                Err(EvalError::TypeError {
                    expected: "Object",
                    found: "other".into(),
                })
            }
        }
        HirExpr::Index(object, index, _) => {
            let obj_val = eval_expr(interp, object)?;
            let idx_val = eval_expr(interp, index)?;
            match (obj_val, idx_val) {
                (VoxValue::List(items), VoxValue::Int(i)) => {
                    let idx = if i < 0 {
                        items.len().saturating_sub((-i) as usize)
                    } else {
                        i as usize
                    };
                    Ok(items.into_iter().nth(idx).unwrap_or(VoxValue::Null))
                }
                _ => Ok(VoxValue::Null),
            }
        }
        _ => Ok(VoxValue::Null),
    }
}
