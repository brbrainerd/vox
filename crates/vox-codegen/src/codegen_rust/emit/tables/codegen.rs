use vox_compiler::ast::scalar_mapping::VoxScalar;
use vox_compiler::hir::{HirIndex, HirModule, HirTable, HirType};
use vox_secrets::SecretId;
use vox_sql::BackendKind;
use vox_sql::SqlDialect;
use vox_sql::build::placeholder_sql;

use super::super::types::emit_type;
use super::projections::db_projection_method_suffix;

fn hir_type_to_sql(ty: &HirType) -> &'static str {
    match ty {
        HirType::Named(n) => {
            if let Some(s) = VoxScalar::parse(n) {
                s.as_sqlite_affinity()
            } else {
                "TEXT" // Fallback: serialize as JSON text
            }
        }
        HirType::Generic(n, _) => match n.as_str() {
            "Id" => "INTEGER",  // Foreign key reference
            "Option" => "TEXT", // Nullable, type handled at Rust level
            _ => "TEXT",        // Complex types serialized as JSON
        },
        _ => "TEXT",
    }
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn table_live_name(table: &HirTable) -> String {
    if table.is_extern {
        table
            .source
            .clone()
            .unwrap_or_else(|| to_snake_case(&table.name))
    } else {
        to_snake_case(&table.name)
    }
}

fn escape_single_quoted_sql_literal(s: &str) -> String {
    s.replace('\'', "''")
}

fn sql_dialect_from_urls(app_url: Option<&str>, codex_url: Option<&str>) -> SqlDialect {
    if let Some(url) = app_url
        && let Ok(kind) = BackendKind::from_url(url)
    {
        return match kind {
            BackendKind::Libsql => SqlDialect::sqlite(),
            BackendKind::Postgres => SqlDialect::postgres(),
            BackendKind::MySql => SqlDialect::mysql(),
        };
    }
    if let Some(url) = codex_url
        && let Ok(kind) = BackendKind::from_url(url)
    {
        return match kind {
            BackendKind::Libsql => SqlDialect::sqlite(),
            BackendKind::Postgres => SqlDialect::postgres(),
            BackendKind::MySql => SqlDialect::mysql(),
        };
    }
    SqlDialect::sqlite()
}

fn generated_sql_dialect() -> SqlDialect {
    let app_url = vox_secrets::resolve_secret(SecretId::VoxAppDbUrl)
        .expose()
        .map(str::to_owned);
    let codex_url = vox_secrets::resolve_secret(SecretId::VoxDbUrl)
        .expose()
        .map(str::to_owned);
    sql_dialect_from_urls(app_url.as_deref(), codex_url.as_deref())
}

