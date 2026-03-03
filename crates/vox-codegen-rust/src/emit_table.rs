use vox_hir::{HirCollection, HirIndex, HirTable, HirType};

use crate::emit::emit_type;

/// Map a Vox HIR type to a Turso-compatible column type.
pub fn hir_type_to_sql(ty: &HirType) -> &'static str {
    match ty {
        HirType::Named(n) => match n.as_str() {
            "int" => "INTEGER",
            "float" | "float64" => "REAL",
            "bool" => "INTEGER", // Turso/SQLite-compat: 0/1
            "str" => "TEXT",
            _ => "TEXT",
        },
        HirType::Generic(n, _) => match n.as_str() {
            "Id" => "INTEGER",
            "Option" => "TEXT",
            _ => "TEXT",
        },
        _ => "TEXT",
    }
}

/// Generate a Rust struct for a @table type with async Turso-backed methods.
pub fn emit_table_struct(table: &HirTable) -> String {
    let mut out = String::new();
    if table.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
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

    let is_json = |ty: &HirType| -> bool {
        match ty {
            HirType::Named(n) => !matches!(n.as_str(), "int" | "float" | "bool" | "str"),
            HirType::Generic(n, args) => {
                if n == "Option" {
                    match &args[0] {
                        HirType::Named(sub) => {
                            !matches!(sub.as_str(), "int" | "float" | "bool" | "str")
                        }
                        _ => true,
                    }
                } else {
                    n != "Id"
                }
            }
            _ => true,
        }
    };

    let table_lower = table.name.to_lowercase();
    let col_names: Vec<&str> = table.fields.iter().map(|f| f.name.as_str()).collect();
    let placeholders: Vec<String> = (1..=col_names.len()).map(|i| format!("?{}", i)).collect();
    let cols_joined = col_names.join(", ");
    let placeholders_joined = placeholders.join(", ");

    out.push_str(&format!("impl {} {{\n", table.name));

    // insert(conn, item) -> Result<i64, turso::Error>
    out.push_str(
        "    pub async fn insert(conn: &turso::Connection, item: &Self) -> Result<i64, turso::Error> {\n",
    );
    let mut param_exprs = Vec::new();
    for field in &table.fields {
        if is_json(&field.type_ann) {
            param_exprs.push(format!(
                "serde_json::to_string(&item.{}).map_err(|e| turso::Error::ConversionFailure(format!(\"json: {{}}\", e)))?",
                field.name
            ));
        } else {
            param_exprs.push(format!("item.{}.clone()", field.name));
        }
    }
    let params_str = param_exprs.join(", ");
    out.push_str(&format!(
        "        conn.execute(\"INSERT INTO {} ({}) VALUES ({})\", turso::params!({})).await?;\n",
        table_lower, cols_joined, placeholders_joined, params_str
    ));
    out.push_str("        Ok(conn.last_insert_rowid())\n");
    out.push_str("    }\n\n");

    // get(conn, id) -> Result<Option<Self>, turso::Error>
    out.push_str("    pub async fn get(conn: &turso::Connection, id: i64) -> Result<Option<Self>, turso::Error> {\n");
    out.push_str(&format!(
        "        let mut rows = conn.query(\"SELECT * FROM {} WHERE _id = ?1\", turso::params!(id)).await?;\n",
        table_lower
    ));
    out.push_str("        if let Some(row) = rows.next().await? {\n");
    out.push_str("            Ok(Some(Self::from_row(&row)?))\n");
    out.push_str("        } else {\n");
    out.push_str("            Ok(None)\n");
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    // query(conn, clause, params) - simplified: single param for common case
    out.push_str("    pub async fn query(conn: &turso::Connection, clause: &str) -> Result<Vec<Self>, turso::Error> {\n");
    out.push_str(&format!(
        "        let sql = format!(\"SELECT * FROM {} {{}} \", clause);\n",
        table_lower
    ));
    out.push_str("        let mut rows = conn.query(&sql, ()).await?;\n");
    out.push_str("        let mut out = Vec::new();\n");
    out.push_str("        while let Some(row) = rows.next().await? {\n");
    out.push_str("            out.push(Self::from_row(&row)?);\n");
    out.push_str("        }\n");
    out.push_str("        Ok(out)\n");
    out.push_str("    }\n\n");

    // delete(conn, id) -> Result<usize, turso::Error>
    out.push_str("    pub async fn delete(conn: &turso::Connection, id: i64) -> Result<usize, turso::Error> {\n");
    out.push_str(&format!(
        "        conn.execute(\"DELETE FROM {} WHERE _id = ?1\", turso::params!(id)).await?;\n",
        table_lower
    ));
    out.push_str("        Ok(1)\n");
    out.push_str("    }\n\n");

    // from_row
    out.push_str("    fn from_row(row: &turso::Row) -> Result<Self, turso::Error> {\n");
    out.push_str("        let _id_val: i64 = row.get::<i64>(0)?;\n");
    out.push_str("        Ok(Self {\n");
    out.push_str("            _id: Some(_id_val),\n");
    for (i, field) in table.fields.iter().enumerate() {
        let idx = i + 1;
        if is_json(&field.type_ann) {
            out.push_str(&format!(
                "            {}: serde_json::from_str(&row.get::<String>({})?).map_err(|e| turso::Error::ConversionFailure(format!(\"json: {{}}\", e)))?,\n",
                field.name, idx
            ));
        } else {
            // bool stored as INTEGER 0/1; Option needs proper handling
            let get_expr = match &field.type_ann {
                HirType::Named(n) if n == "bool" => {
                    format!("row.get::<i64>({})? != 0", idx)
                }
                HirType::Generic(n, args) if n == "Option" => {
                    let inner = &args[0];
                    match inner {
                        HirType::Named(sub) if sub == "str" => {
                            format!("row.get::<Option<String>>({})?", idx)
                        }
                        HirType::Named(sub) if sub == "int" => {
                            format!("row.get::<Option<i64>>({})?", idx)
                        }
                        HirType::Named(sub) if sub == "float" => {
                            format!("row.get::<Option<f64>>({})?", idx)
                        }
                        HirType::Named(sub) if sub == "bool" => {
                            format!("row.get::<Option<i64>>({})?.map(|v| v != 0)", idx)
                        }
                        _ => format!("row.get::<Option<String>>({})?.and_then(|s| serde_json::from_str(&s).ok())", idx),
                    }
                }
                _ => {
                    let rust_ty = emit_type(&field.type_ann);
                    format!("row.get::<{}>({})?", rust_ty, idx)
                }
            };
            out.push_str(&format!("            {}: {},\n", field.name, get_expr));
        }
    }
    out.push_str("        })\n");
    out.push_str("    }\n");

    out.push_str("}\n\n");
    out
}

