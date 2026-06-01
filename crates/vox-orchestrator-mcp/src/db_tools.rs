//! Database introspection tool handlers for the Vox MCP server.
//!
//! Covers: db_schema, db_relationships, db_data_flow, db_sample_data,
//! db_explain_query, db_suggest_query, and the shared parse_vox_module helper.

use std::path::Path;

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_VOX_FILE_PATH: &str =
    "Pass `path` to a `.vox` module (workspace-relative or absolute) that defines your schema.";
const REM_VOXDB_SAMPLE: &str = "Attach VoxDb/Turso to the MCP server for live table samples, or use `vox db` in a configured CLI.";
const REM_DB_QUERY: &str =
    "Provide a non-empty `query` string; optional `schema_path` defaults to `src/main.vox`.";
const REM_DB_INTENT: &str =
    "Provide non-empty `intent`; set `schema_path` if your module is not at `src/main.vox`.";
const REM_DB_DIGEST_JSON: &str = "If this persists, the generated schema digest may be unusually large — simplify the `.vox` module or file an issue.";
const REM_MCP_MODEL_LOCK: &str =
    "Retry; restart the MCP server if `mcp_chat_model_override` stays poisoned.";
const REM_MCP_MODEL_RESOLVE: &str = "Run `list_models`, ensure Ollama/API routes work, and check `vox secrets doctor` for inference secrets.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

/// Parse a .vox file and return its Module AST.
pub(crate) fn parse_vox_module(path: &str) -> Result<vox_compiler::ast::decl::Module, String> {
    let source = vox_bounded_fs::read_utf8_path_capped(Path::new(path))
        .map_err(|e| format!("Cannot read file '{}': {}", path, e))?;
    let tokens = vox_compiler::lexer::cursor::lex(&source);
    let module = vox_compiler::parser::parse(tokens).map_err(|errs| {
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
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Missing 'path' parameter. Provide the path to a .vox file.",
            REM_VOX_FILE_PATH,
        )
        .to_json();
    }

    match parse_vox_module(path) {
        Ok(module) => {
            let digest = vox_db::generate_schema_digest(&module, None);
            match vox_db::digest_to_json(&digest) {
                Ok(json) => ToolResult::ok(
                    serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default(),
                )
                .to_json(),
                Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
                    format!("Serialization error: {}", e),
                    REM_DB_DIGEST_JSON,
                )
                .to_json(),
            }
        }
        Err(e) => {
            ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOX_FILE_PATH).to_json()
        }
    }
}

/// Return the entity-relationship graph.
pub fn vox_db_relationships(args: serde_json::Value) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Missing 'path' parameter.",
            REM_VOX_FILE_PATH,
        )
        .to_json();
    }

    match parse_vox_module(path) {
        Ok(module) => {
            let digest = vox_db::generate_schema_digest(&module, None);
            let context = vox_db::format_llm_context(&digest);
            ToolResult::ok(serde_json::json!({
                "relationships": digest.relationships,
                "llm_context": context,
            }))
            .to_json()
        }
        Err(e) => {
            ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOX_FILE_PATH).to_json()
        }
    }
}

/// Return the data flow map.
pub fn vox_db_data_flow(args: serde_json::Value) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Missing 'path' parameter.",
            REM_VOX_FILE_PATH,
        )
        .to_json();
    }

    match parse_vox_module(path) {
        Ok(module) => {
            let digest = vox_db::generate_schema_digest(&module, None);
            let flow = vox_db::build_data_flow(&digest);
            match serde_json::to_string_pretty(&flow) {
                Ok(json) => ToolResult::ok(
                    serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default(),
                )
                .to_json(),
                Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
                    format!("Serialization error: {}", e),
                    REM_DB_DIGEST_JSON,
                )
                .to_json(),
            }
        }
        Err(e) => {
            ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOX_FILE_PATH).to_json()
        }
    }
}

/// Return sample data for a table (async).
pub async fn vox_db_sample_data(state: &ServerState, args: serde_json::Value) -> String {
    let table = args.get("table").and_then(|v| v.as_str()).unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(3);

    if table.is_empty() {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Missing 'table' parameter.",
            "Pass `table` as the logical table name from your `.vox` schema.",
        )
        .to_json();
    }

    let db = match &state.db {
        Some(db) => db,
        None => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                "VoxDb is not connected.",
                REM_VOXDB_SAMPLE,
            )
            .to_json();
        }
    };

    let results = match db.mcp_diagnostic_sample_table(table, limit).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("DB error (sample data): {e}"),
                REM_VOXDB_SAMPLE,
            )
            .to_json();
        }
    };

    ToolResult::ok(serde_json::json!({
        "table": table,
        "sample_data": results
    }))
    .to_json()
}

