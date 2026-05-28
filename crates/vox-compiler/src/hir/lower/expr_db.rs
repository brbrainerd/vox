//! DB table / query chain lowering helpers for `LowerCtx::lower_expr`.

use crate::ast::expr::{self, Expr};
use crate::hir::*;

use super::LowerCtx;

pub(super) fn db_table_handle_name(obj: &HirExpr) -> Option<String> {
    let HirExpr::FieldAccess(inner, table_name, _) = obj else {
        return None;
    };
    let HirExpr::Ident(db_name, _) = inner.as_ref() else {
        return None;
    };
    if db_name != "db" {
        return None;
    }
    Some(table_name.clone())
}

pub(super) fn extract_filter_record_args(args: &[HirArg]) -> Option<Vec<HirArg>> {
    if args.len() != 1 {
        return None;
    }
    let v = &args[0].value;
    let HirExpr::ObjectLit(fields, _) = v else {
        return None;
    };
    Some(
        fields
            .iter()
            .map(|(k, val)| HirArg {
                name: Some(k.clone()),
                value: val.clone(),
            })
            .collect(),
    )
}

pub(super) fn db_table_op_from_field(
    obj: &HirExpr,
    method: &str,
) -> Option<(String, HirDbTableOp)> {
    let HirExpr::FieldAccess(inner, table_name, _) = obj else {
        return None;
    };
    let HirExpr::Ident(db_name, _) = inner.as_ref() else {
        return None;
    };
    if db_name != "db" {
        return None;
    }
    let op = match method {
        "insert" => HirDbTableOp::Insert,
        "get" | "find" => HirDbTableOp::Get,
        "delete" => HirDbTableOp::Delete,
        "all" => HirDbTableOp::All,
        "count" => HirDbTableOp::Count,
        "query" => HirDbTableOp::UnsafeQueryRawClause,
        _ => return None,
    };
    Some((table_name.clone(), op))
}

pub(super) fn extract_count_chain_args(
    ctx: &mut LowerCtx,
    object: &Expr,
) -> Option<(String, Vec<HirArg>)> {
    let Expr::MethodCall {
        object: inner_obj,
        method: inner_method,
        args: inner_args,
        ..
    } = object
    else {
        return None;
    };
    let inner_obj_hir = ctx.lower_expr(inner_obj);
    let table = db_table_handle_name(&inner_obj_hir)?;
    match inner_method.as_str() {
        "all" => Some((table, Vec::new())),
        "filter" | "where" => {
            let lowered: Vec<HirArg> = inner_args
                .iter()
                .map(|a| HirArg {
                    name: a.name.clone(),
                    value: ctx.lower_expr(&a.value),
                })
                .collect();
            let filter_args = extract_filter_record_args(&lowered)?;
            if filter_args.is_empty() {
                None
            } else {
                Some((table, filter_args))
            }
        }
        _ => None,
    }
}

pub(crate) struct DbQueryChain {
    pub(crate) table: String,
    pub(crate) op: HirDbTableOp,
    /// Flattened per-field args carried for downstream codegen. As of
    /// 2026-05-18 the lowering at expr.rs:83 passes the SURFACE args
    /// onto the MethodCall instead of these flattened ones so typecheck
    /// can match the `filter(record)` signature; the plan carries the
    /// predicate info separately.
    #[allow(dead_code)]
    pub(crate) args: Vec<HirArg>,
    pub(crate) predicate: Option<HirDbPredicate>,
    pub(crate) select_cols: Option<Vec<String>>,
    pub(crate) order_by: Option<(String, bool)>,
    pub(crate) limit: Option<HirExpr>,
    pub(crate) capabilities: HirDbPlanCapabilities,
}

pub(super) fn make_db_plan_from_chain(chain: &DbQueryChain) -> HirDbQueryPlan {
    HirDbQueryPlan {
        table: chain.table.clone(),
        op: chain.op,
        predicate: chain.predicate.clone(),
        projection: chain.select_cols.clone(),
        order_by: chain.order_by.clone(),
        has_limit: chain.limit.is_some(),
        capabilities: chain.capabilities.clone(),
    }
}

