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
}
