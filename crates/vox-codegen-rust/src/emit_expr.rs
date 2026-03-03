use vox_hir::{HirBinOp, HirExpr, HirPattern, HirStmt};

pub fn emit_stmt(stmt: &HirStmt, indent: usize, is_route: bool, is_actor: bool) -> String {
    emit_stmt_inner(stmt, indent, is_route, is_actor, false)
}

/// Like emit_stmt but for warp route handlers (emits Ok(warp::reply::json(...))).
pub fn emit_stmt_warp(stmt: &HirStmt, indent: usize, is_route: bool, is_actor: bool) -> String {
    emit_stmt_inner(stmt, indent, is_route, is_actor, true)
}

/// Emit statement; use_warp=true emits warp::reply::json for route returns.
fn emit_stmt_inner(
    stmt: &HirStmt,
    indent: usize,
    is_route: bool,
    is_actor: bool,
    use_warp: bool,
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
            format!(
                "{pad}let {}{} = {};\n",
                mut_kw,
                emit_pattern(pattern),
                emit_expr(value)
            )
        }
        HirStmt::Assign { target, value, .. } => {
            format!("{pad}{} = {};\n", emit_expr(target), emit_expr(value))
        }
        HirStmt::Return { value, .. } => {
            if is_actor {
                if let Some(v) = value {
                    format!(
                        "{pad}let _ = {}; // return ignored in actor; scaffolding only\n",
                        emit_expr(v)
                    )
                } else {
                    format!("{pad}// return ignored in actor; scaffolding only\n")
                }
            } else if let Some(v) = value {
                let expr_str = emit_expr(v);
                if is_route {
                    if use_warp {
                        format!(
                            "{pad}return Ok(warp::reply::json(&serde_json::to_value({}).unwrap_or(serde_json::Value::Null)));\n",
                            expr_str
                        )
                    } else {
                        format!(
                            "{pad}return Json(serde_json::to_value({}).unwrap_or(serde_json::Value::Null));\n",
                            expr_str
                        )
                    }
                } else {
                    format!("{pad}return {};\n", expr_str)
                }
            } else if is_route {
                if use_warp {
                    format!("{pad}return Ok(warp::reply::json(&serde_json::Value::Null));\n")
                } else {
                    format!("{pad}return Json(serde_json::Value::Null);\n")
                }
            } else {
                format!("{pad}return;\n")
            }
        }
        HirStmt::Expr { expr, .. } => {
            format!("{pad}{};\n", emit_expr(expr))
        }
        HirStmt::Break { label, value, .. } => {
            let label_str = label
                .as_ref()
                .map(|l| format!(" '{} ", l))
                .unwrap_or_default();
            if let Some(v) = value {
                format!("{pad}break{} {};\n", label_str, emit_expr(v))
            } else {
                format!("{pad}break{};\n", label_str.trim_end())
            }
        }
        HirStmt::Continue { label, .. } => {
            let label_str = label
                .as_ref()
                .map(|l| format!(" '{} ", l))
                .unwrap_or_default();
            format!("{pad}continue{};\n", label_str.trim_end())
        }
        HirStmt::Emit { value, .. } => format!("{pad}_emit_tx.send({}).ok();\n", emit_expr(value)),
        HirStmt::Use { path, .. } => format!(
            "{pad}use {};\n",
            path.join("::").replace('/', "::").replace("-", "_")
        ),
        HirStmt::For {
            label,
            binding,
            index_binding,
            iterable,
            body,
            ..
        } => {
            let iter_str = emit_expr(iterable);
            let mut out = String::new();
            let label_str = label
                .as_ref()
                .map(|l| format!("'{}: ", l))
                .unwrap_or_default();

            // Check if binding is a tuple of exactly 2 elements (index, item)
            // or if index_binding is explicitly set.
            let is_enumerated = index_binding.is_some() || matches!(binding, HirPattern::Tuple(elems, _) if elems.len() == 2);
            let pat_str = emit_pattern(binding);

            if is_enumerated {
                let bind_str = if let Some(idx) = index_binding {
                    format!("({idx}, {pat_str})")
                } else {
                    pat_str // already looks like (i, item)
                };
                out.push_str(&format!(
                    "{pad}{}for {} in {iter_str}.into_iter().enumerate() {{\n",
                    label_str,
                    bind_str
                ));
            } else {
                out.push_str(&format!(
                    "{pad}{}for {pat_str} in {iter_str} {{\n",
                    label_str
                ));
            }
            for stmt in body {
                out.push_str(&emit_stmt_inner(
                    stmt,
                    indent + 1,
                    is_route,
                    is_actor,
                    use_warp,
                ));
            }
            out.push_str(&format!("{pad}}}\n"));
            out
        }
    }
}

