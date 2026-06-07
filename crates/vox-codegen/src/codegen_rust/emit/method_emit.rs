//! `emit_expr` helpers for `MethodCall` (db.*, tracing, oratio, etc.).

use vox_compiler::hir::{HirDbPredicate, HirDbQueryPlan, HirDbTableOp, HirExpr};

fn db_ref(fallible: bool) -> &'static str {
    if fallible { "&db" } else { "&*db" }
}

fn await_or_expect_suffix(fallible: bool, expect_msg: &str) -> String {
    if fallible {
        ".await?".into()
    } else {
        format!(".await.expect(\"{expect_msg}\")")
    }
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
    let args_str: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();

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
        HirDbTableOp::Delete => {
            format!(
                "{}::delete({}, {}){}",
                table_name,
                db,
                args_str.first().unwrap_or(&"0".to_string()),
                await_or_expect_suffix(fallible, "vox codegen: db delete")
            )
        }
        HirDbTableOp::All => emit_all_op(emit_expr, table_name, order_by, limit, fallible, db),
        HirDbTableOp::Count => {
            if order_by.is_some() || limit.is_some() {
                return "/* vox codegen: invalid count modifiers (typecheck should reject) */ 0"
                    .into();
            }
            emit_count_op(emit_expr, table_name, args, plan, fallible, db)
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
            db,
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

/// Emit a comma-separated list of argument expressions for a parameterized query.
/// Extracted from `emit_db_table_op`'s nested `emit_params_values` fn to module
/// scope so `emit_count_op` can call it without re-nesting.
fn emit_params_values<F>(emit_expr: &F, args: &[vox_compiler::hir::HirArg]) -> String
where
    F: Fn(&HirExpr) -> String,
{
    args.iter()
        .map(|a| emit_expr(&a.value))
        .collect::<Vec<_>>()
        .join(", ")
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
        let mut next_arg = 0usize;
        emit_predicate_sql_module(emit_expr, pred, args, &mut next_param, &mut next_arg)
    } else {
        args.iter()
            .enumerate()
            .map(|(i, a)| {
                let col = a
                    .name
                    .as_deref()
                    .expect("count filter args must be named columns");
                format!("{col} = ?{}", i + 1)
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let params = emit_params_values(emit_expr, args);
    format!(
        "{}::count_where({}, \"{}\", turso::params![{}]){}",
        table_name,
        db,
        where_sql,
        params,
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
    let params = args
        .iter()
        .map(|a| emit_expr(&a.value))
        .collect::<Vec<_>>()
        .join(", ");
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
            &params,
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
            &params,
            proj.as_deref(),
            fallible,
            db,
        )
    }
}

fn emit_filter_where_order_limit(
    table_name: &str,
    where_sql: &str,
    params: &str,
    order_sql: &str,
    limit_sql: &str,
    proj: Option<&str>,
    fallible: bool,
    db: &str,
) -> String {
    if let Some(sfx) = proj {
        format!(
            "{}::filter_where_order_limit_proj_{}({}, \"{}\", turso::params![{}], \"{}\", {}){}",
            table_name,
            sfx,
            db,
            where_sql,
            params,
            order_sql,
            limit_sql,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where_order_limit_proj")
        )
    } else {
        format!(
            "{}::filter_where_order_limit({}, \"{}\", turso::params![{}], \"{}\", {}){}",
            table_name,
            db,
            where_sql,
            params,
            order_sql,
            limit_sql,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where_order_limit")
        )
    }
}

fn emit_filter_where(
    table_name: &str,
    where_sql: &str,
    params: &str,
    proj: Option<&str>,
    fallible: bool,
    db: &str,
) -> String {
    if let Some(sfx) = proj {
        format!(
            "{}::filter_where_proj_{}({}, \"{}\", turso::params![{}]){}",
            table_name,
            sfx,
            db,
            where_sql,
            params,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where_proj")
        )
    } else {
        format!(
            "{}::filter_where({}, \"{}\", turso::params![{}]){}",
            table_name,
            db,
            where_sql,
            params,
            await_or_expect_suffix(fallible, "vox codegen: db filter_where")
        )
    }
}

fn build_filter_where_sql<F>(
    emit_expr: &F,
    args: &[vox_compiler::hir::HirArg],
    plan: Option<&HirDbQueryPlan>,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    if let Some(pred) = plan.and_then(|p| p.predicate.as_ref()) {
        let mut next_param = 1usize;
        let mut next_arg = 0usize;
        return emit_predicate_sql_module(emit_expr, pred, args, &mut next_param, &mut next_arg);
    }
    args.iter()
        .enumerate()
        .map(|(i, a)| {
            let col = a
                .name
                .as_deref()
                .expect("filter_record args must be named columns");
            format!("{col} = ?{}", i + 1)
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Module-scope twin of the nested `emit_predicate_sql` inside
/// `emit_db_table_op`. Same body, just lifted so the extracted
/// `emit_filter_record` can call it.
fn emit_predicate_sql_module<F>(
    _emit_expr: &F,
    pred: &HirDbPredicate,
    _args: &[vox_compiler::hir::HirArg],
    next_param: &mut usize,
    next_arg: &mut usize,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    match pred {
        HirDbPredicate::Eq { field } => single_param_pred(field, "=", next_param, next_arg),
        HirDbPredicate::Neq { field } => single_param_pred(field, "<>", next_param, next_arg),
        HirDbPredicate::Lt { field } => single_param_pred(field, "<", next_param, next_arg),
        HirDbPredicate::Lte { field } => single_param_pred(field, "<=", next_param, next_arg),
        HirDbPredicate::Gt { field } => single_param_pred(field, ">", next_param, next_arg),
        HirDbPredicate::Gte { field } => single_param_pred(field, ">=", next_param, next_arg),
        HirDbPredicate::Contains { field } => {
            let idx = *next_param;
            *next_param += 1;
            *next_arg += 1;
            format!("{field} LIKE '%' || ?{idx} || '%'")
        }
        HirDbPredicate::IsNull { field } => format!("{field} IS NULL"),
        HirDbPredicate::In { field, arity } => {
            let mut slots = Vec::with_capacity(*arity);
            for _ in 0..*arity {
                let idx = *next_param;
                *next_param += 1;
                *next_arg += 1;
                slots.push(format!("?{idx}"));
            }
            format!("{field} IN ({})", slots.join(", "))
        }
        HirDbPredicate::And(parts) => {
            combine_preds(_emit_expr, parts, _args, next_param, next_arg, " AND ")
        }
        HirDbPredicate::Or(parts) => {
            combine_preds(_emit_expr, parts, _args, next_param, next_arg, " OR ")
        }
        HirDbPredicate::Not(inner) => format!(
            "NOT ({})",
            emit_predicate_sql_module(_emit_expr, inner, _args, next_param, next_arg)
        ),
    }
}

fn single_param_pred(
    field: &str,
    sql_op: &str,
    next_param: &mut usize,
    next_arg: &mut usize,
) -> String {
    let idx = *next_param;
    *next_param += 1;
    *next_arg += 1;
    format!("{field} {sql_op} ?{idx}")
}

fn combine_preds<F>(
    emit_expr: &F,
    parts: &[HirDbPredicate],
    args: &[vox_compiler::hir::HirArg],
    next_param: &mut usize,
    next_arg: &mut usize,
    sep: &str,
) -> String
where
    F: Fn(&HirExpr) -> String,
{
    parts
        .iter()
        .map(|p| {
            format!(
                "({})",
                emit_predicate_sql_module(emit_expr, p, args, next_param, next_arg)
            )
        })
        .collect::<Vec<_>>()
        .join(sep)
}

pub(super) fn emit_method_call<F>(
    emit_expr: &F,
    obj: &HirExpr,
    method: &str,
    args: &[vox_compiler::hir::HirArg],
    plan: Option<&HirDbQueryPlan>,
    fallible_db: bool,
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
    if let HirExpr::Ident(ns, _) = obj
        && super::stmt_expr_tail::is_vox_namespace_ident(ns)
        && let Some(s) =
            vox_compiler::builtin_registry::std_namespace_runtime_call(ns, method, &arg_exprs)
    {
        return s;
    }
    if let Some(s) = try_emit_str_method(method, &o, &arg_exprs) {
        return s;
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
        HirExpr::StringLit(s, _) => format!("\"{}\"", s),
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

/// Try to emit a `db.Table.method(...)` fallback call when HIR lowering missed it.
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
    let op = match method {
        "insert" => HirDbTableOp::Insert,
        "get" | "find" => HirDbTableOp::Get,
        "delete" => HirDbTableOp::Delete,
        "all" => HirDbTableOp::All,
        "count" => HirDbTableOp::Count,
        "query" => HirDbTableOp::UnsafeQueryRawClause,
        _ => return None,
    };
    Some(emit_db_table_op(
        emit_expr,
        table_name,
        op,
        args,
        &plan.and_then(|p| p.projection.clone()),
        &plan.and_then(|p| p.order_by.clone()),
        &None,
        plan,
        fallible_db,
    ))
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
