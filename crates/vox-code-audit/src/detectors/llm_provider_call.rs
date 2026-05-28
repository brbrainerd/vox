use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects direct HTTP calls to known LLM provider hostnames, bypassing `populi.*`.
pub struct LlmProviderCallDetector {
    /// Matches any known provider hostname as a string literal.
    hostname_pattern: Regex,
    /// Matches Vox/TS HTTP call builtins.
    vox_http_call: Regex,
    /// Matches Rust HTTP client library calls.
    rust_http_call: Regex,
    supported_langs: Vec<Language>,
}

impl Default for LlmProviderCallDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProviderCallDetector {
    /// Known LLM provider hostnames.
    const HOSTNAMES: &'static [&'static str] = &[
        "openrouter.ai",
        "api.anthropic.com",
        "api.openai.com",
        "openai.com/v1",
        "cohere.ai",
        "api.mistral.ai",
        "api.together.xyz",
        "api.replicate.com",
        "huggingface.co/api",
        "api.fireworks.ai",
        "api.perplexity.ai",
        "generativelanguage.googleapis.com",
        "aiplatform.googleapis.com",
    ];

    pub fn new() -> Self {
        let hostname_alt = Self::HOSTNAMES.join("|").replace(".", r"\.");
        let hostname_re = format!(r#"(?i)({hostname_alt})"#);

        Self {
            hostname_pattern: Regex::new(&hostname_re).expect("valid hostname regex"),
            vox_http_call: Regex::new(
                r"\b(std\.http\.post_json|std\.http\.get|std\.http\.request|std\.http\.post|http\.post)\b",
            )
            .expect("valid vox_http_call regex"),
            rust_http_call: Regex::new(
                r"\b(reqwest::Client|reqwest::get|isahc|ureq|attohttpc)\b",
            )
            .expect("valid rust_http_call regex"),
            supported_langs: vec![Language::Vox, Language::Rust, Language::TypeScript],
        }
    }

    /// Returns `true` if `line` contains a known provider hostname.
    fn has_hostname(&self, line: &str) -> bool {
        self.hostname_pattern.is_match(line)
    }

    /// Returns `true` if `line` contains an HTTP call pattern appropriate for `lang`.
    fn has_http_call(&self, line: &str, lang: Language) -> bool {
        match lang {
            Language::Rust => self.rust_http_call.is_match(line),
            Language::Vox | Language::TypeScript => self.vox_http_call.is_match(line),
            _ => false,
        }
    }

    /// Returns the matched hostname substring, for the finding message.
    fn matched_hostname<'a>(&self, line: &'a str) -> &'a str {
        self.hostname_pattern
            .find(line)
            .map(|m| m.as_str())
            .unwrap_or("(provider)")
    }

    fn make_finding(&self, file: &SourceFile, line_num: usize, hostname: &str) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: Some(catalog::LLM_DIRECT_PROVIDER_CALL.to_string()),
            rule_name: self.name().to_string(),
            severity: Severity::Error,
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message: format!(
                "Direct HTTP call to LLM provider hostname `{hostname}` detected. \
                 Route inference through `populi.*` builtins instead."
            ),
            suggestion: Some(
                "Route inference through `populi.complete(...)` or another `populi.*` builtin \
                 instead of calling the provider directly."
                    .to_string(),
            ),
            alternatives: vec![
                "populi.stream(...)".to_string(),
                "populi.embed(...)".to_string(),
            ],
            rationale: Some(
                "Direct HTTP calls to LLM provider hostnames bypass the populi telemetry, \
                 cost accounting, capability ledger, and retry policy. All inference traffic \
                 must route through populi.* builtins."
                    .to_string(),
            ),
            context: file.context_around(line_num, 2),
            confidence: Some(FindingConfidence::High),
            evidence: None,
        }
    }
}

impl DetectionRule for LlmProviderCallDetector {
    fn id(&self) -> &'static str {
        "vox/llm/direct-provider-call"
    }

    fn name(&self) -> &'static str {
        "LLM Direct Provider Call Detector"
    }

    fn description(&self) -> &'static str {
        "Detects direct HTTP calls to known LLM provider hostnames that bypass populi.* builtins."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::LLM_DIRECT_PROVIDER_CALL)
    }

    fn explain(&self) -> &'static str {
        "Direct HTTP calls to LLM provider hostnames (e.g. api.openai.com, api.anthropic.com) \
         bypass populi telemetry, cost accounting, the capability ledger, and automatic retry \
         policy. All inference traffic must route through populi.* builtins.\n\n\
         BAD:\n  let resp = std.http.post_json(\"https://api.openai.com/v1/chat/completions\", body);\n\n\
         GOOD:\n  let resp = populi.complete(model, prompt);"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — direct call to LLM provider hostname\n\
             let body = { model: \"gpt-4\", messages: [] }\n\
             let resp = std.http.post_json(\"https://api.openai.com/v1/chat/completions\", body)\n\
             \n\
             // FIX — use populi.* instead\n\
             let resp = populi.complete(\"gpt-4\", prompt)",
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

            if !self.has_hostname(line) {
                continue;
            }

            // Check for HTTP call on the same line first
            if self.has_http_call(line, file.language) {
                let hostname = self.matched_hostname(line).to_string();
                findings.push(self.make_finding(file, line_num, &hostname));
                continue;
            }

            // Check within 3 lines before and after for HTTP calls
            let window_start = i.saturating_sub(3);
            let window_end = (i + 4).min(file.lines.len());
            let nearby_has_http = file.lines[window_start..window_end]
                .iter()
                .any(|l| self.has_http_call(l, file.language));

            if nearby_has_http {
                let hostname = self.matched_hostname(line).to_string();
                findings.push(self.make_finding(file, line_num, &hostname));
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
    fn detects_vox_http_post_with_openai_hostname() {
        let d = LlmProviderCallDetector::new();
        let code = r#"let body = {model: "gpt-4", messages: msgs};
let resp = std.http.post_json("https://api.openai.com/v1/chat/completions", body);
"#;
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect openai.com direct call");
        assert!(
            findings[0].message.contains("api.openai.com")
                || findings[0].message.contains("openai")
        );
        assert_eq!(
            findings[0].diagnostic_id.as_deref(),
            Some("vox/llm/direct-provider-call")
        );
    }

    #[test]
    fn detects_rust_reqwest_with_anthropic_hostname() {
        let d = LlmProviderCallDetector::new();
        let code = r#"
let client = reqwest::Client::new();
let url = "https://api.anthropic.com/v1/messages";
let resp = client.post(url).send().await?;
"#;
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should detect reqwest call to anthropic"
        );
    }

    #[test]
    fn ignores_populi_call() {
        let d = LlmProviderCallDetector::new();
        let code = r#"let resp = populi.complete(model, prompt);"#;
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "populi.* calls should not be flagged");
    }

    #[test]
    fn ignores_hostname_in_comment() {
        let d = LlmProviderCallDetector::new();
        let code = r#"// See https://api.openai.com for docs
let x = populi.complete(m, p);
"#;
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should not be flagged");
    }

    #[test]
    fn hostname_without_http_call_not_flagged() {
        let d = LlmProviderCallDetector::new();
        // Just a URL string stored in a variable with no HTTP call nearby
        let code = r#"let docs_url = "https://api.openai.com/docs";"#;
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "hostname without an HTTP call should not fire"
        );
    }
}
