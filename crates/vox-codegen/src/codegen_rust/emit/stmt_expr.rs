use super::ownership::OwnershipMode;
use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::builtin_registry::{BuiltinArgKind, lookup_builtin, std_namespace_runtime_call};
use vox_compiler::hir::{HirArg, HirBinOp, HirExpr, HirPattern, HirStmt, HirType};

pub(super) fn emit_stmt(
    stmt: &HirStmt,
    indent: usize,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    // Rust expression for `Option<String>` request id (e.g. `vox_rid.clone()`), or omit with `None`.
    http_error_rid: Option<&str>,
) -> String {
    let pad = " ".repeat(indent * 4);
    match stmt {
        HirStmt::Let {
            pattern,
            value,
            mutable,
            ..
        } => {
            let mut_kw = if *mutable { "mut " } else { "" };
            if is_actor {
                format!(
                    "{pad}let {}{} = ctx.heap.allocate({});\n",
                    mut_kw,
                    emit_pattern(pattern, is_route, is_actor, mutation_tx),
                    emit_expr_with(
                        value,
                        is_route,
                        is_actor,
                        mutation_tx,
                        inferred_types,
                        usage,
                        OwnershipMode::Owned
                    )
                )
            } else {
                format!(
                    "{pad}let {}{} = {};\n",
                    mut_kw,
                    emit_pattern(pattern, is_route, is_actor, mutation_tx),
                    emit_expr_with(
                        value,
                        is_route,
                        is_actor,
                        mutation_tx,
                        inferred_types,
                        usage,
                        OwnershipMode::Owned
                    )
                )
            }
        }
        HirStmt::Assign { target, value, .. } => {
            // The target must be an l-value; do not emit `.clone()` on ident targets.
            let lhs = emit_assign_target(target, inferred_types, usage);
            format!(
                "{pad}{lhs} = {};\n",
                emit_expr_with(
                    value,
                    is_route,
                    is_actor,
                    mutation_tx,
                    inferred_types,
                    usage,
                    OwnershipMode::Owned
                )
            )
        }
        HirStmt::Return { value, .. } => emit_return_stmt(
            value.as_ref(),
            &pad,
            is_actor,
            is_route,
            mutation_tx,
            inferred_types,
            usage,
            http_error_rid,
        ),
        HirStmt::Expr { expr, .. } => {
            format!(
                "{pad}{};\n",
                emit_expr_with(
                    expr,
                    is_route,
                    is_actor,
                    mutation_tx,
                    inferred_types,
                    usage,
                    OwnershipMode::Owned
                )
            )
        }
        HirStmt::While {
            condition, body, ..
        } => {
            let mut s = format!(
                "{pad}while {} {{\n",
                emit_expr_with(
                    condition,
                    is_route,
                    is_actor,
                    mutation_tx,
                    inferred_types,
                    usage,
                    OwnershipMode::Owned
                )
            );
            if is_actor {
                push_actor_loop_prelude(&mut s, &pad);
            }
            for stmt in body {
                s.push_str(&emit_stmt(
                    stmt,
                    indent + 1,
                    is_route,
                    is_actor,
                    mutation_tx,
                    inferred_types,
                    usage,
                    http_error_rid,
                ));
            }
            s.push_str(&format!("{pad}}}\n"));
            s
        }
        HirStmt::Loop { body, .. } => {
            let mut s = format!("{pad}loop {{\n");
            if is_actor {
                push_actor_loop_prelude(&mut s, &pad);
            }
            for stmt in body {
                s.push_str(&emit_stmt(
                    stmt,
                    indent + 1,
                    is_route,
                    is_actor,
                    mutation_tx,
                    inferred_types,
                    usage,
                    http_error_rid,
                ));
            }
            s.push_str(&format!("{pad}}}\n"));
            s
        }
        HirStmt::Break { .. } => format!("{pad}break;\n"),
        HirStmt::Continue { .. } => format!("{pad}continue;\n"),
    }
}

/// Emit one statement for script-mode `main` (no route/actor return wrapping).
pub fn emit_main_stmt(
    stmt: &HirStmt,
    indent: usize,
    inferred_types: Option<&HashMap<Span, HirType>>,
) -> String {
    emit_stmt(
        stmt,
        indent,
        false,
        false,
        false,
        inferred_types,
        None,
        None,
    )
}

