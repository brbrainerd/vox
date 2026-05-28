use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `match` over `Result`/`Option` that can be replaced with the `?` operator.
pub struct QuestionMarkDetector {
    /// Matches the opening of a match-on-result: `match ... {` followed by `Ok(`
    match_result_open: Regex,
    /// Matches `Err(e) => return Err(` pattern
    err_return_pattern: Regex,
    /// Matches single-line `match ... { Ok(x) => x, Err(` pattern
    single_line_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for QuestionMarkDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestionMarkDetector {
    pub fn new() -> Self {
        Self {
            match_result_open: Regex::new(r"\bmatch\s+.+\s*\{").expect("valid regex"),
            err_return_pattern: Regex::new(r"Err\s*\(\s*\w+\s*\)\s*=>\s*return\s+Err\s*\(")
                .expect("valid regex"),
            single_line_pattern: Regex::new(
                r"\bmatch\s+.+\s*\{\s*Ok\s*\(\s*\w+\s*\)\s*=>\s*\w+\s*,\s*Err\s*\(",
            )
            .expect("valid regex"),
            supported_langs: vec![Language::Vox, Language::Rust],
        }
    }
}

impl DetectionRule for QuestionMarkDetector {
    fn id(&self) -> &'static str {
        "control-flow/question-mark-opportunity"
    }

    fn name(&self) -> &'static str {
        "Question Mark Opportunity Detector"
    }

    fn description(&self) -> &'static str {
        "Detects `match` over a `Result` that can be replaced with the `?` operator."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::CONTROL_FLOW_QUESTION_MARK_OPPORTUNITY)
    }

    fn explain(&self) -> &'static str {
        "The `?` operator is the idiomatic Vox/Rust way to propagate errors. Long match arms on \
        Results make code harder to read and are a sign of LLM-generated code that didn't learn \
        the idiom.\n\n\
        Bad:\n  match expr {\n      Ok(x) => x,\n      Err(e) => return Err(e),\n  }\n\n\
        Good:\n  expr?"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VERBOSE — manual match on Result for error propagation\n\
             fn load_config(path: &str) -> Result<Config, Error> {\n\
             \x20   let content = match std::fs::read_to_string(path) {\n\
             \x20       Ok(s) => s,\n\
             \x20       Err(e) => return Err(e.into()),\n\
             \x20   };\n\
             \x20   // ...\n\
             }\n\
             \n\
             // IDIOMATIC — use the ? operator\n\
             fn load_config(path: &str) -> Result<Config, Error> {\n\
             \x20   let content = std::fs::read_to_string(path)?;\n\
             \x20   // ...\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !matches!(file.language, Language::Vox | Language::Rust) {
            return vec![];
        }

        let mut findings = Vec::new();
        let lines = &file.lines;
        let n = lines.len();

        let mut i = 0;
        while i < n {
            let line = &lines[i];
            let line_num = i + 1;

            let trimmed = line.trim();
            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
                i += 1;
                continue;
            }

            // Check single-line pattern: `match expr { Ok(x) => x, Err(`
            if self.single_line_pattern.is_match(line) {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "This `match` on a `Result` can be replaced with the `?` operator."
                        .to_string(),
                    suggestion: Some(
                        "Replace `match expr { Ok(x) => x, Err(e) => return Err(e) }` with `expr?`."
                            .into(),
                    ),
                    alternatives: vec![],
                    rationale: Some(
                        "The `?` operator is the idiomatic Vox/Rust way to propagate errors. \
                        Long match arms on Results make code harder to read and are a sign of \
                        LLM-generated code that didn't learn the idiom.".into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
                i += 1;
                continue;
            }

            // Check multi-line pattern: `match ... {` on this line, with `Ok(` in next few lines,
            // and `Err(e) => return Err(` within 5 lines of the match
            if self.match_result_open.is_match(line) {
                let window_end = (i + 6).min(n);
                let window: String = lines[i..window_end].join("\n");

                if window.contains("Ok(") && self.err_return_pattern.is_match(&window) {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Info,
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: "This `match` on a `Result` can be replaced with the `?` operator."
                            .to_string(),
                        suggestion: Some(
                            "Replace `match expr { Ok(x) => x, Err(e) => return Err(e) }` with `expr?`."
                                .into(),
                        ),
                        alternatives: vec![],
                        rationale: Some(
                            "The `?` operator is the idiomatic Vox/Rust way to propagate errors. \
                            Long match arms on Results make code harder to read and are a sign of \
                            LLM-generated code that didn't learn the idiom.".into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::Medium),
                        evidence: None,
                    });
                }
            }

            i += 1;
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
    fn fires_on_match_ok_err_return_multiline() {
        let d = QuestionMarkDetector::new();
        let code = "fn foo() -> Result<i32, String> {\n    let x = match get_value() {\n        Ok(v) => v,\n        Err(e) => return Err(e),\n    };\n    Ok(x)\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should fire on match-ok-err-return pattern"
        );
        assert!(findings[0].message.contains("?"));
    }

    #[test]
    fn does_not_fire_on_question_mark() {
        let d = QuestionMarkDetector::new();
        let code = "fn foo() -> Result<i32, String> {\n    let x = get_value()?;\n    Ok(x)\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "already uses ? should not fire");
    }

    #[test]
    fn fires_on_single_line_ok_err_pattern() {
        let d = QuestionMarkDetector::new();
        let code = "let val = match compute() { Ok(x) => x, Err(e) => return Err(e.into()) };";
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on single-line pattern");
    }

    #[test]
    fn does_not_fire_on_legitimate_match() {
        let d = QuestionMarkDetector::new();
        // A match with different handling for each arm — not a simple propagation
        let code = "match value {\n    Ok(x) if x > 0 => handle_positive(x),\n    Ok(x) => handle_non_positive(x),\n    Err(e) => log_error(e),\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "complex match arms should not fire");
    }
}
