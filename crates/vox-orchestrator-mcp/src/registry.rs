//! RMCP tool descriptors built from the static MCP registry.

use super::TOOL_REGISTRY;
use super::input_schemas;
use rmcp::model::Meta;
use serde_json::Value;

/// Convert the static [`TOOL_REGISTRY`] table into RMCP [`rmcp::model::Tool`] descriptors.
pub fn tool_registry() -> Vec<rmcp::model::Tool> {
    TOOL_REGISTRY
        .iter()
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
