//! T0.3: registry-driven risk classification + `PermissionMode` auto-approve
//! matrix for the dangerous-tool HITL gate in `dispatch.rs`.
//!
//! Mirrors `contracts/orchestration/permission-modes.v1.yaml`. The
//! `RISK_CLASSES` table below is a hand-written Rust mirror of that file's
//! `risk_classes` list; `tests::risk_classes_yaml_matches_rust_table` (in
//! `tests/pending_approvals_tests.rs`) parses the YAML at test time and
//! asserts the two are in lockstep, so they cannot silently drift apart
//! (same pattern as `crates/vox-orchestrator/src/risk_matrix.rs` mirroring
//! `contracts/orchestration/risk-confidence-matrix.v1.yaml`).
//!
//! This module implements precedence tiers 2 (`permission_mode`) and 3
//! (`persisted_allowlist` — see [`crate::approval_allowlist`]) from the
//! contract's documented 5-tier precedence order. Tiers 1/4/5 are documented
//! in the contract as future/independent work; this module does not touch
//! them.

use serde::{Deserialize, Serialize};

/// Same vocabulary as `contracts/gui/action-manifest.v1.yaml`'s
/// `safety_class` enum (`read_only, mutating, destructive, unknown`). Only
/// `Mutating` / `Destructive` ever appear in [`RISK_CLASSES`] — `ReadOnly`
/// tools were never part of the dangerous-tool gate, and `Unknown` is the
/// default for anything absent from the table (never auto-approved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyClass {
    ReadOnly,
    Mutating,
    Destructive,
    Unknown,
}

/// One row of the risk-classification table: a gated tool's safety class and
/// whether its effects are reversible (e.g. via VCS).
#[derive(Debug, Clone, Copy)]
pub struct RiskClass {
    pub tool: &'static str,
    pub class: SafetyClass,
    pub reversible: bool,
}

/// Rust mirror of `contracts/orchestration/permission-modes.v1.yaml`'s
/// `risk_classes` list. Keep in lockstep with the YAML — see module docs.
pub const RISK_CLASSES: &[RiskClass] = &[
    RiskClass {
        tool: "vox_write_file",
        class: SafetyClass::Mutating,
        reversible: true,
    },
    RiskClass {
        tool: "vox_multi_replace",
        class: SafetyClass::Mutating,
        reversible: true,
    },
    RiskClass {
        tool: "vox_multi_replace_file",
        class: SafetyClass::Mutating,
        reversible: true,
    },
    RiskClass {
        tool: "vox_delete_file",
        class: SafetyClass::Destructive,
        reversible: false,
    },
    RiskClass {
        tool: "vox_run_shell",
        class: SafetyClass::Destructive,
        reversible: false,
    },
    RiskClass {
        tool: "vox_deploy",
        class: SafetyClass::Destructive,
        reversible: false,
    },
];

/// Look up a tool's risk classification by its canonical name. Returns
/// `None` for any tool absent from [`RISK_CLASSES`] — callers must treat a
/// `None` result as `unknown` / never-auto-approved (the gate's safe
/// default), NOT as "not gated" vs "gated" ambiguity.
#[must_use]
pub fn classify(tool: &str) -> Option<RiskClass> {
    RISK_CLASSES.iter().copied().find(|r| r.tool == tool)
}

/// Whether `tool` appears in the dangerous-tool risk table at all (i.e.
/// whether the HITL gate applies to it). Mirrors the pre-T0.3 hardcoded
/// `matches!` allowlist membership check.
#[must_use]
pub fn is_gated_tool(tool: &str) -> bool {
    RISK_CLASSES.iter().any(|r| r.tool == tool)
}

/// GUI-selected permission mode, threaded from `DispatchRequest::permission_mode`
/// (T0.3) through to the dispatch gate. Mirrors
/// `contracts/orchestration/permission-modes.v1.yaml`'s `modes` map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Auto-approves nothing. Baseline / fail-safe default — matches
    /// pre-T0.3 always-park behavior.
    Ask,
    /// Auto-approves `mutating` + `reversible: true` tools only.
    AcceptEdits,
    /// Auto-approves `mutating` and `destructive`, regardless of
    /// reversibility.
    AcceptAll,
    /// Read-only/planning mode. For THIS gate, behaves identically to `Ask`
    /// (auto-approves nothing) — see contract file for rationale.
    Plan,
}

