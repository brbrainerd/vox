use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `@pure fn` declarations in Vox files that contain impure builtin calls.
pub struct PureFnImpureDetector {
    /// Matches `@pure` followed (possibly with whitespace) by `fn`
    pure_fn: Regex,
    /// Matches impure builtin calls
    impure_call: Regex,
    supported_langs: Vec<Language>,
}

impl Default for PureFnImpureDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PureFnImpureDetector {
    pub fn new() -> Self {
        Self {
            pure_fn: Regex::new(
                r"@pure\s+fn\b",
            )
            .expect("valid regex"),
            impure_call: Regex::new(
                r"\b(?:http\.|net\.|fs\.read\s*\(|fs\.write\s*\(|db\.|random\.|time\.now\s*\(|std\.http\.|populi\.|spawn\s*\(|await\s+|log\.|tracing\.)",
            )
            .expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for PureFnImpureDetector {
    fn id(&self) -> &'static str {
        "vox/effect/pure-violated"
    }

    fn name(&self) -> &'static str {
        "Pure Function Impure Call Detector"
    }

    fn description(&self) -> &'static str {
        "Detects `@pure fn` declarations in Vox files that contain impure builtin calls such as \
        HTTP, file I/O, database, random, logging, or concurrency primitives."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::EFFECT_PURE_VIOLATED)
    }

    fn explain(&self) -> &'static str {
        "A function annotated `@pure` must not call any impure builtin. The `@pure` annotation \
        declares that the function has no side effects and always returns the same output for the \
        same inputs — impure calls violate that contract and confuse callers who rely on purity \
        for caching, memoization, or formal reasoning.\n\n\
        Bad:   @pure fn compute(x: Int) -> Int { log.info(\"computing\"); x * 2 }\n\
        Good:  @pure fn compute(x: Int) -> Int { x * 2 }"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — @pure fn contains an impure side-effect call\n\
             @pure\n\
             fn format_amount(amount: f64) -> str {\n\
             \x20   log.info(\"formatting\")  // IMPURE: logging is a side effect\n\
             \x20   \"$\" + amount.to_string()\n\
             }\n\
             \n\
             // FIX — remove the side-effect call\n\
             @pure\n\
             fn format_amount(amount: f64) -> str {\n\
             \x20   \"$\" + amount.to_string()\n\
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
        let lines = &file.lines;
        let n = lines.len();

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

            // Check if this line declares a @pure fn
            if !self.pure_fn.is_match(line) {
                continue;
            }

            let pure_fn_line = line_num;

            // Scan the next 50 lines for impure calls
            let body_end = (i + 51).min(n);
            for (j, body_line) in lines[(i + 1)..body_end].iter().enumerate() {
                let body_line_num = i + 2 + j; // 1-indexed
                let body_trimmed = body_line.trim();

                // Skip comments
                if body_trimmed.starts_with("//")
                    || body_trimmed.starts_with('#')
                    || body_trimmed.starts_with('*')
                    || body_trimmed.starts_with("/*")
                {
                    continue;
                }

                // Stop if we hit another top-level fn or @pure fn (new scope)
                let at_col0 = !body_line.starts_with(' ')
                    && !body_line.starts_with('\t')
                    && !body_line.is_empty();
                if at_col0
                    && j > 0
                    && (body_trimmed.starts_with("fn ") || body_trimmed.starts_with("@"))
                {
                    break;
                }

                let Some(m) = self.impure_call.find(body_line) else {
                    continue;
                };

                let call = m.as_str().trim_end_matches('.').to_string();
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: pure_fn_line,
                    column: 1,
                    message: format!(
                        "`@pure fn` calls impure builtin `{call}` at line {body_line_num} — pure functions must be side-effect free."
                    ),
                    suggestion: Some(format!(
                        "Remove the call to `{call}` from this `@pure fn`, or remove the `@pure` annotation if side-effects are intended."
                    )),
                    alternatives: vec![
                        "Extract the impure call into a separate non-pure helper.".into(),
                        "Remove `@pure` if the function truly requires side-effects.".into(),
                    ],
                    rationale: Some(
                        "The `@pure` annotation promises callers that this function has no side effects \
                        and is referentially transparent. Calling impure builtins (HTTP, I/O, random, \
                        logging, etc.) violates this contract, breaking caching, memoization, and \
                        formal reasoning that depends on purity.".into(),
                    ),
                    context: file.context_around(pure_fn_line, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
                // One finding per @pure fn is enough (flag the declaration line once)
                break;
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vox_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_pure_fn_calling_http_get() {
        let d = PureFnImpureDetector::new();
        let code = "@pure fn compute(id: Int) -> Str {\n    http.get(\"/data/\" + id)\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag @pure fn that calls http.get"
        );
        assert!(findings[0].message.contains("@pure fn"));
    }

    #[test]
    fn ignores_pure_fn_with_no_impure_calls() {
        let d = PureFnImpureDetector::new();
        let code = "@pure fn compute(x: Int, y: Int) -> Int {\n    return x + y;\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "@pure fn with only pure arithmetic should not fire"
        );
    }

    #[test]
    fn flags_pure_fn_calling_random_int() {
        let d = PureFnImpureDetector::new();
        let code = "@pure fn rand_val() -> Int {\n    random.int(0, 10)\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag @pure fn calling random.int"
        );
        assert!(findings[0].message.contains("random"));
    }

    #[test]
    fn flags_pure_fn_calling_log() {
        let d = PureFnImpureDetector::new();
        let code = "@pure fn greet(name: Str) -> Str {\n    log.info(\"greeting: \" + name);\n    return \"Hello, \" + name;\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag @pure fn calling log.info"
        );
    }

    #[test]
    fn does_not_fire_on_non_vox_files() {
        let d = PureFnImpureDetector::new();
        let code = "@pure fn compute() {\n    http.get(\"/data\");\n}";
        let f = SourceFile::new(PathBuf::from("test.rs"), code.to_string());
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "should not fire on non-Vox files");
    }

    #[test]
    fn does_not_flag_non_pure_fn_with_impure_calls() {
        let d = PureFnImpureDetector::new();
        let code = "fn load_data(url: Str) -> Data {\n    http.get(url)\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "regular fn (no @pure) with net calls should not fire"
        );
    }
}
