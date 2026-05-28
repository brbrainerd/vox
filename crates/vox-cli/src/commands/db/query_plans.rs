//! Lowered DB query plan collection (`vox db explain`).

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct QueryPlanExplainRow {
    pub(crate) query_fn: String,
    pub(crate) route_path: String,
    pub(crate) plan: vox_compiler::hir::HirDbQueryPlan,
}

#[allow(dead_code)]
fn fallback_plan_from_db_op(
    table: &str,
    op: vox_compiler::hir::HirDbTableOp,
    select_cols: &Option<Vec<String>>,
    order_by: &Option<(String, bool)>,
    limit: &Option<Box<vox_compiler::hir::HirExpr>>,
) -> vox_compiler::hir::HirDbQueryPlan {
    vox_compiler::hir::HirDbQueryPlan {
        table: table.to_string(),
        op,
        predicate: None,
        projection: select_cols.clone(),
        order_by: order_by.clone(),
        has_limit: limit.is_some(),
        capabilities: vox_compiler::hir::HirDbPlanCapabilities::default(),
    }
}

fn collect_query_plans_expr(
    expr: &vox_compiler::hir::HirExpr,
    out: &mut Vec<vox_compiler::hir::HirDbQueryPlan>,
) {
    use vox_compiler::hir::HirExpr;
    match expr {
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                collect_query_plans_expr(v, out);
            }
        }
        HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
            for it in items {
                collect_query_plans_expr(it, out);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            collect_query_plans_expr(l, out);
            collect_query_plans_expr(r, out);
        }
        HirExpr::Unary(_, e, _) => collect_query_plans_expr(e, out),
        HirExpr::Call(callee, args, _, _) => {
            collect_query_plans_expr(callee, out);
            for a in args {
                collect_query_plans_expr(&a.value, out);
            }
        }
        HirExpr::MethodCall(obj, _, args, plan_opt, _) => {
            if let Some(plan) = plan_opt {
                out.push(*plan.clone());
            }
            collect_query_plans_expr(obj, out);
            for a in args {
                collect_query_plans_expr(&a.value, out);
            }
        }
        HirExpr::FieldAccess(o, _, _) => collect_query_plans_expr(o, out),
        HirExpr::Match(subj, arms, _) => {
            collect_query_plans_expr(subj, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_query_plans_expr(g, out);
                }
                collect_query_plans_expr(&arm.body, out);
            }
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            collect_query_plans_expr(cond, out);
            for st in then_b {
                collect_query_plans_stmt(st, out);
            }
            if let Some(else_stmts) = else_b {
                for st in else_stmts {
                    collect_query_plans_stmt(st, out);
                }
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            collect_query_plans_expr(it, out);
            collect_query_plans_expr(body, out);
        }
        HirExpr::Lambda(_, _, body, _, _) => collect_query_plans_expr(body, out),
        HirExpr::Spawn(target, _) => collect_query_plans_expr(target, out),
        HirExpr::With(base, opts, _) => {
            collect_query_plans_expr(base, out);
            collect_query_plans_expr(opts, out);
        }
        HirExpr::Jsx(el) => {
            for a in &el.attributes {
                collect_query_plans_expr(&a.value, out);
            }
            for c in &el.children {
                collect_query_plans_expr(c, out);
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for a in &el.attributes {
                collect_query_plans_expr(&a.value, out);
            }
        }
        HirExpr::Block(stmts, _) => {
            for st in stmts {
                collect_query_plans_stmt(st, out);
            }
        }
        HirExpr::Try(t) => collect_query_plans_expr(t.target.as_ref(), out),
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                collect_query_plans_expr(c, out);
            }
        }
        HirExpr::Index(obj, idx, _) => {
            collect_query_plans_expr(obj, out);
            collect_query_plans_expr(idx, out);
        }
        HirExpr::AsyncView(v) => {
            if let Some(arm) = &v.fetching_arm {
                collect_query_plans_expr(arm, out);
            }
            if let Some(arm) = &v.empty_arm {
                collect_query_plans_expr(arm, out);
            }
            if let Some(arm) = &v.error_arm {
                collect_query_plans_expr(arm, out);
            }
            if let Some(arm) = &v.ok_arm {
                collect_query_plans_expr(arm, out);
            }
        }
        HirExpr::IntLit(_, _)
        | HirExpr::FloatLit(_, _)
        | HirExpr::DecimalLit(_, _)
        | HirExpr::StringLit(_, _)
        | HirExpr::BoolLit(_, _)
        | HirExpr::Ident(_, _)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn collect_query_plans_stmt(
    stmt: &vox_compiler::hir::HirStmt,
    out: &mut Vec<vox_compiler::hir::HirDbQueryPlan>,
) {
    use vox_compiler::hir::HirStmt;
    match stmt {
        HirStmt::Let { value, .. } => collect_query_plans_expr(value, out),
        HirStmt::Assign { target, value, .. } => {
            collect_query_plans_expr(target, out);
            collect_query_plans_expr(value, out);
        }
        HirStmt::Return { value: Some(v), .. } => collect_query_plans_expr(v, out),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Expr { expr, .. } => collect_query_plans_expr(expr, out),
        HirStmt::While {
            condition, body, ..
        } => {
            collect_query_plans_expr(condition, out);
            for st in body {
                collect_query_plans_stmt(st, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for st in body {
                collect_query_plans_stmt(st, out);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

pub(crate) fn collect_query_fn_plans(
    hir: &vox_compiler::hir::HirModule,
    query_filter: Option<&str>,
) -> Vec<QueryPlanExplainRow> {
    let mut out = Vec::new();
    for q in &hir.endpoint_fns {
        if !matches!(q.kind, vox_compiler::hir::HirEndpointKind::Query) {
            continue;
        }
        if query_filter.is_some_and(|f| f != q.name.as_str()) {
            continue;
        }
        let mut plans = Vec::new();
        for st in &q.body {
            collect_query_plans_stmt(st, &mut plans);
        }
        for plan in plans {
            out.push(QueryPlanExplainRow {
                query_fn: q.name.clone(),
                route_path: q.route_path.clone(),
                plan,
            });
        }
    }
    out
}

/// Print lowered DB query plans (`HirDbQueryPlan`) from `@query` functions in a `.vox` file.
pub async fn explain(
    file: Option<&PathBuf>,
    query: Option<&str>,
    pretty: bool,
    jsonl: bool,
) -> Result<()> {
    let path = file
        .cloned()
        .unwrap_or_else(|| PathBuf::from("src/main.vox"));
    if !path.exists() {
        anyhow::bail!(
            "No source file found at {}. Run `vox db explain --file <path>` to specify one.",
            path.display()
        );
    }
    let result = crate::pipeline::run_frontend(&path, false)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse source for explain: {}", e))?;
    if !result.diagnostics.is_empty() {
        crate::pipeline::print_diagnostics(&result, &path, false);
    }
    if result.has_errors() {
        anyhow::bail!(
            "Cannot explain DB query plans due to {} frontend error(s).",
            result.error_count()
        );
    }
    let plans = collect_query_fn_plans(&result.hir, query);
    if jsonl {
        for row in &plans {
            println!("{}", serde_json::to_string(row)?);
        }
        return Ok(());
    }
    let out = serde_json::json!({
        "file": path.display().to_string(),
        "query_filter": query,
        "query_function_count": result.hir.endpoint_fns.iter().filter(|e| matches!(e.kind, vox_compiler::hir::HirEndpointKind::Query)).count(),
        "plan_count": plans.len(),
        "plans": plans,
    });
    if pretty {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
