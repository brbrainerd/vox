//! Corpus generator for the `vox_argument_generation` training lane (B1.2).
//!
//! Generates rows where the model must fill in valid JSON arguments for a tool,
//! given a task description and the tool's JSON Schema. Schema validation is
//! performed inline (draft-07 subset: required fields, type checks).

pub struct ArgGenRow {
    pub task: String,
    pub tool_name: String,
    pub tool_schema: serde_json::Value,
    pub arguments: serde_json::Value,
    pub lane: String,
}

// ─── Schema + argument templates ─────────────────────────────────────────────

/// A tool entry in our static curriculum catalog:
/// (tool_name, task_description, json_schema, example_arguments)
///
/// Schemas are ordered by complexity:
///   Tier 1: single required string field
///   Tier 2: multiple required string fields
///   Tier 3: mixed types (string + integer + optional bool)
///   Tier 4: nested object properties
///   Tier 5: optional fields alongside required
const TOOL_ENTRIES: &[(&str, &str, &str, &str)] = &[
    // ── Tier 1: single required string ──────────────────────────────────────
    (
        "vox_task_status",
        "Check the current status of a running task",
        r#"{"type":"object","properties":{"task_id":{"type":"string","description":"The task identifier"}},"required":["task_id"],"additionalProperties":false}"#,
        r#"{"task_id":"task-abc123"}"#,
    ),
    (
        "vox_cancel_task",
        "Cancel an in-progress task by its identifier",
        r#"{"type":"object","properties":{"task_id":{"type":"string","description":"The task identifier to cancel"}},"required":["task_id"],"additionalProperties":false}"#,
        r#"{"task_id":"task-xyz789"}"#,
    ),
    (
        "vox_tool_search",
        "Search the tool registry for tools matching a query string",
        r#"{"type":"object","properties":{"query":{"type":"string","description":"Search query for tool discovery"}},"required":["query"],"additionalProperties":false}"#,
        r#"{"query":"file reading tools"}"#,
    ),
    // ── Tier 2: multiple required string fields ──────────────────────────────
    (
        "vox_publish_message",
        "Publish a message to a named topic on the agent message bus",
        r#"{"type":"object","properties":{"topic":{"type":"string","description":"Message bus topic"},"payload":{"type":"string","description":"Message payload as JSON string"}},"required":["topic","payload"],"additionalProperties":false}"#,
        r#"{"topic":"agent.events","payload":"{\"event\":\"task_complete\"}"}"#,
    ),
    (
        "vox_spawn_agent",
        "Spawn a new agent with a given role and initial context",
        r#"{"type":"object","properties":{"role":{"type":"string","description":"The agent role identifier"},"context":{"type":"string","description":"Initial context or instructions for the agent"}},"required":["role","context"],"additionalProperties":false}"#,
        r#"{"role":"reviewer","context":"Review the diff and report findings"}"#,
    ),
    (
        "vox_oratio_transcribe",
        "Transcribe an audio file to text",
        r#"{"type":"object","properties":{"path":{"type":"string","description":"Path to the audio file"},"language_hint":{"type":"string","description":"BCP-47 language hint for transcription"}},"required":["path"],"additionalProperties":false}"#,
        r#"{"path":"recordings/meeting.mp3","language_hint":"en-US"}"#,
    ),
    // ── Tier 3: mixed types (string + integer) ────────────────────────────────
    (
        "vox_oratio_listen",
        "Listen to live audio input with a configurable timeout",
        r#"{"type":"object","properties":{"path":{"type":"string","description":"Output path for transcript"},"timeout_ms":{"type":"integer","minimum":1,"description":"Maximum listen duration in milliseconds"},"max_duration_ms":{"type":"integer","minimum":1,"description":"Hard cap on recording duration"}},"required":["path","timeout_ms"],"additionalProperties":false}"#,
        r#"{"path":"live-transcript.txt","timeout_ms":30000,"max_duration_ms":60000}"#,
    ),
    (
        "vox_reorder_task",
        "Change the priority of a task relative to another task",
        r#"{"type":"object","properties":{"task_id":{"type":"string","description":"Task to reorder"},"target_position":{"type":"integer","minimum":0,"description":"0-based position in the queue"},"reason":{"type":"string","description":"Human-readable reason for reordering"}},"required":["task_id","target_position"],"additionalProperties":false}"#,
        r#"{"task_id":"task-001","target_position":0,"reason":"urgent deadline"}"#,
    ),
    // ── Tier 4: nested object ─────────────────────────────────────────────────
    (
        "vox_submit_task",
        "Submit a new task to the orchestrator for background execution",
        r#"{"type":"object","properties":{"description":{"type":"string","description":"Human-readable task description"},"metadata":{"type":"object","description":"Optional metadata for the task","properties":{"priority":{"type":"string"},"tags":{"type":"string"}}},"timeout_secs":{"type":"integer","minimum":1}},"required":["description"],"additionalProperties":false}"#,
        r#"{"description":"Run tests for crate vox-corpus","metadata":{"priority":"high","tags":"ci,test"},"timeout_secs":300}"#,
    ),
    (
        "vox_doubt_task",
        "Raise a doubt about a task requiring user review before proceeding",
        r#"{"type":"object","properties":{"task_id":{"type":"string"},"question":{"type":"string","description":"The doubt or clarification needed"},"context":{"type":"object","description":"Supporting context for the doubt","properties":{"file":{"type":"string"},"line":{"type":"integer"}}}},"required":["task_id","question"],"additionalProperties":false}"#,
        r#"{"task_id":"task-055","question":"Should I overwrite the existing config?","context":{"file":"config.yaml","line":42}}"#,
    ),
    // ── Tier 5: optional fields (schema has required + optional) ─────────────
    (
        "vox_ask_clarification",
        "Ask the user a clarifying question before proceeding with a task",
        r#"{"type":"object","properties":{"question":{"type":"string","description":"The clarifying question to ask"},"options":{"type":"string","description":"Optional comma-separated answer choices"},"default":{"type":"string","description":"Default answer if user does not respond"}},"required":["question"],"additionalProperties":false}"#,
        r#"{"question":"Which output format do you prefer?","options":"json,yaml,toml","default":"json"}"#,
    ),
    (
        "vox_drain_agent",
        "Drain an agent and wait for its current work to finish",
        r#"{"type":"object","properties":{"agent_id":{"type":"string","description":"The agent identifier"},"timeout_secs":{"type":"integer","minimum":1,"description":"Maximum time to wait for drain"},"force":{"type":"boolean","description":"Force drain even if tasks are pending"}},"required":["agent_id"],"additionalProperties":false}"#,
        r#"{"agent_id":"worker-3","timeout_secs":120}"#,
    ),
    (
        "vox_resolve_feedback",
        "Resolve a pending feedback item and resume task execution",
        r#"{"type":"object","properties":{"feedback_id":{"type":"string","description":"The feedback identifier"},"resolution":{"type":"string","description":"How the feedback was resolved"},"approved":{"type":"boolean","description":"Whether the action was approved"}},"required":["feedback_id","resolution"],"additionalProperties":false}"#,
        r#"{"feedback_id":"fb-007","resolution":"Confirmed overwrite is safe","approved":true}"#,
    ),
];

