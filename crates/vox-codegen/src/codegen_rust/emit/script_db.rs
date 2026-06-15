//! Script-mode Codex bootstrap (`vox run` native tier with `@table` types).

use std::collections::HashSet;

use vox_compiler::hir::{HirEndpointFn, HirExpr, HirFn, HirModule, HirStmt, has_async_stmts};

use super::tables::{emit_index_ddl, emit_schema_drift_verify, emit_table_ddl};

thread_local! {
    static SCRIPT_DB_EMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SCRIPT_ASYNC_FNS: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
}

/// True while emitting a script lib so `db.*` lowers to `VOX_SCRIPT_DB`.
pub(crate) fn script_db_emit_mode() -> bool {
    SCRIPT_DB_EMIT.with(|c| c.get())
}

#[allow(dead_code)] // used from stmt_expr when script_db_emit_mode is active
pub(crate) fn script_async_call(name: &str) -> bool {
    SCRIPT_ASYNC_FNS.with(|c| c.borrow().contains(name))
}

pub(crate) fn with_script_db_emit_mode<R>(f: impl FnOnce() -> R) -> R {
    SCRIPT_DB_EMIT.with(|c| {
        c.set(true);
        let out = f();
        c.set(false);
        out
    })
}

pub(crate) fn set_script_async_fns(names: HashSet<String>) {
    SCRIPT_ASYNC_FNS.with(|c| *c.borrow_mut() = names);
}

pub(crate) fn module_uses_db(module: &HirModule) -> bool {
    !module.tables.is_empty()
}

/// Static handle + boot fn prepended to script `lib.rs` when the module has tables.
pub(crate) fn emit_script_db_prelude(module: &HirModule) -> String {
    debug_assert!(!module.tables.is_empty());
    let mut out = String::from(
        "use std::sync::{Arc, OnceLock};\n\
         use vox_db::Codex;\n\n\
         pub static VOX_SCRIPT_DB: OnceLock<Arc<Codex>> = OnceLock::new();\n\n",
    );
    out.push_str("pub async fn vox_script_boot_db() {\n");
    out.push_str("    if VOX_SCRIPT_DB.get().is_some() {\n        return;\n    }\n");
    out.push_str(&emit_codex_connect_and_migrate(module, "    "));
    out.push_str(
        "    let _ = VOX_SCRIPT_DB.set(Arc::new(codex));\n\
         }\n\n",
    );
    out
}

/// Shared Codex open + schema DDL (Axum `emit_db_setup` body without the final `let db = …`).
fn emit_codex_connect_and_migrate(module: &HirModule, pad: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{pad}// ── Database setup (Codex / vox_db) ──\n\
         {pad}if let Ok(app_url) = std::env::var(\"VOX_APP_DB_URL\") {{\n\
         {pad}    let u = app_url.to_ascii_lowercase();\n\
         {pad}    if u.starts_with(\"postgres://\") || u.starts_with(\"postgresql://\") || u.starts_with(\"mysql://\") {{\n\
         {pad}        eprintln!(\"VOX_APP_DB_URL uses non-libsql backend ({{}}) — script table runtime still boots Codex\", app_url);\n\
         {pad}    }}\n\
         {pad}}}\n\
         {pad}let cfg = match vox_db::DbConfig::resolve_canonical() {{\n\
         {pad}    Ok(cfg) => cfg,\n\
         {pad}    Err(e) => {{\n\
         {pad}        panic!(\"Failed to resolve Codex DB config: {{}}\", e);\n\
         {pad}    }}\n\
         {pad}}};\n\
         {pad}let codex = match vox_db::Codex::connect(cfg).await {{\n\
         {pad}    Ok(db) => db,\n\
         {pad}    Err(e) => panic!(\"Failed to open Codex database: {{}}\", e),\n\
         {pad}}};\n"
    ));
    out.push_str(&format!(
        "{pad}{{\n\
         {pad}    let mut __vox_jm = codex.connection().query(\"PRAGMA journal_mode=WAL;\", ()).await.expect(\"PRAGMA journal_mode\");\n\
         {pad}    while let Some(_) = __vox_jm.next().await.expect(\"PRAGMA journal_mode row\") {{}}\n\
         {pad}}}\n\
         {pad}codex.connection().execute_batch(\"PRAGMA foreign_keys=ON;\").await.expect(\"PRAGMA foreign_keys\");\n\
         {pad}codex.connection().execute_batch(r#\"\n"
    ));
    for table in &module.tables {
        out.push_str(&emit_table_ddl(table));
        out.push('\n');
    }
    for index in &module.indexes {
        out.push_str(&emit_index_ddl(index));
        out.push('\n');
    }
    out.push_str(&format!("{pad}\"#).await.expect(\"schema migration\");\n"));
    out.push_str(&emit_schema_drift_verify(module));
    out
}

pub(crate) fn endpoint_as_script_fn(ep: &HirEndpointFn) -> HirFn {
    let mut f = HirFn {
        id: ep.id,
        name: ep.name.clone(),
        generics: vec![],
        params: ep.params.clone(),
        return_type: ep.return_type.clone(),
        body: ep.body.clone(),
        is_async: false,
        is_pub: true,
        is_mobile_native: false,
        is_pure: ep.is_pure,
        is_reactive: false,
        is_versioned: false,
        capabilities: vec![],
        is_remote: false,
        is_llm: false,
        llm_model: None,
        ai_structured_output: None,
        ai_fixture: None,
        embed: None,
        is_deprecated: false,
        deprecated_reason: None,
        is_traced: false,
        schedule_interval: None,
        durability: None,
        actor_state_fields: vec![],
        postconditions: vec![],
        ts_extern_module: None,
        generated_hash: None,
        span: ep.span,
        inference_model: None,
        training_step: false,
        distributed_train: None,
    };
    if has_async_stmts(&f.body) {
        f.is_async = true;
    }
    f
}

