use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct VoxPathLiteralRule;

// No crate-level allowlist. SSOT path literals across the workspace are inside
// `pub const` items, so the extractor's `LiteralContext::ConstDecl` tagging
// (filtered below) makes them invisible to this rule — no per-crate exception
// needed. Per-file legitimate one-offs use `// drift-allow(vox-path-literal)`.
const ALLOWED_CRATES: &[&str] = &[];

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
                // ConstDecl literals are the canonical path constants — that's the goal.
                matches!(lit.ctx, LiteralContext::Code)
                    // drift-allow(vox-path-literal): rule's own pattern literals
                    && (lit.value.starts_with(".vox/") || lit.value.starts_with(".vox-cache"))
            })
            .filter(|lit| !crate::extractor::is_allowed_at(features, self.id(), lit.loc.line))
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
            layers: crate::layers_manifest::LayersManifest::default(),
        }
    }

    #[test]
    fn flags_raw_vox_path_outside_config() {
        let mut f =
            ExtractedFeatures::new(PathBuf::from("crates/vox-cli/src/lib.rs"), Language::Rust);
        f.crate_name = Some("vox-cli".into());
        f.string_literals.push(LiteralLoc {
            // drift-allow(vox-path-literal): rule's own test fixture
            value: ".vox/sessions".into(),
            loc: Loc { line: 10, col: 0 },
            ctx: LiteralContext::Code,
        });
        let rule = VoxPathLiteralRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }

    #[test]
    fn allows_raw_vox_path_in_const_decl() {
        // The SSOT crate (vox-config) defines path strings as `pub const`s;
        // the extractor tags those `LiteralContext::ConstDecl`, which is now
        // the only mechanism that exempts them (no more crate-level allowlist
        // for vox-config — the const-context test is what's load-bearing).
        let mut f = ExtractedFeatures::new(
            PathBuf::from("crates/vox-config/src/paths.rs"),
            Language::Rust,
        );
        f.crate_name = Some("vox-config".into());
        f.string_literals.push(LiteralLoc {
            // drift-allow(vox-path-literal): rule's own test fixture
            value: ".vox/sessions".into(),
            loc: Loc { line: 1, col: 0 },
            ctx: LiteralContext::ConstDecl,
        });
        let rule = VoxPathLiteralRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn allows_raw_vox_path_via_per_line_annotation() {
        let mut f =
            ExtractedFeatures::new(PathBuf::from("crates/vox-cli/src/lib.rs"), Language::Rust);
        f.crate_name = Some("vox-cli".into());
        f.string_literals.push(LiteralLoc {
            // drift-allow(vox-path-literal): rule's own test fixture
            value: ".vox/sessions".into(),
            loc: Loc { line: 42, col: 0 },
            ctx: LiteralContext::Code,
        });
        let mut allowed = std::collections::HashSet::new();
        allowed.insert(42);
        f.allowed_lines.insert("vox-path-literal".to_string(), allowed);
        let rule = VoxPathLiteralRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }
}
