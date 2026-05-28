use crate::rule_pack_detector::pack_rule;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile, rust_byte_is_non_code};
use vox_rule_pack::CompiledRule;

/// Detects `todo!()`, `unimplemented!()`, `panic!("not implemented")`,
/// Python `pass` / `raise NotImplementedError`, GDScript `pass`.
///
/// Patterns are sourced from the embedded rule pack (`stub/*`).
pub struct StubDetector {
    rust_todo: &'static CompiledRule,
    rust_unimplemented: &'static CompiledRule,
    rust_panic_not_impl: &'static CompiledRule,
    py_raise_not_impl: &'static CompiledRule,
    ts_throw_not_impl: &'static CompiledRule,
    generic_placeholder: &'static CompiledRule,
    stub_comment: &'static CompiledRule,
}

impl Default for StubDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// `Regex::find` match counts only when the match starts in a **code** span (not string/comment).
fn stub_regex_match_in_code(
    file: &SourceFile,
    line_num: usize,
    line: &str,
    rule: &CompiledRule,
    rust_ctx: Option<&crate::analysis::RustFileContext>,
) -> bool {
    rule.regex()
        .find_iter(line)
        .any(|m| !rust_byte_is_non_code(file, line_num, m.start(), rust_ctx))
}

/// Line-comment scan for work markers: keep ordinary `//` / `#` lines; rustdoc defers to code spans.
fn stub_todo_comment_line_matches(
    file: &SourceFile,
    line_num: usize,
    line: &str,
    rule: &CompiledRule,
    rust_ctx: Option<&crate::analysis::RustFileContext>,
) -> bool {
    if !rule.regex().is_match(line) {
        return false;
    }
    let t = line.trim_start();
    if t.starts_with("///") || t.starts_with("//!") {
        return rule
            .regex()
            .find_iter(line)
            .any(|m| !rust_byte_is_non_code(file, line_num, m.start(), rust_ctx));
    }
    if t.starts_with("//") || (t.starts_with('#') && !t.starts_with("#[")) {
        return true;
    }
    rule.regex()
        .find_iter(line)
        .any(|m| !rust_byte_is_non_code(file, line_num, m.start(), rust_ctx))
}

fn placeholder_matches_line(
    file: &SourceFile,
    line_num: usize,
    line: &str,
    rule: &CompiledRule,
    rust_ctx: Option<&crate::analysis::RustFileContext>,
) -> bool {
    if !rule.regex().is_match(line) {
        return false;
    }
    let t = line.trim_start();
    if (t.starts_with("//") && !t.starts_with("///") && !t.starts_with("//!")) || t.starts_with('*')
    {
        return true;
    }
    stub_regex_match_in_code(file, line_num, line, rule, rust_ctx)
}

/// True when `stub` appears as its own word but not as the `stub-check` feature name.
fn bare_stub_word_not_stub_check(
    file: &SourceFile,
    line_num: usize,
    line: &str,
    rust_ctx: Option<&crate::analysis::RustFileContext>,
) -> bool {
    let lower = line.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0usize;
    while let Some(rel) = lower[i..].find("stub") {
        let idx = i + rel;
        if rust_byte_is_non_code(file, line_num, idx, rust_ctx) {
            i = idx + 1;
            continue;
        }
        if lower[idx..].starts_with("stub-check") {
            i = idx + 4;
            continue;
        }
        let after = idx + 4;
        // `stub::foo` module paths and `mod stub` / `pub mod stub` declarations.
        if after + 1 < bytes.len() && bytes[after] == b':' && bytes[after + 1] == b':' {
            i = idx + 1;
            continue;
        }
        let before_trim = line[..idx].trim_end();
        if before_trim.ends_with("mod") {
            i = idx + 1;
            continue;
        }
        // Markdown inline code like `stub` is not prose placeholder text.
        if idx > 0 && bytes[idx - 1] == b'`' {
            i = idx + 1;
            continue;
        }
        if after < bytes.len() && bytes[after] == b'`' {
            i = idx + 1;
            continue;
        }
        let left_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric() && bytes[idx - 1] != b'_';
        let right_ok =
            after >= bytes.len() || (!bytes[after].is_ascii_alphanumeric() && bytes[after] != b'_');
        if left_ok && right_ok {
            return true;
        }
        i = idx + 1;
    }
    false
}

impl StubDetector {
    pub fn new() -> Self {
        Self {
            rust_todo: pack_rule("stub/todo"),
            rust_unimplemented: pack_rule("stub/unimplemented"),
            rust_panic_not_impl: pack_rule("stub/panic-not-impl"),
            py_raise_not_impl: pack_rule("stub/not-implemented-error"),
            ts_throw_not_impl: pack_rule("stub/throw-not-implemented"),
            generic_placeholder: pack_rule("stub/placeholder"),
            stub_comment: pack_rule("stub/todo-comment"),
        }
    }

