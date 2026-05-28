use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `match opt { Some(x) => expr, None => expr }` patterns that can use
/// `.map(...).unwrap_or(...)` or similar combinators.
pub struct OptionCombinatorDetector {
    /// Matches the opening `match <ident> {` line
    match_open: Regex,
    supported_langs: Vec<Language>,
}

impl Default for OptionCombinatorDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl OptionCombinatorDetector {
    pub fn new() -> Self {
        Self {
            match_open: Regex::new(r"\bmatch\s+\w+\s*\{").expect("valid regex"),
            supported_langs: vec![Language::Vox, Language::Rust],
        }
    }
}

impl DetectionRule for OptionCombinatorDetector {
    fn id(&self) -> &'static str {
        "control-flow/option-combinator-opportunity"
    }

    fn name(&self) -> &'static str {
        "Option Combinator Opportunity Detector"
    }

    fn description(&self) -> &'static str {
        "Detects `match` over an `Option` that can be replaced with `.map(...)` or \
        `.unwrap_or(...)` combinators."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::CONTROL_FLOW_OPTION_COMBINATOR_OPPORTUNITY)
    }

    fn explain(&self) -> &'static str {
        "Match expressions over Option with two arms (Some and None) can usually be \
        written more concisely as combinator chains like `.map(f).unwrap_or(default)`."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VERBOSE — two-arm match over Option\n\
             let display_name = match user.nickname {\n\
             \x20   Some(n) => n.to_uppercase(),\n\
             \x20   None => \"Anonymous\".to_string(),\n\
             };\n\
             \n\
             // IDIOMATIC — use Option combinator\n\
             let display_name = user.nickname\n\
             \x20   .map(|n| n.to_uppercase())\n\
             \x20   .unwrap_or_else(|| \"Anonymous\".to_string());",
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
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                i += 1;
                continue;
            }

            // Check for `match <ident> {` opening
            if self.match_open.is_match(line) {
                // Look at the next 6 lines for Some( and None =>
                let window_end = (i + 7).min(n);
                let window: String = lines[i..window_end].join("\n");

                // Must have both Some( and None => to be an Option match
                let has_some = window.contains("Some(");
                let has_none = window.contains("None =>");

                // Must NOT have Ok( or Err( which would indicate a Result match
                let is_result = window.contains("Ok(") || window.contains("Err(");

                // Count distinct match arms to avoid flagging complex multi-arm matches
                let arm_count = window.matches("=>").count();

                if has_some && has_none && !is_result && arm_count <= 2 {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Info,
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: "This `match` over an `Option` can be replaced with \
                            `.map(...).unwrap_or(...)` or similar combinators."
                            .to_string(),
                        suggestion: Some(
                            "Replace `match opt { Some(x) => f(x), None => default }` \
                            with `opt.map(f).unwrap_or(default)`.".into(),
                        ),
                        alternatives: vec![
                            "Use `opt.map_or(default, f)` for a single-expression form.".into(),
                            "Use `if let Some(x) = opt { f(x) } else { default }` if the body is complex.".into(),
                        ],
                        rationale: Some(
                            "Option combinators are more concise, composable, and idiomatic \
                            in Rust/Vox than two-arm match expressions over Option.".into(),
                        ),
                        context: file.context_around(line_num, 3),
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
    fn fires_on_match_some_none() {
        let d = OptionCombinatorDetector::new();
        let code = "fn foo(opt: Option<i32>) -> i32 {\n    match opt {\n        Some(x) => x * 2,\n        None => 0,\n    }\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should fire on match-Some/None pattern"
        );
        assert!(findings[0].message.contains("Option"));
    }

    #[test]
    fn does_not_fire_on_match_result() {
        let d = OptionCombinatorDetector::new();
        let code = "fn foo(r: Result<i32, String>) -> i32 {\n    match r {\n        Ok(x) => x,\n        Err(_) => -1,\n    }\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "match on Result should not fire");
    }

    #[test]
    fn does_not_fire_on_multi_arm_match() {
        let d = OptionCombinatorDetector::new();
        // More than 2 arms — not a simple Some/None two-arm match
        let code = "fn foo(opt: Option<i32>) -> i32 {\n    match opt {\n        Some(x) if x > 0 => x,\n        Some(x) => -x,\n        None => 0,\n    }\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "multi-arm match should not fire");
    }
}