pub fn emit_schema_drift_verify(module: &HirModule) -> String {
    let mut out = String::new();
    out.push_str("    // Boot-time schema drift validation (table/column/type parity).\n");
    out.push_str("    {\n");
    out.push_str(
        "        let __vox_norm_sql_ty = |s: &str| {\n\
        \x20           s.to_ascii_lowercase()\n\
        \x20               .replace(' ', \"\")\n\
        \x20               .replace(\"(1)\", \"\")\n\
        \x20               .replace(\"(65,30)\", \"\")\n\
        \x20       };\n",
    );

    for (idx, table) in module.tables.iter().enumerate() {
        let table_name = table_live_name(table);
        let table_sql_lit = escape_single_quoted_sql_literal(&table_name);
        out.push_str(&format!(
            "        let mut __vox_cols_{idx}: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();\n"
        ));
        out.push_str(&format!(
            "        let mut __vox_seen_rows_{idx} = false;\n\
            \x20       let mut __vox_rows_{idx} = match codex.connection().query(\"PRAGMA table_info('{table_sql_lit}');\", ()).await {{\n\
            \x20           Ok(rows) => rows,\n\
            \x20           Err(e) => {{\n\
            \x20               eprintln!(\"schema drift: failed to inspect table '{table_name}': {{}}\", e);\n\
            \x20               std::process::exit(2);\n\
            \x20           }}\n\
            \x20       }};\n\
            \x20       loop {{\n\
            \x20           match __vox_rows_{idx}.next().await {{\n\
            \x20               Ok(Some(__vox_row)) => {{\n\
            \x20                   __vox_seen_rows_{idx} = true;\n\
            \x20                   let __vox_col_name: String = match __vox_row.get(1) {{\n\
            \x20                       Ok(v) => v,\n\
            \x20                       Err(e) => {{\n\
            \x20                           eprintln!(\"schema drift: failed reading column name for table '{table_name}': {{}}\", e);\n\
            \x20                           std::process::exit(2);\n\
            \x20                       }}\n\
            \x20                   }};\n\
            \x20                   let __vox_col_ty: String = match __vox_row.get(2) {{\n\
            \x20                       Ok(v) => v,\n\
            \x20                       Err(e) => {{\n\
            \x20                           eprintln!(\"schema drift: failed reading column type for table '{table_name}': {{}}\", e);\n\
            \x20                           std::process::exit(2);\n\
            \x20                       }}\n\
            \x20                   }};\n\
            \x20                   __vox_cols_{idx}.insert(__vox_col_name.to_ascii_lowercase(), __vox_col_ty);\n\
            \x20               }}\n\
            \x20               Ok(None) => break,\n\
            \x20               Err(e) => {{\n\
            \x20                   eprintln!(\"schema drift: failed iterating columns for table '{table_name}': {{}}\", e);\n\
            \x20                   std::process::exit(2);\n\
            \x20               }}\n\
            \x20           }}\n\
            \x20       }}\n\
            \x20       if !__vox_seen_rows_{idx} {{\n\
            \x20           eprintln!(\"schema drift: live table '{table_name}' is missing\");\n\
            \x20           std::process::exit(2);\n\
            \x20       }}\n"
        ));

        for field in &table.fields {
            let expected_ty = hir_type_to_sql(&field.type_ann);
            let col_lower = field.name.to_ascii_lowercase();
            out.push_str(&format!(
                "        match __vox_cols_{idx}.get(\"{col_lower}\") {{\n\
                \x20           Some(__vox_live_ty) => {{\n\
                \x20               if __vox_norm_sql_ty(__vox_live_ty) != __vox_norm_sql_ty(\"{expected_ty}\") {{\n\
                \x20                   eprintln!(\"schema drift: table '{table_name}' column '{col_lower}' type mismatch (expected {expected_ty}, got {{}})\", __vox_live_ty);\n\
                \x20                   std::process::exit(2);\n\
                \x20               }}\n\
                \x20           }}\n\
                \x20           None => {{\n\
                \x20               eprintln!(\"schema drift: table '{table_name}' missing required column '{col_lower}'\");\n\
                \x20               std::process::exit(2);\n\
                \x20           }}\n\
                \x20       }}\n"
            ));
        }
    }

    out.push_str("    }\n");
    out
}

