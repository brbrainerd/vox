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
            } else if let Some(val) = interp.module_scope.get(name) {
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
                // Integer arithmetic — use checked_* to convert
                // overflow / div-by-zero / mod-by-zero into clean EvalError
                // halts instead of Rust panics that take down the
                // whole interpreter process. Matches the
                // "no silent-wrong-output, no opaque crashes" health
                // commitment from audit doc §10.4.
                (HirBinOp::Add, VoxValue::Int(a), VoxValue::Int(b)) => a
                    .checked_add(b)
                    .map(VoxValue::Int)
                    .ok_or_else(|| EvalError::AssertionFailed(format!(
                        "integer overflow: {a} + {b}"
                    ))),
                (HirBinOp::Sub, VoxValue::Int(a), VoxValue::Int(b)) => a
                    .checked_sub(b)
                    .map(VoxValue::Int)
                    .ok_or_else(|| EvalError::AssertionFailed(format!(
                        "integer underflow: {a} - {b}"
                    ))),
                (HirBinOp::Mul, VoxValue::Int(a), VoxValue::Int(b)) => a
                    .checked_mul(b)
                    .map(VoxValue::Int)
                    .ok_or_else(|| EvalError::AssertionFailed(format!(
                        "integer overflow: {a} * {b}"
                    ))),
                (HirBinOp::Div, VoxValue::Int(_), VoxValue::Int(0)) => {
                    Err(EvalError::AssertionFailed(
                        "integer division by zero".to_string(),
                    ))
                }
                (HirBinOp::Div, VoxValue::Int(a), VoxValue::Int(b)) => a
                    .checked_div(b)
                    .map(VoxValue::Int)
                    .ok_or_else(|| EvalError::AssertionFailed(format!(
                        "integer division overflow: {a} / {b}"
                    ))),
                (HirBinOp::Mod, VoxValue::Int(_), VoxValue::Int(0)) => {
                    Err(EvalError::AssertionFailed(
                        "integer modulo by zero".to_string(),
                    ))
                }
                (HirBinOp::Mod, VoxValue::Int(a), VoxValue::Int(b)) => a
                    .checked_rem(b)
                    .map(VoxValue::Int)
                    .ok_or_else(|| EvalError::AssertionFailed(format!(
                        "integer modulo overflow: {a} % {b}"
                    ))),
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
                // Mixed Int + Float (or any other type pair the arms above
                // didn't catch) used to silently return Null — a health
                // foot-gun (audit doc §10.4 health-corrections section). An
                // explicit `to_float()` conversion is required if the user
                // wants mixed-arithmetic semantics; the error message names
                // both operand types so the diagnostic is actionable.
                (op, l, r) => Err(EvalError::AssertionFailed(format!(
                    "unsupported binary op `{op:?}` for operands {} and {}",
                    crate::eval::builtins::vox_value_type_name(&l),
                    crate::eval::builtins::vox_value_type_name(&r),
                ))),
            }
        }
        HirExpr::Unary(op, inner, _) => {
            let v = eval_expr(interp, inner)?;
            match (op, v) {
                (HirUnOp::Not, VoxValue::Bool(b)) => Ok(VoxValue::Bool(!b)),
                (HirUnOp::Neg, VoxValue::Int(n)) => Ok(VoxValue::Int(-n)),
                (HirUnOp::Neg, VoxValue::Float(f)) => Ok(VoxValue::Float(-f)),
                (op, other) => Err(EvalError::AssertionFailed(format!(
                    "unsupported unary op `{op:?}` for operand {}",
                    crate::eval::builtins::vox_value_type_name(&other),
                ))),
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
                VoxValue::Constructor(name) => match name.as_str() {
                    // Built-in Option/Result constructors lower directly to
                    // VoxValue::Option / ::Result so downstream method dispatch
                    // (`.is_ok()`, `.unwrap()`, `.is_none()`) and pattern
                    // matching on Result/Option both work.
                    "Some" if eval_args.len() == 1 => {
                        Ok(VoxValue::Option(Some(Box::new(eval_args.into_iter().next().unwrap()))))
                    }
                    "None" if eval_args.is_empty() => Ok(VoxValue::Option(None)),
                    "Ok" if eval_args.len() == 1 => {
                        Ok(VoxValue::Result(Ok(Box::new(eval_args.into_iter().next().unwrap()))))
                    }
                    "Err" | "Error" if eval_args.len() == 1 => {
                        let msg = match eval_args.into_iter().next().unwrap() {
                            VoxValue::Str(s) => s,
                            other => format!("{other:?}"),
                        };
                        Ok(VoxValue::Result(Err(msg)))
                    }
                    _ => Ok(VoxValue::Tagged {
                        name,
                        fields: eval_args,
                    }),
                },
                _ => Err(EvalError::TypeError {
                    expected: "function",
                    found: "other".into(),
                }),
            }
        }
        HirExpr::MethodCall(obj, method, args, _, _) => {
            // Detect the `str.method(receiver, ...)` / `list.method(receiver, ...)`
            // free-function-style call. These were never valid in Vox — string and
            // list operations are method-only — but the previous error message
            // ("Method foo not found") was confusing because the call site is
            // syntactically a free function, not a method on a value.
            // See: docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md §6 #9.
            if let HirExpr::Ident(ns_name, _) = obj.as_ref() {
                if (ns_name == "str" || ns_name == "list") && !args.is_empty() {
                    return Err(EvalError::AssertionFailed(format!(
                        "`{ns}.{m}(receiver, ...)` is not a valid call form in Vox; \
                         use the method form `receiver.{m}(...)` instead. \
                         (Vox makes string and list operations method-only \
                         per K-complexity policy.)",
                        ns = ns_name,
                        m = method,
                    )));
                }
            }
            let o = eval_expr(interp, obj)?;
            let mut eval_args = Vec::new();
            for a in args {
                eval_args.push(eval_expr(interp, &a.value)?);
            }

            // Closure-taking method dispatch — handled here (not in
            // `call_builtin_method`) because applying a closure requires
            // mutable interp access (the closure body is evaluated against
            // the captured scope chain). Mirrors the eval impl plan in
            // closures-rfc-2026-05-23.md §9.5.
            if let Some(result) = apply_closure_method(interp, &o, method, &eval_args)? {
                return Ok(result);
            }

            // Namespace-method dispatch: `alias.fn_name(...)` where `alias`
            // is the namespace object produced by an
            // `import "./util.vox" as alias` (RFC §3 scope-merge / alias form).
            // Field is looked up; if it's a callable (Fn or Constructor) it
            // is applied with the call arguments. Falls through to builtin
            // dispatch otherwise so things like `process.run(...)` still work.
            if let VoxValue::Object(fields) = &o {
                if let Some((_, val)) = fields.iter().find(|(k, _)| k == method) {
                    match val.clone() {
                        VoxValue::Fn { .. } => {
                            return apply_closure(interp, &val.clone(), eval_args);
                        }
                        VoxValue::Constructor(name) => {
                            return Ok(VoxValue::Tagged {
                                name,
                                fields: eval_args,
                            });
                        }
                        _ => {
                            // A non-callable field with the method's name — fall
                            // through so builtin namespace dispatch (e.g.
                            // `process.run`) still gets a chance.
                        }
                    }
                }
            }

            if let Some(r) =
                super::builtins::call_builtin_method(&o, method, eval_args, interp.caps.as_ref())
            {
                // Catch the _Panic sentinel produced by `unwrap()`/`expect()`
                // and friends and turn it into a proper EvalError. This
                // replaces the prior silent-Null behavior with a halt that
                // carries the offender's message. See eval/value.rs
                // `_Panic` variant docstring for rationale.
                if let crate::eval::value::VoxValue::_Panic(msg) = r {
                    Err(EvalError::AssertionFailed(msg))
                } else {
                    Ok(r)
                }
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
            // Strict-Option subscript per typed-subscript decision
            // 2026-05-23: list[int] / map[K] / str[int] return Option[T];
            // out-of-bounds and wrong-receiver-type both produce None.
            // Matches the typeck signature in checker/expr.rs Index arm.
            let obj_val = eval_expr(interp, object)?;
            let idx_val = eval_expr(interp, index)?;
            match (obj_val, idx_val) {
                (VoxValue::List(items), VoxValue::Int(i)) => {
                    if i < 0 {
                        return Ok(VoxValue::Option(None));
                    }
                    Ok(VoxValue::Option(
                        items
                            .into_iter()
                            .nth(i as usize)
                            .map(Box::new),
                    ))
                }
                (VoxValue::Str(s), VoxValue::Int(i)) => {
                    if i < 0 {
                        return Ok(VoxValue::Option(None));
                    }
                    Ok(VoxValue::Option(
                        s.chars()
                            .nth(i as usize)
                            .map(|c| Box::new(VoxValue::Str(c.to_string()))),
                    ))
                }
                _ => Ok(VoxValue::Option(None)),
            }
        }
        _ => Ok(VoxValue::Null),
    }
}

