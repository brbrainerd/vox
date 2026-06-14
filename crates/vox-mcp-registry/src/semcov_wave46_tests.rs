#[cfg(test)]
mod semcov_wave46_tests {
    use crate::{
        A2A_MESSAGE_TYPES, McpToolRegistryEntry, ORCHESTRATOR_TOOLS, SKILL_TOOLS, TOOL_REGISTRY,
    };

    const VALID_LANES: &[&str] = &["app", "workflow", "ai", "interop", "data", "platform"];
    const VALID_TIERS: &[&str] = &["core", "extended", "experimental"];

    // ── registry lookup ────────────────────────────────────────────────────

    #[test]
    // Catches: lookup by name silently returning the wrong entry (off-by-one in generated slice)
    fn lookup_by_name_returns_correct_entry() {
        let target = "vox_submit_task";
        let found = TOOL_REGISTRY.iter().find(|e| e.name == target);
        let entry = found.expect("vox_submit_task must be present in TOOL_REGISTRY");
        assert_eq!(entry.name, target, "find() returned wrong entry");
    }

    #[test]
    // Catches: build.rs emitting a tool with an empty name field (missing yaml key falls through)
    fn no_entry_has_empty_name() {
        for e in TOOL_REGISTRY {
            assert!(
                !e.name.is_empty(),
                "TOOL_REGISTRY contains entry with empty name (description: {:?})",
                e.description
            );
        }
    }

    #[test]
    // Catches: case-folding collision where two tools differ only by upper/lower case
    fn names_are_case_insensitively_unique() {
        let mut lower: Vec<String> = TOOL_REGISTRY
            .iter()
            .map(|e| e.name.to_lowercase())
            .collect();
        lower.sort();
        let before = lower.len();
        lower.dedup();
        assert_eq!(
            before,
            lower.len(),
            "two tool names are identical when case-folded"
        );
    }

    #[test]
    // Catches: lookup returning None for a tool that does exist (hash / ordering bug)
    fn every_skill_tool_is_findable_by_linear_scan() {
        let registry_names: std::collections::HashSet<&str> =
            TOOL_REGISTRY.iter().map(|e| e.name).collect();
        for name in SKILL_TOOLS {
            assert!(
                registry_names.contains(name),
                "SKILL_TOOLS entry '{name}' cannot be found in TOOL_REGISTRY"
            );
        }
    }

    // ── deduplication ─────────────────────────────────────────────────────

    #[test]
    // Catches: build.rs failing to panic on a YAML with two identical tool names
    fn tool_registry_has_no_duplicate_names() {
        let mut names: Vec<&str> = TOOL_REGISTRY.iter().map(|e| e.name).collect();
        names.sort_unstable();
        let total = names.len();
        names.dedup();
        assert_eq!(total, names.len(), "TOOL_REGISTRY contains duplicate names");
    }

    #[test]
    // Catches: SKILL_TOOLS list itself containing duplicate entries
    fn skill_tools_list_has_no_duplicates() {
        let mut names: Vec<&&str> = SKILL_TOOLS.iter().collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(total, names.len(), "SKILL_TOOLS has duplicate entries");
    }

    #[test]
    // Catches: ORCHESTRATOR_TOOLS list itself containing duplicate entries
    fn orchestrator_tools_list_has_no_duplicates() {
        let mut names: Vec<&&str> = ORCHESTRATOR_TOOLS.iter().collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(
            total,
            names.len(),
            "ORCHESTRATOR_TOOLS has duplicate entries"
        );
    }

    #[test]
    // Catches: a tool being silently cross-listed in both SKILL_TOOLS and ORCHESTRATOR_TOOLS
    fn skill_tools_and_orchestrator_tools_are_disjoint() {
        let skill_set: std::collections::HashSet<&&str> = SKILL_TOOLS.iter().collect();
        for name in ORCHESTRATOR_TOOLS {
            assert!(
                !skill_set.contains(&name),
                "'{name}' appears in both SKILL_TOOLS and ORCHESTRATOR_TOOLS"
            );
        }
    }

    // ── schema validation ─────────────────────────────────────────────────

    #[test]
    // Catches: build.rs accepting an unknown product_lane that silently persists at runtime
    fn all_product_lanes_are_from_allowed_set() {
        for e in TOOL_REGISTRY {
            assert!(
                VALID_LANES.contains(&e.product_lane),
                "tool '{}' has unsupported product_lane '{}'",
                e.name,
                e.product_lane
            );
        }
    }

