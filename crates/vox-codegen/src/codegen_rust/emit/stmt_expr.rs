use std::collections::HashMap;
use super::ownership::OwnershipMode;
use vox_compiler::ast::span::Span;
use vox_compiler::builtin_registry::{BuiltinArgKind, lookup_builtin, std_namespace_runtime_call};
use vox_compiler::hir::{HirBinOp, HirExpr, HirPattern, HirStmt, HirType};

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
                    emit_expr_with(value, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned)
                )
            } else {
                format!(
                    "{pad}let {}{} = {};\n",
                    mut_kw,
                    emit_pattern(pattern, is_route, is_actor, mutation_tx),
                    emit_expr_with(value, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned)
                )
            }
        }
        HirStmt::Assign { target, value, .. } => {
            // The target must be an l-value; do not emit `.clone()` on ident targets.
            let lhs = emit_assign_target(target, inferred_types, usage);
            format!(
                "{pad}{lhs} = {};\n",
                emit_expr_with(value, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned)
            )
        }
        HirStmt::Return { value, .. } => {
            if is_actor {
                if let Some(v) = value {
                    format!(
                        "{pad}let _ = {}; // return ignored in actor; scaffolding only\n",
                        emit_expr_with(v, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned)
                    )
                } else {
                    format!("{pad}// return ignored in actor; scaffolding only\n")
                }
            } else if let Some(v) = value {
                let expr_str = emit_expr_with(v, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned);
                let rid_tok = http_error_rid.unwrap_or("None");
                if is_route && mutation_tx {
                    format!(
                        "{pad}return Ok(Json(serde_json::to_value({}).map_err(|e| vox_db::StoreError::Serialization(format!(\"{{}}\", e)))?));\n",
                        expr_str
                    )
                } else if is_route {
                    format!(
                        "{pad}return Ok(Json(serde_json::to_value({expr}).map_err(|e| (
    StatusCode::INTERNAL_SERVER_ERROR,
    Json(vox_http_client::envelope::error_json(\"SERIALIZATION_ERROR\", format!(\"{{}}\", e), {rid}, None)),
))?));\n",
                        expr = expr_str,
                        rid = rid_tok,
                    )
                } else {
                    format!("{pad}return {};\n", expr_str)
                }
            } else if is_route {
                // Both `mutation_tx` and non-mutation routes return the same null
                // body when no expression is supplied; collapsed to a single arm.
                format!("{pad}return Ok(Json(serde_json::Value::Null));\n")
            } else {
                format!("{pad}return;\n")
            }
        }
        HirStmt::Expr { expr, .. } => {
            format!(
                "{pad}{};\n",
                emit_expr_with(expr, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned)
            )
        }
        HirStmt::While {
            condition, body, ..
        } => {
            let mut s = format!(
                "{pad}while {} {{\n",
                emit_expr_with(condition, is_route, is_actor, mutation_tx, inferred_types, usage, OwnershipMode::Owned)
            );
            if is_actor {
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
    emit_stmt(stmt, indent, false, false, false, inferred_types, None, None)
}

/// Emit an assignment l-value target without adding `.clone()`.
///
/// The standard `emit_expr_with` appends `.clone()` to every identifier,
/// which produces invalid Rust like `j.clone() = rhs`. This function emits
/// a bare identifier or a simple field-access path instead.
fn emit_assign_target(expr: &HirExpr, inferred_types: Option<&HashMap<Span, HirType>>, usage: Option<&super::usage::UsageTracker>) -> String {
    match expr {
        HirExpr::Ident(n, _) => n.clone(),
        HirExpr::FieldAccess(obj, field, _) => {
            format!("{}.{}", emit_assign_target(obj, inferred_types, usage), field)
        }
        // Fallback: use the generic emitter for complex lvalues (index ops etc.)
        other => emit_expr_with(other, false, false, false, inferred_types, usage, OwnershipMode::Owned),
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
        HirPattern::Literal(lit, _) => emit_expr_with(lit, is_route, is_actor, mutation_tx, None, None, OwnershipMode::Owned),
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
        emit_expr_with(
            e,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            m,
        )
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
            elements.iter().map(|e| emit(e, OwnershipMode::Owned)).collect::<Vec<_>>().join(", ")
        ),
        HirExpr::TupleLit(elements, _) => format!(
            "({})",
            elements.iter().map(|e| emit(e, OwnershipMode::Owned)).collect::<Vec<_>>().join(", ")
        ),

        HirExpr::Ident(n, span) => {
            if n == "request"
                || n == "std"
                || n == "fs"
                || n.chars().next().is_some_and(|c| c.is_uppercase())
            {
                n.clone()
            } else {
                let is_copy = inferred_types.and_then(|m| m.get(span)).is_some_and(|t| {
                    matches!(
                        t,
                        HirType::Named(name) if matches!(name.as_str(), "int" | "bool" | "float" | "char" | "dec")
                    ) || matches!(t, HirType::Unit | HirType::Decimal)
                });

                if is_copy {
                    n.clone()
                } else if usage.is_some_and(|u| u.is_last_use(n, *span)) {
                    // Last use of a non-Copy type: move it.
                    n.clone()
                } else {
                    match mode {
                        OwnershipMode::Owned => format!("{}.clone()", n),
                        OwnershipMode::Borrowed => format!("{}.as_str()", n),
                    }
                }
            }
        }
        HirExpr::Binary(op, l, r, _) => {
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
                HirBinOp::Pipe => return format!("{}({})", emit(r, OwnershipMode::Owned), emit(l, OwnershipMode::Owned)),
            };
            if matches!(
                op,
                HirBinOp::Add | HirBinOp::Sub | HirBinOp::Mul | HirBinOp::Div
            ) {
                format!("({} {} &{})", emit(l, OwnershipMode::Owned), op_str, emit(r, OwnershipMode::Owned))
            } else {
                format!("({} {} {})", emit(l, OwnershipMode::Owned), op_str, emit(r, OwnershipMode::Owned))
            }
        }
        HirExpr::Call(callee, args, is_await, _) => {
            if let HirExpr::Ident(n, _) = &**callee {
                // `broadcast(msg)` — actor handler body emit. Pushes the
                // payload through `SubscriptionManager::notify_payload`
                // keyed on the runtime-resolved actor name (env var
                // `VOX_BROADCAST_CHANNEL`, set when the actor body is
                // spawned). The payload is `as_string`-coerced so
                // `broadcast(42)` and `broadcast("hi")` both work.
                //
                // This is the symmetric send side that pairs with the
                // SSE handler's `subscribe_payload(...)` bridge (B5).
                if n == "broadcast" && args.len() == 1 {
                    return format!(
                        "{{ let __vox_mgr = ::vox_actor_runtime::SubscriptionManager::default(); \
                         let __vox_ch = std::env::var(\"VOX_BROADCAST_CHANNEL\").unwrap_or_default(); \
                         __vox_mgr.notify_payload(&__vox_ch, as_string(&{})).await; }}",
                        emit(&args[0].value, OwnershipMode::Owned)
                    );
                }
                if n == "str" && args.len() == 1 {
                    return format!("as_string(&{})", emit(&args[0].value, OwnershipMode::Owned));
                }
                if n == "assert" && args.len() == 1 {
                    if let HirExpr::Binary(HirBinOp::Is, l, r, _) = &args[0].value {
                        return format!("assert_eq!({}, {})", emit(l, OwnershipMode::Owned), emit(r, OwnershipMode::Owned));
                    }
                    return format!("assert!({})", emit(&args[0].value, OwnershipMode::Owned));
                }
                if n == "assert_eq" && args.len() >= 2 {
                    return format!(
                        "assert_eq!({}, {})",
                        emit(&args[0].value, OwnershipMode::Owned),
                        emit(&args[1].value, OwnershipMode::Owned)
                    );
                }
                if n == "assert_ne" && args.len() >= 2 {
                    return format!(
                        "assert_ne!({}, {})",
                        emit(&args[0].value, OwnershipMode::Owned),
                        emit(&args[1].value, OwnershipMode::Owned)
                    );
                }
                if n == "print" && args.len() == 1 {
                    return format!("println!(\"{{}}\", {})", emit(&args[0].value, OwnershipMode::Owned));
                }
                if n == "len" && args.len() == 1 {
                    // Vec, String, &str, etc. — use Rust `.len()` (db.Table.all() lowers to Vec)
                    return format!("({}).len()", emit(&args[0].value, OwnershipMode::Owned));
                }
            }
            // std.* call forms: std.fs.read(path) → FieldAccess(FieldAccess(Ident("std"), "fs"), "read")
            if let HirExpr::FieldAccess(namespace_expr, fn_name, _) = &**callee {
                if let HirExpr::Ident(module_name, _) = &**namespace_expr {
                    let a: Vec<_> = args.iter().map(|arg| emit(&arg.value, OwnershipMode::Owned)).collect();
                    if module_name == "OpenClaw" || module_name == "Browser" {
                        if let Some(expr) =
                            emit_openclaw_or_browser_registry_call(module_name, fn_name, &a)
                        {
                            return expr;
                        }
                    } else if module_name == "fs" {
                        if let Some(call) = std_namespace_runtime_call("fs", fn_name, &a) {
                            return call;
                        }
                    }
                }
                if let HirExpr::Ident(std_kw, _) = &**namespace_expr {
                    if std_kw == "std" {
                        let a: Vec<_> = args
                            .iter()
                            .enumerate()
                            .map(|(i, arg)| {
                                let mode = if is_builtin_arg_borrowed("std", fn_name, i) {
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
                            .collect();
                        if let Some(call) = emit_registry_runtime_call("std", fn_name, &a) {
                            return if *is_await {
                                format!("{}.await", call)
                            } else {
                                call
                            };
                        }
                    }
                }
                if let HirExpr::FieldAccess(std_expr, ns_name, _) = &**namespace_expr {
                    if let HirExpr::Ident(std_kw, _) = &**std_expr {
                        if std_kw == "std" {
                            let a: Vec<_> = args
                                .iter()
                                .enumerate()
                                .map(|(i, arg)| {
                                    let mode = if is_builtin_arg_borrowed(ns_name, fn_name, i) {
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
                                .collect();
                            let builtin =
                                std_namespace_runtime_call(ns_name.as_str(), fn_name.as_str(), &a);
                            if let Some(b) = builtin {
                                return if *is_await { format!("{}.await", b) } else { b };
                            }
                            let call = format!("::std::{}::{}({})", ns_name, fn_name, a.join(", "));
                            return if *is_await {
                                format!("{}.await", call)
                            } else {
                                call
                            };
                        }
                    }
                }
            }
            let c = emit(callee, OwnershipMode::Owned);
            let a: Vec<_> = args.iter().map(|arg| emit(&arg.value, OwnershipMode::Owned)).collect();
            if *is_await {
                format!("{}({}).await", c, a.join(", "))
            } else {
                format!("{}({})", c, a.join(", "))
            }
        }
        HirExpr::Index(obj, idx, _) => {
            format!("{}[{} as usize]", emit(obj, OwnershipMode::Owned), emit(idx, OwnershipMode::Owned))
        }
        _ => unreachable!(
            "HIR expr variants not handled in stmt_expr::emit_expr_with must be handled by stmt_expr_tail (delegate order bug)"
        ),
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
    match (namespace, fn_name, arg_index) {
        ("fs", "read" | "read_to_string" | "write" | "remove_file", 0) => true,
        ("path", "exists" | "is_dir" | "is_file", 0) => true,
        ("env", "get" | "set" | "remove", 0) => true,
        ("http", "get" | "post" | "put" | "delete", 0) => true,
        ("std", "print" | "println", _) => true,
        _ => false,
    }
}
