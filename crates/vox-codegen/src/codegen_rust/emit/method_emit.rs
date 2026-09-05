//! `emit_expr` helpers for `MethodCall` (db.*, tracing, oratio, etc.).

use std::collections::HashMap;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirDbPredicate, HirDbQueryPlan, HirDbTableOp, HirExpr, HirType};
use vox_secrets::SecretId;
use vox_sql::BackendKind;
use vox_sql::SqlDialect;
use vox_sql::build::{SqlPredicate, equality_predicate_sql, predicate_sql};

fn db_ref(fallible: bool) -> String {
    if super::script_db::script_db_emit_mode() {
        if fallible {
            "VOX_SCRIPT_DB.get().expect(\"vox script db\").as_ref()".into()
        } else {
            "&**VOX_SCRIPT_DB.get().expect(\"vox script db\")".into()
        }
    } else if fallible {
        "&db".into()
    } else {
        "&*db".into()
    }
}

fn await_or_expect_suffix(fallible: bool, expect_msg: &str) -> String {
    if fallible {
        ".await?".into()
    } else {
        format!(".await.expect(\"{expect_msg}\")")
    }
}

fn dialect_for_backend_kind(kind: BackendKind) -> SqlDialect {
    match kind {
        BackendKind::Libsql => SqlDialect::sqlite(),
        BackendKind::Postgres => SqlDialect::postgres(),
        BackendKind::MySql => SqlDialect::mysql(),
    }
}

fn dialect_from_urls(app_url: Option<&str>, codex_url: Option<&str>) -> SqlDialect {
    if let Some(url) = app_url
        && let Ok(kind) = BackendKind::from_url(url)
    {
        return dialect_for_backend_kind(kind);
    }
    if let Some(url) = codex_url
        && let Ok(kind) = BackendKind::from_url(url)
    {
        return dialect_for_backend_kind(kind);
    }
    SqlDialect::sqlite()
}

fn db_query_sql_dialect() -> SqlDialect {
    // Prefer app-plane DB URL when present to keep emitted SQL placeholders
    // aligned with backend-neutral routing work; fall back to Codex URL.
    let app_url = vox_secrets::resolve_secret(SecretId::VoxAppDbUrl)
        .expose()
        .map(str::to_owned);
    let codex_url = vox_secrets::resolve_secret(SecretId::VoxDbUrl)
        .expose()
        .map(str::to_owned);
    dialect_from_urls(app_url.as_deref(), codex_url.as_deref())
}

/// Emit lowered `db.<Table>.<op>(...)` (canonical Codex IR).
pub(super) fn emit_db_table_op<F>(
    emit_expr: &F,
    table_name: &str,
    op: HirDbTableOp,
    args: &[vox_compiler::hir::HirArg],
    select_cols: &Option<Vec<String>>,
    order_by: &Option<(String, bool)>,
    limit: &Option<Box<HirExpr>>,
    plan: Option<&HirDbQueryPlan>,
    fallible: bool,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    let db = db_ref(fallible);
    let args_str = emit_param_values(emit_expr, args);

    let rendered = match op {
        HirDbTableOp::Insert => {
            let val = args_str
                .first()
                .cloned()
                .unwrap_or_else(|| "serde_json::json!({})".to_string());
            if fallible {
                format!(
                    "{{ let item: {table_name} = serde_json::from_value({val}).map_err(|e| vox_db::StoreError::Serialization(format!(\"{{}}\", e)))?; {table_name}::insert({db}, &item).await?; }}"
                )
            } else {
                format!(
                    "{{ let item: {table_name} = serde_json::from_value({val}).expect(\"vox codegen: db insert from_value\"); {table_name}::insert({db}, &item){} }}",
                    await_or_expect_suffix(false, "vox codegen: db insert")
                )
            }
        }
        HirDbTableOp::Get => {
            format!(
                "{}::get({}, {}){}",
                table_name,
                db,
                args_str.first().unwrap_or(&"0".to_string()),
                await_or_expect_suffix(fallible, "vox codegen: db get")
            )
        }
        HirDbTableOp::Update => {
            let id_arg = args_str.first().cloned().unwrap_or_else(|| "0".to_string());
            let val = args_str
                .get(1)
                .cloned()
                .unwrap_or_else(|| "serde_json::json!({})".to_string());
            if fallible {
                format!(
                    "{{ let item: {table_name} = serde_json::from_value({val}).map_err(|e| vox_db::StoreError::Serialization(format!(\"{{}}\", e)))?; {table_name}::update({db}, {id_arg}, &item).await?; }}"
                )
            } else {
                format!(
                    "{{ let item: {table_name} = serde_json::from_value({val}).expect(\"vox codegen: db update from_value\"); {table_name}::update({db}, {id_arg}, &item){} }}",
                    await_or_expect_suffix(false, "vox codegen: db update")
                )
            }
        }
        HirDbTableOp::Delete => {
            format!(
                "{}::delete({}, {}){}",
                table_name,
                db,
                args_str.first().unwrap_or(&"0".to_string()),
                await_or_expect_suffix(fallible, "vox codegen: db delete")
            )
        }
        HirDbTableOp::All => emit_all_op(emit_expr, table_name, order_by, limit, fallible, &db),
        HirDbTableOp::Count => {
            if order_by.is_some() || limit.is_some() {
                return "/* vox codegen: invalid count modifiers (typecheck should reject) */ 0"
                    .into();
            }
            emit_count_op(emit_expr, table_name, args, plan, fallible, &db)
        }
        HirDbTableOp::FilterRecord => emit_filter_record(
            emit_expr,
            table_name,
            args,
            select_cols,
            order_by,
            limit,
            plan,
            fallible,
            &db,
        ),
        HirDbTableOp::UnsafeQueryRawClause => {
            format!(
                "{}::unsafe_query_raw_clause({}, {}){}",
                table_name,
                db,
                args_str.first().unwrap_or(&"\"\"".to_string()),
                await_or_expect_suffix(fallible, "vox codegen: db unsafe_query_raw_clause")
            )
        }
    };
    if plan.is_some_and(|p| p.capabilities.requires_sync)
        && matches!(
            op,
            HirDbTableOp::Get
                | HirDbTableOp::All
                | HirDbTableOp::FilterRecord
                | HirDbTableOp::Count
        )
    {
        if fallible {
            format!("{{ db.sync().await?; {rendered} }}")
        } else {
            format!("{{ db.sync().await.expect(\"vox codegen: db sync\"); {rendered} }}")
        }
    } else {
        rendered
    }
}

