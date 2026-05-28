use crate::features::ExtractedFeatures;
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

pub struct ReqwestBypassRule;

const ALLOWED_CRATES: &[&str] = &["vox-reqwest-defaults"];
const FORBIDDEN: &[&[&str]] = &[
    &["reqwest", "Client", "new"],
    &["reqwest", "Client", "builder"],
];

impl DriftRule for ReqwestBypassRule {
    fn id(&self) -> &'static str {
        "drift/reqwest-bypass"
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
        if is_test_file(&features.file) {
            return vec![];
        }

        features.call_sites.iter()
            .filter(|cs| {
                FORBIDDEN.iter().any(|f| {
                    cs.path.iter().map(|s| s.as_str()).eq(f.iter().copied())
                })
            })
            .map(|cs| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Reqwest Client Bypass".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: cs.loc.line,
                column: cs.loc.col,
                message: format!(
                    "Direct reqwest `{}` bypasses vox-reqwest-defaults (timeouts, UA, pooling)",
                    cs.path.join("::")
                ),
                suggestion: Some(
                    "Use `vox_http_client::client_builder()` or `vox_http_client::client()`".into(),
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

fn is_test_file(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("/tests/") || s.contains("\\tests\\") || s.ends_with("_test.rs")
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

    fn make(crate_name: &str, calls: &[&[&str]]) -> ExtractedFeatures {
        let mut f = ExtractedFeatures::new(
            PathBuf::from(format!("crates/{}/src/lib.rs", crate_name)),
            Language::Rust,
        );
        f.crate_name = Some(crate_name.to_string());
        for &path in calls {
            f.call_sites.push(CallSite {
                path: path.iter().map(|s| s.to_string()).collect(),
                arity: 0,
                loc: Loc { line: 5, col: 0 },
            });
        }
        f
    }

    #[test]
    fn flags_client_new_outside_defaults() {
        let f = make("vox-publisher", &[&["reqwest", "Client", "new"]]);
        let rule = ReqwestBypassRule;
        let findings = rule.check(&f, &ctx());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "drift/reqwest-bypass");
    }

    #[test]
    fn allows_client_new_inside_defaults_crate() {
        let f = make("vox-reqwest-defaults", &[&["reqwest", "Client", "new"]]);
        let rule = ReqwestBypassRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn flags_client_builder() {
        let f = make("vox-search", &[&["reqwest", "Client", "builder"]]);
        let rule = ReqwestBypassRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }
}
