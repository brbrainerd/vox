use super::value::VoxValue;
use super::{EvalError, Interpreter};
use crate::hir::nodes::{HirExpr, HirPattern, HirStmt};

pub fn eval_pattern(
    interp: &mut Interpreter,
    pattern: &HirPattern,
    value: VoxValue,
) -> Result<(), EvalError> {
    match pattern {
        HirPattern::Ident(name, _) => {
            interp.scope.set(name.clone(), value);
            Ok(())
        }
        HirPattern::Wildcard(_) => Ok(()),
        HirPattern::Tuple(pats, _) => {
            if let VoxValue::Tuple(vals) = value {
                if pats.len() == vals.len() {
                    for (p, v) in pats.iter().zip(vals) {
                        eval_pattern(interp, p, v)?;
                    }
                    Ok(())
                } else {
                    Err(EvalError::TypeError {
                        expected: "Tuple of same length",
                        found: "Tuple".into(),
                    })
                }
            } else {
                Err(EvalError::TypeError {
                    expected: "Tuple",
                    found: "other".into(),
                })
            }
        }
        HirPattern::Constructor(name, args, _) => match value {
            VoxValue::Tagged {
                name: tag_name,
                fields,
            } => {
                if tag_name != *name {
                    return Err(EvalError::AssertionFailed(format!(
                        "Variant mismatch: expected {name}, got {tag_name}"
                    )));
                }
                for (pat, val) in args.iter().zip(fields) {
                    eval_pattern(interp, pat, val)?;
                }
                Ok(())
            }
            VoxValue::Option(opt) => {
                if name == "Some" && args.len() == 1 {
                    if let Some(val) = opt {
                        eval_pattern(interp, &args[0], *val)?;
                        Ok(())
                    } else {
                        Err(EvalError::AssertionFailed("Matched Some on None".into()))
                    }
                } else if name == "None" && args.is_empty() {
                    if opt.is_none() {
                        Ok(())
                    } else {
                        Err(EvalError::AssertionFailed("Matched None on Some".into()))
                    }
                } else {
                    Err(EvalError::AssertionFailed("Variant mismatch".into()))
                }
            }
            VoxValue::Result(res) => {
                if name == "Ok" && args.len() == 1 {
                    if let Ok(val) = res {
                        eval_pattern(interp, &args[0], *val)?;
                        Ok(())
                    } else {
                        Err(EvalError::AssertionFailed("Matched Ok on Err".into()))
                    }
                } else if (name == "Err" || name == "Error") && args.len() == 1 {
                    if let Err(payload) = res {
                        // Payload is now a full VoxValue (two-param Result).
                        // Pattern can be a wildcard, a binding, or a nested
                        // constructor like `Error(TitleEmpty(msg))`.
                        eval_pattern(interp, &args[0], *payload)?;
                        Ok(())
                    } else {
                        Err(EvalError::AssertionFailed("Matched Err on Ok".into()))
                    }
                } else {
                    Err(EvalError::AssertionFailed("Variant mismatch".into()))
                }
            }
            _ => Err(EvalError::AssertionFailed("Not a constructor value".into())),
        },
        HirPattern::Literal(lit_expr, _) => {
            let lit_val = super::expr::eval_expr(interp, lit_expr)?;
            if lit_val == value {
                Ok(())
            } else {
                Err(EvalError::AssertionFailed(
                    "Pattern match literal mismatched".into(),
                ))
            }
        }
    }
}

pub fn eval_stmt(interp: &mut Interpreter, stmt: &HirStmt) -> Result<VoxValue, EvalError> {
    interp.track_step()?;
    match stmt {
        HirStmt::Expr { expr, .. } => super::expr::eval_expr(interp, expr),
        HirStmt::Return { value, .. } => {
            if let Some(val) = value {
                let v = super::expr::eval_expr(interp, val)?;
                Ok(VoxValue::_Return(Box::new(v)))
            } else {
                Ok(VoxValue::_Return(Box::new(VoxValue::Null)))
            }
        }
        HirStmt::Break { .. } => Ok(VoxValue::_Break),
        HirStmt::Continue { .. } => Ok(VoxValue::_Continue),
        HirStmt::Let { pattern, value, .. } => {
            let v = super::expr::eval_expr(interp, value)?;
            eval_pattern(interp, pattern, v)?;
            Ok(VoxValue::Null)
        }
        HirStmt::Assign { target, value, .. } => {
            let v = super::expr::eval_expr(interp, value)?;
            if let HirExpr::Ident(name, _) = target {
                interp.scope.set_mut(name, v);
            }
            Ok(VoxValue::Null)
        }
        HirStmt::While {
            condition, body, ..
        } => {
            loop {
                let c = super::expr::eval_expr(interp, condition)?;
                if let VoxValue::Bool(b) = c {
                    if !b {
                        break;
                    }
                } else {
                    return Err(EvalError::TypeError {
                        expected: "bool",
                        found: "other".into(),
                    });
                }
                interp.scope.push_frame();
                for s in body {
                    let v = eval_stmt(interp, s)?;
                    match v {
                        VoxValue::_Break => {
                            interp.scope.pop_frame();
                            return Ok(VoxValue::Null);
                        }
                        VoxValue::_Continue => break,
                        VoxValue::_Return(r) => {
                            interp.scope.pop_frame();
                            return Ok(VoxValue::_Return(r));
                        }
                        _ => {}
                    }
                }
                interp.scope.pop_frame();
            }
            Ok(VoxValue::Null)
        }
        HirStmt::Loop { body, .. } => loop {
            interp.scope.push_frame();
            for s in body {
                let v = eval_stmt(interp, s)?;
                match v {
                    VoxValue::_Break => {
                        interp.scope.pop_frame();
                        return Ok(VoxValue::Null);
                    }
                    VoxValue::_Continue => break,
                    VoxValue::_Return(r) => {
                        interp.scope.pop_frame();
                        return Ok(VoxValue::_Return(r));
                    }
                    _ => {}
                }
            }
            interp.scope.pop_frame();
        },
    }
}