fn emit_select_projection_helpers(
    table: &HirTable,
    proj: &[String],
    tn: &str,
    is_json: &dyn Fn(&HirType) -> bool,
) -> String {
    let sfx = db_projection_method_suffix(proj);
    let col_list = proj.join(", ");
    let mut out = String::new();

    // -- from_row_sel_<suffix>
    out.push_str(&format!(
        "    fn from_row_sel_{sfx}(row: &turso::Row) -> Result<Self, turso::Error> {{\n        let _id_val: i64 = row.get(0)?;\n        Ok(Self {{\n            _id: Some(_id_val),\n"
    ));
    for field in &table.fields {
        let is_opt = matches!(&field.type_ann, HirType::Generic(n, _) if n == "Option");
        out.push_str(&format!("            {}: ", field.name));
        if let Some(ci) = proj.iter().position(|c| c == &field.name) {
            let idx = ci + 1;
            if is_json(&field.type_ann) {
                out.push_str(&format!(
                    "{{\n                let s: String = row.get({idx})?;\n                serde_json::from_str(&s).map_err(|e| turso::Error::ConversionFailure(format!(\"JSON decode on {}.{} row _id {{}}: {{}}\", _id_val, e)))?\n            }},\n",
                    tn, field.name
                ));
            } else {
                out.push_str(&format!("row.get({idx})?,\n"));
            }
        } else if is_opt {
            out.push_str("None,\n");
        } else {
            out.push_str(
                "{ return Err(turso::Error::ConversionFailure(\"vox: required column omitted from SELECT projection\".into())) },\n",
            );
        }
    }
    out.push_str("        })\n    }\n\n");

    // -- all_proj_<suffix>
    out.push_str(&format!(
        "    pub async fn all_proj_{sfx}(db: &Codex) -> Result<Vec<Self>, turso::Error> {{\n        let mut rows = db.connection().query(\"SELECT _id, {col_list} FROM {tn}\", ()).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {{\n            out.push(Self::from_row_sel_{sfx}(&row)?);\n        }}\n        Ok(out)\n    }}\n\n"
    ));

    // -- all_order_limit_proj_<suffix>
    out.push_str(&format!(
        "    pub async fn all_order_limit_proj_{sfx}(db: &Codex, order_clause: &str, limit: Option<i64>) -> Result<Vec<Self>, turso::Error> {{\n        let mut sql = \"SELECT _id, {col_list} FROM {tn}\".to_string();\n        if !order_clause.trim().is_empty() {{\n            sql.push_str(\" ORDER BY \");\n            sql.push_str(order_clause);\n        }}\n        if let Some(l) = limit {{\n            sql.push_str(&format!(\" LIMIT {{}}\", l.max(0)));\n        }}\n        let mut rows = db.connection().query(&sql, ()).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {{\n            out.push(Self::from_row_sel_{sfx}(&row)?);\n        }}\n        Ok(out)\n    }}\n\n"
    ));

    // -- filter_where_proj_<suffix>
    out.push_str(&format!(
        "    pub async fn filter_where_proj_{sfx}(db: &Codex, where_clause: &str, params: impl turso::IntoParams + Send) -> Result<Vec<Self>, turso::Error> {{\n        let sql = format!(\"SELECT _id, {col_list} FROM {tn} WHERE {{}}\", where_clause);\n        let mut rows = db.connection().query(&sql, params).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {{\n            out.push(Self::from_row_sel_{sfx}(&row)?);\n        }}\n        Ok(out)\n    }}\n\n"
    ));

    // -- filter_where_order_limit_proj_<suffix>
    out.push_str(&format!(
        "    pub async fn filter_where_order_limit_proj_{sfx}(db: &Codex, where_clause: &str, params: impl turso::IntoParams + Send, order_clause: &str, limit: Option<i64>) -> Result<Vec<Self>, turso::Error> {{\n        let mut sql = format!(\"SELECT _id, {col_list} FROM {tn} WHERE {{}}\", where_clause);\n        if !order_clause.trim().is_empty() {{\n            sql.push_str(\" ORDER BY \");\n            sql.push_str(order_clause);\n        }}\n        if let Some(l) = limit {{\n            sql.push_str(&format!(\" LIMIT {{}}\", l.max(0)));\n        }}\n        let mut rows = db.connection().query(&sql, params).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {{\n            out.push(Self::from_row_sel_{sfx}(&row)?);\n        }}\n        Ok(out)\n    }}\n\n"
    ));

    out
}

