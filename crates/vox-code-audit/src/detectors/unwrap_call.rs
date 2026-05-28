//! Heuristic detection of panicking Rust patterns (`.unwrap()`, `.expect(...)`, `panic!(...)`).
//!
//! Skips integration test trees, `tests.rs` files, `#[cfg(test)] mod tests { ... }` bodies, and
//! `#[cfg(test)]` / `#[test]` lines. Patterns are sourced from the embedded rule pack.

use crate::rule_pack_detector::pack_rule;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use vox_rule_pack::CompiledRule;

pub(crate) fn should_skip_file_for_rust_prod_heuristic(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/tests/")
        || s.contains("\\tests\\")
        || s.ends_with("_test.rs")
        || s.ends_with("tests.rs")
    {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with("_tests_body.rs"))
}

fn make_finding(rule: &'static CompiledRule, file: &SourceFile, line: usize) -> Finding {
    Finding {
        rule_id: rule.id.clone(),
        rule_name: rule.name.clone(),
        severity: rule.severity.into(),
        file: file.path.clone(),
        line,
        column: 0,
        message: rule.message.clone(),
        suggestion: rule.suggestion.clone(),
        diagnostic_id: None,
        alternatives: vec![],
        rationale: None,
        context: file.context_around(line, 2),
        confidence: rule.confidence.map(Into::into),
        evidence: None,
    }
}

/// `#[cfg(...)]` that enables `test` but is not `not(test)`.
fn line_enables_test_cfg(line: &str) -> bool {
    let s = line.trim_start();
    if !s.starts_with("#[") || !s.contains("cfg(") {
        return false;
    }
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains("not(test)") {
        return false;
    }
    compact.contains("test")
}

fn line_is_mod_tests_with_brace(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains("modtests{") || (compact.contains("modtests") && compact.contains('{'))
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|&c| c == '{').count() as i32;
    let closes = line.chars().filter(|&c| c == '}').count() as i32;
    opens - closes
}

pub(crate) fn detect_rust_line_regex_skipping_tests(
    file: &SourceFile,
    rule: &'static CompiledRule,
) -> Vec<Finding> {
    if should_skip_file_for_rust_prod_heuristic(&file.path) {
        return Vec::new();
    }
    let re = rule.regex();
    let mut out = Vec::new();
    let mut skip_test_mod_depth = 0i32;
    let mut expect_mod_tests_after_cfg_test = false;

    for (i, line) in file.lines.iter().enumerate() {
        let line_no = i + 1;

        if skip_test_mod_depth > 0 {
            skip_test_mod_depth += brace_delta(line);
            if skip_test_mod_depth <= 0 {
                skip_test_mod_depth = 0;
            }
            continue;
        }

        if expect_mod_tests_after_cfg_test {
            let t = line.trim_start();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            expect_mod_tests_after_cfg_test = false;
            if line_is_mod_tests_with_brace(line) {
                let d = brace_delta(line);
                if d > 0 {
                    skip_test_mod_depth = d;
                }
                continue;
            }
        }

        if line_enables_test_cfg(line) {
            if line_is_mod_tests_with_brace(line) {
                let d = brace_delta(line);
                if d > 0 {
                    skip_test_mod_depth = d;
                }
            } else {
                expect_mod_tests_after_cfg_test = true;
            }
            continue;
        }

        if line.contains("#[cfg(test)]")
            || line.contains("#[test]")
            || line.trim_start().starts_with("//")
        {
            continue;
        }
        if re.is_match(line) {
            out.push(make_finding(rule, file, line_no));
        }
    }
    out
}

/// Flags `.unwrap()` calls outside obvious test contexts.
pub struct UnwrapCallDetector {
    rule: &'static CompiledRule,
}

impl UnwrapCallDetector {
    pub fn new() -> Self {
        Self {
            rule: pack_rule("rust/unwrap-call"),
        }
    }

    #[cfg(test)]
    fn should_skip_file(path: &std::path::Path) -> bool {
        should_skip_file_for_rust_prod_heuristic(path)
    }
}

impl Default for UnwrapCallDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DetectionRule for UnwrapCallDetector {
    fn id(&self) -> &'static str {
        "rust/unwrap-call"
    }

    fn name(&self) -> &'static str {
        "UnwrapCallDetector"
    }

    fn description(&self) -> &'static str {
        "Heuristic: `.unwrap()` in Rust; skips common test paths and cfg(test) lines."
    }

    fn severity(&self) -> Severity {
        self.rule.severity.into()
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — .unwrap() can panic at runtime\n\
             fn get_user(id: UserId) -> User {\n\
             \x20   db.users.find(id).unwrap()  // panics if user is None\n\
             }\n\
             \n\
             // FIX — propagate the error or handle it explicitly\n\
             fn get_user(id: UserId) -> Result<User, DbError> {\n\
             \x20   db.users.find(id)?.ok_or(DbError::UserNotFound(id))\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        detect_rust_line_regex_skipping_tests(file, self.rule)
    }
}

/// Flags `.expect(...)` calls outside obvious test contexts.
pub struct ExpectCallDetector {
    rule: &'static CompiledRule,
}

