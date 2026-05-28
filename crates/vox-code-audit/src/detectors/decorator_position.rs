use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects decorator/keyword position mismatches in Vox files.
///
/// Catches bare keywords where decorators are required (`durable fn` → `@durable fn`)
/// and redundant decorator+keyword pairs (`@actor actor` → `actor`).
pub struct DecoratorPositionDetector {
    /// Matches bare `durable fn`, `pure fn`, or `scheduled fn`
    bare_keyword_pattern: Regex,
    /// Matches redundant `@actor actor`, `@workflow workflow`, `@activity activity`
    redundant_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for DecoratorPositionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DecoratorPositionDetector {
    pub fn new() -> Self {
        Self {
            bare_keyword_pattern: Regex::new(r"\b(durable|pure|scheduled)\s+fn\b")
                .expect("valid regex"),
            redundant_pattern: Regex::new(
                r"@(actor|workflow|activity)\s+(actor|workflow|activity)\s+",
            )
            .expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for DecoratorPositionDetector {
    fn id(&self) -> &'static str {
        "decorator/position-mismatch"
    }

    fn name(&self) -> &'static str {
        "Decorator Position Mismatch Detector"
    }

    fn description(&self) -> &'static str {
        "Detects bare keywords where decorators are required, or redundant decorator+keyword pairs."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::DECORATOR_POSITION_MISMATCH)
    }

    fn explain(&self) -> &'static str {
        "Vox grammar rule: behavior expressible as a decorator must be a decorator. Bare keywords \
        declare scope; decorators modify declarations. See AGENTS.md §Grammar Unification.\n\n\
        Bad:  durable fn process() {}\n\
        Good: @durable fn process() {}\n\n\
        Bad:  @actor actor MyActor { ... }\n\
        Good: actor MyActor { ... }"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — bare keyword where @ decorator is required\n\
             durable fn process_order(id: str) {\n\
             \x20   // ...\n\
             }\n\
             \n\
             // FIX — prefix with @\n\
             @durable fn process_order(id: str) {\n\
             \x20   // ...\n\
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
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }

            // Check for bare keyword before fn (missing @)
            if let Some(m) = self.bare_keyword_pattern.find(line) {
                // Only fire if it's not already preceded by @ on this line
                let before = &line[..m.start()];
                if !before.ends_with('@') && !before.trim_end().ends_with('@') {
                    let keyword = self
                        .bare_keyword_pattern
                        .captures(line)
                        .and_then(|c| c.get(1))
                        .map_or("", |m| m.as_str());

                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: line_num,
                        column: m.start() + 1,
                        message: format!(
                            "Bare `{keyword}` before `fn` — did you mean `@{keyword} fn`?"
                        ),
                        suggestion: Some(format!(
                            "Replace `{keyword} fn` with `@{keyword} fn`."
                        )),
                        alternatives: vec![],
                        rationale: Some(
                            "Vox grammar rule: behavior expressible as a decorator must be a decorator. \
                            Bare keywords declare scope; decorators modify declarations. See AGENTS.md §Grammar Unification.".into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::High),
                        evidence: None,
                    });
                }
            }

            // Check for redundant @decorator keyword pairs
            if let Some(m) = self.redundant_pattern.find(line) {
                let caps = self.redundant_pattern.captures(line).unwrap();
                let decorator_kw = caps.get(1).map_or("", |c| c.as_str());
                let bare_kw = caps.get(2).map_or("", |c| c.as_str());

                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: format!(
                        "Redundant `@{decorator_kw}` decorator with `{bare_kw}` keyword — use `{bare_kw}` keyword alone."
                    ),
                    suggestion: Some(format!(
                        "Remove the `@{decorator_kw}` decorator; the `{bare_kw}` keyword is sufficient."
                    )),
                    alternatives: vec![],
                    rationale: Some(
                        "Vox grammar rule: behavior expressible as a decorator must be a decorator. \
                        Bare keywords declare scope; decorators modify declarations. See AGENTS.md §Grammar Unification.".into(),
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
    fn fires_on_bare_durable_fn() {
        let d = DecoratorPositionDetector::new();
        let f = source("durable fn process() {}");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on bare 'durable fn'");
        assert!(findings[0].message.contains("durable"));
        assert!(findings[0].message.contains("@durable fn"));
    }

    #[test]
    fn does_not_fire_on_at_durable_fn() {
        let d = DecoratorPositionDetector::new();
        let f = source("@durable fn process() {}");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "@durable fn is correct syntax");
    }

    #[test]
    fn fires_on_bare_pure_fn() {
        let d = DecoratorPositionDetector::new();
        let f = source("pure fn compute() {}");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on bare 'pure fn'");
        assert!(findings[0].message.contains("pure"));
    }

    #[test]
    fn fires_on_redundant_actor_actor() {
        let d = DecoratorPositionDetector::new();
        let f = source("@actor actor MyActor {}");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on @actor actor");
        assert!(findings[0].message.contains("Redundant"));
    }

    #[test]
    fn does_not_fire_on_correct_actor() {
        let d = DecoratorPositionDetector::new();
        let f = source("actor MyActor {}");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "bare actor keyword is correct");
    }

    #[test]
    fn skips_comment_lines() {
        let d = DecoratorPositionDetector::new();
        let f = source("// durable fn process() is wrong style");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should be skipped");
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = DecoratorPositionDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            "durable fn process() {}".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "rust files should be ignored");
    }
}