/// Generate a Rust struct for a @table type w/ methods (tests and tooling).
///
/// `projections` lists extra `SELECT _id, …` column sets referenced by `.select(...)` in the module.
pub fn emit_table_struct(table: &HirTable, projections: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
    out.push_str(&format!("pub struct {} {{\n", table.name));
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub _id: Option<i64>,\n");
    for field in &table.fields {
        out.push_str(&format!(
            "    pub {}: {},\n",
            field.name,
            emit_type(&field.type_ann)
        ));
    }
    out.push_str("}\n\n");

    let is_json = |ty: &HirType| hir_type_needs_json_serialization(ty);

    out.push_str(&format!("impl {} {{\n", table.name));

    let dialect = generated_sql_dialect();
    let col_names: Vec<&String> = table.fields.iter().map(|f| &f.name).collect();
    let placeholders: Vec<String> = (1..=col_names.len())
        .map(|i| placeholder_sql(&dialect, i))
        .collect();
    let tn = to_snake_case(&table.name);
    let pk_col = table.primary_key.as_deref().unwrap_or("_id");
    let pk_rust_ty = table
        .primary_key
        .as_ref()
        .and_then(|name| table.fields.iter().find(|f| &f.name == name))
        .map(|f| emit_type(&f.type_ann))
        .unwrap_or_else(|| "i64".to_string());

    // -- insert
    out.push_str(
        "    pub async fn insert(db: &Codex, item: &Self) -> Result<i64, turso::Error> {\n",
    );
    for field in &table.fields {
        if is_json(&field.type_ann) {
            out.push_str(&format!(
                "        let {}_json = serde_json::to_string(&item.{}).map_err(|e| turso::Error::ConversionFailure(format!(\"serde_json: {{}}\", e)))?;\n",
                field.name, field.name
            ));
        }
    }
    out.push_str(&format!(
        "        db.connection().execute(\n            \"INSERT INTO {} ({}) VALUES ({})\",\n            turso::params![",
        tn,
        col_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    ));
    for (i, field) in table.fields.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        if is_json(&field.type_ann) {
            out.push_str(&format!("{}_json", field.name));
        } else {
            out.push_str(&format!("item.{}.clone()", field.name));
        }
    }
    out.push_str("],\n        ).await?;\n");
    out.push_str("        Ok(db.connection().last_insert_rowid())\n");
    out.push_str("    }\n\n");

    // -- get
    out.push_str(&format!(
        "    pub async fn get(db: &Codex, id: {pk_rust_ty}) -> Result<Option<Self>, turso::Error> {{\n",
    ));
    out.push_str(&format!(
        "        let mut rows = db.connection().query(\"SELECT * FROM {} WHERE {} = {}\", turso::params![id]).await?;\n",
        tn,
        pk_col,
        placeholder_sql(&dialect, 1)
    ));
    out.push_str("        Ok(match rows.next().await? {\n");
    out.push_str("            Some(row) => Some(Self::from_row(&row)?),\n");
    out.push_str("            None => None,\n");
    out.push_str("        })\n");
    out.push_str("    }\n\n");

    // -- all (safe full scan)
    out.push_str("    pub async fn all(db: &Codex) -> Result<Vec<Self>, turso::Error> {\n");
    out.push_str(&format!(
        "        let mut rows = db.connection().query(\"SELECT * FROM {}\", ()).await?;\n",
        tn
    ));
    out.push_str(
        "        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {\n            out.push(Self::from_row(&row)?);\n        }\n        Ok(out)\n",
    );
    out.push_str("    }\n\n");

    // -- all_order_limit (safe ORDER BY / LIMIT from compiler-validated clauses)
    out.push_str(
        "    pub async fn all_order_limit(db: &Codex, order_clause: &str, limit: Option<i64>) -> Result<Vec<Self>, turso::Error> {\n",
    );
    out.push_str(&format!(
        "        let mut sql = \"SELECT * FROM {}\".to_string();\n",
        tn
    ));
    out.push_str(
        "        if !order_clause.trim().is_empty() {\n            sql.push_str(\" ORDER BY \");\n            sql.push_str(order_clause);\n        }\n        if let Some(l) = limit {\n            sql.push_str(&format!(\" LIMIT {}\", l.max(0)));\n        }\n",
    );
    out.push_str(
        "        let mut rows = db.connection().query(&sql, ()).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {\n            out.push(Self::from_row(&row)?);\n        }\n        Ok(out)\n",
    );
    out.push_str("    }\n\n");

    // -- count
    out.push_str("    pub async fn count(db: &Codex) -> Result<i64, turso::Error> {\n");
    out.push_str(&format!(
        "        let mut rows = db.connection().query(\"SELECT COUNT(*) FROM {}\", ()).await?;\n",
        tn
    ));
    out.push_str(
        "        let row = rows.next().await?.ok_or(turso::Error::QueryReturnedNoRows)?;\n",
    );
    out.push_str("        let c: i64 = row.get(0)?;\n");
    out.push_str("        Ok(c)\n");
    out.push_str("    }\n\n");

    // -- count_where: parameterized count with WHERE fragment
    out.push_str(
        "    pub async fn count_where(db: &Codex, where_clause: &str, params: impl turso::IntoParams + Send) -> Result<i64, turso::Error> {\n",
    );
    out.push_str(&format!(
        "        let sql = format!(\"SELECT COUNT(*) FROM {} WHERE {{}}\", where_clause);\n",
        tn
    ));
    out.push_str(
        "        let mut rows = db.connection().query(&sql, params).await?;\n        let row = rows.next().await?.ok_or(turso::Error::QueryReturnedNoRows)?;\n        let c: i64 = row.get(0)?;\n        Ok(c)\n",
    );
    out.push_str("    }\n\n");

    // -- filter_where: parameterized equality predicates (compiler supplies literal column names)
    out.push_str(
        "    /// `WHERE` fragment uses `?1`, `?2`, … placeholders; bind with `turso::params!`.\n",
    );
    out.push_str(
        "    pub async fn filter_where(db: &Codex, where_clause: &str, params: impl turso::IntoParams + Send) -> Result<Vec<Self>, turso::Error> {\n",
    );
    out.push_str(&format!(
        "        let sql = format!(\"SELECT * FROM {} WHERE {{}}\", where_clause);\n",
        tn
    ));
    out.push_str(
        "        let mut rows = db.connection().query(&sql, params).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {\n            out.push(Self::from_row(&row)?);\n        }\n        Ok(out)\n",
    );
    out.push_str("    }\n\n");

    // -- filter_where_order_limit: parameterized filter + safe ORDER BY / LIMIT
    out.push_str(
        "    pub async fn filter_where_order_limit(db: &Codex, where_clause: &str, params: impl turso::IntoParams + Send, order_clause: &str, limit: Option<i64>) -> Result<Vec<Self>, turso::Error> {\n",
    );
    out.push_str(&format!(
        "        let mut sql = format!(\"SELECT * FROM {} WHERE {{}}\", where_clause);\n",
        tn
    ));
    out.push_str(
        "        if !order_clause.trim().is_empty() {\n            sql.push_str(\" ORDER BY \");\n            sql.push_str(order_clause);\n        }\n        if let Some(l) = limit {\n            sql.push_str(&format!(\" LIMIT {}\", l.max(0)));\n        }\n",
    );
    out.push_str(
        "        let mut rows = db.connection().query(&sql, params).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {\n            out.push(Self::from_row(&row)?);\n        }\n        Ok(out)\n",
    );
    out.push_str("    }\n\n");

    // -- unsafe_query_raw_clause (caller-controlled fragment after SELECT * FROM t)
    out.push_str(
        "    /// Builds `format!(\"SELECT * FROM <table> {}\", clause)` — use [`Self::all`] or [`Self::get`] when possible.\n",
    );
    out.push_str(
        "    pub async fn unsafe_query_raw_clause(db: &Codex, clause: &str) -> Result<Vec<Self>, turso::Error> {\n",
    );
    out.push_str(&format!(
        "        let sql = format!(\"SELECT * FROM {} {{}}\", clause);\n",
        tn
    ));
    out.push_str(
        "        let mut rows = db.connection().query(&sql, ()).await?;\n        let mut out = Vec::new();\n        while let Some(row) = rows.next().await? {\n            out.push(Self::from_row(&row)?);\n        }\n        Ok(out)\n",
    );
    out.push_str("    }\n\n");

    // -- delete
    out.push_str(&format!(
        "    pub async fn delete(db: &Codex, id: {pk_rust_ty}) -> Result<usize, turso::Error> {{\n",
    ));
    out.push_str(&format!(
        "        let n = db.connection().execute(\"DELETE FROM {} WHERE {} = {}\", turso::params![id]).await?;\n",
        tn,
        pk_col,
        placeholder_sql(&dialect, 1)
    ));
    out.push_str("        Ok(n as usize)\n");
    out.push_str("    }\n\n");

    // -- from_row (column order: _id, then fields in declaration order)
    out.push_str("    fn from_row(row: &turso::Row) -> Result<Self, turso::Error> {\n");
    out.push_str("        let _id_val: i64 = row.get(0)?;\n");
    out.push_str("        Ok(Self {\n");
    out.push_str("            _id: Some(_id_val),\n");
    for (col_idx, field) in (1usize..).zip(table.fields.iter()) {
        if is_json(&field.type_ann) {
            out.push_str(&format!(
                "            {}: {{\n                let s: String = row.get({})?;\n                serde_json::from_str(&s).map_err(|e| turso::Error::ConversionFailure(format!(\"JSON decode on {}.{} row _id {{}}: {{}}\", _id_val, e)))?\n            }},\n",
                field.name, col_idx, tn, field.name
            ));
        } else {
            out.push_str(&format!(
                "            {}: row.get({})?,\n",
                field.name, col_idx
            ));
        }
    }
    out.push_str("        })\n");
    out.push_str("    }\n");

    for proj in projections {
        if proj.is_empty() {
            continue;
        }
        let field_names: Vec<&String> = table.fields.iter().map(|f| &f.name).collect();
        let ok = proj.iter().all(|c| field_names.contains(&c));
        if !ok || proj.len() > field_names.len() {
            continue;
        }
        out.push_str(&emit_select_projection_helpers(
            table,
            proj,
            tn.as_str(),
            &is_json,
        ));
    }

    out.push_str("}\n\n");
    out
}

