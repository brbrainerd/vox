use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct VoxPathLiteralRule;

// vox-drift-check: contains the pattern strings the rule searches for as test fixtures.
const ALLOWED_CRATES: &[&str] = &["vox-config", "vox-db", "vox-drift-check"];

impl DriftRule for VoxPathLiteralRule {
    fn id(&self) -> &'static str {
        "drift/vox-path-literal"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::Vox]
    }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        let crate_name = features.crate_name.as_deref().unwrap_or("");
        if ALLOWED_CRATES.contains(&crate_name) {
            return vec![];
        }

        features
            .string_literals
            .iter()
            .filter(|lit| {
                matches!(lit.ctx, LiteralContext::Code)
                    && (lit.value.starts_with(".vox/") || lit.value.starts_with(".vox-cache"))
            })
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Raw .vox/ Path Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: format!(
                    "{:?} is a raw .vox path — use vox_config::paths::* constants",
                    lit.value
                ),
                suggestion: Some(
                    "Import from `vox_config::paths` and use the named constant".into(),
                ),
                context: format!("crate: {}", crate_name),
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
    fn flags_raw_vox_path_outside_config() {
        let mut f =
            ExtractedFeatures::new(PathBuf::from("crates/vox-cli/src/lib.rs"), Language::Rust);
        f.crate_name = Some("vox-cli".into());
        f.string_literals.push(LiteralLoc {
            value: ".vox/sessions".into(),
            loc: Loc { line: 10, col: 0 },
            ctx: LiteralContext::Code,
        });
        let rule = VoxPathLiteralRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }

    #[test]
    fn allows_raw_vox_path_inside_config_crate() {
        let mut f = ExtractedFeatures::new(
            PathBuf::from("crates/vox-config/src/paths.rs"),
            Language::Rust,
        );
        f.crate_name = Some("vox-config".into());
        f.string_literals.push(LiteralLoc {
            value: ".vox/sessions".into(),
            loc: Loc { line: 1, col: 0 },
            ctx: LiteralContext::Code,
        });
        let rule = VoxPathLiteralRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }
}