/// Emit an assignment l-value target without adding `.clone()`.
///
/// The standard `emit_expr_with` appends `.clone()` to every identifier,
/// which produces invalid Rust like `j.clone() = rhs`. This function emits
/// a bare identifier or a simple field-access path instead.
fn emit_assign_target(
    expr: &HirExpr,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> String {
    match expr {
        HirExpr::Ident(n, _) => n.clone(),
        HirExpr::FieldAccess(obj, field, _) => {
            format!(
                "{}.{}",
                emit_assign_target(obj, inferred_types, usage),
                field
            )
        }
        // Index on the LHS of an assignment (`xs[i] = v`) needs a raw,
        // assignable lvalue `xs[i as usize]` — NOT the Option-returning
        // `.get(i).cloned()` READ form (which is not an lvalue -> E0070). The
        // index is itself a read expression.
        HirExpr::Index(obj, idx, _) => format!(
            "{}[{} as usize]",
            emit_assign_target(obj, inferred_types, usage),
            emit_expr_with(
                idx,
                false,
                false,
                false,
                inferred_types,
                usage,
                OwnershipMode::Owned,
            ),
        ),
        // Fallback: use the generic emitter for complex lvalues.
        other => emit_expr_with(
            other,
            false,
            false,
            false,
            inferred_types,
            usage,
            OwnershipMode::Owned,
        ),
    }
}

/// Emit the actor-owned loop reduction/GC prelude (reduction counter bump,
/// yield, heap collect check). Emitted at the top of every `while` / `loop`
/// body when `is_actor` is true.
///
/// Extracted from `emit_stmt`'s While and Loop arms per CR-A1: the identical
/// 6-line block appeared twice, each contributing ~2 DPs to the caller.
fn push_actor_loop_prelude(s: &mut String, pad: &str) {
    s.push_str(&format!("{pad}    ctx.reduction_count += 1;\n"));
    s.push_str(&format!(
        "{pad}    if ctx.reduction_count >= ctx.max_reductions {{\n"
    ));
    s.push_str(&format!("{pad}        ctx.reduction_count = 0;\n"));
    s.push_str(&format!(
        "{pad}        if ctx.heap.should_collect() {{ ctx.heap.collect(); }}\n"
    ));
    s.push_str(&format!("{pad}        tokio::task::yield_now().await;\n"));
    s.push_str(&format!("{pad}    }}\n"));
}

/// Emit a `return` statement, handling actor scaffolding, route wrapping,
/// and plain returns.
///
/// Extracted from `emit_stmt`'s Return arm per CR-A1: the inline block
/// contributed ~7 decision points (is_actor + if-let + is_route + mutation_tx
/// combinations).
#[allow(clippy::too_many_arguments)]
fn emit_return_stmt(
    value: Option<&HirExpr>,
    pad: &str,
    is_actor: bool,
    is_route: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    http_error_rid: Option<&str>,
) -> String {
    if is_actor {
        if let Some(v) = value {
            format!(
                "{pad}let _ = {}; // return ignored in actor; scaffolding only\n",
                emit_expr_with(
                    v,
                    is_route,
                    is_actor,
                    mutation_tx,
                    inferred_types,
                    usage,
                    OwnershipMode::Owned
                )
            )
        } else {
            format!("{pad}// return ignored in actor; scaffolding only\n")
        }
    } else if let Some(v) = value {
        let expr_str = emit_expr_with(
            v,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            OwnershipMode::Owned,
        );
        let rid_tok = http_error_rid.unwrap_or("None");
        let route_inference_safe_expr = if is_route {
            route_json_shortcut(v, is_route, is_actor, mutation_tx, inferred_types, usage)
        } else {
            None
        };
        if is_route && mutation_tx {
            let inner = route_inference_safe_expr.unwrap_or_else(|| format!(
                "serde_json::to_value({}).map_err(|e| vox_db::StoreError::Serialization(format!(\"{{}}\", e)))?",
                expr_str
            ));
            format!("{pad}return Ok(Json({inner}));\n")
        } else if is_route {
            let inner = route_inference_safe_expr.unwrap_or_else(|| format!(
                "serde_json::to_value({expr}).map_err(|e| (\n    StatusCode::INTERNAL_SERVER_ERROR,\n    Json(vox_http_client::envelope::error_json(\"SERIALIZATION_ERROR\", format!(\"{{}}\", e), {rid}, None)),\n))?",
                expr = expr_str,
                rid = rid_tok,
            ));
            format!("{pad}return Ok(Json({inner}));\n")
        } else {
            format!("{pad}return {};\n", expr_str)
        }
    } else if is_route {
        format!("{pad}return Ok(Json(serde_json::Value::Null));\n")
    } else {
        format!("{pad}return;\n")
    }
}

/// Emit a binary expression, handling the Pipe short-circuit and the
/// arithmetic-vs-comparison borrow distinction.
///
/// Extracted from `emit_expr_with` per CR-A1: the 13-arm op match + the Pipe
/// early-return + the arithmetic `&` decoration contributed ~15 DPs inline.
fn emit_binary_expr<F>(
    op: &HirBinOp,
    l: &HirExpr,
    r: &HirExpr,
    bin_span: &Span,
    inferred_types: Option<&HashMap<Span, HirType>>,
    emit: &F,
) -> String
where
    F: Fn(&HirExpr, OwnershipMode) -> String,
{
    if matches!(op, HirBinOp::Pipe) {
        return format!(
            "{}({})",
            emit(r, OwnershipMode::Owned),
            emit(l, OwnershipMode::Owned)
        );
    }
    // `x is null` / `x isnt null` are Option None-checks. `null` is a bare Ident in
    // HIR (not a Rust value), so `== null` is invalid Rust — lower to `.is_none()` /
    // `.is_some()` on the non-null operand.
    if matches!(op, HirBinOp::Is | HirBinOp::Isnt) {
        let is_null = |e: &HirExpr| matches!(e, HirExpr::Ident(n, _) if n == "null");
        let opt = if is_null(r) {
            Some(l)
        } else if is_null(l) {
            Some(r)
        } else {
            None
        };
        if let Some(opt_expr) = opt {
            let method = if matches!(op, HirBinOp::Is) {
                "is_none"
            } else {
                "is_some"
            };
            return format!("({}).{}()", emit(opt_expr, OwnershipMode::Owned), method);
        }
    }
    let op_str = match op {
        HirBinOp::Add => "+",
        HirBinOp::Sub => "-",
        HirBinOp::Mul => "*",
        HirBinOp::Div => "/",
        HirBinOp::Lt => "<",
        HirBinOp::Gt => ">",
        HirBinOp::Lte => "<=",
        HirBinOp::Gte => ">=",
        HirBinOp::And => "&&",
        HirBinOp::Or => "||",
        HirBinOp::Is => "==",
        HirBinOp::Isnt => "!=",
        HirBinOp::Mod => "%",
        HirBinOp::Pipe => unreachable!("handled above"),
    };
    if matches!(
        op,
        HirBinOp::Add | HirBinOp::Sub | HirBinOp::Mul | HirBinOp::Div
    ) {
        // Only `String` concatenation needs a borrowed RHS (`String + &str`).
        // Numeric `+ - * /` do not — emitting `1 + &2` compiles only via Rust's
        // forward-ref `Add<&i64>` impls and triggers an unused-borrow warning.
        // Keep the `&` unless the result type is positively numeric, so string
        // concat stays correct while integer/float/decimal ops are clean.
        // Positively numeric when either operand is a numeric literal (the type
        // checker does not record a result type for pure-literal arithmetic like
        // `1 + 2`), or when the result type is a numeric scalar.
        let is_num_lit = |e: &HirExpr| {
            matches!(
                e,
                HirExpr::IntLit(..) | HirExpr::FloatLit(..) | HirExpr::DecimalLit(..)
            )
        };
        let positively_numeric = is_num_lit(l)
            || is_num_lit(r)
            || inferred_types
                .and_then(|m| m.get(bin_span))
                .is_some_and(|t| {
                    matches!(t, HirType::Named(n) if matches!(n.as_str(), "int" | "float" | "dec"))
                        || matches!(t, HirType::Decimal)
                });
        let rhs = emit(r, OwnershipMode::Owned);
        let rhs = if positively_numeric {
            rhs
        } else {
            format!("&{rhs}")
        };
        format!("({} {} {})", emit(l, OwnershipMode::Owned), op_str, rhs)
    } else {
        format!(
            "({} {} {})",
            emit(l, OwnershipMode::Owned),
            op_str,
            emit(r, OwnershipMode::Owned)
        )
    }
}

/// Emit an identifier reference, applying ownership mode and copy/move heuristics.
///
/// Extracted from `emit_expr_with`'s Ident arm per CR-A1: the arm had ~5 DPs
/// (namespace bypass, is_copy nested match, is_last_use, mode match).
fn emit_ident_expr(
    n: &str,
    span: &vox_compiler::ast::span::Span,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    mode: OwnershipMode,
) -> String {
    // `null` is Vox's typed None literal (typeck binds it as a `Constructor`
    // of type `Option[T]`). In value position it lowers to Rust `None`; the
    // `is null` / `isnt null` comparison forms are handled earlier in
    // `emit_binary_expr` (→ `.is_none()` / `.is_some()`).
    if n == "null" {
        return "None".to_string();
    }
    // These identifiers are always passed bare — no `.clone()` or `.as_str()`.
    if n == "request"
        || n == "std"
        || n == "fs"
        || n.chars().next().is_some_and(|c| c.is_uppercase())
    {
        return n.to_string();
    }
    let is_copy = inferred_types.and_then(|m| m.get(span)).is_some_and(|t| {
        matches!(
            t,
            HirType::Named(name) if matches!(name.as_str(), "int" | "bool" | "float" | "char" | "dec")
        ) || matches!(t, HirType::Unit | HirType::Decimal)
    });
    if is_copy {
        n.to_string()
    } else if usage.is_some_and(|u| u.is_last_use(n, *span)) {
        // Last use of a non-Copy type: move it.
        n.to_string()
    } else {
        match mode {
            OwnershipMode::Owned => format!("{}.clone()", n),
            OwnershipMode::Borrowed => {
                // Borrow without cloning. A `str`-typed value uses `.as_str()`
                // (what the borrowing string builtins expect); anything else
                // uses a plain reference — `&Vec<T>` coerces to `&[T]`, `&T` to
                // `&T`. Emitting `.as_str()` unconditionally (the prior behavior)
                // produced uncompilable Rust the moment a non-string argument was
                // borrowed, a latent landmine for widening borrow inference.
                let is_str = inferred_types
                    .and_then(|m| m.get(span))
                    .is_some_and(|t| matches!(t, HirType::Named(name) if name == "str"));
                if is_str {
                    format!("{}.as_str()", n)
                } else {
                    format!("&{}", n)
                }
            }
        }
    }
}

pub(super) fn emit_pattern(
    pat: &HirPattern,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
) -> String {
    match pat {
        HirPattern::Ident(n, _) => n.clone(),
        HirPattern::Wildcard(_) => "_".into(),
        HirPattern::Literal(lit, _) => emit_expr_with(
            lit,
            is_route,
            is_actor,
            mutation_tx,
            None,
            None,
            OwnershipMode::Owned,
        ),
        HirPattern::Tuple(pats, _) => format!(
            "({})",
            pats.iter()
                .map(|p| emit_pattern(p, is_route, is_actor, mutation_tx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        HirPattern::Constructor(n, pats, _) => {
            // Rust struct variant syntax: Name { field: val }
            // HirPattern::Constructor has positional args?
            // "Ok(text: str)" -> Constructor("Ok", [Ident("text")])
            // Rust enum: Ok { text: ... } or Ok(...) depending on def.
            // Vox ADTs use named fields. So we matched on Struct names.
            // Wait, parse_typedef uses named fields.
            // But pattern matching? "Ok(r) -> r". This is positional.
            // My ADT generator emitted named fields: `Variant { field: Type }`.
            // Rust requires named matching if defined with names.
            // Or use tuple variants if positional.
            // Vox defines `| Ok(text: str)`. This is named.
            // So `Ok(t)` in match needs to be `Ok { text: t }`.
            // BUT the parser/HIR doesn't resolve positional match to named fields yet.
            // This is a semantic gap.
            // Workaround: Use tuple variants in Rust if possible, or assume names match?
            // "Ok(r)" -> pattern is Constructor("Ok", [Ident("r")]).
            // We don't know the field name "text" here without looking up the definition.
            // For now, emit as tuple style `Ok(p1, p2)` and hope the ADT generation uses tuple variants?
            // In emit_lib: `variant.fields` are named.
            // If I change emit_lib to use tuple variants if fields are present?
            // Or structs?
            // Vox syntax `Ok(text: str)` looks like named.
            // But usage `Ok("hi")` looks positional.
            // Let's generate Tuple variants in Rust for simplicity: `Ok(String)`.
            // And ignore field names in TypeDef?
            // Or use the names?
            if pats.is_empty() {
                // Nullary variant (`None`, a unit ADT variant): emit the bare
                // name. `None()` would be `E0532 expected tuple variant, found
                // unit variant` against the unit enum variant emitted by emit_lib.
                n.clone()
            } else {
                format!(
                    "{}({})",
                    n,
                    pats.iter()
                        .map(|p| emit_pattern(p, is_route, is_actor, mutation_tx))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

/// Emit one HIR expression as a Rust expression string (for nested codegen / tools).
pub fn emit_expr(expr: &HirExpr) -> String {
    emit_expr_with(expr, false, false, false, None, None, OwnershipMode::Owned)
}

pub(super) fn emit_expr_with(
    expr: &HirExpr,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    mode: OwnershipMode,
) -> String {
    let fallible_db = mutation_tx;
    let emit = |e: &HirExpr, m: OwnershipMode| {
        emit_expr_with(e, is_route, is_actor, mutation_tx, inferred_types, usage, m)
    };
    if let Some(s) = super::stmt_expr_tail::try_emit_expr_tail(
        expr,
        is_route,
        is_actor,
        mutation_tx,
        fallible_db,
        inferred_types,
        usage,
        mode,
        &emit,
    ) {
        return s;
    }
    match expr {
        HirExpr::IntLit(v, _) => v.to_string(),
        HirExpr::FloatLit(v, _) => v.to_string(),
        HirExpr::StringLit(v, _) => {
            let escaped = v.replace("\"", "\\\"").replace("\n", "\\n");
            match mode {
                OwnershipMode::Owned => format!("\"{}\".to_string()", escaped),
                OwnershipMode::Borrowed => format!("\"{}\"", escaped),
            }
        }
        HirExpr::BoolLit(v, _) => v.to_string(),
        HirExpr::DecimalLit(v, _) => {
            format!("rust_decimal::Decimal::from_str_exact(\"{v}\").unwrap()")
        }
        HirExpr::ListLit(elements, _) => format!(
            "vec![{}]",
            elements
                .iter()
                .map(|e| emit(e, OwnershipMode::Owned))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        HirExpr::TupleLit(elements, _) => format!(
            "({})",
            elements
                .iter()
                .map(|e| emit(e, OwnershipMode::Owned))
                .collect::<Vec<_>>()
                .join(", ")
        ),

        HirExpr::Ident(n, span) => emit_ident_expr(n, span, inferred_types, usage, mode),
        HirExpr::Binary(op, l, r, bin_span) => {
            emit_binary_expr(op, l, r, bin_span, inferred_types, &emit)
        }
        HirExpr::Call(callee, args, is_await, _) => {
            if let HirExpr::Ident(n, _) = &**callee
                && let Some(s) = emit_builtin_ident_call(n, args, &emit)
            {
                return s;
            }
            // std.* call forms (std.fs.read, std.json.parse, OpenClaw.X,
            // Browser.X, etc.) — see helper below.
            if let Some(s) = try_emit_namespace_call(
                callee,
                args,
                *is_await,
                is_route,
                is_actor,
                mutation_tx,
                inferred_types,
                usage,
            ) {
                return s;
            }
            let c = emit(callee, OwnershipMode::Owned);
            let a: Vec<_> = args
                .iter()
                .map(|arg| emit(&arg.value, OwnershipMode::Owned))
                .collect();
            if *is_await {
                format!("{}({}).await", c, a.join(", "))
            } else {
                format!("{}({})", c, a.join(", "))
            }
        }
        HirExpr::Index(obj, idx, _) => {
            // Vox indexing returns `Option[T]` (interpreter eval::expr Index arm
            // wraps in `VoxValue::Option`; typeck `list[i] : Option[T]`), so
            // `match xs[i] { Some(v) => .. None => .. }` is the idiomatic form.
            // Emit `.get(i).cloned()` -> `Option<T>` rather than raw `xs[i]`
            // (which returns `T` and panics out of bounds) so the Option match
            // typechecks and bounds are safe.
            format!(
                "{}.get({} as usize).cloned()",
                emit(obj, OwnershipMode::Owned),
                emit(idx, OwnershipMode::Owned)
            )
        }
        _ => unreachable!(
            "HIR expr variants not handled in stmt_expr::emit_expr_with must be handled by stmt_expr_tail (delegate order bug)"
        ),
    }
}

/// Try to emit a namespaced call: `std.fs.read(path)`, `OpenClaw.X(...)`,
/// `Browser.X(...)`, or `fs.X(...)`. Returns `None` if the call shape
/// doesn't match a namespace path, so the caller falls through to
/// generic Fn dispatch.
///
/// Extracted from `emit_expr_with` per CR-A1 plan §5.6 — the nested
/// if-let chains here contributed ~10-12 decision points and made the
/// caller hard to read.
#[allow(clippy::too_many_arguments)]
fn try_emit_namespace_call(
    callee: &HirExpr,
    args: &[HirArg],
    is_await: bool,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> Option<String> {
    let HirExpr::FieldAccess(namespace_expr, fn_name, _) = callee else {
        return None;
    };
    let emit_owned = |e: &HirExpr| {
        emit_expr_with(
            e,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            OwnershipMode::Owned,
        )
    };
    let with_await = |s: String| -> String { if is_await { format!("{}.await", s) } else { s } };

    // Shape 1: `Module.fn(args)` where Module is OpenClaw / Browser / Scrape / fs.
    if let HirExpr::Ident(module_name, _) = namespace_expr.as_ref() {
        let a: Vec<_> = args.iter().map(|arg| emit_owned(&arg.value)).collect();
        if module_name == "OpenClaw" || module_name == "Browser" || module_name == "Scrape" {
            if let Some(expr) = emit_openclaw_or_browser_registry_call(module_name, fn_name, &a) {
                return Some(expr);
            }
        } else if module_name == "fs"
            && let Some(call) = std_namespace_runtime_call("fs", fn_name, &a)
        {
            return Some(call);
        }
    }

    // Shape 2: `std.fn(args)` — root-namespace call.
    if let HirExpr::Ident(std_kw, _) = namespace_expr.as_ref()
        && std_kw == "std"
    {
        let a = emit_call_args_with_borrow_inference(
            args,
            "std",
            fn_name,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
        );
        if let Some(call) = emit_registry_runtime_call("std", fn_name, &a) {
            return Some(with_await(call));
        }
    }

    // Shape 3: `std.ns.fn(args)` — nested-namespace call.
    if let HirExpr::FieldAccess(std_expr, ns_name, _) = namespace_expr.as_ref()
        && let HirExpr::Ident(std_kw, _) = std_expr.as_ref()
        && std_kw == "std"
    {
        let a = emit_call_args_with_borrow_inference(
            args,
            ns_name,
            fn_name,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
        );
        if let Some(b) = std_namespace_runtime_call(ns_name.as_str(), fn_name.as_str(), &a) {
            return Some(with_await(b));
        }
        let call = format!("::std::{}::{}({})", ns_name, fn_name, a.join(", "));
        return Some(with_await(call));
    }
    None
}

/// Lower a sequence of call arguments with per-position
/// `is_builtin_arg_borrowed(ns, fn, i)` borrow inference. Used by both
/// the `std.fn(...)` and `std.ns.fn(...)` shapes — pulled out to keep
/// `try_emit_namespace_call` readable.
#[allow(clippy::too_many_arguments)]
fn emit_call_args_with_borrow_inference(
    args: &[HirArg],
    namespace: &str,
    fn_name: &str,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> Vec<String> {
    args.iter()
        .enumerate()
        .map(|(i, arg)| {
            let mode = if is_builtin_arg_borrowed(namespace, fn_name, i) {
                OwnershipMode::Borrowed
            } else {
                OwnershipMode::Owned
            };
            emit_expr_with(
                &arg.value,
                is_route,
                is_actor,
                mutation_tx,
                inferred_types,
                usage,
                mode,
            )
        })
        .collect()
}

/// In a route-handler return position, side-step the serde_json
/// inference dead-ends `Ok(...)` / `Err(...)` (Result<T, _> with
/// unconstrained T) and `[]` (Vec<_> with unconstrained element
/// type) by emitting a `serde_json::json!` literal directly. The
/// wire shape matches what `serde_json::to_value` would have produced
/// for the corresponding Vox Result / List.
///
/// Returns `None` when the expression isn't one of the inference-
/// fragile shapes; the caller then falls back to the generic
/// `serde_json::to_value(...)` form.
///
/// Per the 2026-05-23 slot-2 todo-auth bring-up.
fn route_json_shortcut(
    v: &HirExpr,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> Option<String> {
    let emit = |e: &HirExpr| {
        emit_expr_with(
            e,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            OwnershipMode::Owned,
        )
    };
    match v {
        HirExpr::Call(callee, args, _await, _span) => {
            if let HirExpr::Ident(name, _) = &**callee
                && args.len() == 1
            {
                match name.as_str() {
                    "Ok" => Some(format!(
                        "serde_json::json!({{ \"Ok\": {} }})",
                        emit(&args[0].value)
                    )),
                    "Error" | "Err" => Some(format!(
                        "serde_json::json!({{ \"Err\": {} }})",
                        emit(&args[0].value)
                    )),
                    _ => None,
                }
            } else {
                None
            }
        }
        HirExpr::ListLit(items, _) if items.is_empty() => {
            Some("serde_json::Value::Array(Vec::new())".to_string())
        }
        _ => None,
    }
}

/// Try to emit a builtin function call by name (the `Call(Ident("..."), ...)`
/// shape). Returns `None` if the ident isn't a recognized builtin, in which
/// case the caller falls through to namespace / generic dispatch.
///
/// Extracted from `emit_expr_with` per CR-A1 plan §5.6 refactoring pass:
/// the inline if-chain contributed ~16 decision points and obscured the
/// dispatcher pattern.
fn emit_builtin_ident_call<F>(name: &str, args: &[HirArg], emit: &F) -> Option<String>
where
    F: Fn(&HirExpr, OwnershipMode) -> String,
{
    match (name, args.len()) {
        // broadcast(msg) — actor handler body emit. Pushes the payload
        // through SubscriptionManager::notify_payload keyed on the
        // runtime-resolved actor name (env var VOX_BROADCAST_CHANNEL,
        // set when the actor body is spawned). The payload is
        // as_string-coerced so broadcast(42) and broadcast("hi") both
        // work. Symmetric send side that pairs with the SSE handler's
        // subscribe_payload(...) bridge (B5).
        ("broadcast", 1) => Some(format!(
            "{{ let __vox_mgr = ::vox_actor_runtime::SubscriptionManager::default(); \
             let __vox_ch = std::env::var(\"VOX_BROADCAST_CHANNEL\").unwrap_or_default(); \
             __vox_mgr.notify_payload(&__vox_ch, as_string(&{})).await; }}",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        ("str", 1) => Some(format!(
            "as_string(&{})",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        ("assert", 1) => {
            if let HirExpr::Binary(HirBinOp::Is, l, r, _) = &args[0].value {
                Some(format!(
                    "assert_eq!({}, {})",
                    emit(l.as_ref(), OwnershipMode::Owned),
                    emit(r.as_ref(), OwnershipMode::Owned)
                ))
            } else {
                Some(format!(
                    "assert!({})",
                    emit(&args[0].value, OwnershipMode::Owned)
                ))
            }
        }
        ("assert_eq", n) if n >= 2 => Some(format!(
            "assert_eq!({}, {})",
            emit(&args[0].value, OwnershipMode::Owned),
            emit(&args[1].value, OwnershipMode::Owned)
        )),
        ("assert_ne", n) if n >= 2 => Some(format!(
            "assert_ne!({}, {})",
            emit(&args[0].value, OwnershipMode::Owned),
            emit(&args[1].value, OwnershipMode::Owned)
        )),
        ("print", 1) => Some(format!(
            "println!(\"{{}}\", {})",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        // len works on Vec / String / &str (db.Table.all() lowers to Vec).
        ("len", 1) => Some(format!(
            "({}).len()",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        // Vox `Error(VariantName(payload))` is the Result-error
        // constructor. The Vox surface spells it `Error(...)` (matches
        // the ADT-variant terminology); Rust's std spells it `Err(...)`.
        // Without this rewrite, codegen emits a bare `Error(...)` call
        // and rustc errors with "cannot find function Error". Per the
        // 2026-05-23 slot-2 todo-auth bring-up.
        //
        // Type inference at the `return` site is the route wrapper's
        // job — see `emit_stmt`'s HirStmt::Return arm for route
        // contexts, which now wraps Err(...) in `serde_json::json!`
        // form so `serde_json::to_value` doesn't choke on Result<T, _>
        // with unconstrained T.
        ("Error", 1) => Some(format!(
            "Err({})",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        // `panic` is a Rust macro, not a function — emit `panic!(..)`.
        ("panic", 1) => Some(format!(
            "panic!(\"{{}}\", {})",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        // `range(n)` / `range(start, end)` materialize an integer list, matching
        // the interpreter (which returns a `VoxValue::List` of ints).
        ("range", 1) => Some(format!(
            "(0..({}) as i64).map(|__i| __i as i64).collect::<Vec<i64>>()",
            emit(&args[0].value, OwnershipMode::Owned)
        )),
        ("range", 2) => Some(format!(
            "(({}) as i64..({}) as i64).map(|__i| __i as i64).collect::<Vec<i64>>()",
            emit(&args[0].value, OwnershipMode::Owned),
            emit(&args[1].value, OwnershipMode::Owned)
        )),
        _ => None,
    }
}

/// Raw `vox_actor_runtime::builtins::…` invoke (`std.*` root calls).
fn emit_registry_runtime_call(namespace: &str, fn_name: &str, args: &[String]) -> Option<String> {
    let entry = lookup_builtin(namespace, fn_name, args.len())?;
    let symbol = entry.runtime_symbol?;
    let kinds: Vec<BuiltinArgKind> = if entry.arg_kinds.is_empty() {
        vec![BuiltinArgKind::Str; args.len()]
    } else {
        entry.arg_kinds.to_vec()
    };
    if kinds.len() != args.len() {
        return None;
    }
    let mut parts = Vec::with_capacity(args.len());
    for (k, a) in kinds.iter().zip(args.iter()) {
        parts.push(match k {
            BuiltinArgKind::Str => format!("({a}).as_str()"),
            BuiltinArgKind::Bool => a.clone(),
            BuiltinArgKind::Int => format!("({a}) as u64"),
        });
    }
    Some(format!("{}({})", symbol, parts.join(", ")))
}

/// `OpenClaw.*` / `Browser.*` → Vox `Result` ADT (`Browser` is `wasm32`-guarded).
fn emit_openclaw_or_browser_registry_call(
    module_name: &str,
    fn_name: &str,
    args: &[String],
) -> Option<String> {
    let inv = emit_registry_runtime_call(module_name, fn_name, args)?;
    let entry = lookup_builtin(module_name, fn_name, args.len())?;
    let inner = if entry.returns_unit {
        format!("match {inv} {{ Ok(()) => Ok(()), Err(m) => Error(m) }}")
    } else {
        format!("match {inv} {{ Ok(v) => Ok(v), Err(m) => Error(m) }}")
    };
    if module_name == "Browser" {
        Some(format!(
            "({{ #[cfg(target_arch = \"wasm32\")] {{ Error(\"Browser.* is not available in WASI scripts\".to_string()) }} #[cfg(not(target_arch = \"wasm32\"))] {{ {inner} }} }})"
        ))
    } else {
        Some(format!("({inner})"))
    }
}

/// Helper to determine if a builtin function argument should be passed by reference.
fn is_builtin_arg_borrowed(namespace: &str, fn_name: &str, arg_index: usize) -> bool {
    matches!(
        (namespace, fn_name, arg_index),
        ("fs", "read" | "read_to_string" | "write" | "remove_file", 0)
            | ("path", "exists" | "is_dir" | "is_file", 0)
            | ("env", "get" | "set" | "remove", 0)
            | ("http", "get" | "post" | "put" | "delete", 0)
            | ("std", "print" | "println", _)
    )
}

#[cfg(test)]
mod scrape_emit_tests {
    use super::emit_openclaw_or_browser_registry_call;

    #[test]
    fn scrape_lowers_to_runtime_symbol_without_wasm_guard() {
        let out = emit_openclaw_or_browser_registry_call("Scrape", "fetch", &["url".to_string()])
            .expect("Scrape.fetch should be in the builtin registry");
        assert!(
            out.contains("vox_actor_runtime::builtins::vox_scrape_fetch((url).as_str())"),
            "unexpected emit: {out}"
        );
        // Static scraping is pure-Rust — it must NOT carry the Browser wasm32 guard.
        assert!(
            !out.contains("wasm32"),
            "Scrape.* must not be wasm-guarded: {out}"
        );
        assert!(out.contains("Ok(v) => Ok(v)") && out.contains("Err(m) => Error(m)"));
    }

    #[test]
    fn scrape_select_attr_lowers_three_args() {
        let out = emit_openclaw_or_browser_registry_call(
            "Scrape",
            "select_attr",
            &["h".to_string(), "a".to_string(), "href".to_string()],
        )
        .expect("Scrape.select_attr in registry");
        assert!(
            out.contains("vox_scrape_select_attr((h).as_str(), (a).as_str(), (href).as_str())"),
            "unexpected emit: {out}"
        );
    }

    #[test]
    fn browser_still_wasm_guarded() {
        let out = emit_openclaw_or_browser_registry_call(
            "Browser",
            "text",
            &["p".to_string(), "s".to_string()],
        )
        .expect("Browser.text in registry");
        assert!(
            out.contains("wasm32"),
            "Browser.* must keep the wasm guard: {out}"
        );
    }
}

#[cfg(test)]
mod borrow_emission_tests {
    use super::OwnershipMode;
    use super::emit_ident_expr;
    use std::collections::HashMap;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::HirType;

    fn typed(name: &str) -> (Span, HashMap<Span, HirType>) {
        let span = Span::new(0, 0);
        let mut m = HashMap::new();
        m.insert(span, HirType::Named(name.to_string()));
        (span, m)
    }

    /// `str`-typed borrowed args keep `.as_str()` (what borrowing string builtins
    /// expect) — preserves existing behavior, no golden churn.
    #[test]
    fn borrowed_str_emits_as_str() {
        let (span, types) = typed("str");
        let out = emit_ident_expr("x", &span, Some(&types), None, OwnershipMode::Borrowed);
        assert_eq!(out, "x.as_str()");
    }

    /// Non-`str` borrowed args emit a plain reference, NOT `.as_str()` (which
    /// would be uncompilable on a `Vec`/struct). This is the latent-bug fix:
    /// before, this returned `x.as_str()` regardless of type.
    #[test]
    fn borrowed_non_str_emits_reference() {
        let (span, types) = typed("MyRecord");
        let out = emit_ident_expr("x", &span, Some(&types), None, OwnershipMode::Borrowed);
        assert_eq!(out, "&x", "non-str borrow must be `&x`, not `.as_str()`");
    }

    /// Owned, non-last-use, non-Copy still clones (unchanged).
    #[test]
    fn owned_non_copy_still_clones() {
        let (span, types) = typed("str");
        let out = emit_ident_expr("x", &span, Some(&types), None, OwnershipMode::Owned);
        assert_eq!(out, "x.clone()");
    }
}
