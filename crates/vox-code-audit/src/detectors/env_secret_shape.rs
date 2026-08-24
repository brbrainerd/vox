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
                    # SCREAMING_CASE constant, and only when it is the whole argument.
                    # A general identifier arm flagged loop variables (`env::var(key)`)
                    # and even the sanctioned call shape (`SecretId`, `vox_secrets`),
                    # which is ~29 of the 110 findings this rule produced.
                  | (?P<bare>[A-Z][A-Z0-9_]*)\s*[,)]
                )
                "#,
            )
            .expect("valid env_call regex"),
            secret_shape: Regex::new(&format!(r"(?i)(?:{secret_alt})"))
                .expect("valid secret_shape regex"),
            // Applied to the extracted argument, never the raw line: matching the whole
            // line let `// latest`, `contest`, or any path containing `tests/` disable
            // the rule. `_`-delimited so `LATEST` / `CONTEST` are not `TEST`.
            skip_pattern: Regex::new(r"(?i)(?:^|_)(?:EXAMPLE|PLACEHOLDER|DUMMY|FAKE|TEST)(?:_|$)")
                .expect("valid skip_pattern regex"),
            supported_langs: vec![
                Language::Vox,
                Language::Rust,
                Language::TypeScript,
                Language::Python,
            ],
        }
    }

    /// Extract every env-var name argument on `line`, each with the byte offset where
    /// its call expression starts (used to prove it is code).
    ///
    /// `captures_iter`, not `captures`: with a single match, a fixture mention earlier
    /// on the line hid a real read later on it — the `is_code_at` guard skipped the
    /// whole line.
    fn extract_args<'a>(&self, line: &'a str) -> Vec<(&'a str, usize)> {
        self.env_call
            .captures_iter(line)
            .filter_map(|caps| {
                let call_start = caps.get(0)?.start();
                caps.name("dq")
                    .or_else(|| caps.name("sq"))
                    .or_else(|| caps.name("bare"))
                    .map(|m| (m.as_str(), call_start))
            })
            .collect()
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
                "Add a SecretSpec entry in crates/vox-secrets/src/spec/ (ids.rs + registry/), then call \
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
        rust_ctx: Option<&crate::analysis::RustFileContext>,
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

            for (arg, call_start) in self.extract_args(line) {
                // The *call* must be code. The env-var name is a literal (which is why
                // `code_only_line` cannot be used wholesale here), but `env::var(` itself
                // never is inside a real read — when it is, the line is a test fixture or
                // a doc string quoting the violation, not a violation.
                if let Some(ctx) = rust_ctx
                    && !ctx.is_code_at(&file.content, line_num, call_start)
                {
                    continue;
                }

                // Skip false-positive markers in the argument itself.
                if self.skip_pattern.is_match(arg) {
                    continue;
                }

                // Flag only if the argument looks like a secret
                if self.secret_shape.is_match(arg) {
                    findings.push(self.make_finding(file, line_num, arg));
                }
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

    // Regression: this detector used to report its own test fixtures and the
    // `explain()` doc string, which quote the violation inside a literal.
    //
    // The fixture must be a *raw* string: with the escaped-quote spelling the
    // `env_call` regex never matched at all (it hit a backslash), so the test passed
    // identically with the `is_code_at` guard deleted.
    #[test]
    fn ignores_env_call_inside_string_literal() {
        let d = EnvSecretShapeDetector::new();
        let code = "fn f() { let fixture = r#\"std::env::var(\"OPENAI_API_KEY\")\"#; }";
        let f = source("rs", code);
        // Guard against the vacuous version of this test: the regex must actually match.
        assert!(
            !d.env_call
                .captures_iter(code)
                .collect::<Vec<_>>()
                .is_empty(),
            "fixture must exercise the env_call regex, or the guard is untested"
        );
        let ctx = crate::analysis::RustFileContext::parse(code);
        assert!(
            d.detect(&f, Some(&ctx)).is_empty(),
            "env call quoted inside a literal is fixture text, not a real read"
        );
    }

    // Companion: the fix must not blunt the real rule.
    #[test]
    fn still_detects_real_env_call_with_context() {
        let d = EnvSecretShapeDetector::new();
        let code = "fn f() { let k = std::env::var(\"OPENAI_API_KEY\").unwrap(); }";
        let f = source("rs", code);
        let ctx = crate::analysis::RustFileContext::parse(code);
        let findings = d.detect(&f, Some(&ctx));
        assert_eq!(findings.len(), 1, "real env read must still fire");
        assert!(findings[0].message.contains("OPENAI_API_KEY"));
    }

    /// Regression (Bug 2): `captures` took only the first match on a line, so a
    /// fixture mention hid a real read later on the same line.
    #[test]
    fn second_env_call_on_a_line_is_not_hidden_by_the_first() {
        let d = EnvSecretShapeDetector::new();
        let code = r##"fn f() { let doc = r#"env::var("DOC_API_KEY")"#; let k = std::env::var("ANTHROPIC_API_KEY").unwrap(); }"##;
        let f = source("rs", code);
        let ctx = crate::analysis::RustFileContext::parse(code);
        let findings = d.detect(&f, Some(&ctx));
        assert_eq!(findings.len(), 1, "the real read must be reported");
        assert!(findings[0].message.contains("ANTHROPIC_API_KEY"));
    }

    /// Regression (Bug 4): the bare-identifier arm flagged loop variables, locals, and
    /// even the sanctioned `vox_secrets` / `SecretId` call shape.
    #[test]
    fn bare_lowercase_identifier_arguments_do_not_fire() {
        let d = EnvSecretShapeDetector::new();
        for code in [
            "let v = env::var(key);",
            "let v = env::var(name).ok();",
            "let v = env.get(vox_secrets);",
            "let v = env::var(SecretId);",
        ] {
            assert!(
                d.detect(&source("rs", code), None).is_empty(),
                "bare non-SCREAMING_CASE identifier should not fire: {code}"
            );
        }
        // A SCREAMING_CASE constant argument is still a real read.
        assert!(
            !d.detect(&source("rs", "let v = env::var(ANTHROPIC_API_KEY);"), None)
                .is_empty(),
            "SCREAMING_CASE constant must still fire"
        );
    }

    /// Regression (Bug 4): `skip_pattern` matched the whole raw line, so a trailing
    /// `// latest` (or any path containing `tests/`) disabled the rule for the line.
    #[test]
    fn incidental_skip_word_on_the_line_does_not_disable_the_rule() {
        let d = EnvSecretShapeDetector::new();
        for code in [
            r#"let k = std::env::var("ANTHROPIC_API_KEY").unwrap(); // latest"#,
            r#"let k = std::env::var("ANTHROPIC_API_KEY").unwrap(); // see tests/secrets.rs"#,
            r#"let k = std::env::var("ANTHROPIC_API_KEY").unwrap(); // contest winner"#,
        ] {
            assert!(
                !d.detect(&source("rs", code), None).is_empty(),
                "incidental skip word must not mute a real read: {code}"
            );
        }
        // ...while a genuinely fixture-shaped argument is still skipped.
        assert!(
            d.detect(&source("rs", r#"env::var("TEST_API_KEY")"#), None)
                .is_empty(),
            "TEST_ prefixed argument is still skipped"
        );
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