/// Returns `true` when a HIR type must be serialized to JSON before storage
/// in a SQL column (i.e. it is not a SQLite-native scalar).
///
/// Extracted from the `is_json` closure in `emit_table_struct` per CR-A1:
/// the 4-branch match contributed DPs inline and also prevented the closure
/// from being referenced in `emit_select_projection_helpers`.
fn hir_type_needs_json_serialization(ty: &HirType) -> bool {
    match ty {
        HirType::Named(n) => VoxScalar::parse(n).is_none(),
        HirType::Generic(n, args) => {
            if n == "Option" {
                // Option<T>: if T is a SQL scalar, no JSON needed; otherwise JSON.
                match &args[0] {
                    HirType::Named(sub) => VoxScalar::parse(sub).is_none(),
                    _ => true,
                }
            } else if n == "Id" {
                false
            } else {
                true // List, etc.
            }
        }
        _ => true, // Unit, tuple, etc.
    }
}

/// Generate `CREATE TABLE IF NOT EXISTS` DDL for a @table.
pub fn emit_table_ddl(table: &HirTable) -> String {
    let table_name = table_live_name(table);
    let mut cols = vec!["_id INTEGER PRIMARY KEY AUTOINCREMENT".to_string()];
    for field in &table.fields {
        let sql_type = hir_type_to_sql(&field.type_ann);
        let not_null = if matches!(&field.type_ann, HirType::Generic(n, _) if n == "Option") {
            ""
        } else {
            " NOT NULL"
        };
        cols.push(format!("    {} {}{}", field.name, sql_type, not_null));
    }
    if let Some(pk) = &table.primary_key {
        cols.push(format!("    UNIQUE ({pk})"));
    }
    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        table_name,
        cols.join(",\n")
    )
}

