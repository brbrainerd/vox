use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects variables defined on line N whose last use is more than 80 lines later.
pub struct LongRangeCouplingDetector {
    /// Matches `let <ident> =` declarations
    let_decl: Regex,
    supported_langs: Vec<Language>,
}

impl Default for LongRangeCouplingDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LongRangeCouplingDetector {
    pub fn new() -> Self {
        Self {
            let_decl: Regex::new(r"\blet\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[=:]").expect("valid regex"),
            supported_langs: vec![Language::Rust, Language::Vox],
        }
    }

    /// Returns true if `ident` should be skipped (too short, loop variable, `_`-prefixed).
    fn should_skip(ident: &str) -> bool {
        // Skip `_` itself or `_`-prefixed names (intentionally unused)
        if ident == "_" || ident.starts_with('_') {
            return true;
        }
        // Skip very short identifiers (< 4 chars) — likely loop vars like `i`, `x`, `ok`
        if ident.len() < 4 {
            return true;
        }
        // Skip common loop variables by name even if ≥ 4 chars
        matches!(ident, "iter" | "item" | "self" | "this")
    }
}

impl DetectionRule for LongRangeCouplingDetector {
    fn id(&self) -> &'static str {
        "style/long-range-coupling"
    }

    fn name(&self) -> &'static str {
        "Long Range Coupling Detector"
    }

    fn description(&self) -> &'static str {
        "Detects `let` variable bindings whose last use is more than 80 lines from \
        the declaration site — a sign of long-range coupling that makes code harder to follow."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::STYLE_LONG_RANGE_COUPLING)
    }

    fn explain(&self) -> &'static str {
        "A variable used more than 80 lines after its declaration creates long-range coupling \
        that makes the code hard to read locally. Consider refactoring into a smaller function, \
        or passing the value as a parameter to reduce the scope."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — connection_string declared on line 1, used 90 lines later\n\
             let connection_string = build_db_url(&config);  // declared here\n\
             // ... 90 lines of unrelated logic ...\n\
             let conn = Database::connect(&connection_string);  // used 90 lines later!\n\
             \n\
             // FIX — narrow the scope or extract a helper function\n\
             fn open_database(config: &Config) -> Result<Database, Error> {\n\
             \x20   let connection_string = build_db_url(config);  // declared close to use\n\
             \x20   Database::connect(&connection_string)\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !matches!(file.language, Language::Rust | Language::Vox) {
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

            // Find `let <ident> =` declarations
            let Some(caps) = self.let_decl.captures(line) else {
                continue;
            };
            let Some(ident_match) = caps.get(1) else {
                continue;
            };
            let ident = ident_match.as_str();

            if Self::should_skip(ident) {
                continue;
            }

            // Find the last occurrence of `ident` in the rest of the file
            // We look for the identifier as a word boundary match
            let mut last_use_line = i; // 0-indexed; starts at declaration
            for (j, l) in lines.iter().enumerate().skip(i + 1) {
                // Simple word-boundary check: look for ident surrounded by non-word chars
                if contains_word(l, ident) {
                    last_use_line = j;
                }
            }

            let gap = last_use_line.saturating_sub(i);

            if gap > 80 {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: format!(
                        "Variable `{ident}` is declared here but last used {gap} lines later \
                        (line {}) — long-range coupling.",
                        last_use_line + 1
                    ),
                    suggestion: Some(format!(
                        "Refactor the code between the declaration of `{ident}` (line {line_num}) \
                        and its last use (line {}) into a smaller helper function.",
                        last_use_line + 1
                    )),
                    alternatives: vec![
                        "Pass the value as a parameter to reduce its visible scope.".into(),
                        "Use a block expression `{ let ... }` to limit the binding's lifetime."
                            .into(),
                    ],
                    rationale: Some(
                        "Variables used far from their declaration site create implicit couplings \
                        that make local reasoning about code correctness harder."
                            .into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
            }
        }

        findings
    }
}

/// Returns true if `line` contains `word` as a whole word (surrounded by non-alphanumeric/underscore chars).
fn contains_word(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let word_bytes = word.as_bytes();
    let wlen = word_bytes.len();
    if wlen > bytes.len() {
        return false;
    }
    for start in 0..=(bytes.len() - wlen) {
        if &bytes[start..start + wlen] == word_bytes {
            // Check left boundary
            let left_ok = start == 0 || !is_word_char(bytes[start - 1]);
            // Check right boundary
            let right_ok = start + wlen == bytes.len() || !is_word_char(bytes[start + wlen]);
            if left_ok && right_ok {
                return true;
            }
        }
    }
    false
}

#[inline]
fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{lang}")), code.to_string())
    }

    #[test]
    fn flags_variable_used_100_lines_later() {
        let d = LongRangeCouplingDetector::new();
        // Declare `config` on line 1, then pad 100 lines, then use it
        let mut code = String::from("let config = load_config();\n");
        for _ in 0..100 {
            code.push_str("// filler line\n");
        }
        code.push_str("apply(config);\n");

        let f = source("rs", &code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag config used 100 lines later"
        );
        assert!(findings[0].message.contains("config"));
    }

    #[test]
    fn ignores_variable_used_5_lines_later() {
        let d = LongRangeCouplingDetector::new();
        let mut code = String::from("let value = compute();\n");
        for _ in 0..5 {
            code.push_str("// filler\n");
        }
        code.push_str("use_value(value);\n");

        let f = source("rs", &code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "5-line gap should not fire");
    }
}
