use crate::features::ExtractedFeatures;
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct SerdeDefaultDupRule;

const ALLOWED_CRATES: &[&str] = &["vox-config"];
const COMMON_PREFIXES: &[&str] = &[
    "default_true",
    "default_false",
    "default_30",
    "default_60",
    "default_10",
];

impl DriftRule for SerdeDefaultDupRule {
    fn id(&self) -> &'static str {
        "drift/serde-default-dup"
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

        features.fn_definitions.iter()
            .filter(|def| COMMON_PREFIXES.iter().any(|p| def.name.starts_with(p)))
            .map(|def| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Duplicate Serde Default Function".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: def.loc.line,
                column: def.loc.col,
                message: format!(
                    "`{}` is a common serde default — consolidate into `vox_config::serde_defaults`",
                    def.name
                ),
                suggestion: Some(
                    "Move to `vox-config::serde_defaults` and import from there".into(),
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
            layers: crate::layers_manifest::LayersManifest::default(),
        }
    }

    #[test]
    fn flags_default_true_fn_outside_config() {
        let mut f = ExtractedFeatures::new(
            PathBuf::from("crates/vox-publisher/src/types.rs"),
            Language::Rust,
        );
        f.crate_name = Some("vox-publisher".into());
        f.fn_definitions.push(FnDef {
            name: "default_true".into(),
            body_hash: 99,
            sig_hash: 99,
            loc: Loc { line: 3, col: 0 },
        });
        let rule = SerdeDefaultDupRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }
}