impl Default for PermissionMode {
    /// Never defaults to an auto-approving mode — an absent/unrecognized
    /// mode must fail safe to today's always-park behavior.
    fn default() -> Self {
        Self::Ask
    }
}

impl PermissionMode {
    /// Parse a wire-level mode string (as carried on
    /// `DispatchRequest::permission_mode`). Any unrecognized value — `None`,
    /// empty string, typo, garbage — resolves to [`PermissionMode::Ask`]
    /// (fail-safe default), never to an auto-approving mode.
    #[must_use]
    pub fn from_wire(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("accept_edits") => Self::AcceptEdits,
            Some("accept_all") => Self::AcceptAll,
            Some("plan") => Self::Plan,
            Some("ask") | None | Some("") => Self::Ask,
            Some(_other) => Self::Ask,
        }
    }

    /// Whether this mode auto-approves a tool with the given classification,
    /// per `contracts/orchestration/permission-modes.v1.yaml`'s `modes` map.
    #[must_use]
    pub fn auto_approves(self, class: SafetyClass, reversible: bool) -> bool {
        match self {
            Self::Ask | Self::Plan => false,
            Self::AcceptEdits => class == SafetyClass::Mutating && reversible,
            Self::AcceptAll => matches!(class, SafetyClass::Mutating | SafetyClass::Destructive),
        }
    }
}

/// Tier-2 decision: would `mode` auto-approve `tool` on its own (ignoring
/// the tier-3 allowlist)? `false` for any tool not in [`RISK_CLASSES`]
/// (`unknown` is never auto-approved).
#[must_use]
pub fn mode_auto_approves(mode: PermissionMode, tool: &str) -> bool {
    match classify(tool) {
        Some(rc) => mode.auto_approves(rc.class, rc.reversible),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_mode_approves_nothing() {
        for rc in RISK_CLASSES {
            assert!(!mode_auto_approves(PermissionMode::Ask, rc.tool));
        }
    }

    #[test]
    fn plan_mode_approves_nothing() {
        for rc in RISK_CLASSES {
            assert!(!mode_auto_approves(PermissionMode::Plan, rc.tool));
        }
    }

    #[test]
    fn accept_edits_approves_mutating_reversible_only() {
        assert!(mode_auto_approves(
            PermissionMode::AcceptEdits,
            "vox_write_file"
        ));
        assert!(mode_auto_approves(
            PermissionMode::AcceptEdits,
            "vox_multi_replace"
        ));
        assert!(!mode_auto_approves(
            PermissionMode::AcceptEdits,
            "vox_delete_file"
        ));
        assert!(!mode_auto_approves(
            PermissionMode::AcceptEdits,
            "vox_run_shell"
        ));
        assert!(!mode_auto_approves(
            PermissionMode::AcceptEdits,
            "vox_deploy"
        ));
    }

    #[test]
    fn accept_all_approves_mutating_and_destructive() {
        for rc in RISK_CLASSES {
            assert!(mode_auto_approves(PermissionMode::AcceptAll, rc.tool));
        }
    }

    #[test]
    fn unknown_tool_never_auto_approved_by_any_mode() {
        for mode in [
            PermissionMode::Ask,
            PermissionMode::AcceptEdits,
            PermissionMode::AcceptAll,
            PermissionMode::Plan,
        ] {
            assert!(!mode_auto_approves(mode, "vox_totally_unclassified_tool"));
        }
    }

    #[test]
    fn from_wire_defaults_to_ask_on_absent_or_garbage() {
        assert_eq!(PermissionMode::from_wire(None), PermissionMode::Ask);
        assert_eq!(PermissionMode::from_wire(Some("")), PermissionMode::Ask);
        assert_eq!(
            PermissionMode::from_wire(Some("garbage")),
            PermissionMode::Ask
        );
        assert_eq!(PermissionMode::from_wire(Some("ask")), PermissionMode::Ask);
    }

    #[test]
    fn from_wire_parses_known_modes() {
        assert_eq!(
            PermissionMode::from_wire(Some("accept_edits")),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            PermissionMode::from_wire(Some("accept_all")),
            PermissionMode::AcceptAll
        );
        assert_eq!(
            PermissionMode::from_wire(Some("plan")),
            PermissionMode::Plan
        );
    }

    #[test]
    fn is_gated_tool_matches_risk_classes_membership() {
        assert!(is_gated_tool("vox_run_shell"));
        assert!(!is_gated_tool("vox_git_status"));
    }
}
