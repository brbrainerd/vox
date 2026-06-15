use crate::rule_pack_detector::pack_rule;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile, rust_byte_is_comment};
use vox_rule_pack::CompiledRule;

/// Detects hardcoded secrets, API keys, and credentials.
///
/// Patterns sourced from embedded rule pack: `security/hardcoded-secret/{generic,aws-key,jwt}`.
/// Skip logic (comment lines, placeholder strings, env-var reads) kept in Rust.
pub struct SecretDetector {
    generic_secret: &'static CompiledRule,
    aws_key: &'static CompiledRule,
    jwt_token: &'static CompiledRule,
}

impl Default for SecretDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretDetector {
    pub fn new() -> Self {
        Self {
            generic_secret: pack_rule("security/hardcoded-secret/generic"),
            aws_key: pack_rule("security/hardcoded-secret/aws-key"),
            jwt_token: pack_rule("security/hardcoded-secret/jwt"),
        }
    }

    fn make_finding(
        rule: &'static CompiledRule,
        file: &SourceFile,
        line_num: usize,
        message: String,
    ) -> Finding {
        Finding {
            rule_id: rule.id.clone(),
            diagnostic_id: None,
            rule_name: rule.name.clone(),
            severity: rule.severity.into(),
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message,
            suggestion: rule.suggestion.clone(),
            alternatives: vec![],
            rationale: None,
            context: file.context_around(line_num, 1),
            confidence: rule.confidence.map(Into::into),
            evidence: None,
        }
    }

    fn check_line(
        &self,
        file: &SourceFile,
        line: &str,
        line_num: usize,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Skip whole-line comments.
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            return findings;
        }

        // Skip common test/example/placeholder patterns.
        let upper = line.to_uppercase();
        if upper.contains("EXAMPLE")
            || upper.contains("PLACEHOLDER")
            || upper.contains("YOUR_")
            || upper.contains("ENV_VAR")
            || line.contains("std::env::var")
            || line.contains("env::var")
            || line.contains("vox_secrets::resolve_secret")
            || line.contains("resolve_secret")
            || line.contains("dotenv")
            || line.contains("process.env.")
            || line.contains("os.environ")
        {
            return findings;
        }

        let skip_if_comment = |col: usize| {
            file.language == Language::Rust && rust_byte_is_comment(file, line_num, col, rust_ctx)
        };

        if let Some(m) = self.aws_key.regex().find(line) {
            let key = m.as_str();
            if !skip_if_comment(m.start()) && !Self::aws_key_is_synthetic_placeholder(key) {
                findings.push(Self::make_finding(
                    self.aws_key,
                    file,
                    line_num,
                    "Potential AWS Access Key ID detected.".to_string(),
                ));
            }
        }

        if let Some(m) = self.generic_secret.regex().find(line)
            && !skip_if_comment(m.start())
        {
            findings.push(Self::make_finding(
                self.generic_secret,
                file,
                line_num,
                "Potential hardcoded secret or API key detected.".to_string(),
            ));
        }

        if let Some(m) = self.jwt_token.regex().find(line)
            && !skip_if_comment(m.start())
        {
            findings.push(Self::make_finding(
                self.jwt_token,
                file,
                line_num,
                "Potential hardcoded JWT token detected.".to_string(),
            ));
        }

        findings
    }

    /// Test/doc keys like `AKIAZZZZZZZZZZZZZZ` are intentionally repetitive; treat as non-secret.
    fn aws_key_is_synthetic_placeholder(key: &str) -> bool {
        let Some(suffix) = key.strip_prefix("AKIA") else {
            return false;
        };
        if suffix.len() != 16 {
            return false;
        }
        let mut chars = suffix.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        suffix.chars().all(|c| c == first)
    }
}

