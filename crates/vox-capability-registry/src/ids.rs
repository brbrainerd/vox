//! Deterministic capability id helpers (implicit MCP / CLI surfaces).

/// Implicit capability id for an MCP tool name from `tool-registry.canonical.yaml`.
#[must_use]
pub fn implicit_mcp_capability_id(mcp_tool: &str) -> String {
    format!("mcp.{mcp_tool}")
}

/// Implicit capability id for a `vox-cli` command path from `command-registry.yaml`.
#[must_use]
pub fn implicit_cli_capability_id(cli_path: &[String]) -> String {
    format!("cli.{}", cli_path.join("."))
}

#[cfg(test)]
mod semcov_wave9_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: implicit_mcp_capability_id producing a wrong prefix (e.g. "tool."
    // instead of "mcp."), breaking capability lookups for all MCP tools.
    #[test]
    fn mcp_id_has_mcp_prefix() {
        let id = implicit_mcp_capability_id("vox_oratio_transcribe");
        assert!(
            id.starts_with("mcp."),
            "mcp id must start with 'mcp.', got: {id}"
        );
        assert_eq!(id, "mcp.vox_oratio_transcribe");
    }

    // Catches: implicit_mcp_capability_id handling empty tool name by returning
    // "mcp." (dangling dot), which would silently collide in lookup maps.
    #[test]
    fn mcp_id_empty_tool_produces_bare_prefix() {
        let id = implicit_mcp_capability_id("");
        // Must not panic; result is "mcp." — document the actual behavior so
        // callers know empty tool names are not filtered here.
        assert_eq!(
            id, "mcp.",
            "empty tool name yields 'mcp.' (caller must validate)"
        );
    }

    // Catches: implicit_cli_capability_id using a separator other than '.' (e.g.
    // ' ' or '/' or '::'), breaking id-based lookups across the codebase.
    #[test]
    fn cli_id_joins_path_with_dot() {
        let path = vec!["vox".to_string(), "run".to_string(), "scripts".to_string()];
        let id = implicit_cli_capability_id(&path);
        assert_eq!(id, "cli.vox.run.scripts");
    }

    // Catches: implicit_cli_capability_id panicking or returning "cli." on a
    // single-element path, when single-segment commands are valid.
    #[test]
    fn cli_id_single_segment_path() {
        let path = vec!["build".to_string()];
        let id = implicit_cli_capability_id(&path);
        assert_eq!(
            id, "cli.build",
            "single-segment CLI path must produce 'cli.build'"
        );
    }

    // Catches: implicit_cli_capability_id producing a trailing dot for an empty
    // path slice, colliding with other identifiers.
    #[test]
    fn cli_id_empty_path_produces_bare_prefix() {
        let id = implicit_cli_capability_id(&[]);
        // Must not panic; behavior is "cli." — callers must validate.
        assert_eq!(
            id, "cli.",
            "empty CLI path yields 'cli.' (caller must validate)"
        );
    }
}