// ─── Schema validator (draft-07 subset) ──────────────────────────────────────

/// Validate `value` against a JSON Schema (draft-07 subset).
///
/// Checks:
/// - All `required` properties are present in the value
/// - Present properties match their declared `type`
///   (string, integer, number, boolean, object, array)
pub fn validate_against_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
) -> anyhow::Result<()> {
    // 1. The schema must describe an object
    let props = schema.get("properties");
    let required_arr = schema.get("required").and_then(|r| r.as_array());

    // Check required fields are present
    if let Some(required) = required_arr {
        for req in required {
            let field = req.as_str().ok_or_else(|| {
                anyhow::anyhow!("required entry is not a string: {}", req)
            })?;
            if value.get(field).is_none() {
                return Err(anyhow::anyhow!(
                    "required field '{}' is missing from arguments",
                    field
                ));
            }
        }
    }

    // Check present fields match their schema types
    if let (Some(val_obj), Some(prop_map)) = (value.as_object(), props.and_then(|p| p.as_object()))
    {
        for (key, field_val) in val_obj {
            if let Some(field_schema) = prop_map.get(key) {
                if let Some(expected_type) = field_schema.get("type").and_then(|t| t.as_str()) {
                    let type_ok = match expected_type {
                        "string" => field_val.is_string(),
                        "integer" => field_val.is_i64() || field_val.is_u64(),
                        "number" => field_val.is_number(),
                        "boolean" => field_val.is_boolean(),
                        "object" => field_val.is_object(),
                        "array" => field_val.is_array(),
                        "null" => field_val.is_null(),
                        _ => true, // unknown type — pass
                    };
                    if !type_ok {
                        return Err(anyhow::anyhow!(
                            "field '{}' expected type '{}' but got {}",
                            key,
                            expected_type,
                            field_val
                        ));
                    }
                    // Recurse into nested object if the schema has sub-properties
                    if expected_type == "object" {
                        if let Some(sub_schema) = field_schema.as_object() {
                            if sub_schema.contains_key("properties") {
                                validate_against_schema(field_val, field_schema)?;
                            }
                        }
                    }
                    // Check minimum for integers
                    if let Some(min) = field_schema.get("minimum").and_then(|m| m.as_i64()) {
                        let int_val = field_val.as_i64().unwrap_or(0);
                        if int_val < min {
                            return Err(anyhow::anyhow!(
                                "field '{}' value {} is below minimum {}",
                                key,
                                int_val,
                                min
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ─── Generator ────────────────────────────────────────────────────────────────

/// Generate `n` argument-generation rows.
///
/// Rows cycle through the TOOL_ENTRIES catalog in curriculum order
/// (simpler schemas first, complex later). Each row contains:
/// - `task`: a task description
/// - `tool_name`: tool to call
/// - `tool_schema`: the JSON Schema (as Value)
/// - `arguments`: valid arguments conforming to the schema
/// - `lane`: "vox_argument_generation"
pub fn generate_argument_generation_rows(n: usize) -> Vec<ArgGenRow> {
    let catalog_len = TOOL_ENTRIES.len();
    let mut rows = Vec::with_capacity(n);

    for i in 0..n {
        // Curriculum: cycle through entries, starting from simpler (Tier 1) to complex (Tier 5)
        let entry_idx = i % catalog_len;
        let (tool_name, task_desc, schema_str, args_str) = TOOL_ENTRIES[entry_idx];

        let tool_schema: serde_json::Value =
            serde_json::from_str(schema_str).expect("static schema must parse");
        let arguments: serde_json::Value =
            serde_json::from_str(args_str).expect("static arguments must parse");

        rows.push(ArgGenRow {
            task: task_desc.to_string(),
            tool_name: tool_name.to_string(),
            tool_schema,
            arguments,
            lane: "vox_argument_generation".to_string(),
        });
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_validate_against_schema() {
        let rows = generate_argument_generation_rows(20);
        assert!(!rows.is_empty());
        for r in &rows {
            validate_against_schema(&r.arguments, &r.tool_schema)
                .expect("arguments must be schema-valid");
            assert_eq!(r.lane, "vox_argument_generation");
        }
    }

    #[test]
    fn curriculum_includes_nested_and_optional() {
        let rows = generate_argument_generation_rows(50);
        // some rows have nested or optional params
        let complex = rows
            .iter()
            .filter(|r| {
                r.tool_schema
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .map(|m| m.len() > 1)
                    .unwrap_or(false)
            })
            .count();
        assert!(complex > 0, "need rows with multi-field schemas");
    }
}