fn parse_where_object_predicate(
    obj: &HirExpr,
    out_args: &mut Vec<HirArg>,
) -> Option<HirDbPredicate> {
    let HirExpr::ObjectLit(fields, _) = obj else {
        return None;
    };
    let mut out = Vec::new();
    for (k, v) in fields {
        match k.as_str() {
            "and" => {
                let HirExpr::ListLit(items, _) = v else {
                    return None;
                };
                let mut parts = Vec::new();
                for item in items {
                    parts.push(parse_where_object_predicate(item, out_args)?);
                }
                out.push(HirDbPredicate::And(parts));
            }
            "or" => {
                let HirExpr::ListLit(items, _) = v else {
                    return None;
                };
                let mut parts = Vec::new();
                for item in items {
                    parts.push(parse_where_object_predicate(item, out_args)?);
                }
                out.push(HirDbPredicate::Or(parts));
            }
            "not" => {
                out.push(HirDbPredicate::Not(Box::new(parse_where_object_predicate(
                    v, out_args,
                )?)));
            }
            field => {
                if let HirExpr::ObjectLit(op_fields, _) = v
                    && op_fields.len() == 1
                {
                    let (op_name, op_val) = &op_fields[0];
                    match op_name.as_str() {
                        "eq" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Eq {
                                field: field.to_string(),
                            });
                        }
                        "neq" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Neq {
                                field: field.to_string(),
                            });
                        }
                        "lt" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Lt {
                                field: field.to_string(),
                            });
                        }
                        "lte" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Lte {
                                field: field.to_string(),
                            });
                        }
                        "gt" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Gt {
                                field: field.to_string(),
                            });
                        }
                        "gte" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Gte {
                                field: field.to_string(),
                            });
                        }
                        "contains" => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: op_val.clone(),
                            });
                            out.push(HirDbPredicate::Contains {
                                field: field.to_string(),
                            });
                        }
                        "is_null" => {
                            out.push(HirDbPredicate::IsNull {
                                field: field.to_string(),
                            });
                        }
                        "in" => {
                            let HirExpr::ListLit(items, _) = op_val else {
                                return None;
                            };
                            let arity = items.len();
                            if arity == 0 {
                                return None;
                            }
                            for item in items {
                                out_args.push(HirArg {
                                    name: Some(field.to_string()),
                                    value: item.clone(),
                                });
                            }
                            out.push(HirDbPredicate::In {
                                field: field.to_string(),
                                arity,
                            });
                        }
                        _ => {
                            out_args.push(HirArg {
                                name: Some(field.to_string()),
                                value: v.clone(),
                            });
                            out.push(HirDbPredicate::Eq {
                                field: field.to_string(),
                            });
                        }
                    }
                    continue;
                }
                out_args.push(HirArg {
                    name: Some(field.to_string()),
                    value: v.clone(),
                });
                out.push(HirDbPredicate::Eq {
                    field: field.to_string(),
                });
            }
        }
    }
    if out.len() == 1 {
        out.into_iter().next()
    } else {
        Some(HirDbPredicate::And(out))
    }
}