/// Comma-separated argument expressions for parameterized query codegen.
fn emit_param_values<F>(emit_expr: &F, args: &[vox_compiler::hir::HirArg]) -> Vec<String>
where
    F: Fn(&HirExpr) -> String,
{
    args.iter().map(|a| emit_expr(&a.value)).collect()
}

/// Emit the `HirDbTableOp::All` arm — either a plain `.all()` call or
/// an `.all_order_limit()` call when ORDER BY / LIMIT modifiers are present.
/// Extracted from `emit_db_table_op` per CR-A1: the if+two map chains
/// contributed ~4 DPs inline.
fn emit_all_op<F>(
    emit_expr: &F,
    table_name: &str,
    order_by: &Option<(String, bool)>,
    limit: &Option<Box<HirExpr>>,
    fallible: bool,
    db: &str,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    if order_by.is_some() || limit.is_some() {
        let order_sql = order_by
            .as_ref()
            .map(|(col, asc)| format!("{col} {}", if *asc { "ASC" } else { "DESC" }))
            .unwrap_or_default();
        let limit_sql = limit
            .as_ref()
            .map(|e| format!("Some(({}) as i64)", emit_expr(e.as_ref())))
            .unwrap_or_else(|| "None".to_string());
        format!(
            "{}::all_order_limit({}, \"{}\", {}){}",
            table_name,
            db,
            order_sql,
            limit_sql,
            await_or_expect_suffix(fallible, "vox codegen: db all_order_limit")
        )
    } else {
        format!(
            "{}::all({}){}",
            table_name,
            db,
            await_or_expect_suffix(fallible, "vox codegen: db all")
        )
    }
}

