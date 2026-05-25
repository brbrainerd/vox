use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};

/// Detects `@endpoint`-decorated functions in Vox files that lack `@auth(...)` or `@public`.
pub struct AuthEndpointDetector {
    supported_langs: Vec<Language>,
}

impl Default for AuthEndpointDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthEndpointDetector {
    pub fn new() -> Self {
        Self {
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for AuthEndpointDetector {
    fn id(&self) -> &'static str {
        "auth/endpoint-missing-decorator"
    }

    fn name(&self) -> &'static str {
        "Auth Endpoint Missing Decorator"
    }

    fn description(&self) -> &'static str {
        "Detects `@endpoint` functions in Vox files that have neither `@auth(...)` nor \
        `@public` decorator — endpoints must explicitly declare their auth requirements."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::AUTH_ENDPOINT_MISSING_DECORATOR)
    }

    fn explain(&self) -> &'static str {
        "Every `@endpoint` in Vox must be explicitly annotated with either `@auth(...)` \
        (specifying the required permission) or `@public` (opt-in to unauthenticated access). \
        An endpoint without either decorator defaults to no access control, which is a security risk."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — @endpoint without @auth or @public\n\
             @endpoint\n\
             fn get_users() -> List[User] {\n\
             \x20   db.query_all()\n\
             }\n\
             \n\
             // FIX — add @auth or @public\n\
             @auth(\"users.read\")\n\
             @endpoint\n\
             fn get_users() -> List[User] {\n\
             \x20   db.query_all()\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox {
            return vec![];
        }

        let mut findings = Vec::new();
        let lines = &file.lines;
        let n = lines.len();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
                continue;
            }

            // Look for @endpoint on this line
            if !trimmed.contains("@endpoint") {
                continue;
            }

            // Look backward up to 5 lines for @auth or @public
            let look_back_start = i.saturating_sub(5);
            let window_lines = &lines[look_back_start..=i];

            let window = window_lines.join("\n");
            let has_auth = window.contains("@auth") || window.contains("@public");

            if !has_auth {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "`@endpoint` function has neither `@auth(...)` nor `@public` \
                        decorator — auth requirements are unspecified."
                        .to_string(),
                    suggestion: Some(
                        "Add `@auth(\"permission.name\")` before `@endpoint` to require a \
                        permission, or add `@public` if the endpoint is intentionally unauthenticated.".into(),
                    ),
                    alternatives: vec![
                        "Use `@public` only for truly unauthenticated endpoints (health checks, login pages).".into(),
                    ],
                    rationale: Some(
                        "Endpoints without explicit auth decorators have undefined access control, \
                        which is a security risk — every endpoint must declare its auth posture.".into(),
                    ),
                    context: file.context_around(line_num, 3),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }

            let _ = n;
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_endpoint_without_auth() {
        let d = AuthEndpointDetector::new();
        let code = "@endpoint\nfn get_users() -> List[User] {\n    db.query_all()\n}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag @endpoint with no @auth or @public"
        );
        assert!(findings[0].message.contains("@auth"));
    }

    #[test]
    fn ignores_endpoint_with_auth_decorator() {
        let d = AuthEndpointDetector::new();
        let code = "@auth(\"users.read\")\n@endpoint\nfn get_users() -> List[User] {\n    db.query_all()\n}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "@auth(...) @endpoint should not fire");
    }

    #[test]
    fn ignores_endpoint_with_public_decorator() {
        let d = AuthEndpointDetector::new();
        let code = "@public\n@endpoint\nfn health_check() -> Status {\n    Status::Ok\n}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "@public @endpoint should not fire");
    }
}