impl DetectionRule for SecretDetector {
    fn detect(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            findings.extend(self.check_line(file, line, i + 1, rust_ctx));
        }
        findings
    }

    fn id(&self) -> &'static str {
        "security/hardcoded-secret"
    }

    fn name(&self) -> &'static str {
        "Hardcoded Secret Detector"
    }

    fn description(&self) -> &'static str {
        "Detects potential API keys, passwords, and other credentials in source code."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — hardcoded API key in source code\n\
             fn connect() {\n\
             \x20   let client = ApiClient::new(\"sk-live-abc123xyz789\");  // NEVER hardcode secrets\n\
             }\n\
             \n\
             // FIX — read from environment variable\n\
             fn connect() -> Result<ApiClient, ConfigError> {\n\
             \x20   let key = std::env::var(\"API_KEY\")?;\n\
             \x20   Ok(ApiClient::new(key))\n\
             }",
        )
    }

    fn languages(&self) -> &[Language] {
        const LANGS: &[Language] = &[
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::GDScript,
            Language::Vox,
        ];
        LANGS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", lang)), code.to_string())
    }

    #[test]
    fn detects_aws_key() {
        let d = SecretDetector::new();
        // Split so repo-wide scan of secrets.rs does not contain a contiguous AKIA+16 match.
        let rs = ["let key = \"AKIA", "1234567890ABCDEF\";"].concat();
        let f = source("rs", &rs);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("AWS"));
        assert_eq!(findings[0].rule_id, "security/hardcoded-secret/aws-key");
    }

    #[test]
    fn detects_generic_password() {
        let d = SecretDetector::new();
        // Split so the Rust source line does not match the generic-secret regex (repo-wide scan).
        let py = ["DB_PASSWORD = 'super", "-secret-pass-123'"].concat();
        let f = source("py", &py);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("hardcoded secret"));
        assert_eq!(findings[0].rule_id, "security/hardcoded-secret/generic");
    }

    #[test]
    fn ignores_example_key() {
        let d = SecretDetector::new();
        let f = source("rs", r#"let k = "AKIAIOSFODNN7EXAMPLE";"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "example key should be excluded");
    }

    #[test]
    fn ignores_uniform_synthetic_aws_key() {
        let d = SecretDetector::new();
        let f = source("rs", r#"let key = "AKIAZZZZZZZZZZZZZZ";"#);
        assert!(
            d.detect(&f, None).is_empty(),
            "uniform synthetic AWS keys are treated as fixtures"
        );
    }

    #[test]
    fn ignores_env_var_reads() {
        let d = SecretDetector::new();
        let f = source("rs", r#"let key = std::env::var("STUB_API_KEY").unwrap();"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "env var reads should not be flagged");
    }

    #[test]
    fn ignores_comment_lines() {
        let d = SecretDetector::new();
        let rs = ["// password: \"super", "-secret-123\""].concat();
        let f = source("rs", &rs);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should not be flagged");
    }

    #[test]
    fn ignores_secret_patterns_in_trailing_rust_comment() {
        let d = SecretDetector::new();
        let f = source(
            "rs",
            "fn x() {}\nlet _ = 1; // password: \"aaaaaaaa\" \"bbbbbbbb\"\n",
        );
        assert!(
            d.detect(&f, None).is_empty(),
            "credential-shaped text only in // comment should not fire"
        );
    }

    #[test]
    fn ignores_aws_key_in_block_comment_on_code_line() {
        let d = SecretDetector::new();
        // Split so a repo-wide scan of this file has no contiguous AKIA+16 match in source text.
        let body = ["fn y() { let _ = 1; /* AKIA", "1234567890ABCDEF */ }\n"].concat();
        let f = source("rs", &body);
        assert!(
            d.detect(&f, None).is_empty(),
            "AWS-shaped id in block comment should not fire"
        );
    }

    // Catches: synthetic-placeholder filter widening to swallow a real key
    // (suffix-length check dropped) or narrowing to flag legit doc keys.
    #[test]
    fn aws_synthetic_placeholder_only_for_repeated_16char_suffix() {
        // All-same 16-char suffix => synthetic (must be ignored as a secret).
        assert!(SecretDetector::aws_key_is_synthetic_placeholder(
            "AKIAZZZZZZZZZZZZZZZZ"
        ));
        // Real-looking mixed suffix => NOT synthetic.
        assert!(!SecretDetector::aws_key_is_synthetic_placeholder(
            "AKIA1234567890ABCDEF"
        ));
        // Wrong prefix => not synthetic.
        assert!(!SecretDetector::aws_key_is_synthetic_placeholder(
            "BKIAZZZZZZZZZZZZZZZZ"
        ));
        // Wrong suffix length (15 chars) => not synthetic.
        assert!(!SecretDetector::aws_key_is_synthetic_placeholder(
            "AKIAZZZZZZZZZZZZZZZ"
        ));
    }
}