fn parse_select_columns(args: &[expr::Arg]) -> Option<Vec<String>> {
    if args.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    if args.len() == 1 {
        match &args[0].value {
            Expr::ListLit { elements, .. } => {
                for e in elements {
                    match e {
                        Expr::StringLit { value, .. } => out.push(value.clone()),
                        Expr::Ident { name, .. } => out.push(name.clone()),
                        _ => return None,
                    }
                }
            }
            Expr::StringLit { value, .. } => out.push(value.clone()),
            Expr::Ident { name, .. } => out.push(name.clone()),
            _ => return None,
        }
    } else {
        for a in args {
            match &a.value {
                Expr::StringLit { value, .. } => out.push(value.clone()),
                Expr::Ident { name, .. } => out.push(name.clone()),
                _ => return None,
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

pub(super) fn extract_db_query_chain(ctx: &mut LowerCtx, expr: &Expr) -> Option<DbQueryChain> {
    let Expr::MethodCall {
        object,
        method,
        args,
        ..
    } = expr
    else {
        return None;
    };

    if let Some(mut chain) = extract_db_query_chain(ctx, object) {
        apply_chain_modifier(ctx, method, args, &mut chain)?;
        return Some(chain);
    }

    let obj_hir = ctx.lower_expr(object);
    let table = db_table_handle_name(&obj_hir)?;
    match method.as_str() {
        "all" if args.is_empty() => Some(DbQueryChain {
            table,
            op: HirDbTableOp::All,
            args: Vec::new(),
            predicate: None,
            select_cols: None,
            order_by: None,
            limit: None,
            capabilities: HirDbPlanCapabilities::default(),
        }),
        "filter" => {
            let lowered: Vec<HirArg> = args
                .iter()
                .map(|a| HirArg {
                    name: a.name.clone(),
                    value: ctx.lower_expr(&a.value),
                })
                .collect();
            let filter_args = extract_filter_record_args(&lowered)?;
            if filter_args.is_empty() {
                return None;
            }
            Some(DbQueryChain {
                table,
                op: HirDbTableOp::FilterRecord,
                args: filter_args.clone(),
                predicate: Some(HirDbPredicate::And(
                    filter_args
                        .iter()
                        .filter_map(|a| {
                            let k = a.name.clone()?;
                            Some(HirDbPredicate::Eq { field: k })
                        })
                        .collect(),
                )),
                select_cols: None,
                order_by: None,
                limit: None,
                capabilities: HirDbPlanCapabilities::default(),
            })
        }
        "where" => {
            if args.len() != 1 {
                return None;
            }
            let lowered = ctx.lower_expr(&args[0].value);
            let mut filter_args = Vec::new();
            let predicate = parse_where_object_predicate(&lowered, &mut filter_args)?;
            if filter_args.is_empty() && !matches!(predicate, HirDbPredicate::IsNull { .. }) {
                return None;
            }
            Some(DbQueryChain {
                table,
                op: HirDbTableOp::FilterRecord,
                args: filter_args,
                predicate: Some(predicate),
                select_cols: None,
                order_by: None,
                limit: None,
                capabilities: HirDbPlanCapabilities::default(),
            })
        }
        _ => None,
    }
}

/// Apply a chain-modifier method (`order_by`, `limit`, `select`, `sync`,
/// `using`, `live`, `scope`) to an existing query chain. Returns
/// `Some(())` on success, `None` on argument shape mismatch or
/// unrecognized method (so the caller can fall back to base-case
/// handling). Extracted from `extract_db_query_chain` per CR-A1
/// refactoring pass — the inline 7-method match contributed ~14
/// decision points and obscured the dispatch.
fn apply_chain_modifier(
    ctx: &mut LowerCtx,
    method: &str,
    args: &[expr::Arg],
    chain: &mut DbQueryChain,
) -> Option<()> {
    match method {
        "order_by" => apply_order_by(args, chain),
        "limit" => apply_limit(ctx, args, chain),
        "select" => apply_select(args, chain),
        "sync" => apply_sync(args, chain),
        "using" => apply_using(args, chain),
        "live" => apply_live(args, chain),
        "scope" => apply_scope(args, chain),
        _ => None,
    }
}

fn apply_order_by(args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if args.is_empty() || args.len() > 2 {
        return None;
    }
    let col = match &args[0].value {
        Expr::StringLit { value, .. } => value.clone(),
        Expr::Ident { name, .. } => name.clone(),
        _ => return None,
    };
    let asc = if args.len() == 2 {
        match &args[1].value {
            Expr::StringLit { value, .. } => !value.eq_ignore_ascii_case("desc"),
            Expr::BoolLit { value, .. } => *value,
            _ => return None,
        }
    } else {
        true
    };
    chain.order_by = Some((col, asc));
    Some(())
}

fn apply_limit(ctx: &mut LowerCtx, args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if args.len() != 1 {
        return None;
    }
    chain.limit = Some(ctx.lower_expr(&args[0].value));
    Some(())
}

fn apply_select(args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if chain.select_cols.is_some() {
        return None;
    }
    let cols = parse_select_columns(args)?;
    chain.select_cols = Some(cols);
    Some(())
}

fn apply_sync(args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if !args.is_empty() {
        return None;
    }
    chain.capabilities.requires_sync = true;
    Some(())
}

fn apply_using(args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if args.len() != 1 {
        return None;
    }
    let mode = match &args[0].value {
        Expr::StringLit { value, .. } => value.to_ascii_lowercase(),
        Expr::Ident { name, .. } => name.to_ascii_lowercase(),
        _ => return None,
    };
    chain.capabilities.retrieval_mode = match mode.as_str() {
        "fts" | "search" => Some(HirDbRetrievalMode::Fts),
        "vector" => Some(HirDbRetrievalMode::Vector),
        "hybrid" => Some(HirDbRetrievalMode::Hybrid),
        _ => None,
    };
    Some(())
}

fn apply_live(args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if args.len() != 1 {
        return None;
    }
    let topic = match &args[0].value {
        Expr::StringLit { value, .. } => value.clone(),
        Expr::Ident { name, .. } => name.clone(),
        _ => return None,
    };
    chain.capabilities.emits_change_log = true;
    chain.capabilities.live_topic = Some(topic);
    Some(())
}

fn apply_scope(args: &[expr::Arg], chain: &mut DbQueryChain) -> Option<()> {
    if args.len() != 1 {
        return None;
    }
    let scope = match &args[0].value {
        Expr::StringLit { value, .. } => value.clone(),
        Expr::Ident { name, .. } => name.clone(),
        _ => return None,
    };
    chain.capabilities.orchestration_scope = Some(scope);
    Some(())
}
