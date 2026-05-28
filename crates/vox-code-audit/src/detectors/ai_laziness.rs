//! AI-laziness patterns the existing stub/empty_body/hollow_fn detectors don't catch.
//!
//! Each of the seven sub-rules emits a distinct `rule_id` so reviewers can act on them
//! independently. Patterns are sourced from the embedded rule pack (`ai-laziness/*`).

use crate::rule_pack_detector::pack_rule;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use vox_rule_pack::CompiledRule;

pub struct AiLazinessDetector {
    placeholder_return: &'static CompiledRule,
    implement_later_comment: &'static CompiledRule,
    mock_named_fn: &'static CompiledRule,
    custom_type_default_return: &'static CompiledRule,
    conditional_stub: &'static CompiledRule,
    assertion_only_body: &'static CompiledRule,
    early_return_only: &'static CompiledRule,
}

impl Default for AiLazinessDetector {
    fn default() -> Self {
        Self::new()
    }
}

const BUILTIN_DEFAULT_TYPES: &[&str] = &[
    "String", "Vec", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque", "Box", "Option",
    "Result", "Default", "PathBuf", "OsString",
];

impl AiLazinessDetector {
    pub fn new() -> Self {
        Self {
            placeholder_return: pack_rule("ai-laziness/placeholder-return"),
            implement_later_comment: pack_rule("ai-laziness/implement-later-comment"),
            mock_named_fn: pack_rule("ai-laziness/mock-named-fn"),
            custom_type_default_return: pack_rule("ai-laziness/custom-type-default-return"),
            conditional_stub: pack_rule("ai-laziness/conditional-stub"),
            assertion_only_body: pack_rule("ai-laziness/assertion-only-body"),
            early_return_only: pack_rule("ai-laziness/early-return-only"),
        }
    }

    fn is_builtin_default_type(name: &str) -> bool {
        BUILTIN_DEFAULT_TYPES.contains(&name)
    }

    fn is_test_gated(file: &SourceFile) -> bool {
        let path = file.path.to_string_lossy().replace('\\', "/");
        let in_tests_dir = path.contains("/tests/") || path.starts_with("tests/");
        let test_suffix = path.ends_with("_test.rs")
            || path.ends_with("_tests.rs")
            || path.ends_with(".test.ts")
            || path.ends_with(".test.tsx")
            || path.ends_with("_test.py");
        let inner_attr = file
            .lines
            .iter()
            .take(5)
            .any(|l| l.trim_start().starts_with("#![cfg(test)]"));
        let has_cfg_test_mod = {
            let mut found = false;
            let lines = &file.lines;
            for i in 0..lines.len() {
                let trimmed = lines[i].trim_start();
                if trimmed.starts_with("#[cfg(test)]") {
                    let rest = trimmed.trim_start_matches("#[cfg(test)]").trim_start();
                    if rest.starts_with("mod ") || rest.starts_with("pub mod ") {
                        found = true;
                        break;
                    }
                    if let Some(next) = lines.get(i + 1) {
                        let n = next.trim_start();
                        if n.starts_with("mod ") || n.starts_with("pub mod ") {
                            found = true;
                            break;
                        }
                    }
                }
            }
            found
        };
        in_tests_dir || test_suffix || inner_attr || has_cfg_test_mod
    }
}