    fn detect_rust(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if line.contains("toestub-ignore(all)") || line.contains("toestub-ignore(stub)") {
                continue;
            }

            if line.contains("DEAD-CODE:")
                || line.contains("DEAD PATTERNS:")
                || line.contains("todo!()/unimplemented!()")
                || line.contains("make_file(")
            {
                continue;
            }

            if stub_regex_match_in_code(file, line_num, line, self.rust_todo, rust_ctx) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/todo",
                    "`todo!()` macro — incomplete implementation",
                    Some("Replace `todo!()` with the actual implementation.".into()),
                ));
            }
            if stub_regex_match_in_code(file, line_num, line, self.rust_unimplemented, rust_ctx) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/unimplemented",
                    "`unimplemented!()` macro — missing implementation",
                    Some("Implement the function body or remove the stub.".into()),
                ));
            }
            if stub_regex_match_in_code(file, line_num, line, self.rust_panic_not_impl, rust_ctx) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/panic-not-impl",
                    "`panic!(\"not implemented\")` — stub placeholder",
                    Some(
                        "Replace the panic with actual logic or use `todo!()` during development."
                            .into(),
                    ),
                ));
            }
            if placeholder_matches_line(file, line_num, line, self.generic_placeholder, rust_ctx)
                || bare_stub_word_not_stub_check(file, line_num, line, rust_ctx)
            {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/placeholder",
                    "Placeholder text detected (STUB, FIXME, or PLACEHOLDER)",
                    Some("Replace placeholders with actual implementation or high-quality documentation.".into()),
                ));
            }
            if stub_todo_comment_line_matches(file, line_num, line, self.stub_comment, rust_ctx) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/todo-comment",
                    "TODO comment found — incomplete code",
                    Some("Address the TODO or track it in an issue tracker.".into()),
                ));
            }
        }
        findings
    }

    fn detect_python(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if self.py_raise_not_impl.regex().is_match(line) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/not-implemented-error",
                    "`raise NotImplementedError` — stub placeholder",
                    Some("Implement the function body.".into()),
                ));
            }
        }

        self.detect_python_pass_stubs(file, &mut findings);
        findings
    }

    fn detect_python_pass_stubs(&self, file: &SourceFile, findings: &mut Vec<Finding>) {
        // Simple heuristic: look for `def ...:\n    pass`
        let pass_re = {
            // py_pass_sub was repurposed to stub/placeholder; use a local pattern for `pass`
            regex::Regex::new(r"^\s*pass\s*$").expect("pass regex")
        };
        for i in 0..file.lines.len().saturating_sub(1) {
            let line = &file.lines[i];
            let next_line = &file.lines[i + 1];
            if line.trim_start().starts_with("def ") && pass_re.is_match(next_line) {
                let has_more_body = file
                    .lines
                    .get(i + 2)
                    .map(|l| {
                        let trimmed = l.trim();
                        !trimmed.is_empty()
                            && !trimmed.starts_with("def ")
                            && !trimmed.starts_with("class ")
                            && !trimmed.starts_with('#')
                            && l.starts_with(char::is_whitespace)
                            && l.len() > next_line.len() - next_line.trim_start().len()
                    })
                    .unwrap_or(false);

                if !has_more_body {
                    findings.push(self.make_finding(
                        file,
                        i + 2,
                        "stub/pass",
                        "`pass` stub — empty function body",
                        Some("Implement the function body or add a docstring explaining why it's empty.".into()),
                    ));
                }
            }
        }
    }

    fn detect_gdscript(&self, file: &SourceFile) -> Vec<Finding> {
        let pass_re = regex::Regex::new(r"^\s*pass\s*$").expect("pass regex");
        let mut findings = Vec::new();
        for i in 0..file.lines.len().saturating_sub(1) {
            let line = &file.lines[i];
            let next_line = &file.lines[i + 1];
            if line.trim_start().starts_with("func ") && pass_re.is_match(next_line) {
                findings.push(self.make_finding(
                    file,
                    i + 2,
                    "stub/gdscript-pass",
                    "`pass` stub in GDScript function — empty implementation",
                    Some("Implement the function body.".into()),
                ));
            }
        }
        findings
    }

    fn detect_typescript(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            if self.ts_throw_not_impl.regex().is_match(line) {
                findings.push(self.make_finding(
                    file,
                    i + 1,
                    "stub/throw-not-implemented",
                    "`throw new Error('not implemented')` — stub placeholder",
                    Some("Implement the function body.".into()),
                ));
            }
        }
        findings
    }

    fn make_finding(
        &self,
        file: &SourceFile,
        line: usize,
        sub_id: &str,
        message: &str,
        suggestion: Option<String>,
    ) -> Finding {
        Finding {
            rule_id: sub_id.to_string(),
            diagnostic_id: None,
            rule_name: self.name().to_string(),
            severity: self.severity(),
            file: file.path.clone(),
            line,
            column: 0,
            message: message.to_string(),
            suggestion,
            alternatives: vec![],
            rationale: None,
            context: file.context_around(line, 2),
            confidence: None,
            evidence: None,
        }
    }
}