pub fn emit_pattern(pat: &HirPattern) -> String {
    match pat {
        HirPattern::Ident(n, _) => n.clone(),
        HirPattern::Wildcard(_) => "_".into(),
        HirPattern::Literal(lit, _) => emit_expr(lit),
        HirPattern::Tuple(pats, _) => format!(
            "({})",
            pats.iter().map(emit_pattern).collect::<Vec<_>>().join(", ")
        ),
        HirPattern::Constructor(n, pats, _) => {
            format!(
                "{}({})",
                n,
                pats.iter().map(emit_pattern).collect::<Vec<_>>().join(", ")
            )
        }
        HirPattern::Or(pats, _) => pats
            .iter()
            .map(emit_pattern)
            .collect::<Vec<_>>()
            .join(" | "),
        HirPattern::Binding(name, inner, _) => {
            format!("{} @ {}", name, emit_pattern(inner))
        }
        HirPattern::Rest(name, _) => {
            if let Some(n) = name {
                format!("{}@..", n)
            } else {
                "..".into()
            }
        }
        HirPattern::Record(fields, _) => {
            let flds: Vec<String> = fields
                .iter()
                .map(|(name, pat)| {
                    if let Some(p) = pat {
                        format!("{}: {}", name, emit_pattern(p))
                    } else {
                        name.clone()
                    }
                })
                .collect();
            format!("{{ {} }}", flds.join(", "))
        }
    }
}

