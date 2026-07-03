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

    /// T0.3 Part A.3 hardening-scope regression guard: `canonical_tool_name`
    /// does EXACT, case-SENSITIVE matching only, and silently returns the
    /// raw input unchanged when nothing matches — it does NOT normalize
    /// case, trim whitespace, or fuzzy-match. This is a documented,
    /// intentional property this test locks in (not a bug fix): the
    /// dangerous-tool risk-classification lookup in `dispatch.rs` runs on
    /// this function's output, so a differently-cased or unregistered tool
    /// name resolves to `unknown` in `crate::permission_modes` and SKIPS the
    /// HITL gate entirely (mirrors the pre-T0.3 hardcoded `matches!` list,
    /// which was also an allowlist-of-dangerous-tools, not a denylist — an
    /// unrecognized name always fell through ungated). Broadening this to
    /// reject/normalize unrecognized names is explicitly flagged as
    /// follow-up work in T0.3's report (touches ~50+ other call sites:
    /// scope_guard, lock_guard, telemetry, ...) rather than attempted here.
    #[test]
    fn canonical_tool_name_is_case_sensitive_and_passes_through_unknown_names_unchanged() {
        // A differently-cased variant of a real dangerous tool does NOT
        // resolve to the canonical (lowercase) name — it passes through
        // unchanged, which means it will NOT be found in
        // `permission_modes::RISK_CLASSES` and will skip the gate.
        assert_eq!(canonical_tool_name("VOX_RUN_SHELL"), "VOX_RUN_SHELL");
        assert_eq!(canonical_tool_name("Vox_Run_Shell"), "Vox_Run_Shell");
        assert_ne!(canonical_tool_name("VOX_RUN_SHELL"), "vox_run_shell");

        // A genuinely unregistered/unknown name passes through unchanged
        // (no panic, no substitution, no case coercion).
        assert_eq!(
            canonical_tool_name("totally_unregistered_tool_xyz"),
            "totally_unregistered_tool_xyz"
        );

        // The real canonical name (already correctly cased) is unaffected.
        assert_eq!(canonical_tool_name("vox_run_shell"), "vox_run_shell");
    }
}