fn stmt_calls_async(stmts: &[HirStmt], async_names: &HashSet<String>) -> bool {
    stmts.iter().any(|s| match s {
        HirStmt::Let { value, .. } | HirStmt::Assign { value, .. } => {
            expr_calls_async(value, async_names)
        }
        HirStmt::Return { value, .. } => value
            .as_ref()
            .is_some_and(|e| expr_calls_async(e, async_names)),
        HirStmt::Expr { expr, .. } => expr_calls_async(expr, async_names),
        HirStmt::While {
            condition, body, ..
        } => expr_calls_async(condition, async_names) || stmt_calls_async(body, async_names),
        HirStmt::Loop { body, .. } => stmt_calls_async(body, async_names),
        HirStmt::Break { .. } | HirStmt::Continue { .. } => false,
    })
}

fn expr_calls_async(expr: &HirExpr, async_names: &HashSet<String>) -> bool {
    match expr {
        HirExpr::Call(callee, args, _, _) => {
            if let HirExpr::Ident(name, _) = &**callee {
                if async_names.contains(name) {
                    return true;
                }
            }
            expr_calls_async(callee, async_names)
                || args.iter().any(|a| expr_calls_async(&a.value, async_names))
        }
        HirExpr::MethodCall(obj, _, args, _, _) => {
            expr_calls_async(obj, async_names)
                || args.iter().any(|a| expr_calls_async(&a.value, async_names))
        }
        HirExpr::Binary(_, l, r, _) => {
            expr_calls_async(l, async_names) || expr_calls_async(r, async_names)
        }
        HirExpr::Unary(_, e, _) => expr_calls_async(e, async_names),
        HirExpr::FieldAccess(obj, _, _) => expr_calls_async(obj, async_names),
        HirExpr::Match(subj, arms, _) => {
            expr_calls_async(subj, async_names)
                || arms.iter().any(|arm| {
                    expr_calls_async(&arm.body, async_names)
                        || arm
                            .guard
                            .as_ref()
                            .is_some_and(|g| expr_calls_async(g, async_names))
                })
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            expr_calls_async(cond, async_names)
                || stmt_calls_async(then_b, async_names)
                || else_b
                    .as_ref()
                    .is_some_and(|b| stmt_calls_async(b, async_names))
        }
        HirExpr::For(_, _, iter, body, _, _) => {
            expr_calls_async(iter, async_names) || expr_calls_async(body, async_names)
        }
        HirExpr::Lambda(_, _, body, _, _) => expr_calls_async(body, async_names),
        HirExpr::With(l, r, _) => {
            expr_calls_async(l, async_names) || expr_calls_async(r, async_names)
        }
        HirExpr::Block(stmts, _) => stmt_calls_async(stmts, async_names),
        HirExpr::Try(t) => expr_calls_async(t.target.as_ref(), async_names),
        HirExpr::Index(obj, idx, _) => {
            expr_calls_async(obj, async_names) || expr_calls_async(idx, async_names)
        }
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            elems.iter().any(|e| expr_calls_async(e, async_names))
        }
        HirExpr::ObjectLit(fields, _) => {
            fields.iter().any(|(_, v)| expr_calls_async(v, async_names))
        }
        HirExpr::JsxFragment(children, _) => {
            children.iter().any(|e| expr_calls_async(e, async_names))
        }
        _ => false,
    }
}

/// Promote callers of async fns so script emission can append `.await`.
pub(crate) fn mark_transitive_async(functions: &mut [HirFn]) {
    loop {
        let async_names: HashSet<String> = functions
            .iter()
            .filter(|f| f.is_async)
            .map(|f| f.name.clone())
            .collect();
        let mut changed = false;
        for f in functions.iter_mut() {
            if !f.is_async && stmt_calls_async(&f.body, &async_names) {
                f.is_async = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Merge `@query` / `@mutation` / `@server` endpoints into `functions` and
/// promote transitive async callers for script `.await` emission.
pub(crate) fn prepare_script_module(module: &mut HirModule) {
    let endpoints: Vec<HirFn> = module
        .endpoint_fns
        .iter()
        .map(endpoint_as_script_fn)
        .collect();
    module.functions.extend(endpoints);
    mark_transitive_async(&mut module.functions);
}

pub(crate) fn refresh_script_async_metadata(module: &HirModule) {
    set_script_async_fns(collect_script_async_names(module));
}

pub(crate) fn collect_script_async_names(module: &HirModule) -> HashSet<String> {
    module
        .functions
        .iter()
        .filter(|f| f.is_async)
        .map(|f| f.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{DefId, HirModule, HirTable, HirTableField, HirType};

    fn table_module() -> HirModule {
        let mut m = HirModule::default();
        m.tables.push(HirTable {
            id: DefId(1),
            name: "Task".into(),
            fields: vec![HirTableField {
                name: "id".into(),
                type_ann: HirType::Named("int".into()),
                span: Span::new(0, 0),
            }],
            primary_key: Some("id".into()),
            is_extern: false,
            source: None,
            is_pub: true,
            is_deprecated: false,
            span: Span::new(0, 0),
        });
        m
    }

    #[test]
    fn script_db_prelude_mentions_once_lock_and_boot() {
        let prelude = emit_script_db_prelude(&table_module());
        assert!(prelude.contains("VOX_SCRIPT_DB"));
        assert!(prelude.contains("vox_script_boot_db"));
    }
}