impl DetectionRule for AiLazinessDetector {
    fn id(&self) -> &'static str {
        "ai-laziness"
    }

    fn name(&self) -> &'static str {
        "AI Laziness Patterns"
    }

    fn description(&self) -> &'static str {
        "Catches subtle AI cop-outs: placeholder string returns, 'implement later' comments, \
         mock-named functions shipped to prod, custom-type default returns, conditional stubs, \
         assertion-only bodies, and early-return-only bodies."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        const LANGS: &[Language] = &[
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::Vox,
        ];
        LANGS
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — AI laziness pattern: placeholder return or 'implement later' comment\n\
             fn calculate_tax(amount: f64, region: &str) -> f64 {\n\
             \x20   // TODO: implement tax calculation\n\
             \x20   0.0  // placeholder\n\
             }\n\
             \n\
             // FIX — real implementation; scope down if full implementation is out of scope\n\
             fn calculate_tax(amount: f64, region: &str) -> f64 {\n\
             \x20   let rate = TAX_RATES.get(region).copied().unwrap_or(0.0);\n\
             \x20   (amount * rate * 100.0).round() / 100.0\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let test_gated = Self::is_test_gated(file);

        // Multiline patterns: scan the joined content once.
        for caps in self.conditional_stub.regex().captures_iter(&file.content) {
            let m = caps.get(0).expect("regex match has group 0");
            let line = file.content[..m.start()].lines().count() + 1;
            findings.push(Finding {
                rule_id: "ai-laziness/conditional-stub".into(),
                rule_name: "Conditional stub branch".into(),
                severity: Severity::Warning,
                file: file.path.clone(),
                line,
                column: 0,
                message: self.conditional_stub.message.clone(),
                suggestion: self.conditional_stub.suggestion.clone(),
                diagnostic_id: None,
                alternatives: vec![],
                rationale: None,
                context: file.context_around(line, 2),
                confidence: Some(FindingConfidence::Medium),
                evidence: None,
            });
        }
        for m in self.assertion_only_body.regex().find_iter(&file.content) {
            let line = file.content[..m.start()].lines().count() + 1;
            findings.push(Finding {
                rule_id: "ai-laziness/assertion-only-body".into(),
                rule_name: "Assertion-only function body".into(),
                severity: Severity::Warning,
                file: file.path.clone(),
                line,
                column: 0,
                message: self.assertion_only_body.message.clone(),
                suggestion: self.assertion_only_body.suggestion.clone(),
                diagnostic_id: None,
                alternatives: vec![],
                rationale: None,
                context: file.context_around(line, 2),
                confidence: Some(FindingConfidence::Medium),
                evidence: None,
            });
        }
        for m in self.early_return_only.regex().find_iter(&file.content) {
            let line = file.content[..m.start()].lines().count() + 1;
            findings.push(Finding {
                rule_id: "ai-laziness/early-return-only".into(),
                rule_name: "Early-return-only function body".into(),
                severity: Severity::Warning,
                file: file.path.clone(),
                line,
                column: 0,
                message: self.early_return_only.message.clone(),
                suggestion: self.early_return_only.suggestion.clone(),
                diagnostic_id: None,
                alternatives: vec![],
                rationale: None,
                context: file.context_around(line, 2),
                confidence: Some(FindingConfidence::Medium),
                evidence: None,
            });
        }

        // Per-line patterns.
        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if self.placeholder_return.regex().is_match(line) {
                findings.push(Finding {
                    rule_id: "ai-laziness/placeholder-return".into(),
                    rule_name: "Placeholder string return".into(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: self.placeholder_return.message.clone(),
                    suggestion: self.placeholder_return.suggestion.clone(),
                    diagnostic_id: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }

            if self.implement_later_comment.regex().is_match(line) {
                findings.push(Finding {
                    rule_id: "ai-laziness/implement-later-comment".into(),
                    rule_name: "Implement-later comment".into(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: self.implement_later_comment.message.clone(),
                    suggestion: self.implement_later_comment.suggestion.clone(),
                    diagnostic_id: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }

            if !test_gated && let Some(caps) = self.mock_named_fn.regex().captures(line) {
                let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                findings.push(Finding {
                    rule_id: "ai-laziness/mock-named-fn".into(),
                    rule_name: "Mock-named function in non-test code".into(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: format!(
                        "Function name starts with `{}` but the file is not gated as test \
                                 code — mocks should not ship.",
                        prefix
                    ),
                    suggestion: Some(
                        "Move the function under `#[cfg(test)]`, rename it to its real \
                                 responsibility, or delete it if unused."
                            .into(),
                    ),
                    diagnostic_id: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
            }

            if let Some(caps) = self.custom_type_default_return.regex().captures(line) {
                let ty = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                if !ty.is_empty() && !Self::is_builtin_default_type(ty) {
                    findings.push(Finding {
                        rule_id: "ai-laziness/custom-type-default-return".into(),
                        diagnostic_id: None,
                        rule_name: "Custom-type default return".into(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: format!(
                            "`return {}::default()` (or `::new()`) for a project-defined type \
                             usually means the function isn't really implemented.",
                            ty
                        ),
                        suggestion: Some(
                            "Compute the actual return value, or document why the default is \
                             semantically correct here."
                                .into(),
                        ),
                        alternatives: vec![],
                        rationale: None,
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::Low),
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

    fn run(src: &str) -> Vec<Finding> {
        let file = SourceFile::new(PathBuf::from("src/foo.rs"), src.to_string());
        AiLazinessDetector::new().detect(&file, None)
    }

    fn ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.rule_id.as_str()).collect()
    }

    #[test]
    fn catches_placeholder_string_return() {
        let f = run(r#"fn x() -> &'static str { return "TODO" }"#);
        assert!(ids(&f).contains(&"ai-laziness/placeholder-return"));
    }

    #[test]
    fn catches_placeholder_in_result() {
        let f = run(r#"fn x() -> Result<&str> { return Ok("placeholder") }"#);
        assert!(ids(&f).contains(&"ai-laziness/placeholder-return"));
    }

    #[test]
    fn ignores_legitimate_string_return() {
        let f = run(r#"fn x() -> &'static str { return "hello world" }"#);
        assert!(!ids(&f).contains(&"ai-laziness/placeholder-return"));
    }

    #[test]
    fn catches_implement_later_comment() {
        let f = run("// implement later\nfn x() {}");
        assert!(ids(&f).contains(&"ai-laziness/implement-later-comment"));
    }

    #[test]
    fn catches_finish_this_comment() {
        let f = run("// finish this when we have a real auth backend\nfn x() {}");
        assert!(ids(&f).contains(&"ai-laziness/implement-later-comment"));
    }

    #[test]
    fn catches_mock_named_fn() {
        let f = run("pub fn mock_database_call() -> u32 { 0 }");
        assert!(ids(&f).contains(&"ai-laziness/mock-named-fn"));
    }

    #[test]
    fn ignores_mock_in_test_path() {
        let file = SourceFile::new(
            PathBuf::from("crates/foo/tests/test_helpers.rs"),
            "pub fn mock_database_call() -> u32 { 0 }".into(),
        );
        let f = AiLazinessDetector::new().detect(&file, None);
        assert!(!ids(&f).contains(&"ai-laziness/mock-named-fn"));
    }

    #[test]
    fn catches_custom_type_default_return() {
        let f = run("fn x() -> MyConfig { return MyConfig::default() }");
        assert!(ids(&f).contains(&"ai-laziness/custom-type-default-return"));
    }

    #[test]
    fn ignores_builtin_type_default() {
        let f = run("fn x() -> Vec<u8> { return Vec::new() }");
        assert!(!ids(&f).contains(&"ai-laziness/custom-type-default-return"));
    }

    #[test]
    fn catches_conditional_stub() {
        let f = run(
            "fn x(b: bool) -> Result<()> { if b { return Ok(()); } else { do_real_work(); Ok(()) } }",
        );
        assert!(ids(&f).contains(&"ai-laziness/conditional-stub"));
    }

    #[test]
    fn catches_assertion_only_body() {
        let f = run("fn x(n: u32) { assert!(n > 0) }");
        assert!(ids(&f).contains(&"ai-laziness/assertion-only-body"));
    }

    #[test]
    fn catches_log_only_body() {
        let f = run(r#"fn x() { log::info!("called") }"#);
        assert!(ids(&f).contains(&"ai-laziness/assertion-only-body"));
    }

    #[test]
    fn catches_early_return_only_body() {
        let f = run("fn x() { return; }");
        assert!(ids(&f).contains(&"ai-laziness/early-return-only"));
    }

    #[test]
    fn catches_log_then_early_return() {
        let f = run(r#"fn x() { println!("done"); return; }"#);
        assert!(ids(&f).contains(&"ai-laziness/early-return-only"));
    }
}
