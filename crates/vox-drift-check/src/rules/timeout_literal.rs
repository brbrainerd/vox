use crate::features::{ExtractedFeatures, UnitHint};
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct TimeoutLiteralRule;

const COMMON_TIMEOUTS_SECS: &[u64] = &[5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600];
const COMMON_TIMEOUTS_MS: &[u64] = &[100, 250, 500, 1000, 5000, 10000, 30000, 60000];

// SSOT crates: where the named constants themselves live, drift-check's own
// test fixtures, and L1 foundation crates that cannot depend on vox-config (L2).
const ALLOWED_CRATES: &[&str] = &[
    "vox-config",
    "vox-http-client",
    "vox-drift-check",
    "vox-foundation",
];

impl DriftRule for TimeoutLiteralRule {
    fn id(&self) -> &'static str {
        "drift/timeout-literal"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        let crate_name = features.crate_name.as_deref().unwrap_or("");
        if ALLOWED_CRATES.contains(&crate_name) {
            return vec![];
        }
        features.numeric_literals.iter()
            .filter(|n| match &n.unit {
                Some(UnitHint::Seconds) => COMMON_TIMEOUTS_SECS.contains(&(n.value as u64)),
                Some(UnitHint::Millis) => COMMON_TIMEOUTS_MS.contains(&(n.value as u64)),
                _ => false,
            })
            .map(|n| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Inline Timeout Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: n.loc.line,
                column: n.loc.col,
                message: format!(
                    "Inline timeout {}{} — define a named constant (e.g. `vox_config::timeouts::HTTP_REQUEST`)",
                    n.value,
                    match n.unit { Some(UnitHint::Seconds) => "s", _ => "ms" }
                ),
                suggestion: Some("Add const to `vox-config::timeouts` module".into()),
                context: String::new(),
                confidence: Some(FindingConfidence::High),
                evidence: None,
                diagnostic_id: None,
                alternatives: vec![],
                rationale: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::*;
    use crate::rules::WorkspaceContext;
    use std::path::PathBuf;
    use vox_code_audit::rules::Language;

    fn ctx() -> WorkspaceContext {
        WorkspaceContext {
            workspace_version: "0.5.0".into(),
            workspace_root: PathBuf::from("."),
        }
    }

    #[test]
    fn flags_duration_from_secs_without_const() {
        let mut f = ExtractedFeatures::new(
            PathBuf::from("crates/vox-orchestrator/src/catalog.rs"),
            Language::Rust,
        );
        f.crate_name = Some("vox-orchestrator".into());
        f.numeric_literals.push(NumericLoc {
            value: 30.0,
            unit: Some(UnitHint::Seconds),
            loc: Loc { line: 5, col: 0 },
        });
        let rule = TimeoutLiteralRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }
}