/// Apply a `VoxValue::Fn` closure to a list of argument values. The
/// closure captures the scope at the point of creation (see
/// `HirExpr::Lambda` arm above); calling it pushes a frame, binds the
/// args to the param names, runs the body, and restores the prior
/// interp scope.
///
/// Returns the closure's body return value. Honors `_Return` (early-exit
/// from inside the closure) and `_Panic` (propagated up as `EvalError`).
///
/// Per closures RFC §11: `return` inside a closure body returns from the
/// closure, NOT from the enclosing function — the only `_Return` we
/// honor here is one produced inside this closure's body.
fn apply_closure(
    interp: &mut crate::eval::Interpreter,
    closure: &VoxValue,
    args: Vec<VoxValue>,
) -> Result<VoxValue, EvalError> {
    let (params, body, env) = match closure {
        VoxValue::Fn { params, body, env } => (params.clone(), body.clone(), env.clone()),
        other => {
            return Err(EvalError::TypeError {
                expected: "function",
                found: super::builtins::vox_value_type_name(other).into(),
            });
        }
    };

    let mut new_env = env;
    new_env.push_frame();
    for (p, arg) in params.iter().zip(args) {
        new_env.set(p.clone(), arg);
    }

    let old_scope = std::mem::replace(&mut interp.scope, new_env);
    let mut val = VoxValue::Null;
    for stmt in &body {
        val = super::stmt::eval_stmt(interp, stmt)?;
        if let VoxValue::_Return(v) = val {
            val = *v;
            break;
        }
        if matches!(val, VoxValue::_Break | VoxValue::_Continue) {
            break;
        }
    }
    interp.scope = old_scope;

    if let VoxValue::_Panic(msg) = val {
        return Err(EvalError::AssertionFailed(msg));
    }
    Ok(val)
}

