use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `env.get(...)` / `env::var(...)` calls whose argument looks like a secret.
pub struct EnvSecretShapeDetector {
    /// Matches env-read calls with a string-literal argument.
    env_call: Regex,
    /// Matches secret-shaped substrings in variable names (case-insensitive).
    secret_shape: Regex,
    /// Skip lines containing these patterns (false-positive reduction).
    skip_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for EnvSecretShapeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvSecretShapeDetector {
    /// Secret-shaped substrings that indicate a sensitive variable name.
    const SECRET_SUBSTRINGS: &'static [&'static str] = &[
        "KEY",
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "CREDENTIAL",
        "APIKEY",
        "API_KEY",
        "PRIVATE",
        "PASSWD",
    ];

    pub fn new() -> Self {
        let secret_alt = Self::SECRET_SUBSTRINGS.join("|");

        Self {
            // Match env.get("..."), env::var("..."), std::env::var("..."), env.get(var)
            env_call: Regex::new(
                r#"(?x)
                \b(?:std::env::var|env::var|env\.get)
                \s*\(
                \s*
                (?:
                    "(?P<dq>[^"]*)"     # double-quoted literal
                  | '(?P<sq>[^']*)'     # single-quoted literal (Vox/TS/Python)
                  | (?P<bare>[A-Za-z_][A-Za-z0-9_]*)  # bare identifier (low confidence)
                )
                "#,
            )
            .expect("valid env_call regex"),
            secret_shape: Regex::new(&format!(r"(?i)(?:{secret_alt})"))
                .expect("valid secret_shape regex"),
            skip_pattern: Regex::new(r"(?i)(EXAMPLE|PLACEHOLDER|DUMMY|FAKE|TEST)")
                .expect("valid skip_pattern regex"),
            supported_langs: vec![
                Language::Vox,
                Language::Rust,
                Language::TypeScript,
                Language::Python,
            ],
        }
    }

    /// Extract the env-var name argument from a matched call on `line`, if present.
    fn extract_arg<'a>(&self, line: &'a str) -> Option<&'a str> {
        let caps = self.env_call.captures(line)?;
        caps.name("dq")
            .or_else(|| caps.name("sq"))
            .or_else(|| caps.name("bare"))
            .map(|m| m.as_str())
    }

    fn make_finding(&self, file: &SourceFile, line_num: usize, var_name: &str) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: Some(catalog::SECRET_ENV_GET_SHAPE.to_string()),
            rule_name: self.name().to_string(),
            severity: Severity::Error,
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message: format!(
                "Direct env read of secret-shaped variable `{var_name}` detected. \
                 Use `vox_secrets.resolve(SecretId::...)` instead."
            ),
            suggestion: Some(
                "Use `vox_secrets.resolve(SecretId::YourSecret)` instead of reading secrets \
                 from environment variables directly."
                    .to_string(),
            ),
            alternatives: vec![
                "Add a SecretSpec entry in crates/vox-secrets/src/spec.rs, then call \
                 vox_secrets.resolve(SecretId::YourKey)"
                    .to_string(),
            ],
            rationale: Some(
                "Direct env reads for secret-shaped variable names bypass the Clavis secret \
                 manager (vox_secrets). This breaks telemetry, rotation, and audit logging. \
                 All secrets must route through vox_secrets::resolve_secret(...)."
                    .to_string(),
            ),
            context: file.context_around(line_num, 2),
            confidence: Some(FindingConfidence::High),
            evidence: None,
        }
    }
}

impl DetectionRule for EnvSecretShapeDetector {
    fn id(&self) -> &'static str {
        "vox/secret/env-get-shape"
    }

    fn name(&self) -> &'static str {
        "Env Secret Shape Detector"
    }

    fn description(&self) -> &'static str {
        "Detects env.get / env::var calls whose argument looks like a secret-shaped variable name."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::SECRET_ENV_GET_SHAPE)
    }

    fn explain(&self) -> &'static str {
        "Reading secrets from environment variables directly (env::var, env.get) bypasses the \
         Clavis secret manager (vox_secrets), which provides rotation, audit logging, and \
         telemetry. Any env-read whose argument name contains KEY, SECRET, TOKEN, PASSWORD, \
         CREDENTIAL, APIKEY, API_KEY, PRIVATE, or PASSWD is flagged.\n\n\
         BAD:\n  let token = std::env::var(\"OPENAI_API_KEY\").unwrap();\n\n\
         GOOD:\n  let token = vox_secrets::resolve_secret(SecretId::OpenAiApiKey)?;"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — reading secret-shaped env var directly\n\
             let api_key = env.get(\"OPENAI_API_KEY\")\n\
             \n\
             // FIX — use the Clavis secret manager\n\
             let api_key = vox_secrets::resolve_secret(SecretId::OpenAiApiKey)?",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            // Skip comment lines
            let trimmed = line.trim();
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                continue;
            }

            // Skip lines with false-positive markers
            if self.skip_pattern.is_match(line) {
                continue;
            }

            // Extract the argument name from the env call
            let Some(arg) = self.extract_arg(line) else {
                continue;
            };

            // Flag only if the argument looks like a secret
            if self.secret_shape.is_match(arg) {
                let var_name = arg.to_string();
                findings.push(self.make_finding(file, line_num, &var_name));
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{lang}")), code.to_string())
    }

    #[test]
    fn detects_env_var_with_api_key() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"let key = std::env::var("OPENAI_API_KEY").unwrap();"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect API_KEY shaped var");
        assert!(findings[0].message.contains("OPENAI_API_KEY"));
        assert_eq!(
            findings[0].diagnostic_id.as_deref(),
            Some("vox/secret/env-get-shape")
        );
    }

    #[test]
    fn detects_vox_env_get_with_token() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"let tok = env.get("STRIPE_TOKEN");"#;
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect TOKEN shaped var");
        assert!(findings[0].message.contains("STRIPE_TOKEN"));
    }

    #[test]
    fn detects_rust_env_var_with_password() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"let pass = env::var("DB_PASSWORD").expect("set");"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect PASSWORD shaped var");
    }

    #[test]
    fn ignores_example_placeholder() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"let k = std::env::var("EXAMPLE_SECRET_KEY").unwrap();"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "EXAMPLE in line should be skipped");
    }

    #[test]
    fn ignores_non_secret_env_var() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"let host = std::env::var("DATABASE_HOST").unwrap();"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "non-secret-shaped var should not fire");
    }

    #[test]
    fn ignores_comment_lines() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"// let secret = env::var("API_KEY").unwrap();"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should not be flagged");
    }

    #[test]
    fn ignores_test_prefixed_lines() {
        let d = EnvSecretShapeDetector::new();
        let code = r#"let v = env::var("FAKE_API_KEY").unwrap_or_default();"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "FAKE in line should be skipped");
    }
}