pub fn emit_expr(expr: &HirExpr) -> String {
    match expr {
        HirExpr::IntLit(v, _) => v.to_string(),
        HirExpr::FloatLit(v, _) => v.to_string(),
        HirExpr::StringLit(v, _) => format!("\"{}\".to_string()", v),
        HirExpr::BoolLit(v, _) => v.to_string(),
        HirExpr::Ident(n, _) => {
            if n == "request" {
                "request".into()
            } else if n.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                n.clone()
            } else {
                format!("{}.clone()", n)
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
                HirBinOp::BitAnd => "&",
                HirBinOp::BitOr => "|",
                HirBinOp::BitXor => "^",
                HirBinOp::Shl => "<<",
                HirBinOp::Shr => ">>",
                HirBinOp::Range => return format!("({})..({})", emit_expr(l), emit_expr(r)),
                HirBinOp::RangeInclusive => {
                    return format!("({})..=({})", emit_expr(l), emit_expr(r))
                }
                HirBinOp::NullCoalesce => {
                    return format!("{}.unwrap_or({})", emit_expr(l), emit_expr(r))
                }
                HirBinOp::Pipe => return format!("{}({})", emit_expr(r), emit_expr(l)),
            };
            format!("({} {} {})", emit_expr(l), op_str, emit_expr(r))
        }
        HirExpr::Unary(op, e, _) => {
            let op_str = match op {
                vox_hir::HirUnOp::Not => "!",
                vox_hir::HirUnOp::Neg => "-",
                vox_hir::HirUnOp::BitNot => "!",
            };
            format!("({}{})", op_str, emit_expr(e))
        }
        HirExpr::Call(callee, args, is_await, _) => {
            // Special-case: assertion builtins → Rust macros
            if let HirExpr::Ident(name, _) = &**callee {
                match name.as_str() {
                    "assert" if !args.is_empty() => {
                        if let vox_hir::HirExpr::Binary(vox_hir::HirBinOp::Is, l, r, _) =
                            &args[0].value
                        {
                            return format!("assert_eq!({}, {})", emit_expr(l), emit_expr(r));
                        } else {
                            return format!("assert!({})", emit_expr(&args[0].value));
                        }
                    }
                    "assert_eq" if args.len() >= 2 => {
                        return format!(
                            "assert_eq!({}, {})",
                            emit_expr(&args[0].value),
                            emit_expr(&args[1].value)
                        );
                    }
                    "assert_ne" if args.len() >= 2 => {
                        return format!(
                            "assert_ne!({}, {})",
                            emit_expr(&args[0].value),
                            emit_expr(&args[1].value)
                        );
                    }
                    "assert_match" if !args.is_empty() => {
                        return format!("assert!(!{}.is_empty())", emit_expr(&args[0].value));
                    }
                    "print" if !args.is_empty() => {
                        return format!("println!(\"{{:?}}\", {})", emit_expr(&args[0].value));
                    }
                    "llm_embed" if !args.is_empty() => {
                        return format!(
                            "(if let vox_runtime::ActivityResult::Ok(Ok(v)) = vox_runtime::llm::llm_embed(&vox_runtime::ActivityOptions::default(), &{}, vox_runtime::llm::LlmConfig::openai(\"openai/text-embedding-3-small\")).await {{ v }} else {{ Vec::new() }})",
                            emit_expr(&args[0].value)
                        );
                    }
                    _ => {}
                }
            }
            // Special-case: std.fs / std.path builtins
            // Detect calls on field-access chains: std.fs.read(path) → FieldAccess(FieldAccess(Ident("std"), "fs"), "read")
            if let HirExpr::FieldAccess(namespace_expr, fn_name, _) = &**callee {
                // First check single-level: std.uuid() → FieldAccess(Ident("std"), fn_name)
                if let HirExpr::Ident(std_kw, _) = &**namespace_expr {
                    if std_kw == "std" {
                        let a: Vec<_> = args.iter().map(|arg| emit_expr(&arg.value)).collect();
                        match fn_name.as_str() {
                            "uuid" => return "vox_runtime::builtins::vox_uuid()".to_string(),
                            "now_ms" => return "vox_runtime::builtins::vox_now_ms()".to_string(),
                            "hash_fast" if !a.is_empty() => return format!("vox_runtime::builtins::vox_hash_fast(&{})", a[0]),
                            "hash_secure" if !a.is_empty() => return format!("vox_runtime::builtins::vox_hash_secure(&{})", a[0]),
                            _ => {}
                        }
                    }
                }
                // Two-level: std.crypto.hash_fast(x) / std.fs.read(x) / etc.
                if let HirExpr::FieldAccess(std_expr, ns_name, _) = &**namespace_expr {
                    if let HirExpr::Ident(std_kw, _) = &**std_expr {
                        if std_kw == "std" {
                            let a: Vec<_> = args.iter().map(|arg| emit_expr(&arg.value)).collect();
                            let builtin = match (ns_name.as_str(), fn_name.as_str()) {
                                // crypto namespace
                                ("crypto", "hash_fast") if !a.is_empty() =>
                                    format!("vox_runtime::builtins::vox_hash_fast(&{})", a[0]),
                                ("crypto", "hash_secure") if !a.is_empty() =>
                                    format!("vox_runtime::builtins::vox_hash_secure(&{})", a[0]),
                                ("crypto", "uuid") =>
                                    "vox_runtime::builtins::vox_uuid()".to_string(),
                                // time namespace
                                ("time", "now_ms") =>
                                    "vox_runtime::builtins::vox_now_ms()".to_string(),
                                // fs namespace
                                ("fs", "read") if !a.is_empty() =>
                                    format!("std::fs::read_to_string({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?", a[0]),
                                ("fs", "write") if a.len() >= 2 =>
                                    format!("std::fs::write({}, {}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?", a[0], a[1]),
                                ("fs", "exists") if !a.is_empty() =>
                                    format!("std::path::Path::new(&{}).exists()", a[0]),
                                ("fs", "remove") if !a.is_empty() =>
                                    format!("std::fs::remove_file({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?", a[0]),
                                ("fs", "read_bytes") if !a.is_empty() =>
                                    format!("std::fs::read({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?", a[0]),
                                ("fs", "mkdir") if !a.is_empty() =>
                                    format!("std::fs::create_dir_all({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?", a[0]),
                                ("path", "join") if a.len() >= 2 =>
                                    format!("std::path::Path::new(&{}).join(&{}).to_string_lossy().to_string()", a[0], a[1]),
                                ("path", "basename") if !a.is_empty() =>
                                    format!("std::path::Path::new(&{}).file_name().unwrap_or_default().to_string_lossy().to_string()", a[0]),
                                ("path", "dirname") if !a.is_empty() =>
                                    format!("std::path::Path::new(&{}).parent().unwrap_or(std::path::Path::new(\".\")).to_string_lossy().to_string()", a[0]),
                                ("path", "extension") if !a.is_empty() =>
                                    format!("std::path::Path::new(&{}).extension().unwrap_or_default().to_string_lossy().to_string()", a[0]),
                                _ => {
                                    // Fall through to generic call
                                    let call = format!("{ns_name}::{fn_name}({})", a.join(", "));
                                    return call;
                                }
                            };
                            return builtin;
                        }
                    }
                }
            }
            let c = emit_expr(callee);
            let a: Vec<_> = args.iter().map(|arg| emit_expr(&arg.value)).collect();
            let call = format!("{}({})", c, a.join(", "));
            let needs_await = *is_await
                || c.ends_with("::insert")
                || c.ends_with("::get")
                || c.ends_with("::query")
                || c.ends_with("::delete")
                || c.ends_with(".insert")
                || c.ends_with(".get")
                || c.ends_with(".query")
                || c.ends_with(".delete");
            if needs_await {
                format!("{}.await", call)
            } else {
                call
            }
        }
        HirExpr::MethodCall(obj, m, args, _) => {
            // D12-84: log.info/warn/error → tracing macros
            if let HirExpr::Ident(ref obj_name, _) = **obj {
                if obj_name == "log" && !args.is_empty() {
                    let mut args_iter = args.iter();
                    if let Some(first_arg) = args_iter.next() {
                        let fmt = match &first_arg.value {
                            HirExpr::StringLit(s, _) => format!("\"{}\"", s),
                            other => emit_expr(other),
                        };
                        let remaining: Vec<String> =
                            args_iter.map(|a| emit_expr(&a.value)).collect();
                        let macro_name = match m.as_str() {
                            "info" => "info",
                            "warn" => "warn",
                            "error" => "error",
                            "debug" => "debug",
                            _ => "info",
                        };
                        if remaining.is_empty() {
                            return format!("tracing::{}!(\"{{:?}}\", {})", macro_name, fmt);
                        } else {
                            return format!(
                                "tracing::{}!({}, {})",
                                macro_name,
                                fmt,
                                remaining.join(", ")
                            );
                        }
                    }
                }
            }
            let o = emit_expr(obj);
            let a: Vec<_> = args.iter().map(|arg| emit_expr(&arg.value)).collect();
            let call = format!("{}.{}({})", o, m, a.join(", "));
            let is_table_method = ["insert", "get", "query", "delete"].contains(&m.as_str())
                && o.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
            if is_table_method {
                format!("{}.await", call)
            } else {
                call
            }
        }
        HirExpr::FieldAccess(obj, f, _) => {
            format!("{}.{}", emit_expr(obj), f)
        }
        HirExpr::IndexAccess(obj, idx, _) => {
            format!("{}[{}]", emit_expr(obj), emit_expr(idx))
        }
        HirExpr::Slice(obj, start, end, _) => {
            let s = start.as_ref().map(|e| emit_expr(e)).unwrap_or_default();
            let e = end.as_ref().map(|e| emit_expr(e)).unwrap_or_default();
            format!("{}[{}..{}]", emit_expr(obj), s, e)
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            let mut s = format!("if {} {{\n", emit_expr(cond));
            for stmt in then_b {
                s.push_str(&emit_stmt(stmt, 1, false, false));
            }
            if let Some(eb) = else_b {
                s.push_str("} else {\n");
                for stmt in eb {
                    s.push_str(&emit_stmt(stmt, 1, false, false));
                }
            }
            s.push('}');
            s
        }
        HirExpr::While {
            label,
            condition,
            body,
            ..
        } => {
            let l = label
                .as_ref()
                .map(|lbl| format!("'{lbl}: "))
                .unwrap_or_default();
            let mut s = format!("{l}while {} {{\n", emit_expr(condition));
            for stmt in body {
                s.push_str(&emit_stmt(stmt, 1, false, false));
            }
            s.push('}');
            s
        }
        HirExpr::Loop { label, body, .. } => {
            let l = label
                .as_ref()
                .map(|lbl| format!("'{lbl}: "))
                .unwrap_or_default();
            let mut s = format!("{l}loop {{\n");
            for stmt in body {
                s.push_str(&emit_stmt(stmt, 1, false, false));
            }
            s.push('}');
            s
        }
        HirExpr::For(pattern, iter, body, _) => {
            let is_enumerated = matches!(pattern, HirPattern::Tuple(ref elems, _) if elems.len() == 2);
            let pat_str = emit_pattern(pattern);
            let iter_str = emit_expr(iter);

            let mut s = if is_enumerated {
                format!("for {} in {}.into_iter().enumerate() {{\n", pat_str, iter_str)
            } else {
                format!("for {} in {} {{\n", pat_str, iter_str)
            };
            s.push_str(&emit_expr(body));
            s.push('}');
            s
        }
        HirExpr::Match(subj, arms, _) => {
            let mut s = format!("match {} {{\n", emit_expr(subj));
            for arm in arms {
                s.push_str(&format!(
                    "  {} => {},\n",
                    emit_pattern(&arm.pattern),
                    emit_expr(&arm.body)
                ));
            }
            s.push('}');
            s
        }
        HirExpr::Lambda(params, _, body, _) => {
            let p: Vec<_> = params.iter().map(|param| param.name.clone()).collect();
            format!("|{}| {}", p.join(", "), emit_expr(body))
        }
        HirExpr::Block(stmts, _) => {
            let mut s = "{\n".to_string();
            for stmt in stmts {
                s.push_str(&emit_stmt(stmt, 1, false, false));
            }
            s.push('}');
            s
        }
        HirExpr::StreamBlock(stmts, _) => {
            let mut s = "async_stream::stream! {\n".to_string();
            for stmt in stmts {
                s.push_str(&emit_stmt(stmt, 1, false, false));
            }
            s.push('}');
            s
        }
        HirExpr::TryCatch {
            body,
            catch_binding,
            catch_body,
            ..
        } => {
            let mut s = "{ let _res = (|| async {\n".to_string();
            for stmt in body {
                s.push_str(&emit_stmt(stmt, 2, false, false));
            }
            s.push_str("  })().await;\n");
            s.push_str(&format!("  if let Err({catch_binding}) = _res {{\n"));
            for stmt in catch_body {
                s.push_str(&emit_stmt(stmt, 2, false, false));
            }
            s.push_str("  }\n}");
            s
        }
        HirExpr::Await(e, _) => format!("{}.await", emit_expr(e)),
        HirExpr::TryOp(e, _) => format!("{}?", emit_expr(e)),
        HirExpr::OptionalChain(obj, f, _) => format!("{}?.{}", emit_expr(obj), f),
        HirExpr::Pipe(l, r, _) => format!("{}({})", emit_expr(r), emit_expr(l)),
        HirExpr::Spawn(e, _) => format!("tokio::spawn(async move {{ {} }})", emit_expr(e)),
        HirExpr::With(obj, opts, _) => format!("with({}, {})", emit_expr(obj), emit_expr(opts)),
        HirExpr::StringInterp { parts, .. } => {
            let mut s = "format!(\"".to_string();
            let mut args = Vec::new();
            for part in parts {
                match part {
                    vox_hir::HirStringPart::Literal(l) => s.push_str(&l.replace("\"", "\\\"")),
                    vox_hir::HirStringPart::Interpolation(e) => {
                        s.push_str("{}");
                        args.push(emit_expr(e));
                    }
                }
            }
            s.push('\"');
            for a in args {
                s.push_str(", ");
                s.push_str(&a);
            }
            s.push(')');
            s
        }
        HirExpr::ListLit(items, _) => {
            let it: Vec<_> = items.iter().map(emit_expr).collect();
            format!("vec![{}]", it.join(", "))
        }
        HirExpr::TupleLit(items, _) => {
            let it: Vec<_> = items.iter().map(emit_expr).collect();
            format!("({})", it.join(", "))
        }
        HirExpr::ObjectLit(fields, _) => {
            let f: Vec<_> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", emit_expr(v)))
                .collect();
            format!("Object {{ {} }}", f.join(", "))
        }
        HirExpr::MapLit(items, _) => {
            let mut s = "{ let mut m = HashMap::new(); ".to_string();
            for (k, v) in items {
                s.push_str(&format!("m.insert({}, {}); ", emit_expr(k), emit_expr(v)));
            }
            s.push_str("m }");
            s
        }
        HirExpr::SetLit(items, _) => {
            let mut s = "{ let mut s = HashSet::new(); ".to_string();
            for v in items {
                s.push_str(&format!("s.insert({}); ", emit_expr(v)));
            }
            s.push_str("s }");
            s
        }
        HirExpr::ListComprehension { .. } | HirExpr::Jsx(_) | HirExpr::JsxSelfClosing(_) => {
            // Validation rejects these at codegen time; this branch is unreachable
            "std::unreachable!(\"validated at codegen\")".into()
        }
        HirExpr::TypeCast(expr, t, _) => {
            // Very simplistic type casting for code-gen
            format!("({} as {:?})", emit_expr(expr), t)
        }
    }
}
