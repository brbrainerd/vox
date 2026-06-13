use super::ownership::OwnershipMode;
use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirExpr, HirType};

pub(super) fn try_emit_expr_tail<F>(
    expr: &HirExpr,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    fallible_db: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    _mode: OwnershipMode,
    fn_return_type: Option<&HirType>,
    emit: &F,
) -> Option<String>
where
    F: Fn(&HirExpr, OwnershipMode) -> String,
{
    Some(match expr {
        HirExpr::ObjectLit(fields, span) => {
            let struct_from_inferred = |t: &HirType| -> Option<String> {
                match t {
                    // User struct/enum names are PascalCase; skip builtins (`str`, `int`, …).
                    HirType::Named(n)
                        if n != "Json"
                            && n != "Result"
                            && n != "Any"
                            && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()) =>
                    {
                        Some(n.clone())
                    }
                    other => super::types::result_ok_struct_name(other),
                }
            };
            let returns_json = matches!(
                fn_return_type,
                Some(HirType::Named(n)) if n == "Json"
            );
            let inferred_named = if returns_json {
                None
            } else {
                inferred_types
                    .and_then(|m| m.get(span))
                    .and_then(struct_from_inferred)
                    // Script lane: `inferred_types` is empty, so fall back to the
                    // `<Type>_from_json` body hint recorded by `enter_from_json`.
                    .or_else(super::json_as_ctx::current_from_json_struct)
            };
            if let Some(type_name) = inferred_named {
                let props: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", emit(v, OwnershipMode::Owned)))
                    .collect();
                format!("{type_name} {{ {} }}", props.join(", "))
            } else {
                let props: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| {
                        let val = if matches!(
                            v,
                            HirExpr::Ident(name, _)
                                if name == "None" || name == "null"
                        ) {
                            "null".to_string()
                        } else {
                            emit(v, OwnershipMode::Owned)
                        };
                        format!("\"{k}\": {val}")
                    })
                    .collect();
                let json_lit = format!("serde_json::json!({{ {} }})", props.join(", "));
                let is_json_val = inferred_types
                    .and_then(|m| m.get(span))
                    .is_some_and(|t| matches!(t, HirType::Named(n) if n == "Json"));
                if is_json_val {
                    format!("VoxJson({json_lit})")
                } else {
                    json_lit
                }
            }
        }
        HirExpr::MethodCall(obj, method, args, plan, _) => {
            let e = |expr: &HirExpr| emit(expr, OwnershipMode::Owned);
            super::method_emit::emit_method_call(
                &e,
                obj.as_ref(),
                method.as_str(),
                args,
                plan.as_ref().map(|v| &**v),
                fallible_db,
                inferred_types,
            )
        }
        HirExpr::Block(stmts, _) => emit_block_tail(
            stmts,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
        ),
        HirExpr::If(cond, then_b, else_b, _) => emit_if_tail(
            cond,
            then_b,
            else_b.as_deref(),
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            emit,
        ),
        HirExpr::FieldAccess(obj, field, _) => {
            let o = emit(obj, OwnershipMode::Owned);
            if o == "std" && field == "args" {
                "std::env::args().skip(1).map(|s| s.to_string()).collect::<Vec<String>>()"
                    .to_string()
            } else if is_vox_namespace_ident(&o) {
                format!("{}::{}", o, field)
            } else {
                format!("{}.{}", o, field)
            }
        }
        HirExpr::With(operand, options, _) => {
            let e = |expr: &HirExpr| emit(expr, OwnershipMode::Owned);
            super::with_emit::emit_with(&e, operand.as_ref(), options.as_ref())
        }
        HirExpr::Lambda(params, _ret_ty, body, _, _) => {
            let mut s = String::new();
            s.push_str("move | ");
            let param_strs: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            s.push_str(&param_strs.join(", "));
            s.push_str("| ");
            s.push_str(&emit(body, OwnershipMode::Owned));
            s
        }
        HirExpr::Binary(vox_compiler::hir::HirBinOp::Pipe, left, right, _) => {
            format!(
                "({})({})",
                emit(right, OwnershipMode::Owned),
                emit(left, OwnershipMode::Owned)
            )
        }
        HirExpr::For(name, index, iter, body, _, _) => {
            // Indexed form `for v, i in xs` binds the 0-based index too. Mirror
            // the interpreter (eval::expr For arm): enumerate and shadow the
            // index as `i64` (Vox `int`) so body arithmetic on it typechecks.
            // Without this the index var is undefined in the emitted Rust
            // (`E0425 cannot find value i`).
            let mut s = match index {
                Some(idx) => format!(
                    "for ({idx}, {name}) in {}.into_iter().enumerate() {{\n    let {idx} = {idx} as i64;\n",
                    emit(iter, OwnershipMode::Owned),
                ),
                None => format!("for {} in {} {{\n", name, emit(iter, OwnershipMode::Owned)),
            };
            if let HirExpr::Block(stmts, _) = &**body {
                for stmt in stmts {
                    s.push_str(&super::stmt_expr::emit_stmt(
                        stmt,
                        1,
                        is_route,
                        is_actor,
                        mutation_tx,
                        inferred_types,
                        usage,
                        None,
                        None,
                    ));
                }
            } else {
                s.push_str(&format!("  {};\n", emit(body, OwnershipMode::Owned)));
            }
            s.push_str("}\n");
            s
        }
        HirExpr::Unary(op, expr, _) => {
            let op_str = match op {
                vox_compiler::hir::HirUnOp::Not => "!",
                vox_compiler::hir::HirUnOp::Neg => "-",
            };
            format!("{}({})", op_str, emit(expr, OwnershipMode::Owned))
        }
        HirExpr::Match(obj, arms, _) => {
            let mut s = format!("match {} {{\n", emit(obj, OwnershipMode::Owned));
            for arm in arms {
                s.push_str(&format!(
                    "    {} => {{\n",
                    super::stmt_expr::emit_pattern(&arm.pattern, is_route, is_actor, mutation_tx)
                ));
                s.push_str(&emit(&arm.body, OwnershipMode::Owned));
                s.push_str("\n    }\n");
            }
            s.push('}');
            s
        }
        HirExpr::Try(h) => format!("({})?", emit(h.target.as_ref(), OwnershipMode::Owned)),

        _ => return None,
    })
}

