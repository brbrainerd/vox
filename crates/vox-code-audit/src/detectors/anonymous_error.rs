use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects anonymous error types (`Result[T, str]`) on public function boundaries in Vox files.
///
/// Using bare `str` as an error type loses type information and makes error handling
/// fragile. Named error types or enums should be used instead.
pub struct AnonymousErrorDetector {
    /// Matches `fn name(...) -> Result[T, str]`
    result_str_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for AnonymousErrorDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnonymousErrorDetector {
    pub fn new() -> Self {
        Self {
            result_str_pattern: Regex::new(r"\bfn\s+\w+[^{]*Result\s*\[[^\]]*,\s*str\s*\]")
                .expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for AnonymousErrorDetector {
    fn id(&self) -> &'static str {
        "types/anonymous-error-type"
    }

    fn name(&self) -> &'static str {
        "Anonymous Error Type Detector"
    }

    fn description(&self) -> &'static str {
        "Detects functions returning `Result[T, str]` where a named error type should be used."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::TYPES_ANONYMOUS_ERROR_TYPE)
    }

    fn explain(&self) -> &'static str {
        "Functions returning `Result[T, str]` use an anonymous error type; define a named error enum or struct to preserve error context and enable exhaustive handling."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — anonymous error type (str) loses error context\n\
             fn load_config(path: str) -> Result[Config, str] {\n\
             \x20   parse_file(path).map_err(|e| e.to_string())\n\
             }\n\
             \n\
             // FIX — named error type with meaningful variants\n\
             enum ConfigError {\n\
             \x20   NotFound(str),\n\
             \x20   ParseFailure(str),\n\
             }\n\
             fn load_config(path: str) -> Result[Config, ConfigError] {\n\
             \x20   parse_file(path).map_err(|e| ConfigError::ParseFailure(e.to_string()))\n\
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

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
                continue;
            }

            if let Some(m) = self.result_str_pattern.find(line) {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: "Function returns `Result[..., str]` — use a named error type instead of bare `str`.".to_string(),
                    suggestion: Some(
                        "Define a named error enum (e.g. `FetchError`) and use `Result[T, FetchError]` \
                        to preserve error context.".to_string(),
                    ),
                    alternatives: vec![],
                    rationale: Some(
                        "Bare `str` as an error type loses type information, prevents exhaustive \
                        error handling, and makes error propagation fragile. Named error types \
                        document the failure modes of a function explicitly.".into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }
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
    fn flags_result_str_error_type() {
        let d = AnonymousErrorDetector::new();
        let f = source("fn fetch() -> Result[User, str] {}");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should flag Result[User, str]");
        assert!(findings[0].message.contains("Result[..., str]"));
    }

    #[test]
    fn ignores_named_error_type() {
        let d = AnonymousErrorDetector::new();
        let f = source("fn fetch() -> Result[User, FetchError] {}");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "Result[User, FetchError] should not fire"
        );
    }

    #[test]
    fn ignores_plain_str_return_non_result() {
        let d = AnonymousErrorDetector::new();
        let f = source("fn name() -> str {}");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "fn returning str (not Result) should not fire"
        );
    }

    #[test]
    fn ignores_comment_lines() {
        let d = AnonymousErrorDetector::new();
        let f = source("// fn fetch() -> Result[User, str] {}");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should be skipped");
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = AnonymousErrorDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            "fn fetch() -> Result[User, str] {}".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "rust files should be ignored");
    }
}