/// Generate `CREATE TABLE IF NOT EXISTS` DDL for a @table.
pub fn emit_table_ddl(table: &HirTable) -> String {
    let table_name = table.name.to_lowercase();
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

/// Generate a Rust struct for a @collection type wrapping serde_json::Value.
pub fn emit_collection_struct(coll: &HirCollection) -> String {
    let mut out = String::new();
    if coll.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
    out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
    out.push_str(&format!("pub struct {} {{\n", coll.name));
    out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    out.push_str("    pub _id: Option<i64>,\n");
    out.push_str("    #[serde(flatten)]\n");
    out.push_str("    pub _data: serde_json::Value,\n");
    out.push_str("}\n\n");

    out.push_str(&format!("impl {} {{\n", coll.name));

    // Typed accessor methods for defined fields
    for field in &coll.fields {
        let rust_ty = emit_type(&field.type_ann);
        out.push_str(&format!(
            "    pub fn {}(&self) -> Option<{}> {{\n",
            field.name, rust_ty
        ));
        out.push_str(&format!(
            "        serde_json::from_value(self._data.get(\"{}\")?.clone()).ok()\n",
            field.name
        ));
        out.push_str("    }\n");
    }

    // Async database methods
    out.push_str(&format!(
        "    pub async fn insert(conn: &turso::Connection, item: &Self) -> Result<i64, turso::Error> {{\n\
         \x20       let data_json = serde_json::to_string(&item._data).map_err(|e| turso::Error::ConversionFailure(format!(\"json: {{}}\", e)))?;\n\
         \x20       conn.execute(\"INSERT INTO {} (_data) VALUES (?1)\", turso::params!(data_json)).await?;\n\
         \x20       Ok(conn.last_insert_rowid())\n\
         \x20   }}\n\n",
        coll.name.to_lowercase()
    ));

    out.push_str(&format!(
        "    pub async fn get(conn: &turso::Connection, id: i64) -> Result<Option<Self>, turso::Error> {{\n\
         \x20       let mut rows = conn.query(\"SELECT * FROM {} WHERE _id = ?1\", turso::params!(id)).await?;\n\
         \x20       if let Some(row) = rows.next().await? {{\n\
         \x20           Ok(Some(Self::from_row(&row)?))\n\
         \x20       }} else {{\n\
         \x20           Ok(None)\n\
         \x20       }}\n\
         \x20   }}\n\n",
        coll.name.to_lowercase()
    ));

    out.push_str(&format!(
        "    pub async fn query(conn: &turso::Connection, clause: &str) -> Result<Vec<Self>, turso::Error> {{\n\
         \x20       let sql = format!(\"SELECT * FROM {} {{}}\", clause);\n\
         \x20       let mut rows = conn.query(&sql, ()).await?;\n\
         \x20       let mut out = Vec::new();\n\
         \x20       while let Some(row) = rows.next().await? {{\n\
         \x20           out.push(Self::from_row(&row)?);\n\
         \x20       }}\n\
         \x20       Ok(out)\n\
         \x20   }}\n\n",
        coll.name.to_lowercase()
    ));

    out.push_str(&format!(
        "    pub async fn delete(conn: &turso::Connection, id: i64) -> Result<usize, turso::Error> {{\n\
         \x20       conn.execute(\"DELETE FROM {} WHERE _id = ?1\", turso::params!(id)).await?;\n\
         \x20       Ok(1)\n\
         \x20   }}\n\n",
        coll.name.to_lowercase()
    ));

    // from_row
    out.push_str(
        "    fn from_row(row: &turso::Row) -> Result<Self, turso::Error> {\n\
         \x20       let id_val: i64 = row.get::<i64>(0)?;\n\
         \x20       let data_str: String = row.get::<String>(1)?;\n\
         \x20       let data: serde_json::Value = serde_json::from_str(&data_str)\n\
         \x20           .map_err(|e| turso::Error::ConversionFailure(format!(\"collection json: {{}}\", e)))?;\n\
         \x20       Ok(Self {\n\
         \x20           _id: Some(id_val),\n\
         \x20           _data: data,\n\
         \x20       })\n\
         \x20   }\n",
    );

    out.push_str("}\n\n");
    out
}

/// Generate `CREATE TABLE IF NOT EXISTS` DDL for a @collection.
pub fn emit_collection_ddl(coll: &HirCollection) -> String {
    let lower_name = coll.name.to_lowercase();
    format!(
        "CREATE TABLE IF NOT EXISTS {} (
    _id INTEGER PRIMARY KEY AUTOINCREMENT,
    _data TEXT NOT NULL,
    _created_at TEXT DEFAULT (datetime('now')),
    _updated_at TEXT DEFAULT (datetime('now'))
);",
        lower_name
    )
}