/// Dispatch closure-taking collection methods. Returns `Ok(Some(result))`
/// when this site handled the call, `Ok(None)` when the method isn't
/// closure-taking (caller falls through to `call_builtin_method`).
fn apply_closure_method(
    interp: &mut crate::eval::Interpreter,
    obj: &VoxValue,
    method: &str,
    args: &[VoxValue],
) -> Result<Option<VoxValue>, EvalError> {
    // Only dispatch when the last arg is a closure — otherwise let the
    // non-closure-aware path handle the call (e.g. `List.contains(v)` is
    // not a closure method).
    let closure = match args.last() {
        Some(c @ VoxValue::Fn { .. }) => c.clone(),
        _ => return Ok(None),
    };

    match (obj, method) {
        // ── List ────────────────────────────────────────────────────
        (VoxValue::List(items), "map") => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter().cloned() {
                out.push(apply_closure(interp, &closure, vec![item])?);
            }
            Ok(Some(VoxValue::List(out)))
        }
        (VoxValue::List(items), "filter") => {
            let mut out = Vec::new();
            for item in items.iter().cloned() {
                let keep = apply_closure(interp, &closure, vec![item.clone()])?;
                if matches!(keep, VoxValue::Bool(true)) {
                    out.push(item);
                }
            }
            Ok(Some(VoxValue::List(out)))
        }
        (VoxValue::List(items), "for_each") => {
            for item in items.iter().cloned() {
                apply_closure(interp, &closure, vec![item])?;
            }
            Ok(Some(VoxValue::Null))
        }
        (VoxValue::List(items), "any") => {
            for item in items.iter().cloned() {
                let r = apply_closure(interp, &closure, vec![item])?;
                if matches!(r, VoxValue::Bool(true)) {
                    return Ok(Some(VoxValue::Bool(true)));
                }
            }
            Ok(Some(VoxValue::Bool(false)))
        }
        (VoxValue::List(items), "all") => {
            for item in items.iter().cloned() {
                let r = apply_closure(interp, &closure, vec![item])?;
                if !matches!(r, VoxValue::Bool(true)) {
                    return Ok(Some(VoxValue::Bool(false)));
                }
            }
            Ok(Some(VoxValue::Bool(true)))
        }
        (VoxValue::List(items), "fold") => {
            // fold(init, fn(acc, x) { ... }) — args = [init, closure]
            if args.len() < 2 {
                return Ok(None);
            }
            let mut acc = args[0].clone();
            for item in items.iter().cloned() {
                acc = apply_closure(interp, &closure, vec![acc, item])?;
            }
            Ok(Some(acc))
        }
        // ── Option ──────────────────────────────────────────────────
        (VoxValue::Option(opt), "map") => match opt.as_ref() {
            Some(v) => {
                let mapped = apply_closure(interp, &closure, vec![(**v).clone()])?;
                Ok(Some(VoxValue::Option(Some(Box::new(mapped)))))
            }
            None => Ok(Some(VoxValue::Option(None))),
        },
        (VoxValue::Option(opt), "and_then") => match opt.as_ref() {
            Some(v) => {
                let result = apply_closure(interp, &closure, vec![(**v).clone()])?;
                // Closure should return Option; pass through directly.
                Ok(Some(result))
            }
            None => Ok(Some(VoxValue::Option(None))),
        },
        (VoxValue::Option(opt), "filter") => match opt.as_ref() {
            Some(v) => {
                let keep = apply_closure(interp, &closure, vec![(**v).clone()])?;
                if matches!(keep, VoxValue::Bool(true)) {
                    Ok(Some(VoxValue::Option(opt.clone())))
                } else {
                    Ok(Some(VoxValue::Option(None)))
                }
            }
            None => Ok(Some(VoxValue::Option(None))),
        },
        // ── Result ──────────────────────────────────────────────────
        (VoxValue::Result(res), "map") => match res.as_ref() {
            Ok(v) => {
                let mapped = apply_closure(interp, &closure, vec![(**v).clone()])?;
                Ok(Some(VoxValue::Result(Ok(Box::new(mapped)))))
            }
            Err(e) => Ok(Some(VoxValue::Result(Err(e.clone())))),
        },
        (VoxValue::Result(res), "map_err") => match res.as_ref() {
            Ok(v) => Ok(Some(VoxValue::Result(Ok(v.clone())))),
            Err(e) => {
                let mapped = apply_closure(
                    interp,
                    &closure,
                    vec![VoxValue::Str(e.clone())],
                )?;
                if let VoxValue::Str(new_msg) = mapped {
                    Ok(Some(VoxValue::Result(Err(new_msg))))
                } else {
                    Err(EvalError::TypeError {
                        expected: "str",
                        found: super::builtins::vox_value_type_name(&mapped).into(),
                    })
                }
            }
        },
        (VoxValue::Result(res), "and_then") => match res.as_ref() {
            Ok(v) => {
                let r = apply_closure(interp, &closure, vec![(**v).clone()])?;
                Ok(Some(r))
            }
            Err(e) => Ok(Some(VoxValue::Result(Err(e.clone())))),
        },
        _ => Ok(None),
    }
}