/// Generate `CREATE INDEX IF NOT EXISTS` DDL for a @index.
pub fn emit_index_ddl(index: &HirIndex) -> String {
    let table_name = index.table_name.to_lowercase();
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{table}_{name} ON {table} ({cols});",
        table = table_name,
        name = index.index_name,
        cols = index.columns.join(", "),
    )
}

/// Generate the DB initialization code for `main()` (Turso / libSQL).
///
/// Opens **Codex** via `vox_db::DbConfig::resolve_canonical` (VOX_DB_*, legacy TURSO_*, or local file).
pub fn emit_db_setup(module: &HirModule) -> String {
    let mut out = String::new();
    out.push_str("    // ── Database setup (Codex / vox_db) ──\n");
    out.push_str("    // Guard rail: app-plane DB URL may target non-libsql in this phase.\n");
    out.push_str(
        "    // Keep Codex setup for table runtime and emit a warning instead of hard-failing.\n",
    );
    out.push_str("    if let Some(app_url) = vox_db::resolve_app_db_url() {\n");
    out.push_str("        let u = app_url.to_ascii_lowercase();\n");
    out.push_str(
        "        if u.starts_with(\"postgres://\") || u.starts_with(\"postgresql://\") || u.starts_with(\"mysql://\") {\n",
    );
    out.push_str(
        "            eprintln!(\"VOX_APP_DB_URL uses non-libsql backend ({}) — generated Axum table runtime still boots Codex while backend-specific table dispatch is completed incrementally\", app_url);\n",
    );
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str(
        "    let cfg = match vox_db::DbConfig::resolve_canonical() {\n\
        \x20       Ok(cfg) => cfg,\n\
        \x20       Err(e) => {\n\
        \x20           eprintln!(\"Failed to resolve Codex DB config (VOX_DB_URL+TOKEN, or VOX_DB_PATH): {}\", e);\n\
        \x20           std::process::exit(2);\n\
        \x20       }\n\
        \x20   };\n",
    );
    out.push_str(
        "    let codex = match vox_db::Codex::connect(cfg).await {\n\
        \x20       Ok(db) => db,\n\
        \x20       Err(e) => {\n\
        \x20           eprintln!(\"Failed to open Codex database: {}\", e);\n\
        \x20           std::process::exit(2);\n\
        \x20       }\n\
        \x20   };\n",
    );
    // PRAGMA setup. `journal_mode=WAL` returns a result row (the
    // mode that ended up being set), which turso's `execute_batch`
    // can't consume — it panics with "Misuse: unexpected row during
    // execution". Run it as a query and drain the rows, then exec
    // the row-less PRAGMAs in a batch. Per the 2026-05-23 slot-2
    // todo-auth runtime bring-up.
    out.push_str(
        "    {\n\
        \x20       let mut __vox_jm = match codex.connection().query(\"PRAGMA journal_mode=WAL;\", ()).await {\n\
        \x20           Ok(rows) => rows,\n\
        \x20           Err(e) => {\n\
        \x20               eprintln!(\"PRAGMA journal_mode failed: {}\", e);\n\
        \x20               std::process::exit(2);\n\
        \x20           }\n\
        \x20       };\n\
        \x20       loop {\n\
        \x20           match __vox_jm.next().await {\n\
        \x20               Ok(Some(_)) => {}\n\
        \x20               Ok(None) => break,\n\
        \x20               Err(e) => {\n\
        \x20                   eprintln!(\"PRAGMA journal_mode row iteration failed: {}\", e);\n\
        \x20                   std::process::exit(2);\n\
        \x20               }\n\
        \x20           }\n\
        \x20       }\n\
        \x20   }\n",
    );
    out.push_str(
        "    if let Err(e) = codex.connection().execute_batch(\"PRAGMA foreign_keys=ON;\").await {\n\
        \x20       eprintln!(\"PRAGMA foreign_keys failed: {}\", e);\n\
        \x20       std::process::exit(2);\n\
        \x20   }\n",
    );
    out.push_str("    if let Err(e) = codex.connection().execute_batch(r#\"\n");
    for table in &module.tables {
        out.push_str(&emit_table_ddl(table));
        out.push('\n');
    }
    for index in &module.indexes {
        out.push_str(&emit_index_ddl(index));
        out.push('\n');
    }
    // Raw-string close: `"#` ends `r#"..."#`. The previous code
    // emitted a stray `#` before the close, producing `#"#)` which
    // SQLite parsed as "bad variable name '#'". Per the 2026-05-23
    // slot-2 todo-auth runtime bring-up.
    out.push_str("\"#).await {\n");
    out.push_str("        eprintln!(\"schema migration failed: {}\", e);\n");
    out.push_str("        std::process::exit(2);\n");
    out.push_str("    }\n");
    out.push_str(&emit_schema_drift_verify(module));
    out.push_str("    let db = Arc::new(codex);\n\n");
    out
}

#[cfg(test)]
mod tests {
    use super::sql_dialect_from_urls;
    use vox_sql::SqlDialect;

    #[test]
    fn app_plane_url_precedes_codex_url_for_dialect() {
        let dialect = sql_dialect_from_urls(
            Some("postgres://user:pass@localhost:5432/app"),
            Some("libsql://example.turso.io"),
        );
        assert_eq!(
            dialect.placeholder_style,
            SqlDialect::postgres().placeholder_style
        );
    }

    #[test]
    fn falls_back_to_codex_url_then_sqlite_default() {
        let mysql = sql_dialect_from_urls(None, Some("mysql://localhost/db"));
        assert_eq!(
            mysql.placeholder_style,
            SqlDialect::mysql().placeholder_style
        );

        let fallback = sql_dialect_from_urls(Some("invalid"), Some("also-invalid"));
        assert_eq!(
            fallback.placeholder_style,
            SqlDialect::sqlite().placeholder_style
        );
    }
}
