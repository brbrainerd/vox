use super::value::VoxValue;
use super::{EvalError, Interpreter};
use crate::hir::nodes::{HirExpr, HirPattern, HirStmt};

/// For `pop()` calls on a named list variable, shrink the list variable by one.
///
/// `pop()` is the only built-in method whose return value is not the mutated
/// receiver — it returns the *popped element* instead of the shorter list.
/// The generic "reassign if same runtime kind" heuristic used for `push`,
/// `reverse`, etc. never fires for `pop`, so we handle it explicitly here.
///
/// Must be called **after** the enclosing expression has been evaluated so
/// that `eval_expr` has already read the original (un-popped) list.
fn apply_pop_side_effect(interp: &mut Interpreter, expr: &HirExpr) {
    if let HirExpr::MethodCall(obj_expr, method_name, _, _, _) = expr
        && method_name == "pop"
        && let HirExpr::Ident(name, _) = obj_expr.as_ref()
        && let Some(VoxValue::List(items)) = interp.scope.get_mut(name)
    {
        // CoW in place: O(1) when the binding solely owns the list.
        std::rc::Rc::make_mut(items).pop();
    }
}

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
                    for (p, v) in pats.iter().zip(vals.iter().cloned()) {
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
                    if let Err(msg) = res {
                        eval_pattern(interp, &args[0], *msg)?;
                        Ok(())
                    } else {
                        Err(EvalError::AssertionFailed("Matched Err on Ok".into()))
                    }
                } else {
                    Err(EvalError::AssertionFailed("Variant mismatch".into()))
                }
            }
            // A bare *nullary* variant value (e.g. `Red`) is held as
            // `VoxValue::Constructor("Red")` — never applied to fields, so it is
            // never lowered to `Tagged`. A nullary constructor pattern matches it
            // when the names agree and the pattern itself takes no fields.
            VoxValue::Constructor(tag_name) => {
                if tag_name == *name && args.is_empty() {
                    Ok(())
                } else {
                    Err(EvalError::AssertionFailed(format!(
                        "Variant mismatch: expected {name}, got {tag_name}"
                    )))
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
        HirStmt::Expr { expr, .. } => {
            // Auto-reassignment for mutable-receiver method calls.
            //
            // In Vox's value-based scope model, `arr.push(x)` as a statement
            // would normally discard the returned new list — leaving `arr`
            // unchanged.  We detect this pattern and write the result back to
            // the variable so that common Vox idioms (`arr.push(x)`,
            // `arr.reverse()`, etc.) behave intuitively.
            //
            // Rule: if a method call is made on a plain identifier and the
            // returned value has the same runtime kind as the receiver, the
            // result is written back to that identifier via `set_mut`.
            //
            // This only affects statement-level method calls (i.e. where the
            // return value would otherwise be thrown away).  Expressions used
            // in let/assign/return still get the original return value as
            // normal.
            //
            // Special case — pop(): `list.pop()` returns the popped element
            // (not the shorter list), so the kind-match heuristic above would
            // never fire.  We detect this case explicitly: evaluate the call
            // (getting the popped element), then write the shortened list back
            // to the variable.
            if let HirExpr::MethodCall(obj_expr, _, _, _, _) = expr
                && let HirExpr::Ident(name, _) = obj_expr.as_ref()
            {
                let result = super::expr::eval_expr(interp, expr)?;
                // pop() special case: shrink the list variable (handled by
                // the same helper used in Let/Assign; must run before the
                // "reassign if same kind" check, which wouldn't fire for pop
                // since the return value is the popped element, not a List).
                apply_pop_side_effect(interp, expr);
                let should_reassign = match &result {
                    VoxValue::List(_) => {
                        matches!(interp.scope.get(name), Some(VoxValue::List(_)))
                    }
                    VoxValue::Str(_) => {
                        matches!(interp.scope.get(name), Some(VoxValue::Str(_)))
                    }
                    // dict.insert / dict.remove / dict.update all return
                    // a new Object — auto-write it back to the variable.
                    VoxValue::Object(_) => {
                        matches!(interp.scope.get(name), Some(VoxValue::Object(_)))
                    }
                    _ => false,
                };
                if should_reassign {
                    interp.scope.set_mut(name, result.clone());
                }
                return Ok(result);
            }
            super::expr::eval_expr(interp, expr)
        }
        HirStmt::Return { value, .. } => {
            if let Some(val) = value {
                let v = super::expr::eval_expr(interp, val)?;
                // Normalize zero-arg constructors: `return None` produces
                // `Constructor("None")`; normalize to `Option(None)` so callers
                // can call `.is_none()` etc. on the returned value.
                let v = super::expr::normalize_constructor(v);
                Ok(VoxValue::_Return(Box::new(v)))
            } else {
                Ok(VoxValue::_Return(Box::new(VoxValue::Null)))
            }
        }
        HirStmt::Break { .. } => Ok(VoxValue::_Break),
        HirStmt::Continue { .. } => Ok(VoxValue::_Continue),
        HirStmt::Let { pattern, value, .. } => {
            let v = super::expr::eval_expr(interp, value)?;
            // Propagate early-return from the `?` operator in the RHS expression.
            // `HirExpr::Try` on Err/None produces `VoxValue::_Return(...)` which
            // must bubble up through the statement loop, not be bound to the pattern.
            if matches!(v, VoxValue::_Return(_)) {
                return Ok(v);
            }
            eval_pattern(interp, pattern, v)?;
            // If the RHS was `list_var.pop()`, shrink the list variable.
            apply_pop_side_effect(interp, value);
            Ok(VoxValue::Null)
        }
        HirStmt::Assign { target, value, .. } => {
            let v = super::expr::eval_expr(interp, value)?;
            // Propagate early-return from `?` on RHS.
            if matches!(v, VoxValue::_Return(_)) {
                return Ok(v);
            }
            match target {
                HirExpr::Ident(name, _) => {
                    interp.scope.set_mut(name, v);
                }
                // Index assignment: `arr[i] = value` or `dict["key"] = value`.
                HirExpr::Index(obj_expr, idx_expr, _) => {
                    if let HirExpr::Ident(name, _) = obj_expr.as_ref() {
                        let idx_val = super::expr::eval_expr(interp, idx_expr)?;
                        match idx_val {
                            VoxValue::Int(i) => {
                                if i >= 0
                                    && let Some(VoxValue::List(items)) = interp.scope.get_mut(name)
                                {
                                    let ui = i as usize;
                                    if ui < items.len() {
                                        // CoW in place: clones only if `items` is aliased.
                                        std::rc::Rc::make_mut(items)[ui] = v;
                                    }
                                }
                            }
                            VoxValue::Str(key) => {
                                if let Some(VoxValue::Object(fields)) = interp.scope.get_mut(name) {
                                    let fields = std::rc::Rc::make_mut(fields);
                                    if let Some(entry) = fields.iter_mut().find(|(k, _)| k == &key)
                                    {
                                        entry.1 = v;
                                    } else {
                                        fields.push((key, v));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            // If the RHS was `list_var.pop()`, shrink the list variable.
            apply_pop_side_effect(interp, value);
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
