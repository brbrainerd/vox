use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct BearerHeaderRule;

impl DriftRule for BearerHeaderRule {
    fn id(&self) -> &'static str {
        "drift/bearer-header-inline"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        features.string_literals.iter()
            .filter(|lit| {
                // Skip const-bound literals (the SSOT `BEARER_PREFIX` lives here)
                // and non-code contexts (doc strings).
                matches!(lit.ctx, LiteralContext::Code)
                    // drift-allow(bearer-header-inline): rule's own pattern literal
                    && lit.value.starts_with("Bearer ")
            })
            .filter(|lit| !crate::extractor::is_allowed_at(features, self.id(), lit.loc.line))
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Inline Bearer Header Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: "Inline Bearer token literal — use `vox_http_client::bearer_auth_header(token)` helper".into(),
                suggestion: Some(
                    "Use `vox_http_client::bearer_auth_header(token)` (defined in vox-http-client)".into(),
                ),
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
            layers: crate::layers_manifest::LayersManifest::default(),
        }
    }

    #[test]
    fn flags_bearer_header_literal() {
        let mut f = ExtractedFeatures::new(
            PathBuf::from("crates/vox-orchestrator-mcp/src/gateway.rs"),
            Language::Rust,
        );
        f.string_literals.push(LiteralLoc {
            // drift-allow(bearer-header-inline): rule's own test fixture
            value: "Bearer secret-token".into(),
            loc: Loc { line: 47, col: 0 },
            ctx: LiteralContext::Code,
        });
        let rule = BearerHeaderRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }
}
