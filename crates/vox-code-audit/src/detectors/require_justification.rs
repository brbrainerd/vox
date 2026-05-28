use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects complex `@require(...)` expressions without a justification comment.
pub struct RequireJustificationDetector {
    require_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for RequireJustificationDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RequireJustificationDetector {
    pub fn new() -> Self {
        Self {
            require_pattern: Regex::new(r"@require\(([^)]+)\)").expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }

    fn count_operators(expr: &str) -> usize {
        // Count all operator occurrences; use multi-char ops first so `>=` counts as one op,
        // not as `>` + `=`.
        let ops = ["&&", "||", ">=", "<=", "!=", "==", " > ", " < "];
        let mut total = 0;
        for op in &ops {
            let mut start = 0;
            while let Some(pos) = expr[start..].find(op) {
                total += 1;
                start += pos + op.len();
            }
        }
        total
    }

    fn has_justification_comment(line: &str, next_line: Option<&str>) -> bool {
        // Check trailing comment on same line
        if let Some(comment_start) = line.find("//") {
            let comment_text = &line[comment_start + 2..];
            let non_ws_count = comment_text.chars().filter(|c| !c.is_whitespace()).count();
            if non_ws_count >= 40 {
                return true;
            }
        }

        // Check next line for a comment
        if let Some(next) = next_line {
            let trimmed = next.trim();
            if let Some(comment_text) = trimmed.strip_prefix("//") {
                let non_ws_count = comment_text.chars().filter(|c| !c.is_whitespace()).count();
                if non_ws_count >= 40 {
                    return true;
                }
            }
        }

        false
    }
}

impl DetectionRule for RequireJustificationDetector {
    fn id(&self) -> &'static str {
        "require/justification-prose-required"
    }

    fn name(&self) -> &'static str {
        "Require Justification Prose Required Detector"
    }

    fn description(&self) -> &'static str {
        "Detects complex `@require(...)` expressions that lack an adequate justification comment."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::REQUIRE_JUSTIFICATION_PROSE_REQUIRED)
    }

    fn explain(&self) -> &'static str {
        "Complex preconditions that aren't self-explanatory from the code should explain *why* \
        they exist, not just *what* they check. This is especially important for LLM agents \
        reading the code later.\n\n\
        Bad:  @require(x > 0 && y < 100 && z != null)\n      fn process(x, y, z) {}\n\n\
        Good: @require(x > 0 && y < 100 && z != null)\n      // because: x must be positive for the logarithm, y bounded by protocol limit, z required by schema\n      fn process(x, y, z) {}"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — complex @require with no explanation of why\n\
             @require(amount > 0 && amount <= 1_000_000 && currency != null && !user.frozen)\n\
             fn process_payment(amount: f64, currency: str, user: User) { ... }\n\
             \n\
             // FIX — add a justification comment explaining the constraints\n\
             @require(amount > 0 && amount <= 1_000_000 && currency != null && !user.frozen)\n\
             // because: amount must be positive (no zero-value payments); 1M cap is the\n\
             // regulatory limit; currency required by ISO-4217 schema; frozen users blocked\n\
             // by compliance policy (see ADR-031).\n\
             fn process_payment(amount: f64, currency: str, user: User) { ... }",
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
        let lines = &file.lines;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;

            let trimmed = line.trim();
            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }

            if let Some(caps) = self.require_pattern.captures(line) {
                let expr = caps.get(1).map_or("", |m| m.as_str());
                let op_count = Self::count_operators(expr);

                if op_count > 1 {
                    let next_line = lines.get(i + 1).map(String::as_str);
                    if !Self::has_justification_comment(line, next_line) {
                        findings.push(Finding {
                            rule_id: self.id().to_string(),
                            diagnostic_id: self.diagnostic_id().map(str::to_string),
                            rule_name: self.name().to_string(),
                            severity: Severity::Info,
                            file: file.path.clone(),
                            line: line_num,
                            column: 0,
                            message: "Complex `@require(...)` expression lacks a justification comment (need ≥ 40 chars after `//`).".to_string(),
                            suggestion: Some(
                                "Add `// because: <reason>` comment after the `@require(...)` line explaining why this invariant holds.".into(),
                            ),
                            alternatives: vec![],
                            rationale: Some(
                                "Complex preconditions that aren't self-explanatory from the code should explain *why* \
                                they exist, not just *what* they check. This is especially important for LLM agents reading the code later.".into(),
                            ),
                            context: file.context_around(line_num, 2),
                            confidence: Some(FindingConfidence::Medium),
                            evidence: None,
                        });
                    }
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

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn fires_on_complex_require_without_comment() {
        let d = RequireJustificationDetector::new();
        let code = "@require(x > 0 && y < 100 && z != null)\nfn process(x, y, z) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should fire on complex @require without comment"
        );
        assert!(findings[0].message.contains("@require"));
    }

    #[test]
    fn does_not_fire_on_simple_require() {
        let d = RequireJustificationDetector::new();
        let code = "@require(x > 0)\nfn process(x) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "simple @require should not fire");
    }

    #[test]
    fn does_not_fire_when_justification_comment_present_on_next_line() {
        let d = RequireJustificationDetector::new();
        let code = "@require(x > 0 && y < 100 && z != null)\n// because: x must be positive for the logarithm calculation used downstream\nfn process(x, y, z) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "sufficient comment on next line should satisfy rule"
        );
    }

    #[test]
    fn does_not_fire_when_trailing_comment_is_long_enough() {
        let d = RequireJustificationDetector::new();
        // trailing comment with 40+ non-ws chars
        let code = "@require(x > 0 && y < 100 && z != null) // because x must be positive for logarithm and y is bounded by protocol\nfn process(x, y, z) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "long trailing comment should satisfy rule"
        );
    }

    #[test]
    fn fires_when_next_line_comment_is_too_short() {
        let d = RequireJustificationDetector::new();
        let code = "@require(x > 0 && y < 100 && z != null)\n// short\nfn process(x, y, z) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "short comment should still fire");
    }

    #[test]
    fn does_not_fire_on_two_operator_boundary() {
        let d = RequireJustificationDetector::new();
        // exactly 1 operator — should NOT fire (need > 1)
        let code = "@require(x > 0)\nfn check(x) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "single operator should not trigger rule"
        );
    }
}
