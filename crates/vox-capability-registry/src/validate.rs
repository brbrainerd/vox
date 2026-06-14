//! Cross-registry consistency checks (MCP tools, CLI operations, curated rows).

use std::collections::HashSet;

use crate::document::CapabilityRegistryDoc;
use crate::ids::{implicit_cli_capability_id, implicit_mcp_capability_id};

/// Validate curated rows against MCP and CLI registries. Returns human-readable errors.
pub fn validate_cross_registry(
    doc: &CapabilityRegistryDoc,
    mcp_tools: &[String],
    cli_paths_active: &[Vec<String>],
) -> Vec<String> {
    let mut errs = Vec::new();
    let mcp_set: HashSet<&str> = mcp_tools.iter().map(String::as_str).collect();
    let cli_set: HashSet<Vec<String>> = cli_paths_active.iter().cloned().collect();

    let mut seen_ids: HashSet<String> = HashSet::new();
    for row in &doc.curated {
        if !seen_ids.insert(row.id.clone()) {
            errs.push(format!("duplicate curated capability id: {}", row.id));
        }
        if let Some(ref tool) = row.mcp_tool {
            if !mcp_set.contains(tool.as_str()) {
                errs.push(format!(
                    "curated capability '{}' references unknown MCP tool '{}'",
                    row.id, tool
                ));
            }
            let expected = implicit_mcp_capability_id(tool);
            if row.id != expected {
                errs.push(format!(
                    "curated id '{}' must equal implicit MCP id '{}' (mcp_tool={})",
                    row.id, expected, tool
                ));
            }
        }
        if let Some(ref path) = row.cli_path {
            if !cli_set.contains(path) {
                errs.push(format!(
                    "curated capability '{}' references unknown CLI path {:?}",
                    row.id, path
                ));
            }
            let expected = implicit_cli_capability_id(path);
            if row.id != expected {
                errs.push(format!(
                    "curated id '{}' must equal implicit CLI id '{}' (cli_path={:?})",
                    row.id, expected, path
                ));
            }
        }
    }

    let mut seen_rt: HashSet<(String, String)> = HashSet::new();
    for m in &doc.runtime_builtin_maps {
        let key = (m.namespace.clone(), m.method.clone());
        if !seen_rt.insert(key) {
            errs.push(format!(
                "duplicate runtime_builtin_maps entry: {}.{}",
                m.namespace, m.method
            ));
        }
    }

    if doc.auto_mcp_capabilities {
        // Implicit ids cover all MCP tools; nothing else required.
    } else {
        let mut covered: HashSet<&str> = HashSet::new();
        for row in &doc.curated {
            if let Some(ref t) = row.mcp_tool {
                covered.insert(t.as_str());
            }
        }
        for t in mcp_tools {
            if !covered.contains(t.as_str()) {
                errs.push(format!(
                    "auto_mcp_capabilities=false but MCP tool '{t}' has no curated row with mcp_tool"
                ));
            }
        }
    }

    errs
}

#[cfg(test)]
mod semcov_wave9_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use crate::document::{CapabilityRegistryDoc, CuratedCapability};

    fn minimal_doc() -> CapabilityRegistryDoc {
        CapabilityRegistryDoc {
            schema_version: 1,
            auto_mcp_capabilities: true,
            auto_cli_capabilities: false,
            curated: vec![],
            runtime_builtin_maps: vec![],
            exemptions: None,
        }
    }

    fn make_curated(id: &str, mcp_tool: Option<&str>) -> CuratedCapability {
        CuratedCapability {
            id: id.to_string(),
            title: None,
            description_human: None,
            description_model: None,
            intent_tags: vec![],
            side_effect_class: None,
            scope_kind: None,
            reversible: None,
            requires_repo: None,
            requires_git: None,
            preferred_for_models: None,
            human_takeover_friendly: None,
            mens_planner_visible: None,
            mcp_tool: mcp_tool.map(str::to_string),
            cli_path: None,
            parameters: None,
        }
    }

    // Catches: validate_cross_registry not detecting duplicate curated capability ids,
    // allowing two rows with the same id to silently coexist and overwrite each other
    // in downstream maps.
    #[test]
    fn duplicate_curated_id_produces_error() {
        let mut doc = minimal_doc();
        doc.curated = vec![
            make_curated("mcp.my_tool", Some("my_tool")),
            make_curated("mcp.my_tool", Some("my_tool")), // duplicate
        ];
        let errs = validate_cross_registry(&doc, &["my_tool".to_string()], &[]);
        assert!(
            errs.iter().any(|e| e.contains("duplicate")),
            "must report duplicate id, got: {errs:?}"
        );
    }

    // Catches: validate_cross_registry not detecting a curated row that references
    // an MCP tool not in the active tool list, allowing stale/ghost curated entries.
    #[test]
    fn unknown_mcp_tool_reference_produces_error() {
        let mut doc = minimal_doc();
        doc.auto_mcp_capabilities = false;
        doc.curated = vec![make_curated("mcp.ghost_tool", Some("ghost_tool"))];
        let errs = validate_cross_registry(&doc, &[], &[]); // ghost_tool not in mcp_tools
        assert!(
            errs.iter().any(|e| e.contains("ghost_tool")),
            "unknown MCP tool must be flagged, got: {errs:?}"
        );
    }

    // Catches: validate_cross_registry accepting a curated id that doesn't match
    // the implicit MCP id format (e.g. "oratio.transcribe" instead of "mcp.vox_oratio_transcribe"),
    // allowing drift between id conventions.
    #[test]
    fn mismatched_id_vs_implicit_mcp_id_produces_error() {
        let mut doc = minimal_doc();
        doc.auto_mcp_capabilities = false;
        // id is "wrong.id" but mcp_tool is "my_tool" → implicit would be "mcp.my_tool"
        doc.curated = vec![make_curated("wrong.id", Some("my_tool"))];
        let errs = validate_cross_registry(&doc, &["my_tool".to_string()], &[]);
        assert!(
            errs.iter().any(|e| e.contains("wrong.id") || e.contains("mcp.my_tool")),
            "id mismatch must be reported, got: {errs:?}"
        );
    }

    // Catches: validate_cross_registry returning errors for uncovered MCP tools
    // when auto_mcp_capabilities=false, then silently passing when the flag is true —
    // verifying the auto flag is correctly respected.
    #[test]
    fn auto_mcp_capabilities_true_does_not_require_curated_rows() {
        let mut doc = minimal_doc();
        doc.auto_mcp_capabilities = true;
        // No curated rows, but auto_mcp_capabilities=true means all tools are auto-covered
        let errs = validate_cross_registry(
            &doc,
            &["tool_a".to_string(), "tool_b".to_string()],
            &[],
        );
        let coverage_errs: Vec<_> = errs
            .iter()
            .filter(|e| e.contains("no curated row"))
            .collect();
        assert!(
            coverage_errs.is_empty(),
            "auto_mcp_capabilities=true must not require curated rows, got: {coverage_errs:?}"
        );
    }
}
