use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct VersionStringRule;

impl DriftRule for VersionStringRule {
    fn id(&self) -> &'static str {
        "drift/version-string"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::Vox]
    }

    fn check(&self, features: &ExtractedFeatures, ctx: &WorkspaceContext) -> Vec<Finding> {
        if ctx.workspace_version.is_empty() {
            return vec![];
        }
        if features.file.file_name().is_some_and(|n| n == "Cargo.toml") {
            return vec![];
        }

        features
            .string_literals
            .iter()
            .filter(|lit| {
                matches!(lit.ctx, LiteralContext::Code) && lit.value == ctx.workspace_version
            })
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Hardcoded Version String".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: format!(
                    "Hardcoded version {:?} — use env!(\"CARGO_PKG_VERSION\") instead",
                    lit.value
                ),
                suggestion: Some("Replace with `env!(\"CARGO_PKG_VERSION\")`".into()),
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

    #[test]
    fn flags_hardcoded_version_string() {
        let ctx = WorkspaceContext {
            workspace_version: "0.5.0".into(),
            workspace_root: PathBuf::from("."),
        };
        let mut f =
            ExtractedFeatures::new(PathBuf::from("crates/vox-cli/tests/foo.rs"), Language::Rust);
        f.string_literals.push(LiteralLoc {
            value: "0.5.0".into(),
            loc: Loc { line: 78, col: 0 },
            ctx: LiteralContext::Code,
        });
        let rule = VersionStringRule;
        assert_eq!(rule.check(&f, &ctx).len(), 1);
    }
}
