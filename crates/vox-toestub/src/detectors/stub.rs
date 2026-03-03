use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `todo!()`, `unimplemented!()`, `panic!("not implemented")`,
/// Python `pass` / `raise NotImplementedError`, GDScript `pass`.
pub struct StubDetector {
    rust_todo: Regex,
    rust_unimplemented: Regex,
    rust_panic_not_impl: Regex,
    py_raise_not_impl: Regex,
    py_pass_stub: Regex,
    ts_throw_not_impl: Regex,
    generic_placeholder: Regex,
    stub_comment: Regex,
}

impl Default for StubDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StubDetector {
    pub fn new() -> Self {
        Self {
            rust_todo: Regex::new(r"\btodo!\s*\(").expect("valid regex"),
            rust_unimplemented: Regex::new(r"\bunimplemented!\s*\(").expect("valid regex"),
            rust_panic_not_impl: Regex::new(r#"\bpanic!\s*\(\s*"not\s+implemented"#)
                .expect("valid regex"),
            py_raise_not_impl: Regex::new(r"\braise\s+NotImplementedError\b").expect("valid regex"),
            py_pass_stub: Regex::new(r"^\s*pass\s*$").expect("valid regex"),
            ts_throw_not_impl: Regex::new(r#"throw\s+new\s+Error\s*\(\s*["']not\s+implemented"#)
                .expect("valid regex"),
            generic_placeholder: Regex::new(r"(?i)\bPLACEHOLDER\b|\bFIXME\b|\bSTUB\b").expect("valid regex"),
            stub_comment: Regex::new(r"(?i)//\s*TODO\b|#\s*TODO\b").expect("valid regex"),
        }
    }

    fn detect_rust(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            // Skip known false positive strings in our own prompts and tests
            if line.contains("DEAD-CODE:")
                || line.contains("DEAD PATTERNS:")
                || line.contains("todo!()/unimplemented!()")
                || line.contains("make_file(")
            {
                continue;
            }

            if self.rust_todo.is_match(line) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/todo",
                    "`todo!()` macro — incomplete implementation",
                    Some("Replace `todo!()` with the actual implementation.".into()),
                ));
            }
            if self.rust_unimplemented.is_match(line) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/unimplemented",
                    "`unimplemented!()` macro — missing implementation",
                    Some("Implement the function body or remove the stub.".into()),
                ));
            }
            if self.rust_panic_not_impl.is_match(line) {
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
            if self.generic_placeholder.is_match(line) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/placeholder",
                    "Placeholder text detected (STUB, FIXME, or PLACEHOLDER)",
                    Some("Replace placeholders with actual implementation or high-quality documentation.".into()),
                ));
            }
            if self.stub_comment.is_match(line) {
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

            if self.py_raise_not_impl.is_match(line) {
                findings.push(self.make_finding(
                    file,
                    line_num,
                    "stub/not-implemented-error",
                    "`raise NotImplementedError` — stub placeholder",
                    Some("Implement the function body.".into()),
                ));
            }
        }

        // Detect `pass` as a stub only when it's the sole statement in a function body
        self.detect_python_pass_stubs(file, &mut findings);
        findings
    }

    fn detect_python_pass_stubs(&self, file: &SourceFile, findings: &mut Vec<Finding>) {
        // Simple heuristic: look for `def ...:\n    pass`
        for i in 0..file.lines.len().saturating_sub(1) {
            let line = &file.lines[i];
            let next_line = &file.lines[i + 1];
            if line.trim_start().starts_with("def ") && self.py_pass_stub.is_match(next_line) {
                // Check if `pass` is the only thing in the body (next non-empty line after `pass`)
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
                        i + 2, // the `pass` line
                        "stub/pass",
                        "`pass` stub — empty function body",
                        Some("Implement the function body or add a docstring explaining why it's empty.".into()),
                    ));
                }
            }
        }
    }

    fn detect_gdscript(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for i in 0..file.lines.len().saturating_sub(1) {
            let line = &file.lines[i];
            let next_line = &file.lines[i + 1];
            if line.trim_start().starts_with("func ") && self.py_pass_stub.is_match(next_line) {
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
            if self.ts_throw_not_impl.is_match(&line.to_lowercase()) {
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
            rule_name: self.name().to_string(),
            severity: self.severity(),
            file: file.path.clone(),
            line,
            column: 0,
            message: message.to_string(),
            suggestion,
            context: file.context_around(line, 2),
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
    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file),
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
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "stub/todo");
    }

    #[test]
    fn detects_rust_unimplemented() {
        let d = StubDetector::new();
        let f = source("rs", "fn bar() -> i32 {\n    unimplemented!()\n}");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "stub/unimplemented");
    }

    #[test]
    fn detects_python_raise() {
        let d = StubDetector::new();
        let f = source("py", "def foo():\n    raise NotImplementedError\n");
        let findings = d.detect(&f);
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
        let findings = d.detect(&f);
        assert!(
            findings.iter().any(|f| f.rule_id == "stub/pass"),
            "should detect pass stub"
        );
    }

    #[test]
    fn clean_rust_produces_no_findings() {
        let d = StubDetector::new();
        let f = source("rs", "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "clean code should have no findings");
    }

    #[test]
    fn test_excludes_internal_prompt_text() {
        let d = StubDetector::new();
        let f = source("rs", r#"const P: &str = "DEAD-CODE: todo!()...";"#);
        let findings = d.detect(&f);
        assert!(
            findings.is_empty(),
            "should exclude internal prompt strings"
        );
    }
}
