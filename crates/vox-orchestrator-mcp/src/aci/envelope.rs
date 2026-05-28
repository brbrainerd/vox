//! Attach and validate `aci` sibling metadata on [`crate::params::ToolResult`]-shaped JSON.

use std::sync::LazyLock;

use anyhow::Context as _;
use serde_json::{Value, json};
use vox_jsonschema_util::{compile_validator, validate};
use vox_orchestrator::agentos::checkpoint_engine::should_sparse_checkpoint;
use vox_orchestrator::agentos::mutation_classifier::mutation_kind_for_tool;

static ACI_TOOL_RESPONSE_SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../../contracts/aci/agent-computer-interface.v1.schema.json"
    ))
    .expect("parse embedded agent-computer-interface.v1.schema.json")
});

static ACI_TOOL_RESPONSE_VALIDATOR: LazyLock<vox_jsonschema_util::Validator> =
    LazyLock::new(|| {
        compile_validator(&ACI_TOOL_RESPONSE_SCHEMA, "aci mcp_tool_response_v1")
            .expect("compile ACI response schema")
    });

fn aci_side_effect_tags(canon: &str) -> Vec<String> {
    let mk = mutation_kind_for_tool(canon);
    let mut tags = Vec::new();
    match mk {
        "local_mutation" => tags.push("workspace_state_change".to_string()),
        "external_side_effect" => tags.push("external_io".to_string()),
        _ => {}
    }
    if canon == "vox_run_shell" {
        tags.push("shell_exec".to_string());
    } else if canon.starts_with("vox_browser_") {
        tags.push("browser_automation".to_string());
    }
    tags.sort();
    tags.dedup();
    tags
}

fn aci_shell_backend_for_tool(canon: &str, tool_args: Option<&serde_json::Value>) -> Value {
    if canon != "vox_run_shell" {
        return Value::Null;
    }
    let Some(args) = tool_args else {
        return Value::String("powershell".into());
    };
    let backend = args
        .get("backend")
        .and_then(|v| v.as_str())
        .unwrap_or("powershell");
    let normalized = match backend.to_ascii_lowercase().as_str() {
        "nu" | "nushell" => "nushell",
        "pwsh" | "powershell" | "powershell_core" | "powershell-core" => "powershell",
        _ => "powershell",
    };
    Value::String(normalized.into())
}

fn aci_execution_probe_from_inner(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    let from_meta = obj
        .get("meta")
        .and_then(|m| m.as_object())
        .and_then(|m| m.get("execution_probe"))
        .cloned();
    if from_meta.as_ref().is_some_and(|v| v.is_object()) {
        return from_meta;
    }
    obj.get("execution_probe")
        .cloned()
        .filter(|v| v.is_object())
}

/// Parses `inner_json`, inserts `aci`, validates against the repo schema, returns compact JSON.
pub fn attach_aci_envelope(
    tool: &str,
    inner_json: &str,
    checkpoint_hints: bool,
    tool_args: Option<&serde_json::Value>,
) -> anyhow::Result<String> {
    let canon = super::normalization::tool_name_for_aci(tool);
    let mut val: Value = serde_json::from_str(inner_json)
        .with_context(|| format!("aci attach: tool {tool} output was not JSON"))?;
    let obj = val
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("aci attach: expected JSON object"))?;

    let success = obj
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let error_kind = if success {
        Value::Null
    } else {
        Value::String("tool_error".into())
    };

    let sparse = checkpoint_hints && should_sparse_checkpoint(canon);
    let side_tags = aci_side_effect_tags(canon);
    let side_effects: Vec<Value> = side_tags.into_iter().map(Value::String).collect();

    let mut aci_obj = serde_json::Map::new();
    aci_obj.insert("version".into(), json!(1));
    aci_obj.insert("tool".into(), Value::String(canon.to_string()));
    aci_obj.insert(
        "mutation_kind".into(),
        Value::String(mutation_kind_for_tool(canon).to_string()),
    );
    aci_obj.insert(
        "shell_backend".into(),
        aci_shell_backend_for_tool(canon, tool_args),
    );
    aci_obj.insert("error_kind".into(), error_kind);
    aci_obj.insert("side_effects".into(), Value::Array(side_effects));
    if let Some(probe) = aci_execution_probe_from_inner(obj) {
        aci_obj.insert("execution_probe".into(), probe);
    }
    aci_obj.insert(
        "checkpoint_hint".into(),
        json!({ "sparse_checkpoint_recommended": sparse }),
    );

    obj.insert("aci".to_string(), Value::Object(aci_obj));

    validate(
        &val,
        &ACI_TOOL_RESPONSE_VALIDATOR,
        format!("mcp tool {canon} aci envelope"),
    )?;

    Ok(serde_json::to_string(&val)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aci_round_trip_on_tool_result_shape() {
        let inner = r#"{"success":true,"data":{"x":1}}"#;
        let out = attach_aci_envelope("vox_git_status", inner, false, None).expect("attach");
        let v: Value = serde_json::from_str(&out).expect("parse");
        assert_eq!(v["success"], true);
        assert_eq!(v["aci"]["tool"], "vox_git_status");
        assert_eq!(v["aci"]["mutation_kind"], "read_only");
        assert_eq!(v["aci"]["side_effects"], json!([]));
        assert!(v["aci"]["shell_backend"].is_null());
    }

    #[test]
    fn aci_side_effects_for_local_mutation_tool() {
        let inner = r#"{"success":true,"data":{}}"#;
        let out = attach_aci_envelope("vox_write_file", inner, false, None).expect("attach");
        let v: Value = serde_json::from_str(&out).expect("parse");
        assert_eq!(v["aci"]["mutation_kind"], "local_mutation");
        let tags = v["aci"]["side_effects"]
            .as_array()
            .expect("side_effects array");
        assert!(
            tags.iter()
                .any(|t| t.as_str() == Some("workspace_state_change")),
            "{tags:?}"
        );
    }

    #[test]
    fn aci_shell_backend_for_run_shell_default_pwsh() {
        let inner = r#"{"success":true,"data":"ok"}"#;
        let out = attach_aci_envelope("vox_run_shell", inner, false, None).expect("attach");
        let v: Value = serde_json::from_str(&out).expect("parse");
        assert_eq!(v["aci"]["shell_backend"], "powershell");
        let tags = v["aci"]["side_effects"].as_array().expect("side_effects");
        assert!(tags.iter().any(|t| t.as_str() == Some("shell_exec")));
        assert!(tags.iter().any(|t| t.as_str() == Some("external_io")));
    }

    #[test]
    fn aci_shell_backend_for_run_shell_nushell_when_requested() {
        let inner = r#"{"success":true,"data":"ok"}"#;
        let args = json!({ "backend": "nu" });
        let out = attach_aci_envelope("vox_run_shell", inner, false, Some(&args)).expect("attach");
        let v: Value = serde_json::from_str(&out).expect("parse");
        assert_eq!(v["aci"]["shell_backend"], "nushell");
    }

    #[test]
    fn aci_execution_probe_from_meta_passes_through() {
        let inner = r#"{"success":true,"data":{},"meta":{"execution_probe":{"stdout":"x","stderr":"","exit_code":0}}}"#;
        let out = attach_aci_envelope("vox_git_status", inner, false, None).expect("attach");
        let v: Value = serde_json::from_str(&out).expect("parse");
        assert_eq!(v["aci"]["execution_probe"]["stdout"], "x");
    }
}
