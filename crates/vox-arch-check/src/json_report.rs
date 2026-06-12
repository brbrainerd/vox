//! `--json` projection of the arch-check `Report` into per-rule results
//! (Phase 1c policy-status overlay). Mirrors the `arch-rule` registry ids.

use serde::Serialize;

/// One per-rule outcome, JSON-serialized for the policy-status overlay.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArchRuleResult {
    /// Registry id, e.g. `arch-rule/fan_in` (matches the Plan 1b `arch-rule` ids).
    pub id: String,
    /// `pass` | `fail` | `warn`.
    pub status: String,
    /// Number of findings for this rule.
    pub count: usize,
}

/// Build a status string from (has_findings, is_strict).
pub fn status_str(has_findings: bool, strict: bool) -> &'static str {
    match (has_findings, strict) {
        (false, _) => "pass",
        (true, true) => "fail",
        (true, false) => "warn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_finding_is_fail_else_warn() {
        assert_eq!(status_str(false, true), "pass");
        assert_eq!(status_str(false, false), "pass");
        assert_eq!(status_str(true, true), "fail");
        assert_eq!(status_str(true, false), "warn");
    }

    #[test]
    fn result_serializes() {
        let r = ArchRuleResult {
            id: "arch-rule/fan_in".into(),
            status: "warn".into(),
            count: 2,
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"id\":\"arch-rule/fan_in\""));
        assert!(j.contains("\"status\":\"warn\""));
    }
}