// Per-variant tail emitters extracted from `try_emit_expr_tail` per CR-A1
// refactor pass — the inline blocks each carry their own stmt loop +
// option handling, contributing ~4-6 decision points apiece.

/// Emit one branch of an `if`/`else` used as an expression. The last
/// `HirStmt::Expr` in the branch is a tail value (no trailing `;`), matching
/// `emit_block_tail` and Rust if-expression semantics.
fn emit_if_branch_body(
    stmts: &[vox_compiler::hir::HirStmt],
    indent: usize,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> String {
    use vox_compiler::hir::HirStmt;
    let pad = " ".repeat(indent * 4);
    if stmts.is_empty() {
        return String::new();
    }
    let last = stmts.len().saturating_sub(1);
    let mut out = String::new();
    for (i, stmt) in stmts.iter().enumerate() {
        if i == last {
            if let HirStmt::Expr { expr, .. } = stmt {
                out.push_str(&format!(
                    "{pad}{}\n",
                    super::stmt_expr::emit_expr_with(
                        expr,
                        is_route,
                        is_actor,
                        mutation_tx,
                        inferred_types,
                        usage,
                        super::ownership::OwnershipMode::Owned,
                        None,
                    )
                ));
                return out;
            }
        }
        out.push_str(&super::stmt_expr::emit_stmt(
            stmt,
            indent,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            None,
            None,
        ));
    }
    out
}

fn emit_block_tail(
    stmts: &[vox_compiler::hir::HirStmt],
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
) -> String {
    use vox_compiler::hir::HirStmt;
    let mut s = String::from("{\n");
    let last = stmts.len().saturating_sub(1);
    for (i, stmt) in stmts.iter().enumerate() {
        if i == last {
            if let HirStmt::Expr { expr, .. } = stmt {
                // Rust block tail expression: last stmt in a value block must not
                // end with `;` or the block type becomes `()`.
                s.push_str(&format!(
                    "    {}\n",
                    super::stmt_expr::emit_expr_with(
                        expr,
                        is_route,
                        is_actor,
                        mutation_tx,
                        inferred_types,
                        usage,
                        super::ownership::OwnershipMode::Owned,
                        None,
                    )
                ));
                s.push('}');
                return s;
            }
        }
        s.push_str(&super::stmt_expr::emit_stmt(
            stmt,
            1,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
            None,
            None,
        ));
    }
    s.push('}');
    s
}

#[allow(clippy::too_many_arguments)]
fn emit_if_tail<F>(
    cond: &HirExpr,
    then_b: &[vox_compiler::hir::HirStmt],
    else_b: Option<&[vox_compiler::hir::HirStmt]>,
    is_route: bool,
    is_actor: bool,
    mutation_tx: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
    usage: Option<&super::usage::UsageTracker>,
    emit: &F,
) -> String
where
    F: Fn(&HirExpr, OwnershipMode) -> String,
{
    let mut s = format!("if {} {{\n", emit(cond, OwnershipMode::Owned));
    s.push_str(&emit_if_branch_body(
        then_b,
        1,
        is_route,
        is_actor,
        mutation_tx,
        inferred_types,
        usage,
    ));
    s.push_str("    }");
    if let Some(eb) = else_b {
        s.push_str(" else {\n");
        s.push_str(&emit_if_branch_body(
            eb,
            1,
            is_route,
            is_actor,
            mutation_tx,
            inferred_types,
            usage,
        ));
        s.push_str("    }");
    }
    s
}

/// Returns `true` when the identifier is a Vox namespace module that should
/// be lowered to Rust path syntax (`fs::read` rather than `fs.read`).
///
/// Extracted from the `FieldAccess` arm of `try_emit_expr_tail` per CR-A1:
/// the original `||` chain contributed 16 decision points to the caller.
pub(super) fn is_vox_namespace_ident(name: &str) -> bool {
    matches!(
        name,
        "fs" | "path"
            | "env"
            | "process"
            | "csv"
            | "toml"
            | "yaml"
            | "io"
            | "json"
            | "http"
            | "crypto"
            | "time"
            | "log"
            | "mobile"
            | "regex"
            | "agentos"
    )
}
