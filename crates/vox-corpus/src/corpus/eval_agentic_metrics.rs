//! Compute agentic spoke eval metrics (JSON validity, known tool names) from model outputs.
use serde_json::Value;
use std::path::Path;

/// Returns true if the JSON value represents a valid tool call (has required keys).
pub fn is_valid_tool_call(val: &Value) -> bool {
    let Some(obj) = val.as_object() else {
        return false;
    };
    ["tool_name", "arguments", "result", "success"]
        .iter()
        .all(|k| obj.contains_key(*k))
}

/// Returns true if the tool name is in the known registry or starts with "vox ".
pub fn tool_name_exists(name: &str) -> bool {
    if name.starts_with("vox ") {
        return true;
    }
    let static_registry = vox_mcp_registry::TOOL_REGISTRY
        .iter()
        .any(|entry| entry.name == name);
    let skill_tools = vox_mcp_registry::SKILL_TOOLS.iter().any(|&t| t == name);
    let orchestrator_tools = vox_mcp_registry::ORCHESTRATOR_TOOLS
        .iter()
        .any(|&t| t == name);
    static_registry || skill_tools || orchestrator_tools
}

fn extract_json_block(s: &str) -> String {
    if let Some(start) = s.find("```json\n") {
        let content = &s[start + 8..];
        if let Some(end) = content.find("\n```") {
            return content[..end].to_string();
        }
    }
    s.to_string()
}

/// Compute agentic metrics (tool_call_valid_json_rate, tool_name_exists_rate) for
/// agentic/tool-use samples in `input_jsonl`.
pub fn compute_agentic_spoke_metrics(input_jsonl: &Path) -> anyhow::Result<(f64, f64)> {
    let content = std::fs::read_to_string(input_jsonl)?;
    let mut total_checks = 0;
    let mut valid_json_count = 0;
    let mut known_tool_count = 0;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let val: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let category = val.get("category").and_then(|c| c.as_str()).unwrap_or("");
        let lane = val.get("lane").and_then(|l| l.as_str()).unwrap_or("");

        let is_agentic = category == "agent_trace"
            || category == "tool_trace"
            || lane == "vox_dogfood_agent"
            || lane == "vox_tooling";

        if !is_agentic {
            continue;
        }

        let response = val.get("response").or_else(|| val.get("output"));
        if let Some(resp_val) = response {
            // Response could be a direct array/object or string containing JSON
            let parsed_resp: Option<Value> = if resp_val.is_string() {
                let s = resp_val.as_str().unwrap();
                let extracted = extract_json_block(s);
                serde_json::from_str(&extracted).ok()
            } else {
                Some(resp_val.clone())
            };

            if let Some(json_val) = parsed_resp {
                if let Some(arr) = json_val.as_array() {
                    for step in arr {
                        total_checks += 1;
                        if is_valid_tool_call(step) {
                            valid_json_count += 1;
                            let tool_name =
                                step.get("tool_name").and_then(|n| n.as_str()).unwrap_or("");
                            if tool_name_exists(tool_name) {
                                known_tool_count += 1;
                            }
                        }
                    }
                } else {
                    total_checks += 1;
                    if is_valid_tool_call(&json_val) {
                        valid_json_count += 1;
                        let tool_name = json_val
                            .get("tool_name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("");
                        if tool_name_exists(tool_name) {
                            known_tool_count += 1;
                        }
                    }
                }
            } else {
                // If it fails to parse as JSON but was marked agentic, we count it as a failure
                total_checks += 1;
            }
        }
    }

    let json_rate = if total_checks > 0 {
        valid_json_count as f64 / total_checks as f64
    } else {
        0.0
    };
    let name_rate = if total_checks > 0 {
        known_tool_count as f64 / total_checks as f64
    } else {
        0.0
    };

    Ok((json_rate, name_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_valid_tool_call() {
        let tc = json!({
            "tool_name": "vox_skill_list",
            "arguments": {},
            "result": "[]",
            "success": true
        });
        assert!(is_valid_tool_call(&tc));
    }

    #[test]
    fn test_invalid_tool_call() {
        let tc = json!({
            "tool_name": "vox_skill_list"
        });
        assert!(!is_valid_tool_call(&tc));
    }

    #[test]
    fn test_tool_name_exists() {
        assert!(tool_name_exists("vox_skill_list"));
        assert!(tool_name_exists("vox ci affected-crates"));
        assert!(!tool_name_exists("nonexistent_tool"));
    }
}