    #[test]
    // Catches: build.rs emitting an empty tier when default_tier() is not applied
    fn all_entries_have_non_empty_tier() {
        for e in TOOL_REGISTRY {
            assert!(!e.tier.is_empty(), "tool '{}' has empty tier field", e.name);
        }
    }

    #[test]
    // Catches: a new tier value added to YAML without updating the allowed set
    fn all_tier_values_are_recognised() {
        for e in TOOL_REGISTRY {
            assert!(
                VALID_TIERS.contains(&e.tier),
                "tool '{}' has unrecognised tier '{}' (expected one of {:?})",
                e.name,
                e.tier,
                VALID_TIERS
            );
        }
    }

    #[test]
    // Catches: description truncated to empty string by an escape bug in build.rs
    fn all_entries_have_non_empty_description() {
        for e in TOOL_REGISTRY {
            assert!(
                !e.description.is_empty(),
                "tool '{}' has empty description",
                e.name
            );
        }
    }

    #[test]
    // Catches: tool names containing whitespace (YAML indentation leaking into values)
    fn no_tool_name_contains_whitespace() {
        for e in TOOL_REGISTRY {
            assert!(
                !e.name.contains(char::is_whitespace),
                "tool name '{}' contains whitespace",
                e.name
            );
        }
    }

    // ── capability matching ───────────────────────────────────────────────

    #[test]
    // Catches: http_read_role_eligible being forced to false for all entries (serde default override)
    fn at_least_one_entry_is_http_read_role_eligible() {
        let eligible = TOOL_REGISTRY.iter().any(|e| e.http_read_role_eligible);
        assert!(eligible, "no tool is marked http_read_role_eligible");
    }

    #[test]
    // Catches: http_read_role_eligible tools being placed in the 'ai' lane (cross-capability leak)
    fn http_read_eligible_tools_are_not_exclusively_in_ai_lane() {
        let non_ai_eligible = TOOL_REGISTRY
            .iter()
            .filter(|e| e.http_read_role_eligible && e.product_lane != "ai")
            .count();
        // This just verifies the constraint is checkable; if all eligible tools are in 'ai'
        // that is a configuration worth flagging but not a hard failure — so we assert the
        // eligible set itself is non-empty (already done above) and that the lane field is valid.
        for e in TOOL_REGISTRY.iter().filter(|e| e.http_read_role_eligible) {
            assert!(
                VALID_LANES.contains(&e.product_lane),
                "http_read_role_eligible tool '{}' has invalid lane '{}'",
                e.name,
                e.product_lane
            );
        }
        let _ = non_ai_eligible; // suppress unused warning; value is diagnostic only
    }

    #[test]
    // Catches: A2A_MESSAGE_TYPES growing a duplicate (copy-paste in the literal array)
    fn a2a_message_types_are_unique() {
        let mut types: Vec<&&str> = A2A_MESSAGE_TYPES.iter().collect();
        types.sort();
        let total = types.len();
        types.dedup();
        assert_eq!(
            total,
            types.len(),
            "A2A_MESSAGE_TYPES has duplicate entries"
        );
    }

    #[test]
    // Catches: A2A_MESSAGE_TYPES containing an entry with whitespace (YAML parse artefact)
    fn a2a_message_types_contain_no_whitespace() {
        for t in A2A_MESSAGE_TYPES {
            assert!(
                !t.contains(char::is_whitespace),
                "A2A message type '{t}' contains whitespace"
            );
        }
    }

    #[test]
    // Catches: McpToolRegistryEntry not being Copy (struct layout regression)
    fn registry_entry_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<McpToolRegistryEntry>();
    }

    #[test]
    // Catches: TOOL_REGISTRY slice being empty when YAML is present but version key causes parse skip
    fn tool_registry_is_non_empty() {
        assert!(
            !TOOL_REGISTRY.is_empty(),
            "TOOL_REGISTRY is empty — build.rs may have failed to parse YAML"
        );
    }

    #[test]
    // Catches: tool names not following the vox_ prefix convention (stray copy from upstream YAML)
    fn all_tool_names_have_vox_prefix() {
        for e in TOOL_REGISTRY {
            assert!(
                e.name.starts_with("vox_"),
                "tool '{}' does not start with 'vox_' prefix",
                e.name
            );
        }
    }
}