impl ExpectCallDetector {
    pub fn new() -> Self {
        Self {
            rule: pack_rule("rust/expect-call"),
        }
    }
}

impl Default for ExpectCallDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DetectionRule for ExpectCallDetector {
    fn id(&self) -> &'static str {
        "rust/expect-call"
    }

    fn name(&self) -> &'static str {
        "ExpectCallDetector"
    }

    fn description(&self) -> &'static str {
        "Heuristic: `.expect(...)` in Rust; skips common test paths and cfg(test) lines."
    }

    fn severity(&self) -> Severity {
        self.rule.severity.into()
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — .expect() panics with a message at runtime\n\
             fn load_config() -> Config {\n\
             \x20   let data = std::fs::read_to_string(\"config.json\")\n\
             \x20       .expect(\"config.json must exist\");  // panics in production!\n\
             \x20   serde_json::from_str(&data).expect(\"invalid JSON\")\n\
             }\n\
             \n\
             // FIX — propagate errors with ?\n\
             fn load_config() -> Result<Config, Error> {\n\
             \x20   let data = std::fs::read_to_string(\"config.json\")?;\n\
             \x20   Ok(serde_json::from_str(&data)?)\n\
             }",
        )
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        detect_rust_line_regex_skipping_tests(file, self.rule)
    }
}

/// Flags `panic!(...)` outside obvious test contexts.
pub struct PanicCallDetector {
    rule: &'static CompiledRule,
}

impl PanicCallDetector {
    pub fn new() -> Self {
        Self {
            rule: pack_rule("rust/panic-call"),
        }
    }
}

impl Default for PanicCallDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DetectionRule for PanicCallDetector {
    fn id(&self) -> &'static str {
        "rust/panic-call"
    }

    fn name(&self) -> &'static str {
        "PanicCallDetector"
    }

    fn description(&self) -> &'static str {
        "Heuristic: `panic!(...)` in Rust; skips common test paths and cfg(test) lines."
    }

    fn severity(&self) -> Severity {
        self.rule.severity.into()
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — explicit panic! crashes the process\n\
             fn get_config_value(key: &str) -> String {\n\
             \x20   match config.get(key) {\n\
             \x20       Some(v) => v.clone(),\n\
             \x20       None => panic!(\"missing required config key: {key}\"),  // crashes!\n\
             \x20   }\n\
             }\n\
             \n\
             // FIX — return a Result instead\n\
             fn get_config_value(key: &str) -> Result<String, ConfigError> {\n\
             \x20   config.get(key)\n\
             \x20       .cloned()\n\
             \x20       .ok_or_else(|| ConfigError::MissingKey(key.to_string()))\n\
             }",
        )
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        detect_rust_line_regex_skipping_tests(file, self.rule)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::rules::Language;

    fn sf(lines: &[&str]) -> SourceFile {
        SourceFile {
            path: PathBuf::from("crates/demo/src/lib.rs"),
            language: Language::Rust,
            content: lines.join("\n"),
            lines: lines.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn skips_cfg_test_mod_block_body() {
        let lines = vec![
            "pub fn f() { x.unwrap(); }",
            "#[cfg(test)]",
            "mod tests {",
            "    #[test]",
            "    fn t() { let _ = y.unwrap(); }",
            "}",
            "pub fn g() { z.unwrap(); }",
        ];
        let file = sf(&lines);
        let d = UnwrapCallDetector::new();
        let hits: Vec<_> = d.detect(&file, None).into_iter().map(|f| f.line).collect();
        assert_eq!(hits, vec![1, 7]);
    }

    #[test]
    fn skips_single_line_cfg_test_mod() {
        let lines = vec![
            "#[cfg(test)] mod tests { fn x() { a.unwrap(); } }",
            "pub fn prod() { b.unwrap(); }",
        ];
        let file = sf(&lines);
        let d = UnwrapCallDetector::new();
        let hits: Vec<_> = d.detect(&file, None).into_iter().map(|f| f.line).collect();
        assert_eq!(hits, vec![2]);
    }

    #[test]
    fn skips_all_feature_gated_test_mod() {
        let lines = vec![
            "#[cfg(all(test, feature = \"foo\"))]",
            "mod tests {",
            "    fn x() { a.unwrap(); }",
            "}",
        ];
        let file = sf(&lines);
        let d = UnwrapCallDetector::new();
        assert!(d.detect(&file, None).is_empty());
    }

    #[test]
    fn does_not_skip_not_test_cfg() {
        let lines = vec!["#[cfg(not(test))]", "fn x() { a.unwrap(); }"];
        let file = sf(&lines);
        let d = UnwrapCallDetector::new();
        assert_eq!(d.detect(&file, None).len(), 1);
    }

    #[test]
    fn skips_tests_body_include_files_by_name() {
        let file = SourceFile {
            path: PathBuf::from("crates/vox-cli/src/commands/mens/populi/gpu_tests_body.rs"),
            language: Language::Rust,
            content: String::new(),
            lines: vec!["fn x() { a.unwrap(); }".into()],
        };
        assert!(UnwrapCallDetector::should_skip_file(&file.path));
        let d = UnwrapCallDetector::new();
        assert!(d.detect(&file, None).is_empty());
    }
}
