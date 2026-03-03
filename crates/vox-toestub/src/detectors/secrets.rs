use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects hardcoded secrets, API keys, and credentials.
pub struct SecretDetector {
    generic_secret: Regex,
    aws_key: Regex,
    jwt_token: Regex,
    supported_langs: Vec<Language>,
}

impl Default for SecretDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretDetector {
    pub fn new() -> Self {
        Self {
            generic_secret: Regex::new(r#"(?i)(password|passwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token|bearer)\s*[:=]\s*["'][^"']{8,}["']"#)
                .expect("valid regex"),
            aws_key: Regex::new(r#"\bAKIA[0-9A-Z]{16}\b"#).expect("valid regex"),
            jwt_token: Regex::new(r#"\beyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+"#)
                .expect("valid regex"),
            supported_langs: vec![
                Language::Rust,
                Language::TypeScript,
                Language::Python,
                Language::GDScript,
                Language::Vox,
            ],
        }
    }

    fn make_finding(
        &self,
        file: &SourceFile,
        line_num: usize,
        message: String,
        severity: Severity,
    ) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message,
            suggestion: Some(
                "Use environment variables or a secret manager instead of hardcoding credentials."
                    .into(),
            ),
            context: file.context_around(line_num, 1),
        }
    }

    fn check_line(&self, file: &SourceFile, line: &str, line_num: usize) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Skip comments
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            return findings;
        }

        // Skip common test/example/placeholder patterns
        let upper = line.to_uppercase();
        if upper.contains("EXAMPLE")
            || upper.contains("PLACEHOLDER")
            || upper.contains("YOUR_")
            || upper.contains("ENV_VAR")
            || line.contains("std::env::var")
            || line.contains("env::var")
            || line.contains("dotenv")
            || line.contains("process.env.")
            || line.contains("os.environ")
        {
            return findings;
        }

        if self.aws_key.is_match(line) {
            findings.push(self.make_finding(
                file,
                line_num,
                "Potential AWS Access Key ID detected.".to_string(),
                Severity::Critical,
            ));
        }

        if self.generic_secret.is_match(line) {
            findings.push(self.make_finding(
                file,
                line_num,
                "Potential hardcoded secret or API key detected.".to_string(),
                Severity::Error,
            ));
        }

        if self.jwt_token.is_match(line) {
            findings.push(self.make_finding(
                file,
                line_num,
                "Potential hardcoded JWT token detected.".to_string(),
                Severity::Error,
            ));
        }

        findings
    }
}

impl DetectionRule for SecretDetector {
    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            findings.extend(self.check_line(file, line, i + 1));
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

    fn languages(&self) -> &[Language] {
        &self.supported_langs
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
        // Realistic-looking key (NOT the EXAMPLE pattern which is documentation)
        let f = source("rs", r#"let key = "AKIAZZZZZZZZZZZZZZZZ";"#);
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("AWS"));
    }

    #[test]
    fn detects_generic_password() {
        let d = SecretDetector::new();
        let f = source("py", "DB_PASSWORD = 'super-secret-pass-123'");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("hardcoded secret"));
    }

    #[test]
    fn ignores_example_key() {
        let d = SecretDetector::new();
        // The word EXAMPLE in an AWS key is a common doc pattern — skip it
        let f = source("rs", r#"let k = "AKIAIOSFODNN7EXAMPLE";"#);
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "example key should be excluded");
    }

    #[test]
    fn ignores_env_var_reads() {
        let d = SecretDetector::new();
        let f = source("rs", r#"let key = std::env::var("API_KEY").unwrap();"#);
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "env var reads should not be flagged");
    }

    #[test]
    fn ignores_comment_lines() {
        let d = SecretDetector::new();
        let f = source("rs", r#"// password: "super-secret-123""#);
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "comment lines should not be flagged");
    }
}
