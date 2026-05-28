//! Compile-time MCP registry constants (`vox-mcp-registry`).

use vox_mcp_registry::{
    A2A_MESSAGE_TYPES, McpToolRegistryEntry, ORCHESTRATOR_TOOLS, SKILL_TOOLS, TOOL_REGISTRY,
};

#[test]
fn tool_registry_non_empty_and_unique_names() {
    assert!(
        !TOOL_REGISTRY.is_empty(),
        "TOOL_REGISTRY must list MCP tools"
    );

    let names: Vec<&str> = TOOL_REGISTRY.iter().map(|e| e.name).collect();
    let uniq: std::collections::HashSet<&str> = names.iter().copied().collect();
    assert_eq!(
        uniq.len(),
        names.len(),
        "duplicate tool names leaked into TOOL_REGISTRY"
    );
}

#[test]
fn sample_entry_has_expected_shape() {
    let row: Option<&McpToolRegistryEntry> =
        TOOL_REGISTRY.iter().find(|e| e.name == "vox_submit_task");
    let entry = row.expect("canonical registry should include `vox_submit_task`");
    assert!(!entry.description.is_empty());
    assert!(!entry.product_lane.is_empty());
    assert!(!entry.tier.is_empty());
}

#[test]
fn skill_and_orchestrator_tool_lists_are_subsets_of_registry() {
    let names: std::collections::HashSet<&str> = TOOL_REGISTRY.iter().map(|e| e.name).collect();
    for name in SKILL_TOOLS {
        assert!(
            names.contains(name),
            "SKILL_TOOLS entry {name} missing from TOOL_REGISTRY"
        );
    }
    for name in ORCHESTRATOR_TOOLS {
        assert!(
            names.contains(name),
            "ORCHESTRATOR_TOOLS entry {name} missing from TOOL_REGISTRY"
        );
    }
}

#[test]
fn a2a_message_types_non_empty() {
    assert!(A2A_MESSAGE_TYPES.contains(&"plan_handoff"));
}

#[test]
fn http_read_role_eligible_entries_are_present_and_named() {
    let eligible: Vec<&McpToolRegistryEntry> = TOOL_REGISTRY
        .iter()
        .filter(|e| e.http_read_role_eligible)
        .collect();
    assert!(
        !eligible.is_empty(),
        "expected at least one http_read_role_eligible tool in TOOL_REGISTRY"
    );
    for e in eligible {
        assert!(!e.name.is_empty(), "eligible tool missing name");
        assert!(
            !e.description.is_empty(),
            "eligible tool {} missing description",
            e.name
        );
    }
}

#[test]
fn tool_registry_product_lanes_match_build_contract() {
    const LANES: &[&str] = &["app", "workflow", "ai", "interop", "data", "platform"];
    for e in TOOL_REGISTRY {
        assert!(
            LANES.contains(&e.product_lane),
            "tool `{}` has unexpected product_lane `{}`",
            e.name,
            e.product_lane
        );
    }
}
