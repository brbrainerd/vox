//! Canonical MCP tool naming for ACI metadata.

/// Resolve aliases so `aci.tool` matches dispatch routing keys.
#[must_use]
pub fn tool_name_for_aci(raw: &str) -> &str {
    crate::tool_aliases::canonical_tool_name(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_matches_git_status_tool() {
        assert_eq!(tool_name_for_aci("vox_git_status"), "vox_git_status");
    }
}
