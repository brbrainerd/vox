//! RMCP tool descriptors built from the static MCP registry.

use super::TOOL_REGISTRY;
use super::input_schemas;
use rmcp::model::Meta;
use serde_json::Value;

/// True when `name`'s dispatch arm is compiled in under the current feature set.
///
/// `TOOL_REGISTRY` is generated unconditionally from the canonical YAML, but some
/// dispatch arms in `dispatch.rs` are `#[cfg(feature = ...)]`-gated. When a backing
/// feature is off, `handle_tool_call` returns `Unknown tool` for those names, so the
/// advertised registry (`tools/list`) and the dispatch-parity probe must both exclude
/// them to stay consistent with what can actually be dispatched.
pub(crate) fn dispatchable_under_features(name: &str) -> bool {
    let _ = name; // referenced only under some feature combinations
    #[cfg(not(feature = "news-publish"))]
    if name.starts_with("vox_news_") || name.starts_with("vox_scientia_") {
        return false;
    }
    #[cfg(not(feature = "gui-visual-review"))]
    if name == "vox_visus_audit" || name == "vox_visus_baseline" {
        return false;
    }
    #[cfg(not(feature = "heavy-browser"))]
    if name.starts_with("vox_browser_") {
        return false;
    }
    true
}

#[cfg(test)]
mod feature_gate_tests {
    use super::dispatchable_under_features;

    #[test]
    fn heavy_browser_feature_gates_browser_tools() {
        #[cfg(not(feature = "heavy-browser"))]
        {
            assert!(!dispatchable_under_features("vox_browser_open"));
            assert!(!dispatchable_under_features("vox_browser_act"));
        }
        #[cfg(feature = "heavy-browser")]
        {
            assert!(dispatchable_under_features("vox_browser_open"));
            assert!(dispatchable_under_features("vox_browser_act"));
        }
    }
}

/// Convert the static [`TOOL_REGISTRY`] table into RMCP [`rmcp::model::Tool`] descriptors.
///
/// Feature-gated tools whose dispatch arm is compiled out under the active feature
/// set are filtered out so a default build never advertises a tool it cannot dispatch.
pub fn tool_registry() -> Vec<rmcp::model::Tool> {
    TOOL_REGISTRY
        .iter()
        .filter(|e| dispatchable_under_features(e.name))
        .map(|e| {
            let n = e.name;
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "vox_product_lane".to_string(),
                Value::String(e.product_lane.to_string()),
            );
            meta_map.insert(
                "vox_http_read_role_eligible".to_string(),
                Value::Bool(e.http_read_role_eligible),
            );
            meta_map.insert("vox_tier".to_string(), Value::String(e.tier.to_string()));
            rmcp::model::Tool::new_with_raw(
                std::borrow::Cow::Owned(n.to_string()),
                Some(std::borrow::Cow::Owned(e.description.to_string())),
                std::sync::Arc::new(input_schemas::tool_input_schema(n)),
            )
            .with_meta(Meta(meta_map))
        })
        .collect()
}

/// Static registry plus federated workspace tools from `ServerState`.
pub fn merged_tool_registry(state: &crate::server_state::ServerState) -> Vec<rmcp::model::Tool> {
    let mut tool_list = tool_registry();
    let ws = state.workspace_mcp.read();
    for entry in &ws.tools {
        if tool_list.iter().any(|t| t.name == entry.name) {
            continue;
        }
        let schema = entry.input_schema.as_object().cloned().unwrap_or_default();
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(
            "vox_tier".to_string(),
            Value::String("workspace".to_string()),
        );
        tool_list.push(
            rmcp::model::Tool::new_with_raw(
                std::borrow::Cow::Owned(entry.name.clone()),
                Some(std::borrow::Cow::Owned(entry.description.clone())),
                std::sync::Arc::new(schema),
            )
            .with_meta(Meta(meta_map)),
        );
    }
    tool_list
}

#[cfg(test)]
mod merged_registry_tests {
    use super::merged_tool_registry;
    use crate::ServerState;
    use vox_orchestrator::OrchestratorConfig;

    #[tokio::test]
    async fn merged_workspace_tools_carry_workspace_tier_meta() {
        let state = ServerState::new_full(OrchestratorConfig::default());
        let tools = merged_tool_registry(&state);
        let read_file = tools.iter().find(|t| t.name == "read_file").unwrap();
        assert_eq!(read_file.meta.as_ref().unwrap().0["vox_tier"], "workspace");
    }
}
