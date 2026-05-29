use std::collections::{HashMap, HashSet};

use vox_compiler::hir::{HirDbTableOp, HirExpr, HirModule};

/// Stable Rust method suffix for `SELECT _id, col1, …` projections (columns in declaration order).
pub(crate) fn db_projection_method_suffix(cols: &[String]) -> String {
    cols.join("_")
}

/// Fail codegen when two different column projections collapse to the same Rust helper suffix
/// (`join("_")`), which would emit duplicate/inconsistent `from_row_sel_*` methods.
pub fn validate_db_projection_suffixes_unique(
    table_name: &str,
    projections: &[Vec<String>],
) -> Result<(), miette::Error> {
    let mut by_suffix: HashMap<String, Vec<String>> = HashMap::new();
    for cols in projections {
        let sfx = db_projection_method_suffix(cols);
        if let Some(prev) = by_suffix.get(&sfx) {
            if prev != cols {
                return Err(miette::miette!(
                    "table '{}': `.select([…])` projections {:?} and {:?} both codegen to suffix '{}'; disambiguate column lists (suffix is columns joined with '_')",
                    table_name,
                    prev,
                    cols,
                    sfx
                ));
            }
        } else {
            by_suffix.insert(sfx, cols.clone());
        }
    }
    Ok(())
}

/// Distinct column projections per table (post-HIR normalization — declaration order).
pub fn collect_table_select_projections(module: &HirModule) -> HashMap<String, Vec<Vec<String>>> {
    let mut sets: HashMap<String, HashSet<Vec<String>>> = HashMap::new();
    let mut record = |table_name: &str, cols: &[String]| {
        if cols.is_empty() {
            return;
        }
        sets.entry(table_name.to_string())
            .or_default()
            .insert(cols.to_vec());
    };

    vox_compiler::hir::db_op_walk::for_each_hir_expr_in_module(module, &mut |expr| {
        if let HirExpr::MethodCall(_, _, _, Some(plan), _) = expr
            && matches!(plan.op, HirDbTableOp::All | HirDbTableOp::FilterRecord)
        {
            if let Some(cols) = plan.projection.as_ref() {
                record(&plan.table, cols);
            }
        }
    });

    sets.into_iter()
        .map(|(k, v)| {
            let mut projections: Vec<Vec<String>> = v.into_iter().collect();
            projections.sort();
            (k, projections)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::collect_table_select_projections;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{
        DefId, HirDbPlanCapabilities, HirDbQueryPlan, HirDbTableOp, HirExpr, HirModule,
    };

    #[test]
    fn collects_select_projections_from_endpoint_fn_body() {
        let span = Span::new(0, 0);
        let expr = HirExpr::MethodCall(
            Box::new(HirExpr::Ident("db".to_string(), span)),
            "filter".to_string(),
            vec![],
            Some(Box::new(HirDbQueryPlan {
                table: "Task".to_string(),
                op: HirDbTableOp::FilterRecord,
                predicate: None,
                projection: Some(vec!["done".to_string(), "title".to_string()]),
                order_by: None,
                has_limit: false,
                capabilities: HirDbPlanCapabilities::default(),
            })),
            span,
        );
        let mut module = HirModule::default();
        module.endpoint_fns.push(vox_compiler::hir::HirEndpointFn {
            id: DefId(1),
            name: "TaskList".to_string(),
            params: vec![],
            return_type: None,
            body: vec![vox_compiler::hir::HirStmt::Expr { expr, span }],
            kind: vox_compiler::hir::HirEndpointKind::Query,
            route_path: "/test".to_string(),
            is_pure: false,
            effects: vec![],
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            span,
        });

        let got = collect_table_select_projections(&module);
        let task = got.get("Task").expect("Task projections");
        assert_eq!(task.len(), 1);
        assert_eq!(
            task[0],
            vec!["done".to_string(), "title".to_string()],
            "endpoint fn body projection should be collected"
        );
    }
}