/// Ordered canonical journey steps from Codex (`developer_journey_steps`).
pub async fn vox_journey_canonical_steps(state: &ServerState, args: serde_json::Value) -> String {
    let journey_id = args
        .get("journey_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("canonical_journey.v1.greenfield_vox_mens_devloop");

    let db = match &state.db {
        Some(db) => db,
        None => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                "VoxDb is not connected.",
                REM_VOXDB_SAMPLE,
            )
            .to_json();
        }
    };

    match db.list_developer_journey_steps(journey_id).await {
        Ok(steps) => ToolResult::ok(serde_json::json!({
            "journey_id": journey_id,
            "steps": steps,
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            format!("DB error (journey steps): {e}"),
            REM_VOXDB_SAMPLE,
        )
        .to_json(),
    }
}

/// Use LLM to explain a query/mutation in plain English.
pub async fn vox_db_explain_query(state: &ServerState, args: serde_json::Value) -> String {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);
    let schema_path = args
        .get("schema_path")
        .and_then(|v| v.as_str())
        .unwrap_or("src/main.vox");

    if query.is_empty() {
        return ToolResult::<String>::err_with_remediation(
            "Missing 'query' parameter.",
            REM_DB_QUERY,
        )
        .to_json();
    }

    let schema_digest = match parse_vox_module(schema_path) {
        Ok(module) => vox_db::generate_schema_digest(&module, None),
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("Failed to parse schema: {e}"),
                REM_VOX_FILE_PATH,
            )
            .to_json();
        }
    };

    let llm_context = vox_db::format_llm_context(&schema_digest);
    let prompt = format!(
        "You are an expert Vox/Rust database engineer. Explain what this query does in plain English.\n\nQuery:\n```vox\n{query}\n```\n\nSchema Context:\n{llm_context}\n\nExplanation:"
    );

    let system_prompt = format!(
        "You are an expert Vox/Rust database engineer. Explain .vox queries clearly and thoroughly.\n\n{}",
        crate::chat_tools::ANTI_LAZINESS_RIDER
    );

    let resolution_template = crate::llm_bridge::McpChatModelResolution {
        complexity: 1,
        ..Default::default()
    };

    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(e.to_string(), REM_MCP_MODEL_LOCK)
                .to_json();
        }
    };
    let (model, free_only) = match crate::chat_model_resolve::resolve_chat_llm_model(
        state,
        &prompt,
        resolution_template.clone(),
        session_id.as_deref(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("No model: {e}"),
                REM_MCP_MODEL_RESOLVE,
            )
            .to_json();
        }
    };

    let routing = crate::llm_bridge::McpInferRouting {
        user_prompt: &prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template,
        free_only,
        allow_cloud_ollama_fallback: true,
        selection_rationale: None,
        user_id: session_id.as_deref(),
    };

    match crate::llm_bridge::mcp_infer_completion(
        state,
        model,
        "vox_db_explain",
        &system_prompt,
        &routing,
        1024,
        0.3,
        None,
        None,
        false,
        None,
    )
    .await
    {
        Ok((completion, _, _)) => ToolResult::ok(completion).to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("LLM error: {e}"),
            REM_LLM_COMPLETION,
        )
        .to_json(),
    }
}

/// Use LLM to suggest the actual Vox query code given a natural language intent.
pub async fn vox_db_suggest_query(state: &ServerState, args: serde_json::Value) -> String {
    let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("");
    let schema_path = args
        .get("schema_path")
        .and_then(|v| v.as_str())
        .unwrap_or("src/main.vox");

    if intent.is_empty() {
        return ToolResult::<String>::err_with_remediation(
            "Missing 'intent' parameter.",
            REM_DB_INTENT,
        )
        .to_json();
    }

    let schema_digest = match parse_vox_module(schema_path) {
        Ok(module) => vox_db::generate_schema_digest(&module, None),
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("Failed to parse schema: {e}"),
                REM_VOX_FILE_PATH,
            )
            .to_json();
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

    super::compiler_tools::generate_vox_code(state, new_args).await
}
