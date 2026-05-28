//! Wire-level MCP tool name aliases (same JSON args as canonical tools).
//!
//! `(alias, canonical)` pairs accepted by `super::handle_tool_call` and `super::input_schemas::tool_input_schema`.
include!(concat!(env!("OUT_DIR"), "/tool_aliases_wire.rs"));

/// Resolve an incoming tool name to the canonical handler name.
#[must_use]
pub fn canonical_tool_name(name: &str) -> &str {
    for (alias, canonical) in TOOL_WIRE_ALIASES {
        if *alias == name {
            return canonical;
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::canonical_tool_name;

    #[test]
    fn ludus_wire_alias_maps_to_gamify() {
        assert_eq!(
            canonical_tool_name("vox_ludus_progress_snapshot"),
            "vox_gamify_progress_snapshot"
        );
        assert_eq!(
            canonical_tool_name("vox_gamify_progress_snapshot"),
            "vox_gamify_progress_snapshot"
        );
    }
}
