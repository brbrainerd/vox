use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects functions with empty or trivially-defaulted bodies.
pub struct EmptyBodyDetector {
    /// Matches `fn name(...) { }` or `fn name(...) -> T { }` with only whitespace inside.
    rust_empty_fn: Regex,
    /// Matches TypeScript/JS `function name() {}` or `() => {}`.
    ts_empty_fn: Regex,
    ts_empty_arrow: Regex,
    /// Matches Python `def name(): ...` (ellipsis body).
    py_ellipsis: Regex,
    /// Matches `impl Trait for Type { }` with nothing inside.
    rust_empty_impl: Regex,
}

impl EmptyBodyDetector {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for EmptyBodyDetector {
    fn default() -> Self {
        Self {
            rust_empty_fn: Regex::new(r"fn\s+\w+").expect("valid regex"),
            ts_empty_fn: Regex::new(r"function\s+\w+\s*\([^)]*\)\s*\{\s*\}").expect("valid regex"),
            ts_empty_arrow: Regex::new(r"=>\s*\{\s*\}").expect("valid regex"),
            py_ellipsis: Regex::new(r"^\s*\.\.\.\s*$").expect("valid regex"),
            rust_empty_impl: Regex::new(r"impl\s+[\w<>]+\s+for\s+[\w<>]+").expect("valid regex"),
        }
    }
}

impl EmptyBodyDetector {
    fn detect_rust(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut i = 0;
        while i < file.lines.len() {
            let line = &file.lines[i];
            let trimmed = line.trim();

            // Look for `fn ident(...)` lines
            if self.rust_empty_fn.is_match(trimmed) && !trimmed.starts_with("//") {
                // Check for single-line empty body: `fn foo() {}`
                if trimmed.ends_with("{}") || trimmed.ends_with("{ }") {
                    // Allow `fn main() {}` — skip specifically
                    if !trimmed.contains("main()") {
                        findings.push(self.make_finding(file, i + 1, "Function has an empty body"));
                    }
                }
                // Check for multi-line empty body
                else if trimmed.ends_with('{') || line.contains('{') {
                    // Find the closing brace
                    if let Some(body_range) = self.find_brace_body(file, i) {
                        if body_range.0 <= body_range.1 {
                            let body_content: String = file.lines[body_range.0..body_range.1]
                                .iter()
                                .map(|l| l.trim())
                                .filter(|l| !l.is_empty() && *l != "{" && *l != "}")
                                .collect::<Vec<_>>()
                                .join("");

                            if body_content.is_empty() && !trimmed.contains("main()") {
                                findings.push(self.make_finding(
                                    file,
                                    i + 1,
                                    "Function has an empty body",
                                ));
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        // Detect empty impl blocks: `impl Trait for Type {}`
        for (idx, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim();
            if self.rust_empty_impl.is_match(trimmed) && (trimmed.ends_with("{}") || trimmed.ends_with("{ }")) {
                findings.push(self.make_finding(file, idx + 1, "Implementation block is empty"));
            } else if self.rust_empty_impl.is_match(trimmed) && trimmed.ends_with('{') {
                if let Some(body_range) = self.find_brace_body(file, idx) {
                    let has_content = file.lines[body_range.0..body_range.1]
                        .iter()
                        .any(|l| {
                            let t = l.trim();
                            !t.is_empty() && t != "{" && t != "}" && !t.starts_with("//")
                        });
                    if !has_content {
                        findings.push(self.make_finding(file, idx + 1, "Implementation block is empty"));
                    }
                }
            }
        }

        findings
    }

    fn detect_typescript(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            if self.ts_empty_fn.is_match(line) {
                findings.push(self.make_finding(
                    file,
                    i + 1,
                    "TypeScript function has an empty body",
                ));
            }
            // Only flag arrow fns when they look like a standalone method stub
            if self.ts_empty_arrow.is_match(line) && line.contains("=>") {
                // Avoid false positives on callbacks like `.then(() => {})` — only flag if
                // the line looks like a variable declaration or object method
                let trimmed = line.trim();
                if trimmed.starts_with("const ")
                    || trimmed.starts_with("let ")
                    || trimmed.starts_with("export ")
                {
                    findings.push(self.make_finding(
                        file,
                        i + 1,
                        "Arrow function has an empty body",
                    ));
                }
            }
        }
        findings
    }

    fn detect_python(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        for i in 0..file.lines.len().saturating_sub(1) {
            let line = &file.lines[i];
            let next_line = &file.lines[i + 1];
            if line.trim_start().starts_with("def ") && self.py_ellipsis.is_match(next_line) {
                findings.push(self.make_finding(
                    file,
                    i + 2,
                    "Python function body is just `...` (ellipsis stub)",
                ));
            }
        }
        findings
    }

    fn find_brace_body(&self, file: &SourceFile, start: usize) -> Option<(usize, usize)> {
        let mut depth = 0i32;
        let mut body_start = None;
        for j in start..file.lines.len() {
            for ch in file.lines[j].chars() {
                if ch == '{' {
                    if depth == 0 {
                        body_start = Some(j + 1);
                    }
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return body_start.map(|s| (s, j));
                    }
                }
            }
        }
        None
    }

    fn make_finding(&self, file: &SourceFile, line: usize, message: &str) -> Finding {
        Finding {
            rule_id: "empty-body".to_string(),
            rule_name: self.name().to_string(),
            severity: self.severity(),
            file: file.path.clone(),
            line,
            column: 0,
            message: message.to_string(),
            suggestion: Some("Implement the function body or remove the empty stub.".to_string()),
            context: file.context_around(line, 2),
        }
    }
}

impl DetectionRule for EmptyBodyDetector {
    fn id(&self) -> &'static str {
        "arch/empty_body"
    }
    fn name(&self) -> &'static str {
        "Empty Body Detector"
    }
    fn description(&self) -> &'static str {
        "Detects functions with empty or trivially-defaulted bodies"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript, Language::Python]
    }
    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file),
            Language::TypeScript => self.detect_typescript(file),
            Language::Python => self.detect_python(file),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn detects_rust_empty_fn() {
        let d = EmptyBodyDetector::new();
        let f = source("rs", "fn process_event() {}");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn detects_rust_empty_impl() {
        let d = EmptyBodyDetector::new();
        let f = source("rs", "impl Default for MyStruct {}");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("Implementation block is empty"));
    }

    #[test]
    fn detects_rust_multi_line_empty_impl() {
        let d = EmptyBodyDetector::new();
        let f = source("rs", "impl MyTrait for MyType {\n    \n}");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_rust_fn_with_body() {
        let d = EmptyBodyDetector::new();
        let f = source("rs", "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
        let findings = d.detect(&f);
        assert!(findings.is_empty());
    }

    #[test]
    fn detects_ts_empty_function() {
        let d = EmptyBodyDetector::new();
        let f = source("ts", "function handleClick() {}");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn detects_python_ellipsis_stub() {
        let d = EmptyBodyDetector::new();
        let f = source("py", "def process():\n    ...\n");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
    }
}