/// Emit the `HirDbTableOp::Count` arm body (after the order_by/limit guard).
/// Extracted from `emit_db_table_op` per CR-A1: the if-args + predicate
/// if-let chain contributed ~4 DPs inline.
fn emit_count_op<F>(
    emit_expr: &F,
    table_name: &str,
    args: &[vox_compiler::hir::HirArg],
    plan: Option<&HirDbQueryPlan>,
    fallible: bool,
    db: &str,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    let dialect = db_query_sql_dialect();
    if args.is_empty() {
        return format!(
            "{}::count({}){}",
            table_name,
            db,
            await_or_expect_suffix(fallible, "vox codegen: db count")
        );
    }
    let where_sql = if let Some(pred) = plan.and_then(|p| p.predicate.as_ref()) {
        let mut next_param = 1usize;
        let pred = hir_predicate_to_sql_predicate(pred);
        predicate_sql(&dialect, &pred, &mut next_param)
    } else {
        args.iter()
            .enumerate()
            .map(|(i, a)| {
                let col = a
                    .name
                    .as_deref()
                    .expect("count filter args must be named columns");
                equality_predicate_sql(&dialect, col, i + 1)
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let param_exprs = emit_param_values(emit_expr, args);
    let param_binds = super::tables::emit_turso_positional_binds(
        &param_exprs.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    format!(
        "{}::count_where({}, \"{}\", {}){}",
        table_name,
        db,
        where_sql,
        param_binds,
        await_or_expect_suffix(fallible, "vox codegen: db count_where")
    )
}

/// Emit the `HirDbTableOp::FilterRecord` arm. Extracted from
/// `emit_db_table_op` per CR-A1 refactor — the inline arm was ~93
/// lines and contributed ~12 decision points.
#[allow(clippy::too_many_arguments)]
fn emit_filter_record<F>(
    emit_expr: &F,
    table_name: &str,
    args: &[vox_compiler::hir::HirArg],
    select_cols: &Option<Vec<String>>,
    order_by: &Option<(String, bool)>,
    limit: &Option<Box<HirExpr>>,
    plan: Option<&HirDbQueryPlan>,
    fallible: bool,
    db: &str,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    if args.is_empty() {
        return format!(
            "{{ /* vox codegen: empty filter */ {}::all({}){} }}",
            table_name,
            db,
            await_or_expect_suffix(fallible, "")
        );
    }
    let where_sql = build_filter_where_sql(emit_expr, args, plan);
    let param_exprs = emit_param_values(emit_expr, args);
    let param_binds = super::tables::emit_turso_positional_binds(
        &param_exprs.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    let proj = select_cols.as_ref().and_then(|c| {
        if c.is_empty() {
            None
        } else {
            Some(super::tables::db_projection_method_suffix(c.as_slice()))
        }
    });
    if order_by.is_some() || limit.is_some() {
        let order_sql = order_by
            .as_ref()
            .map(|(col, asc)| format!("{col} {}", if *asc { "ASC" } else { "DESC" }))
            .unwrap_or_default();
        let limit_sql = limit
            .as_ref()
            .map(|e| format!("Some(({}) as i64)", emit_expr(e.as_ref())))
            .unwrap_or_else(|| "None".to_string());
        emit_filter_where_order_limit(
            table_name,
            &where_sql,
            &param_binds,
            &order_sql,
            &limit_sql,
            proj.as_deref(),
            fallible,
            db,
        )
    } else {
        emit_filter_where(
            table_name,
            &where_sql,
            &param_binds,
            proj.as_deref(),
            fallible,
            db,
        )
    }
}

fn emit_filter_where_order_limit(
    table_name: &str,
    where_sql: &str,
    param_binds: &str,
    order_sql: &str,
    limit_sql: &str,
    proj: Option<&str>,
    fallible: bool,
    db: &str,
) -> String {
    if let Some(sfx) = proj {
        format!(
            "{}::filter_where_order_limit_proj_{}({}, \"{}\", {}, \"{}\", {}){}",
            table_name,
            sfx,
            db,
            where_sql,
            param_binds,
            order_sql,
            limit_sql,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where_order_limit_proj")
        )
    } else {
        format!(
            "{}::filter_where_order_limit({}, \"{}\", {}, \"{}\", {}){}",
            table_name,
            db,
            where_sql,
            param_binds,
            order_sql,
            limit_sql,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where_order_limit")
        )
    }
}

fn emit_filter_where(
    table_name: &str,
    where_sql: &str,
    param_binds: &str,
    proj: Option<&str>,
    fallible: bool,
    db: &str,
) -> String {
    if let Some(sfx) = proj {
        format!(
            "{}::filter_where_proj_{}({}, \"{}\", {}){}",
            table_name,
            sfx,
            db,
            where_sql,
            param_binds,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where_proj")
        )
    } else {
        format!(
            "{}::filter_where({}, \"{}\", {}){}",
            table_name,
            db,
            where_sql,
            param_binds,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where")
        )
    }
}

fn build_filter_where_sql<F>(
    _emit_expr: &F,
    args: &[vox_compiler::hir::HirArg],
    plan: Option<&HirDbQueryPlan>,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    let dialect = db_query_sql_dialect();
    if let Some(pred) = plan.and_then(|p| p.predicate.as_ref()) {
        let mut next_param = 1usize;
        let pred = hir_predicate_to_sql_predicate(pred);
        return predicate_sql(&dialect, &pred, &mut next_param);
    }
    args.iter()
        .enumerate()
        .map(|(i, a)| {
            let col = a
                .name
                .as_deref()
                .expect("filter_record args must be named columns");
            equality_predicate_sql(&dialect, col, i + 1)
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn hir_predicate_to_sql_predicate(pred: &HirDbPredicate) -> SqlPredicate {
    match pred {
        HirDbPredicate::Eq { field } => SqlPredicate::Eq {
            field: field.clone(),
        },
        HirDbPredicate::Neq { field } => SqlPredicate::Neq {
            field: field.clone(),
        },
        HirDbPredicate::Lt { field } => SqlPredicate::Lt {
            field: field.clone(),
        },
        HirDbPredicate::Lte { field } => SqlPredicate::Lte {
            field: field.clone(),
        },
        HirDbPredicate::Gt { field } => SqlPredicate::Gt {
            field: field.clone(),
        },
        HirDbPredicate::Gte { field } => SqlPredicate::Gte {
            field: field.clone(),
        },
        HirDbPredicate::Contains { field } => SqlPredicate::Contains {
            field: field.clone(),
        },
        HirDbPredicate::IsNull { field } => SqlPredicate::IsNull {
            field: field.clone(),
        },
        HirDbPredicate::In { field, arity } => SqlPredicate::In {
            field: field.clone(),
            arity: *arity,
        },
        HirDbPredicate::And(parts) => SqlPredicate::And(
            parts
                .iter()
                .map(hir_predicate_to_sql_predicate)
                .collect::<Vec<_>>(),
        ),
        HirDbPredicate::Or(parts) => SqlPredicate::Or(
            parts
                .iter()
                .map(hir_predicate_to_sql_predicate)
                .collect::<Vec<_>>(),
        ),
        HirDbPredicate::Not(inner) => {
            SqlPredicate::Not(Box::new(hir_predicate_to_sql_predicate(inner)))
        }
    }
}

/// Returns `true` when the HIR expression's inferred type is a list (`Generic("list", _)`
/// or `Generic("List", _)`).  Used to disambiguate method names shared between `str`
/// and `List` (e.g. `count`, `contains`).
fn obj_is_list(obj: &HirExpr, inferred_types: Option<&HashMap<Span, HirType>>) -> bool {
    let Some(types) = inferred_types else {
        return false;
    };
    let span = match obj {
        HirExpr::Ident(_, s)
        | HirExpr::FieldAccess(_, _, s)
        | HirExpr::MethodCall(_, _, _, _, s)
        | HirExpr::Call(_, _, _, s)
        | HirExpr::Index(_, _, s) => *s,
        _ => return false,
    };
    matches!(
        types.get(&span),
        Some(HirType::Generic(name, _)) if name.eq_ignore_ascii_case("list")
    )
}

pub(super) fn emit_method_call<F>(
    emit_expr: &F,
    obj: &HirExpr,
    method: &str,
    args: &[vox_compiler::hir::HirArg],
    plan: Option<&HirDbQueryPlan>,
    fallible_db: bool,
    inferred_types: Option<&HashMap<Span, HirType>>,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    if let HirExpr::Ident(obj_name, _) = obj {
        if obj_name == "Speech" && method == "transcribe" && args.len() == 1 {
            let p = emit_expr(&args[0].value);
            return format!(
                "(match vox_speech::transcribe_path(std::path::Path::new(({}).as_str())) {{ Ok(t) => Ok(t.display_text().to_string()), Err(e) => Error(format!(\"{{}}\", e)) }})",
                p
            );
        }
        if obj_name == "log" && !args.is_empty() {
            if let Some(s) = emit_log_method_call(emit_expr, method, args) {
                return s;
            }
        }
    }
    // HIR attaches a `HirDbQueryPlan` for canonical `db.Table.*` ops (including
    // `filter(record)`). Route through Codex IR before generic list/str method
    // lowerings mis-handle `db.User.filter({ .. })` as a Vec HOF.
    if let Some(plan) = plan {
        let exec_args = if !plan.predicate_args.is_empty() {
            &plan.predicate_args
        } else {
            args
        };
        return emit_db_table_op(
            emit_expr,
            &plan.table,
            plan.op,
            exec_args,
            &plan.projection,
            &plan.order_by,
            &plan.limit_value,
            Some(plan),
            fallible_db,
        );
    }
    // Fallback: `db.Table.method` if lowering missed (should be rare).
    if let Some(s) = try_emit_db_fallback(emit_expr, obj, method, args, plan, fallible_db) {
        return s;
    }

    let o = emit_expr(obj);
    if method == "json" && o == "request" {
        return "request.clone()".into();
    }
    // Vox str method lowering for Rust: snake_case names that don't exist directly on String.
    let arg_exprs: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();
    // Namespace-module calls in statement position (e.g. `process.exit(1)`,
    // `process.run(..)`, `fs.write(..)`) parse as MethodCall, bypassing the
    // expression-position FieldAccess lowering. Route them through the same
    // runtime-call registry so the receiver lowers to the builtin instead of a
    // method on an undefined `process`/`fs`/… value.
    // `std.env.get(..)` is the documented long form: the receiver is
    // FieldAccess(std, env), not a bare `env` ident, so unwrap the `std.`
    // prefix before the namespace check.
    let namespace_recv = match obj {
        HirExpr::Ident(ns, _) => Some(ns.as_str()),
        HirExpr::FieldAccess(inner, ns, _) if matches!(inner.as_ref(), HirExpr::Ident(s, _) if s == "std") => {
            Some(ns.as_str())
        }
        _ => None,
    };
    if let Some(ns) = namespace_recv
        && super::stmt_expr_tail::is_vox_namespace_ident(ns)
        && let Some(s) =
            vox_compiler::builtin_registry::std_namespace_runtime_call(ns, method, &arg_exprs)
    {
        return s;
    }
    if let Some(s) = try_emit_list_hof(emit_expr, method, &o, args) {
        return s;
    }
    // For methods whose names are shared between str and List (e.g. `count`,
    // `contains`, `join`), check the receiver's inferred type first so the
    // correct lowering fires.
    //
    // Value-semantic list mutators (`push`) in VALUE/reassign position lower to a
    // block that clones, mutates, and yields the new vec (matches the interpreter,
    // which returns the new list). Bare STATEMENT-position mutation (`result.push(0)`
    // for its side effect) is intercepted earlier in `emit_stmt` and emitted as an
    // in-place `result.push(0);` so the original binding actually grows.
    let recv_is_list = obj_is_list(obj, inferred_types);
    if recv_is_list {
        if let Some(s) = try_emit_list_method(method, &o, &arg_exprs) {
            return s;
        }
    }
    // `x.get(key)` with a STRING-typed key (literal or inferred `str`) is an
    // object/JSON keyed lookup — `VoxJson::get(String) -> Option<VoxJson>`,
    // already owned — never a usize list index. Must run before
    // `try_emit_str_method`, whose `get` arm only screens out string LITERALS
    // and would otherwise cast a string VARIABLE to `usize` (E0308/E0605) and
    // bolt `.cloned()` onto an owned Option (E0599).
    if method == "get"
        && args.len() == 1
        && super::stmt_expr::index_key_is_string(&args[0].value, inferred_types)
    {
        return format!("({}).get({})", o, arg_exprs[0]);
    }
    if let Some(s) = try_emit_str_method(method, &o, &arg_exprs) {
        return s;
    }
    if !recv_is_list {
        if let Some(s) = try_emit_list_method(method, &o, &arg_exprs) {
            return s;
        }
    }
    // Vox unwrap() on a fallible db op peels the Result layer only.
    // Codegen lowers that to `.await.expect(...)`, which already yields the
    // Ok payload (Option<Row> for get, etc.) — not Option::unwrap().
    // Insert ids bound via `let id = db.T.insert(...)` are already `i64` in
    // Rust even though Vox types them as `Result[int]`.
    if method == "unwrap"
        && arg_exprs.is_empty()
        && (o.contains(".expect(\"vox codegen: db")
            || insert_result_already_unwrapped(obj, inferred_types))
    {
        return o;
    }
    let call = format!("{}.{}({})", o, method, arg_exprs.join(", "));
    if method == "send" {
        format!("{}.await", call)
    } else {
        call
    }
}

/// Emit a `log.info/warn/error/debug(...)` tracing macro call.
/// Returns `None` only when `args` is empty (caller guards this).
fn emit_log_method_call<F>(
    emit_expr: &F,
    method: &str,
    args: &[vox_compiler::hir::HirArg],
) -> Option<String>
where
    F: Fn(&HirExpr) -> String,
{
    let mut args_iter = args.iter();
    let first_arg = args_iter.next()?;
    let fmt = match &first_arg.value {
        HirExpr::StringLit(s, _) => {
            format!(
                "\"{}\"",
                super::stmt_expr::escape_rust_double_quoted_content(s)
            )
        }
        other => emit_expr(other),
    };
    let remaining: Vec<String> = args_iter.map(|a| emit_expr(&a.value)).collect();
    let macro_name = match method {
        "info" => "info",
        "warn" => "warn",
        "error" => "error",
        "debug" => "debug",
        _ => "info",
    };
    if remaining.is_empty() {
        Some(format!("tracing::{}!(\"{{:?}}\", {})", macro_name, fmt))
    } else {
        Some(format!(
            "tracing::{}!({}, {})",
            macro_name,
            fmt,
            remaining.join(", ")
        ))
    }
}

/// True when a Vox `Result[int]` binding already lowered to plain `i64` (db insert).
fn insert_result_already_unwrapped(
    obj: &HirExpr,
    inferred_types: Option<&HashMap<Span, HirType>>,
) -> bool {
    let HirExpr::Ident(_, span) = obj else {
        return false;
    };
    inferred_types.and_then(|m| m.get(span)).is_some_and(|t| {
        matches!(
            t,
            HirType::Generic(name, args)
                if name == "Result"
                    && args.first().is_some_and(|ok| {
                        matches!(ok, HirType::Named(n) if n == "int")
                    })
        )
    })
}

/// Try to emit a `db.Table.method(...)` call. When HIR attached a query plan
/// (e.g. `filter({ col: val })` → `FilterRecord`), use `plan.op` so we do not
/// fall through to list `.filter` lowerings on `db.User`.
fn try_emit_db_fallback<F>(
    emit_expr: &F,
    obj: &HirExpr,
    method: &str,
    args: &[vox_compiler::hir::HirArg],
    plan: Option<&HirDbQueryPlan>,
    fallible_db: bool,
) -> Option<String>
where
    F: Fn(&HirExpr) -> String,
{
    let HirExpr::FieldAccess(inner, table_name, _) = obj else {
        return None;
    };
    let HirExpr::Ident(n, _) = inner.as_ref() else {
        return None;
    };
    if n != "db" {
        return None;
    }
    let op = if let Some(p) = plan {
        p.op
    } else {
        match method {
            "insert" => HirDbTableOp::Insert,
            "get" | "find" => HirDbTableOp::Get,
            "update" => HirDbTableOp::Update,
            "delete" => HirDbTableOp::Delete,
            "all" => HirDbTableOp::All,
            "count" => HirDbTableOp::Count,
            "query" => HirDbTableOp::UnsafeQueryRawClause,
            _ => return None,
        }
    };
    Some(emit_db_table_op(
        emit_expr,
        table_name,
        op,
        args,
        &plan.and_then(|p| p.projection.clone()),
        &plan.and_then(|p| p.order_by.clone()),
        &plan.and_then(|p| p.limit_value.clone()),
        plan,
        fallible_db,
    ))
}

/// Try to emit Vox list higher-order-function lowerings to Rust iterator chains.
///
/// Vox list HOFs (`map`/`filter`/`fold`/`sorted_by_key`) have no direct method
/// on Rust's `Vec`, so lower them to `into_iter()` adapter chains. `map`/`fold`
/// pass the closure straight to the adapter (whose `Fn` bound pins the param
/// type). `filter`/`sorted_by_key` invoke the Vox predicate INDIRECTLY
/// (`pred(x.clone())`) — Vox closures take their param BY VALUE, but Rust's
/// `filter`/`sort_by_key` pass `&T`, so we `.clone()` before calling. That
/// indirect call defeats param inference, so those predicates are emitted with
/// their declared param TYPE annotated (`annotate = true`) to avoid E0282.
///
/// Gated strictly on the exact Vox HOF name + arity. `map`/`filter`/`fold` are
/// emitted only when the sole/first arg is a closure (lambda or closure-typed
/// var ident) — this avoids shadowing unrelated `.map`/`.filter` method calls.
fn try_emit_list_hof<F>(
    emit_expr: &F,
    method: &str,
    o: &str,
    args: &[vox_compiler::hir::HirArg],
) -> Option<String>
where
    F: Fn(&HirExpr) -> String,
{
    let is_closure_arg = |e: &HirExpr| matches!(e, HirExpr::Lambda(..) | HirExpr::Ident(..));
    match method {
        "map" if args.len() == 1 && is_closure_arg(&args[0].value) => Some(format!(
            "({}).into_iter().map({}).collect::<Vec<_>>()",
            o,
            emit_expr(&args[0].value)
        )),
        "filter" if args.len() == 1 && is_closure_arg(&args[0].value) => Some(format!(
            "({}).into_iter().filter(|__x| ({})(__x.clone())).collect::<Vec<_>>()",
            o,
            emit_hof_predicate(emit_expr, &args[0].value)
        )),
        "fold" if args.len() == 2 && is_closure_arg(&args[1].value) => Some(format!(
            "({}).into_iter().fold({}, {})",
            o,
            emit_expr(&args[0].value),
            emit_expr(&args[1].value)
        )),
        // `sorted_by_key`/`sort_by_key` are the SAME value-semantic Vox method
        // (eval/expr.rs matches both spellings): sort by a key function, returning
        // a NEW list. The imperative spelling is NOT an in-place mutation — it is
        // deliberately routed here (and excluded from stmt_expr::SELF_MUTATORS) so
        // `xs = xs.sort_by_key(f)` lowers to this clone-and-sort value block.
        "sorted_by_key" | "sort_by_key" if args.len() == 1 && is_closure_arg(&args[0].value) => {
            Some(format!(
                "{{ let mut __v = ({}).clone(); __v.sort_by_key(|__e| ({})(__e.clone())); __v }}",
                o,
                emit_hof_predicate(emit_expr, &args[0].value)
            ))
        }
        // `sorted_by`/`sort_by` sort by a comparator `fn(a, b) -> int` (<0 / 0 / >0).
        // Map the Vox int result to `std::cmp::Ordering` via `.cmp(&0)`, mirroring
        // the interpreter's `n > 0 => swap` ascending semantics (stable sort).
        "sorted_by" | "sort_by" if args.len() == 1 && is_closure_arg(&args[0].value) => {
            Some(format!(
                "{{ let mut __v = ({}).clone(); __v.sort_by(|__a, __b| ({})(__a.clone(), __b.clone()).cmp(&0)); __v }}",
                o,
                emit_hof_predicate(emit_expr, &args[0].value)
            ))
        }
        // Vox `list.first()`/`.last()` return an owned `Option<T>`; Rust's
        // `Vec::first`/`last` return `Option<&T>`, so `.cloned()` aligns the type
        // with `Some(<owned>)` comparisons (matches the interpreter arm).
        "first" if args.is_empty() => Some(format!("({}).first().cloned()", o)),
        "last" if args.is_empty() => Some(format!("({}).last().cloned()", o)),
        _ => None,
    }
}

/// Try to emit Vox **value-semantic** list method lowerings.
///
/// Vox lists are value types: the interpreter's `.push` (eval/builtins.rs) clones
/// the vec, mutates the clone, and returns the NEW list. Rust's `Vec::push` instead
/// mutates in place and returns `()`, so a naive `xs = xs.push(y)` assigns `()` to a
/// `Vec` (E0308). Emit a block that performs the mutation and yields the updated vec.
/// (Bare statement-position `result.push(0)` is intercepted in `emit_stmt` and
/// lowered to an in-place mutation instead — see `try_emit_stmt_mutation`.)
///
/// Scope: `push` only for now (the value-semantic mutator goldens actually hit).
/// Other list mutators (`pop` returns the popped value, not the list; `insert`/
/// `remove`/etc.) have their own shapes and are added when a golden needs them.
fn try_emit_list_method(method: &str, o: &str, arg_exprs: &[String]) -> Option<String> {
    match method {
        // ── Value-semantic mutators (bind owned, mutate, yield) ──────────────
        "push" if arg_exprs.len() == 1 => Some(format!(
            "({{ let mut __lst = {}; __lst.push({}); __lst }})",
            o, arg_exprs[0]
        )),
        "reverse" | "reversed" => Some(format!(
            "({{ let mut __lst = {}; __lst.reverse(); __lst }})",
            o
        )),
        // `sorted()` (arity 0): natural-order sort, returning a NEW list (value
        // semantics, matching `reverse`/`reversed` above). Was typechecked
        // (typeck/builtins.rs) and interpreted (eval/builtins.rs, via
        // `vox_value_cmp`) but had NO codegen arm at all — `vox run` (compiled)
        // rejected any `.sorted()` call with "no method named `sorted` found for
        // struct `Vec<String>`", while `vox check`/`vox run --interp` both
        // accepted it. `partial_cmp` (not `Ord::cmp`/`.sort()`) is used
        // deliberately: it works for every concrete element type this codegen
        // backend can produce a `Vec<T>` of (str/String, i64, bool, f64 — floats
        // implement `PartialOrd` but not `Ord`), so this one arm covers all of
        // them without needing per-type dispatch via `inferred_types`. A NaN
        // comparison degrades to `Equal` (stable, non-panicking) rather than the
        // `.unwrap()`-and-panic a naive `partial_cmp(..).unwrap()` would risk.
        "sorted" if arg_exprs.is_empty() => Some(format!(
            "({{ let mut __lst = {}; __lst.sort_by(|__a, __b| __a.partial_cmp(__b).unwrap_or(std::cmp::Ordering::Equal)); __lst }})",
            o
        )),
        "extend" if arg_exprs.len() == 1 => Some(format!(
            "({{ let mut __lst = {}; __lst.extend(({}).into_iter()); __lst }})",
            o, arg_exprs[0]
        )),
        // remove(val): remove first occurrence; no-op if not found.
        "remove" if arg_exprs.len() == 1 => Some(format!(
            "({{ let mut __lst = {}; let __val = {}; if let Some(__pos) = __lst.iter().position(|x| x == &__val) {{ __lst.remove(__pos); }} __lst }})",
            o, arg_exprs[0]
        )),
        // remove_at(i): bounds-checked remove at index i (Vox int → usize).
        "remove_at" if arg_exprs.len() == 1 => Some(format!(
            "({{ let mut __lst = {}; let __i = ({}) as usize; if __i < __lst.len() {{ __lst.remove(__i); }} __lst }})",
            o, arg_exprs[0]
        )),

        // ── Slice ────────────────────────────────────────────────────────────
        // slice_list(start, end?) → sub-list [start, end), bounds-clamped.
        "slice_list" if !arg_exprs.is_empty() => {
            let start = &arg_exprs[0];
            let end_expr = if arg_exprs.len() >= 2 {
                format!("({} as usize).min(__lst.len())", arg_exprs[1])
            } else {
                "__lst.len()".to_string()
            };
            Some(format!(
                "({{ let __lst = {}; let __start = ({} as usize).min(__lst.len()); let __end = ({}).min(__lst.len()); __lst[__start..__end].to_vec() }})",
                o, start, end_expr
            ))
        }

        // `sum` is intentionally NOT handled: `(<recv>).iter().copied().sum()` has
        // no inferable target type in a bare `let total = xs.sum()` (E0283 — `Sum`
        // is impl'd for both i64 and f64). A correct emission needs an explicit
        // `.sum::<i64>()`/`.sum::<f64>()` annotation derived from the list element
        // type via `inferred_types`; deferred until a golden needs it.

        // index/find_index: first index of val, or -1.
        "index" | "find_index" if arg_exprs.len() == 1 => Some(format!(
            "({{ let __lst = {}; let __val = {}; __lst.iter().position(|x| x == &__val).map(|i| i as i64).unwrap_or(-1i64) }})",
            o, arg_exprs[0]
        )),

        // count(val): number of occurrences of val.
        "count" if arg_exprs.len() == 1 => Some(format!(
            "({{ let __lst = {}; let __val = {}; __lst.iter().filter(|x| *x == &__val).count() as i64 }})",
            o, arg_exprs[0]
        )),

        // join(sep): Display each element joined with sep.
        "join" if arg_exprs.len() == 1 => Some(format!(
            "({{ let __lst = {}; let __sepowned = {}; let __sep: &str = __sepowned.as_ref(); __lst.iter().map(|x| format!(\"{{}}\", x)).collect::<Vec<_>>().join(__sep) }})",
            o, arg_exprs[0]
        )),

        // contains(val): bool.
        "contains" if arg_exprs.len() == 1 => Some(format!(
            "({{ let __lst = {}; let __val = {}; __lst.contains(&__val) }})",
            o, arg_exprs[0]
        )),

        // first / last: owned Option<T> (Vec::first/last → Option<&T> → .cloned()).
        "first" if arg_exprs.is_empty() => Some(format!("({}.first().cloned())", o)),
        "last" if arg_exprs.is_empty() => Some(format!("({}.last().cloned())", o)),

        // Higher-order list transforms (closures_hof golden).
        "map" if arg_exprs.len() == 1 => Some(format!(
            "({}).into_iter().map({}).collect::<Vec<_>>()",
            o, arg_exprs[0]
        )),
        "filter" if arg_exprs.len() == 1 => Some(format!(
            "({}).into_iter().filter({}).collect::<Vec<_>>()",
            o, arg_exprs[0]
        )),
        "fold" if arg_exprs.len() == 2 => Some(format!(
            "({}).iter().fold({}, {})",
            o, arg_exprs[0], arg_exprs[1]
        )),
        "sorted_by_key" if arg_exprs.len() == 1 => Some(format!(
            "({{ let mut __lst = {}; __lst.sort_by_key({}); __lst }})",
            o, arg_exprs[0]
        )),

        _ => None,
    }
}

/// Emit a `filter`/`sorted_by_key` predicate. When it is a lambda literal, emit
/// it BARE but with its declared param TYPE annotated (the predicate is invoked
/// indirectly via `pred(x.clone())`, which defeats closure-param inference →
/// E0282). A closure-typed variable arg (already `Rc<dyn Fn>`) is emitted as-is.
fn emit_hof_predicate<F>(emit_expr: &F, arg: &HirExpr) -> String
where
    F: Fn(&HirExpr) -> String,
{
    if let HirExpr::Lambda(params, _ret, body, _, _) = arg {
        super::stmt_expr::emit_bare_lambda(params, body, true, emit_expr)
    } else {
        emit_expr(arg)
    }
}

/// Try to emit Vox string method lowerings that have no direct Rust String equivalent.
///
/// # Lifetime discipline
///
/// Every block that takes `let __s: &str = …` MUST first bind the owned `String`
/// (`let __owned = …`) so that the reference outlives the block expression.
/// The pattern `let __s: &str = (complex_expr).as_ref()` drops the temporary
/// `String` at the end of the `let` statement, leaving `__s` dangling (E0716).
fn try_emit_str_method(method: &str, o: &str, arg_exprs: &[String]) -> Option<String> {
    match method {
        // Vox JSON accessors lower to `VoxJson` methods (script mode Json alias).
        // `as_int`/`as_float`/`as_bool` have no `String` analogue, so they are
        // unambiguously JSON-value accessors. `VoxJson::as_str()` already returns
        // `Option<String>` (RFC §4.3 strict-Option API).
        // (The string methods to_upper/to_lower/replace/contains/split are handled
        // by the E0716-safe arms below — merged from origin/main.)
        "as_int" if arg_exprs.is_empty() => Some(format!("({}).as_int()", o)),
        "as_float" if arg_exprs.is_empty() => Some(format!("({}).as_float()", o)),
        "as_bool" if arg_exprs.is_empty() => Some(format!("({}).as_bool()", o)),
        "as_str" if arg_exprs.is_empty() => Some(format!("({}).as_str()", o)),
        // `xs.get(i)` (list, integer index) returns an owned `Option<T>` in Vox
        // (matching the `Index` emit). Rust's `Vec::get` returns `Option<&T>`, so
        // `.cloned()` aligns the type with `Some(<owned>)` comparisons, and the
        // index is cast to `usize`. String-keyed `get` (serde_json `Value::get`,
        // map `.get`) is left to the generic path — detected by a string-literal
        // arg, which must NOT be cast to `usize`.
        "get" if arg_exprs.len() == 1 && !arg_exprs[0].trim_start().starts_with('"') => {
            Some(format!("({}).get(({}) as usize).cloned()", o, arg_exprs[0]))
        }
        // ── Existing arms (updated for E0716 safety) ────────────────────────
        "slice" if arg_exprs.len() == 2 => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __start = ({}) as usize; let __end = ({}) as usize; let __cnt = __s.chars().count(); let __end = __end.min(__cnt); let __start = __start.min(__end); __s.chars().skip(__start).take(__end - __start).collect::<String>() }})",
            o, arg_exprs[0], arg_exprs[1]
        )),
        "char_at" if arg_exprs.len() == 1 => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __i = ({}) as usize; __s.chars().nth(__i).map(|c| c.to_string()) }})",
            o, arg_exprs[0]
        )),
        "index_of" if arg_exprs.len() == 1 => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __nowned = {}; let __n: &str = __nowned.as_ref(); __s.find(__n).map(|byte_pos| __s[..byte_pos].chars().count() as i64) }})",
            o, arg_exprs[0]
        )),

        // ── len / is_empty ──────────────────────────────────────────────────
        // Rust `.len()` returns `usize`; Vox `int` is `i64`.
        "len" => Some(format!("({}.len() as i64)", o)),
        "is_empty" => Some(format!("({}.is_empty())", o)),

        // ── Case conversion ─────────────────────────────────────────────────
        "to_upper" | "to_uppercase" => Some(format!("({}.to_uppercase())", o)),
        "to_lower" | "to_lowercase" => Some(format!("({}.to_lowercase())", o)),

        // ── Trim ────────────────────────────────────────────────────────────
        "trim" => Some(format!(
            "({{ let __owned = {}; __owned.trim().to_string() }})",
            o
        )),
        "trim_start" => Some(format!(
            "({{ let __owned = {}; __owned.trim_start().to_string() }})",
            o
        )),
        "trim_end" => Some(format!(
            "({{ let __owned = {}; __owned.trim_end().to_string() }})",
            o
        )),

        // ── Pattern predicates (arg must be &str, NOT String) ───────────────
        "contains" if !arg_exprs.is_empty() => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __powned = {}; let __p: &str = __powned.as_ref(); __s.contains(__p) }})",
            o, arg_exprs[0]
        )),
        "starts_with" if !arg_exprs.is_empty() => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __powned = {}; let __p: &str = __powned.as_ref(); __s.starts_with(__p) }})",
            o, arg_exprs[0]
        )),
        "ends_with" if !arg_exprs.is_empty() => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __powned = {}; let __p: &str = __powned.as_ref(); __s.ends_with(__p) }})",
            o, arg_exprs[0]
        )),

        // ── split ───────────────────────────────────────────────────────────
        // With explicit delimiter (common case).
        "split" if !arg_exprs.is_empty() => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __downed = {}; let __d: &str = __downed.as_ref(); __s.split(__d).map(|p| p.to_string()).collect::<Vec<String>>() }})",
            o, arg_exprs[0]
        )),
        // No-arg split: default delimiter is " ".
        "split" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); __s.split(' ').map(|p| p.to_string()).collect::<Vec<String>>() }})",
            o
        )),

        // ── replace ─────────────────────────────────────────────────────────
        "replace" if arg_exprs.len() >= 2 => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __fowned = {}; let __from: &str = __fowned.as_ref(); let __towned = {}; let __to: &str = __towned.as_ref(); __s.replace(__from, __to) }})",
            o, arg_exprs[0], arg_exprs[1]
        )),

        // ── repeat ──────────────────────────────────────────────────────────
        // arg is Vox int (i64); Rust `.repeat()` takes usize.
        "repeat" if !arg_exprs.is_empty() => {
            Some(format!("({}.repeat(({}) as usize))", o, arg_exprs[0]))
        }

        // ── chars_count ─────────────────────────────────────────────────────
        "chars_count" => Some(format!("({}.chars().count() as i64)", o)),

        // ── count(sub) — non-overlapping occurrences ─────────────────────────
        // Mirrors the interpreter's semantics (empty sub → char-count + 1).
        "count" if !arg_exprs.is_empty() => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); let __subowned = {}; let __sub: &str = __subowned.as_ref(); if __sub.is_empty() {{ __s.chars().count() as i64 + 1 }} else {{ let mut __c = 0i64; let mut __i = 0usize; while let Some(__pos) = __s[__i..].find(__sub) {{ __c += 1; __i += __pos + __sub.len(); }} __c }} }})",
            o, arg_exprs[0]
        )),

        // ── Predicate methods ────────────────────────────────────────────────
        "is_alpha" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); !__s.is_empty() && __s.chars().all(|c| c.is_alphabetic()) }})",
            o
        )),
        "is_digit" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); !__s.is_empty() && __s.chars().all(|c| c.is_ascii_digit()) }})",
            o
        )),
        "is_alnum" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); !__s.is_empty() && __s.chars().all(|c| c.is_alphanumeric()) }})",
            o
        )),
        "is_upper" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); !__s.is_empty() && __s.chars().any(|c| c.is_alphabetic()) && __s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()) }})",
            o
        )),
        "is_lower" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); !__s.is_empty() && __s.chars().any(|c| c.is_alphabetic()) && __s.chars().all(|c| !c.is_alphabetic() || c.is_lowercase()) }})",
            o
        )),

        // ── ord ──────────────────────────────────────────────────────────────
        // Unicode code point of the first character; returns 0 for empty string.
        "ord" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); __s.chars().next().map(|c| c as i64).unwrap_or(0) }})",
            o
        )),

        // ── chars ────────────────────────────────────────────────────────────
        "chars" => Some(format!(
            "({}.chars().map(|c| c.to_string()).collect::<Vec<String>>())",
            o
        )),

        // ── to_str / to_string ───────────────────────────────────────────────
        "to_str" | "to_string" => Some(format!("({}.to_string())", o)),

        // ── to_int / to_float ────────────────────────────────────────────────
        // Returns Option: None if parse fails (mirrors interpreter's VoxValue::Option).
        "to_int" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); __s.trim().parse::<i64>().ok() }})",
            o
        )),
        "to_float" => Some(format!(
            "({{ let __owned = {}; let __s: &str = __owned.as_ref(); __s.trim().parse::<f64>().ok() }})",
            o
        )),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SqlDialect, dialect_for_backend_kind, dialect_from_urls};
    use vox_sql::BackendKind;

    #[test]
    fn backend_kind_maps_to_expected_dialect_shape() {
        let sqlite = dialect_for_backend_kind(BackendKind::Libsql);
        assert_eq!(
            sqlite.placeholder_style,
            SqlDialect::sqlite().placeholder_style
        );

        let postgres = dialect_for_backend_kind(BackendKind::Postgres);
        assert_eq!(
            postgres.placeholder_style,
            SqlDialect::postgres().placeholder_style
        );

        let mysql = dialect_for_backend_kind(BackendKind::MySql);
        assert_eq!(
            mysql.placeholder_style,
            SqlDialect::mysql().placeholder_style
        );
    }

    #[test]
    fn app_plane_url_takes_precedence_for_dialect_selection() {
        let dialect = dialect_from_urls(
            Some("postgres://user:pass@localhost:5432/app"),
            Some("libsql://example.turso.io"),
        );
        assert_eq!(
            dialect.placeholder_style,
            SqlDialect::postgres().placeholder_style
        );
    }

    #[test]
    fn codex_url_used_when_app_plane_url_absent_or_invalid() {
        let dialect = dialect_from_urls(Some("not-a-db-url"), Some("mysql://localhost/app"));
        assert_eq!(
            dialect.placeholder_style,
            SqlDialect::mysql().placeholder_style
        );
    }

    #[test]
    fn dialect_for_backend_kind_maps_each_backend() {
        // Catches: a mis-wired arm (e.g. Postgres accidentally mapped to sqlite),
        // which would emit the wrong placeholder style ($1 vs ?) in generated SQL.
        assert_eq!(
            dialect_for_backend_kind(BackendKind::Postgres).placeholder_style,
            SqlDialect::postgres().placeholder_style
        );
        assert_eq!(
            dialect_for_backend_kind(BackendKind::MySql).placeholder_style,
            SqlDialect::mysql().placeholder_style
        );
        assert_eq!(
            dialect_for_backend_kind(BackendKind::Libsql).placeholder_style,
            SqlDialect::sqlite().placeholder_style
        );
    }

    #[test]
    fn dialect_from_urls_prefers_app_url_then_falls_back_to_sqlite() {
        // Catches: precedence regression (codex url winning over app url) or a
        // bad-URL path that panics instead of defaulting to sqlite.
        assert_eq!(
            dialect_from_urls(
                Some("postgres://u:p@localhost:5432/app"),
                Some("libsql://example.turso.io"),
            )
            .placeholder_style,
            SqlDialect::postgres().placeholder_style
        );
        // Both unparseable → sqlite default.
        assert_eq!(
            dialect_from_urls(Some("nope"), Some("also-nope")).placeholder_style,
            SqlDialect::sqlite().placeholder_style
        );
    }
}
