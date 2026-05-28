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
                matches!(lit.ctx, LiteralContext::Code)
                    && lit.value.starts_with("Bearer ")
            })
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Inline Bearer Header Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: "Inline Bearer token literal — use `vox_http_client::bearer_auth_header(token)` helper".into(),
                suggestion: Some(
                    "Add `bearer_auth_header(token: &str) -> HeaderValue` to vox-reqwest-defaults".into(),
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
        }
    }

    #[test]
    fn flags_bearer_header_literal() {
        let mut f = ExtractedFeatures::new(
            PathBuf::from("crates/vox-orchestrator-mcp/src/gateway.rs"),
            Language::Rust,
        );
        f.string_literals.push(LiteralLoc {
            value: "Bearer secret-token".into(),
            loc: Loc { line: 47, col: 0 },
            ctx: LiteralContext::Code,
        });
        let rule = BearerHeaderRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }
}
