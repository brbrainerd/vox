//! `vox_tool_search` — progressive tool disclosure.
//!
//! Mirrors Claude Code's MCP tool search: instead of pre-loading every tool's full
//! schema into the model's context, the model issues a keyword search and gets back
//! only the matching tools (name + description + input schema) on demand. This keeps
//! context usage low as the tool surface (500+ tools) grows.
//!
//! The ranking ([`rank_tools`]) is a pure keyword match over [`TOOL_REGISTRY`]; the
//! handler ([`vox_tool_search`]) attaches each hit's input schema from
//! [`crate::input_schemas`]. Tools remain dispatchable by name regardless of whether
//! they were surfaced here — discovery and execution are independent.

use vox_mcp_registry::{McpToolRegistryEntry, TOOL_REGISTRY};

/// Per-term score when the term exactly matches an underscore-separated name segment.
const SCORE_NAME_SEGMENT_EXACT: u32 = 8;
/// Per-term score when the term is a substring of the tool name.
const SCORE_NAME_SUBSTRING: u32 = 4;
/// Per-term score when the term appears in the tool description.
const SCORE_DESCRIPTION: u32 = 1;

/// Rank registry tools against a whitespace-separated keyword `query`.
///
/// Each lowercased term scores per tool: exact name-segment match
/// ([`SCORE_NAME_SEGMENT_EXACT`]) > name substring ([`SCORE_NAME_SUBSTRING`]) >
/// description hit ([`SCORE_DESCRIPTION`]); term scores sum. Tools with score 0
/// are dropped; ties break by name ascending; at most `limit` entries return.
pub fn rank_tools(query: &str, limit: usize) -> Vec<&'static McpToolRegistryEntry> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(u32, &'static McpToolRegistryEntry)> = TOOL_REGISTRY
        .iter()
        .filter_map(|entry| {
            let name = entry.name.to_lowercase();
            let description = entry.description.to_lowercase();
            let score: u32 = terms
                .iter()
                .map(|term| {
                    if name.split('_').any(|seg| seg == term) {
                        SCORE_NAME_SEGMENT_EXACT
                    } else if name.contains(term.as_str()) {
                        SCORE_NAME_SUBSTRING
                    } else if description.contains(term.as_str()) {
                        SCORE_DESCRIPTION
                    } else {
                        0
                    }
                })
                .sum();
            (score > 0).then_some((score, entry))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(b.1.name)));
    scored.truncate(limit);
    scored.into_iter().map(|(_, entry)| entry).collect()
}

/// `vox_tool_search` handler: keyword search over the tool registry, returning
/// each hit's name, description, and input schema.
pub fn vox_tool_search(params: crate::params::ToolSearchParams) -> String {
    let limit = params.limit.unwrap_or(10).clamp(1, 100) as usize;
    let hits = rank_tools(&params.query, limit);
    let tools: Vec<serde_json::Value> = hits
        .iter()
        .map(|entry| {
            serde_json::json!({
                "name": entry.name,
                "description": entry.description,
                "input_schema": crate::input_schemas::tool_input_schema(entry.name),
            })
        })
        .collect();
    crate::params::ToolResult::ok(serde_json::json!({
        "query": params.query,
        "total": tools.len(),
        "tools": tools,
    }))
    .to_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_exact_name_terms_highest() {
        let hits = rank_tools("git status", 10);
        assert!(
            hits.first().map(|e| e.name) == Some("vox_git_status"),
            "expected vox_git_status first, got: {:?}",
            hits.first().map(|e| e.name)
        );
    }

    #[test]
    fn finds_tools_by_description_keyword() {
        let hits = rank_tools("memory", 50);
        assert!(
            hits.iter().any(|e| e.name == "vox_memory_store"),
            "expected a memory tool in results"
        );
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(rank_tools("", 10).is_empty());
        assert!(rank_tools("   ", 10).is_empty());
    }

    #[test]
    fn gibberish_returns_nothing() {
        assert!(rank_tools("zzqqxnotarealtoolword", 10).is_empty());
    }

    #[test]
    fn respects_limit() {
        let hits = rank_tools("vox", 3);
        assert!(hits.len() <= 3);
    }

    #[test]
    fn results_are_deterministic_by_score_then_name() {
        let a = rank_tools("git", 10);
        let b = rank_tools("git", 10);
        let names_a: Vec<_> = a.iter().map(|e| e.name).collect();
        let names_b: Vec<_> = b.iter().map(|e| e.name).collect();
        assert_eq!(names_a, names_b);
    }

    #[test]
    fn handler_returns_hits_with_schemas() {
        let out = vox_tool_search(crate::params::ToolSearchParams {
            query: "git status".to_string(),
            limit: Some(5),
        });
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["success"], true);
        let data = &v["data"];
        assert_eq!(data["query"], "git status");
        let tools = data["tools"].as_array().expect("tools array");
        assert_eq!(data["total"], tools.len() as u64);
        assert!(tools.len() <= 5);
        let first = &tools[0];
        assert_eq!(first["name"], "vox_git_status");
        assert!(first["description"].is_string());
        assert!(first["input_schema"].is_object());
    }

    #[test]
    fn handler_defaults_limit_and_handles_no_hits() {
        let out = vox_tool_search(crate::params::ToolSearchParams {
            query: "zzqqxnotarealtoolword".to_string(),
            limit: None,
        });
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["success"], true);
        assert_eq!(v["data"]["total"], 0);
        assert!(v["data"]["tools"].as_array().expect("array").is_empty());
    }
}
