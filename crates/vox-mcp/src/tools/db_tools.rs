//! Database introspection tool handlers for the Vox MCP server.
//!
//! Covers: db_schema, db_relationships, db_data_flow, db_sample_data,
//! db_explain_query, db_suggest_query, and the shared parse_vox_module helper.

use crate::params::ToolResult;
use crate::server::ServerState;

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

/// Parse a .vox file and return its Module AST.
pub(crate) fn parse_vox_module(path: &str) -> Result<vox_ast::decl::Module, String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("Cannot read file '{}': {}", path, e))?;
    let tokens = vox_lexer::cursor::lex(&source);
    let module = vox_parser::parse(tokens).map_err(|errs| {
        let msgs: Vec<String> = errs.iter().map(|e| format!("{:?}", e)).collect();
        format!("Parse errors: {}", msgs.join("; "))
    })?;
    Ok(module)
}

// ---------------------------------------------------------------------------
// Schema tools
// ---------------------------------------------------------------------------

/// Return the complete schema digest for a .vox file as JSON.
pub fn vox_db_schema(args: serde_json::Value) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult::<serde_json::Value>::err(
            "Missing 'path' parameter. Provide the path to a .vox file.",
        )
        .to_json();
    }

    match parse_vox_module(path) {
        Ok(module) => {
            let digest = vox_db::generate_schema_digest(&module);
            match vox_db::digest_to_json(&digest) {
                Ok(json) => ToolResult::ok(
                    serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default(),
                )
                .to_json(),
                Err(e) => {
                    ToolResult::<serde_json::Value>::err(format!("Serialization error: {}", e))
                        .to_json()
                }
            }
        }
        Err(e) => ToolResult::<serde_json::Value>::err(e).to_json(),
    }
}

/// Return the entity-relationship graph.
pub fn vox_db_relationships(args: serde_json::Value) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult::<serde_json::Value>::err("Missing 'path' parameter.").to_json();
    }

    match parse_vox_module(path) {
        Ok(module) => {
            let digest = vox_db::generate_schema_digest(&module);
            let context = vox_db::format_llm_context(&digest);
            ToolResult::ok(serde_json::json!({
                "relationships": digest.relationships,
                "llm_context": context,
            }))
            .to_json()
        }
        Err(e) => ToolResult::<serde_json::Value>::err(e).to_json(),
    }
}

/// Return the data flow map.
pub fn vox_db_data_flow(args: serde_json::Value) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult::<serde_json::Value>::err("Missing 'path' parameter.").to_json();
    }

    match parse_vox_module(path) {
        Ok(module) => {
            let digest = vox_db::generate_schema_digest(&module);
            let flow = vox_db::build_data_flow(&digest);
            match serde_json::to_string_pretty(&flow) {
                Ok(json) => ToolResult::ok(
                    serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default(),
                )
                .to_json(),
                Err(e) => {
                    ToolResult::<serde_json::Value>::err(format!("Serialization error: {}", e))
                        .to_json()
                }
            }
        }
        Err(e) => ToolResult::<serde_json::Value>::err(e).to_json(),
    }
}

/// Return sample data for a table (async).
pub async fn vox_db_sample_data(state: &ServerState, args: serde_json::Value) -> String {
    let table = args.get("table").and_then(|v| v.as_str()).unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(3);

    if table.is_empty() {
        return ToolResult::<serde_json::Value>::err("Missing 'table' parameter.").to_json();
    }

    let db = match &state.db {
        Some(db) => db,
        None => {
            return ToolResult::<serde_json::Value>::err("VoxDb is not connected.").to_json()
        }
    };

    let info_sql = format!("PRAGMA table_info({})", table);
    let mut info_rows = match db.store().conn.query(&info_sql, ()).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("DB error (table info): {e}"))
                .to_json()
        }
    };

    let mut col_names = Vec::new();
    while let Ok(Some(row)) = info_rows.next().await {
        if let Ok(name) = row.get::<String>(1) {
            col_names.push(name);
        }
    }

    if col_names.is_empty() {
        return ToolResult::<serde_json::Value>::err(format!(
            "Table '{table}' does not exist or has no columns."
        ))
        .to_json();
    }

    let sql = format!("SELECT * FROM {} LIMIT {}", table, limit);
    let mut rows = match db.store().conn.query(&sql, ()).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("DB error (select): {e}"))
                .to_json()
        }
    };

    let mut results = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let mut map = serde_json::Map::new();
        for (i, col_name) in col_names.iter().enumerate() {
            let val = match row.get_value(i) {
                Ok(v) => match v {
                    turso::Value::Null => serde_json::Value::Null,
                    turso::Value::Integer(i) => serde_json::Value::Number(i.into()),
                    turso::Value::Real(f) => serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                    turso::Value::Text(s) => serde_json::Value::String(s),
                    turso::Value::Blob(b) => {
                        serde_json::Value::String(format!("(blob {} bytes)", b.len()))
                    }
                },
                Err(_) => serde_json::Value::String("<error>".to_string()),
            };
            map.insert(col_name.to_string(), val);
        }
        results.push(serde_json::Value::Object(map));
    }

    ToolResult::ok(serde_json::json!({
        "table": table,
        "sample_data": results
    }))
    .to_json()
}

/// Use LLM to explain a query/mutation in plain English.
pub async fn vox_db_explain_query(args: serde_json::Value) -> String {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let schema_path = args
        .get("schema_path")
        .and_then(|v| v.as_str())
        .unwrap_or("src/main.vox");

    if query.is_empty() {
        return ToolResult::<String>::err("Missing 'query' parameter.").to_json();
    }

    let schema_digest = match parse_vox_module(schema_path) {
        Ok(module) => vox_db::generate_schema_digest(&module),
        Err(e) => {
            return ToolResult::<String>::err(format!("Failed to parse schema: {e}")).to_json()
        }
    };

    let llm_context = vox_db::format_llm_context(&schema_digest);
    let prompt = format!(
        "You are an expert Vox/Rust database engineer. Explain what this query does in plain English.\n\nQuery:\n```vox\n{query}\n```\n\nSchema Context:\n{llm_context}\n\nExplanation:"
    );

    let mut new_args = args.clone();
    if let serde_json::Value::Object(ref mut map) = new_args {
        map.insert("prompt".to_string(), serde_json::Value::String(prompt));
        map.insert(
            "validate".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    super::compiler_tools::generate_vox_code(new_args).await
}

/// Use LLM to suggest the actual Vox query code given a natural language intent.
pub async fn vox_db_suggest_query(args: serde_json::Value) -> String {
    let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("");
    let schema_path = args
        .get("schema_path")
        .and_then(|v| v.as_str())
        .unwrap_or("src/main.vox");

    if intent.is_empty() {
        return ToolResult::<String>::err("Missing 'intent' parameter.").to_json();
    }

    let schema_digest = match parse_vox_module(schema_path) {
        Ok(module) => vox_db::generate_schema_digest(&module),
        Err(e) => {
            return ToolResult::<String>::err(format!("Failed to parse schema: {e}")).to_json()
        }
    };

    let llm_context = vox_db::format_llm_context(&schema_digest);
    let prompt = format!(
        "You are an expert Vox database engineer. Write a Vox DB query/mutation that satisfies the following intent.\n\nIntent: {intent}\n\nSchema Context:\n{llm_context}\n\nRespond with only valid code in a ```vox ... ``` block."
    );

    let mut new_args = args.clone();
    if let serde_json::Value::Object(ref mut map) = new_args {
        map.insert("prompt".to_string(), serde_json::Value::String(prompt));
    }

    super::compiler_tools::generate_vox_code(new_args).await
}