impl DetectionRule for StubDetector {
    fn id(&self) -> &'static str {
        "arch/stub"
    }
    fn name(&self) -> &'static str {
        "Stub / Placeholder Detector"
    }
    fn description(&self) -> &'static str {
        "Detects todo!(), unimplemented!(), pass stubs, and throw-not-implemented patterns"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn languages(&self) -> &[Language] {
        &[
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::GDScript,
        ]
    }
    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — unimplemented stub left in shipped code\n\
             fn calculate_discount(price: f64) -> f64 {\n\
             \x20   todo!()\n\
             }\n\
             \n\
             // FIX — real implementation or scope down to what compiles\n\
             fn calculate_discount(price: f64) -> f64 {\n\
             \x20   price * 0.10\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file, rust_ctx),
            Language::Python => self.detect_python(file),
            Language::GDScript => self.detect_gdscript(file),
            Language::TypeScript => self.detect_typescript(file),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang_ext: &str, code: &str) -> SourceFile {
        SourceFile::new(
            PathBuf::from(format!("test.{}", lang_ext)),
            code.to_string(),
        )
    }

    #[test]
    fn detects_rust_todo() {
        let d = StubDetector::new();
        let f = source("rs", "fn foo() {\n    todo!()\n}");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "stub/todo");
    }

    #[test]
    fn detects_rust_unimplemented() {
        let d = StubDetector::new();
        let f = source("rs", "fn bar() -> i32 {\n    unimplemented!()\n}");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "stub/unimplemented");
    }

    #[test]
    fn detects_python_raise() {
        let d = StubDetector::new();
        let f = source("py", "def foo():\n    raise NotImplementedError\n");
        let findings = d.detect(&f, None);
        assert!(
            findings
                .iter()
                .any(|f| f.rule_id == "stub/not-implemented-error"),
            "should detect raise NotImplementedError"
        );
    }

    #[test]
    fn detects_python_pass_stub() {
        let d = StubDetector::new();
        let f = source("py", "def foo():\n    pass\n");
        let findings = d.detect(&f, None);
        assert!(
            findings.iter().any(|f| f.rule_id == "stub/pass"),
            "should detect pass stub"
        );
    }

    #[test]
    fn clean_rust_produces_no_findings() {
        let d = StubDetector::new();
        let f = source("rs", "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "clean code should have no findings");
    }

    #[test]
    fn test_excludes_internal_prompt_text() {
        let d = StubDetector::new();
        let f = source("rs", r#"const P: &str = "DEAD-CODE: todo!()...";"#);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "should exclude internal prompt strings"
        );
    }

    #[test]
    fn placeholder_ignores_stub_check_feature_name() {
        let d = StubDetector::new();
        let f = source(
            "rs",
            "/// `vox stub-check` / `vox mens stub-check`\n#[cfg(feature = \"stub-check\")]\n",
        );
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/placeholder"),
            "stub-check should not trip generic STUB placeholder rule"
        );
    }

    #[test]
    fn placeholder_ignores_stub_word_in_doc_comment() {
        let d = StubDetector::new();
        let f = source("rs", "/// Run a workflow (stub for future runtime)\n");
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/placeholder"),
            "doc comments are non-code spans; narrative 'stub' should not fire placeholder rule"
        );
    }

    #[test]
    fn placeholder_detects_stub_word_in_code_line() {
        let d = StubDetector::new();
        let f = source("rs", "fn foo() { let stub = 1u32; }\n");
        assert!(
            d.detect(&f, None)
                .iter()
                .any(|x| x.rule_id == "stub/placeholder"),
            "bare `stub` token in code (not comment/string) should still trip placeholder rule"
        );
    }

    #[test]
    fn placeholder_ignores_lowercase_english_placeholder_word() {
        let d = StubDetector::new();
        let f = source(
            "rs",
            "// This is a placeholder token for documentation only.\nfn ok() {}\n",
        );
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/placeholder"),
            "common English 'placeholder' must not match"
        );
    }

    #[test]
    fn placeholder_detects_shouty_placeholder_marker() {
        let d = StubDetector::new();
        let f = source("rs", "// PLACEHOLDER: wire real API\nfn ok() {}\n");
        let findings = d.detect(&f, None);
        assert!(findings.iter().any(|x| x.rule_id == "stub/placeholder"));
    }
}
