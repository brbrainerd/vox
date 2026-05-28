use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects secret-shaped field names appearing in tracing span attributes or log calls.
pub struct SecretSpanDetector {
    /// Matches tracing macros with a field assignment: `tracing::info!(field = val, ...)`
    tracing_field: Regex,
    /// Matches `span.record("field_name", ...)` calls
    span_record: Regex,
    supported_langs: Vec<Language>,
}

/// Secret-shaped field names (case-insensitive).
const SECRET_NAMES: &[&str] = &[
    "password",
    "passwd",
    "token",
    "secret",
    "api_key",
    "apikey",
    "credential",
    "private_key",
    "auth_token",
];

impl Default for SecretSpanDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretSpanDetector {
    pub fn new() -> Self {
        Self {
            // Matches `tracing::<macro>!(` or just `info!(`, `debug!(`, etc. (the tracing macros)
            tracing_field: Regex::new(
                r"(?:tracing\s*::\s*)?(?:trace|debug|info|warn|error|event)\s*!\s*\(",
            )
            .expect("valid regex"),
            // Matches `span.record("field_name",` or `.record("field_name",`
            span_record: Regex::new(r#"\.record\s*\(\s*"([^"]+)""#).expect("valid regex"),
            supported_langs: vec![Language::Rust, Language::TypeScript],
        }
    }

    /// Returns Some(secret_name) if the line contains a secret-shaped field name in a tracing context.
    fn find_secret_field_in_tracing_call<'a>(&self, line: &'a str) -> Option<&'a str> {
        if !self.tracing_field.is_match(line) {
            return None;
        }
        // Look for `field_name = ` patterns in the macro arguments
        // We search for `word =` tokens and check if the word is secret-shaped
        let lower = line.to_ascii_lowercase();
        for secret in SECRET_NAMES {
            // Match `, secret_name =` or `( secret_name =` style patterns
            if lower.contains(&format!("{secret} ="))
                || lower.contains(&format!("{secret}="))
                || lower.contains(&format!(",{secret} "))
                || lower.contains(&format!(", {secret} "))
            {
                // Find the actual position in the original line
                return Some(secret);
            }
        }
        None
    }
}

impl DetectionRule for SecretSpanDetector {
    fn id(&self) -> &'static str {
        "secret/leaked-to-span"
    }

    fn name(&self) -> &'static str {
        "Secret Leaked to Span Detector"
    }

    fn description(&self) -> &'static str {
        "Detects secret-shaped field names (password, token, api_key, …) appearing as \
        tracing span attributes or log call arguments."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::SECRET_LEAKED_TO_SPAN)
    }

    fn explain(&self) -> &'static str {
        "Logging secret-shaped values (passwords, tokens, API keys) via tracing spans or \
        log macros can leak credentials into structured logs, log aggregators, or traces. \
        Redact secrets before logging, or avoid logging them entirely."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — token value logged directly to a tracing span\n\
             tracing::info!(user = %user_id, token = %auth_token, \"request processed\");\n\
             \n\
             // FIX — omit the secret field entirely, or use a redacted placeholder\n\
             tracing::info!(user = %user_id, \"request processed\");  // omit token\n\
             // or: tracing::info!(user = %user_id, token = \"[REDACTED]\", \"request processed\");",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !matches!(file.language, Language::Rust | Language::TypeScript) {
            return vec![];
        }

        let mut findings = Vec::new();
        let lines = &file.lines;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                continue;
            }

            // Check tracing macro calls with secret-shaped field names
            if let Some(secret_name) = self.find_secret_field_in_tracing_call(line) {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: format!(
                        "Secret-shaped field `{secret_name}` passed to a tracing/log macro — \
                        this may leak credentials into logs or trace aggregators."
                    ),
                    suggestion: Some(
                        "Redact or omit secret-shaped values before logging. \
                        Use a placeholder like `[REDACTED]` or log only the presence/absence."
                            .into(),
                    ),
                    alternatives: vec![
                        "Use a `tracing::field::Empty` sentinel to exclude the field from spans."
                            .into(),
                    ],
                    rationale: Some(
                        "Logging secret-shaped values can leak credentials into structured logs, \
                        log aggregators, or distributed traces accessible to operators."
                            .into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
                continue;
            }

            // Check `span.record("secret_name", ...)` calls
            if let Some(caps) = self.span_record.captures(line)
                && let Some(field_match) = caps.get(1)
            {
                let field_name = field_match.as_str().to_ascii_lowercase();
                if SECRET_NAMES.iter().any(|s| field_name.contains(s)) {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: format!(
                            "Secret-shaped field name `{}` passed to `span.record(...)` — \
                                this may leak credentials into trace spans.",
                            field_match.as_str()
                        ),
                        suggestion: Some(
                            "Omit secret-shaped fields from span records. \
                                If needed, use a redacted placeholder."
                                .into(),
                        ),
                        alternatives: vec![],
                        rationale: Some(
                            "Recording secret-shaped values in spans can expose credentials \
                                to any trace collector or observability backend."
                                .into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::High),
                        evidence: None,
                    });
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
    fn flags_tracing_info_with_token_field() {
        let d = SecretSpanDetector::new();
        let code = r#"tracing::info!(user = %user_id, token = %tok, "request");"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag token field in tracing::info!"
        );
        assert!(findings[0].message.contains("token"));
    }

    #[test]
    fn flags_span_record_with_password() {
        let d = SecretSpanDetector::new();
        let code = r#"span.record("password", &password_value);"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag span.record with password field"
        );
        assert!(findings[0].message.contains("password"));
    }

    #[test]
    fn ignores_non_secret_fields() {
        let d = SecretSpanDetector::new();
        let code = r#"tracing::info!(user_id = %id, request_path = %path, "handling request");"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "user_id and request_path should not fire"
        );
    }
}
