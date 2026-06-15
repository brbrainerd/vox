//! Flags touch-test anti-patterns inside `#[test]` functions: tests that assert
//! nothing, tautological self-compares (`assert_eq!(x, x)`), and shallow-only
//! assertions (`.is_ok()`/`.is_some()`/`!is_empty()`) that pin no value or variant.
//! These are the "useless touch" tests the semantic-coverage initiative rejects.
//!
//! Exemptions (to avoid false positives that would wrongly block legitimate tests):
//! - `#[should_panic]` tests need no explicit assertion — the panic *is* the assertion.
//! - `fn` signatures returning a `Result` use `?` to signal failure, so an absent
//!   `assert!` is not a touch test.
//!
//! KNOWN LIMITATION (recall gap, not a precision gap): assertion macros are classified
//! per *physical line*. A multi-line assertion such as an `assert_eq!(` whose arguments
//! span several lines is only inspected line-by-line, so shallow/tautological forms that
//! are split across lines may be under-detected. This is acceptable because it can only
//! cause false negatives (a weak test slips through), never false positives (a good test
//! is never wrongly flagged on this account).

use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detector for weak / touch tests.
pub struct WeakTestDetector;

impl WeakTestDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WeakTestDetector {
    fn default() -> Self {
        Self::new()
    }
}

const SHALLOW: &[&str] = &[
    ".is_ok()",
    ".is_some()",
    ".is_err()",
    ".is_none()",
    ".is_empty()",
];

impl DetectionRule for WeakTestDetector {
    fn id(&self) -> &'static str {
        "weak_test"
    }
    fn name(&self) -> &'static str {
        "Weak or touch test"
    }
    fn description(&self) -> &'static str {
        "Detects #[test] functions that assert nothing, assert a tautology, or only use \
         shallow .is_ok()/.is_some() checks without pinning a value or variant."
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let lines = &file.lines;
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let t = lines[i].trim_start();
            if t.starts_with("#[test]") || t.contains("#[tokio::test]") {
                let (start, end) = fn_body_range(lines, i);
                let fn_sig = lines[start].trim().to_string();
                // FIX 1: a `#[should_panic]` test (attribute appears on any line
                // between the `#[test]` line and the `fn` line) is intentionally
                // assertion-free — the panic is the assertion. Suppress only the
                // no-assertion finding; tautology/shallow checks still apply.
                let should_panic = (i..start).any(|k| lines[k].contains("should_panic"));
                // FIX 2: a test whose `fn` signature returns a `Result` uses `?`
                // for failure, so an absent `assert!` is not a touch test.
                let returns_result = fn_sig.contains("->") && fn_sig.contains("Result");
                let assertion_optional = should_panic || returns_result;
                let asserts: Vec<String> = (start..end)
                    .map(|k| lines[k].trim().to_string())
                    .filter(|l| l.contains("assert") || l.contains("panic!"))
                    .collect();
                if asserts.is_empty() && !assertion_optional {
                    out.push(self.finding(
                        file,
                        start,
                        Severity::Warning,
                        format!("test has NO assertion (touch test): {fn_sig}"),
                    ));
                } else {
                    for a in &asserts {
                        if is_self_compare(a) {
                            out.push(self.finding(
                                file,
                                start,
                                Severity::Warning,
                                format!("tautological self-compare assertion: {a}"),
                            ));
                        }
                    }
                    // Guard against the empty-asserts case: an exempted test
                    // (`#[should_panic]` / Result-returning) with no asserts must
                    // not trip `all()`-over-empty == true into a false "only shallow".
                    let only_shallow = !asserts.is_empty()
                        && asserts.iter().all(|a| {
                            SHALLOW.iter().any(|s| a.contains(s)) && !a.contains("assert_eq!")
                        });
                    if only_shallow {
                        out.push(self.finding(
                            file,
                            start,
                            Severity::Info,
                            format!(
                                "only shallow .is_ok()/.is_some() assertions — pin a value or variant: {fn_sig}"
                            ),
                        ));
                    }
                }
                i = end;
                continue;
            }
            i += 1;
        }
        out
    }
}

impl WeakTestDetector {
    fn finding(
        &self,
        file: &SourceFile,
        line0: usize,
        severity: Severity,
        message: String,
    ) -> Finding {
        let line = line0 + 1;
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: None,
            rule_name: self.name().to_string(),
            severity,
            file: file.path.clone(),
            line,
            column: 0,
            message,
            suggestion: Some(
                "Assert a specific output value, error variant, or invariant — not just that code ran."
                    .to_string(),
            ),
            alternatives: vec![],
            rationale: None,
            context: file.context_around(line, 2),
            confidence: None,
            evidence: None,
        }
    }
}

fn fn_body_range(lines: &[String], test_attr: usize) -> (usize, usize) {
    let mut j = test_attr;
    while j < lines.len() && !lines[j].contains("fn ") {
        j += 1;
    }
    let start = j.min(lines.len().saturating_sub(1));
    let mut depth: i32 = 0;
    let mut seen = false;
    let mut k = start;
    while k < lines.len() {
        // FIX 3: count braces on a code-only view of the line — string/char
        // literal contents and trailing line comments are stripped so that a
        // `let s = "}";` or `// }` does not falsely decrement depth and
        // truncate the body early (which would cause a false "NO assertion").
        let code = strip_literals_and_comments(&lines[k]);
        depth += code.matches('{').count() as i32;
        if depth > 0 {
            seen = true;
        }
        depth -= code.matches('}').count() as i32;
        if seen && depth <= 0 {
            return (start, (k + 1).min(lines.len()));
        }
        k += 1;
    }
    (start, lines.len())
}

/// Returns a copy of `line` with the *contents* of string literals (`"..."`),
/// char literals (`'.'`), and any trailing line comment (`//...`) removed, so the
/// result can be safely scanned for structural braces. Delimiters are preserved
/// (only their inner contents are blanked) and backslash escapes inside string
/// and char literals are honored, so an escaped quote does not prematurely close
/// the literal. Only used for brace counting — never for assertion classification.
fn strip_literals_and_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Trailing line comment: drop the rest of the line.
            '/' if chars.peek() == Some(&'/') => break,
            // String literal: keep the quotes, blank the contents.
            '"' => {
                out.push('"');
                while let Some(s) = chars.next() {
                    if s == '\\' {
                        // Skip the escaped character (e.g. \" or \\).
                        chars.next();
                    } else if s == '"' {
                        out.push('"');
                        break;
                    }
                }
            }
            // Char literal: keep the quotes, blank the contents.
            '\'' => {
                out.push('\'');
                while let Some(s) = chars.next() {
                    if s == '\\' {
                        chars.next();
                    } else if s == '\'' {
                        out.push('\'');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn is_self_compare(a: &str) -> bool {
    if let Some((_, rest)) = a.split_once("assert_eq!(") {
        let inner = rest.trim_end().trim_end_matches(';').trim_end_matches(')');
        if let Some((l, r)) = split_top_comma(inner) {
            return l.trim() == r.trim() && !l.trim().is_empty();
        }
    }
    false
}

fn split_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut depth: i32 = 0;
    for (idx, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return Some((&s[..idx], &s[idx + 1..])),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn findings(src: &str) -> Vec<Finding> {
        let f = SourceFile::new(PathBuf::from("t.rs"), src.to_string());
        WeakTestDetector::new().detect(&f, None)
    }

    #[test]
    fn flags_test_with_no_assertion() {
        let src = "#[test]\nfn t() {\n    let _ = compute(1);\n}\n";
        assert!(
            findings(src)
                .iter()
                .any(|f| f.message.contains("NO assertion")),
            "a #[test] with no assert must be flagged"
        );
    }

    #[test]
    fn flags_self_compare_literal() {
        let src = "#[test]\nfn t() {\n    assert_eq!(3, 3);\n}\n";
        assert!(findings(src).iter().any(|f| f.message.contains("tautolog")));
    }

    #[test]
    fn flags_is_ok_only_assertion() {
        let src = "#[test]\nfn t() {\n    assert!(run().is_ok());\n}\n";
        assert!(findings(src).iter().any(|f| f.message.contains("shallow")));
    }

    #[test]
    fn does_not_flag_real_behavioral_assertion() {
        let src = "#[test]\nfn t() {\n    assert_eq!(compute(2), 4);\n}\n";
        assert!(
            findings(src).is_empty(),
            "a value-pinning assert must NOT be flagged, got: {:?}",
            findings(src)
        );
    }

    #[test]
    fn does_not_flag_should_panic_without_assert() {
        let src = "#[test]\n#[should_panic]\nfn t() {\n    parse(\"bad\");\n}\n";
        assert!(
            findings(src).is_empty(),
            "a #[should_panic] test needs no explicit assert, got: {:?}",
            findings(src)
        );
    }

    #[test]
    fn does_not_flag_result_returning_test() {
        let src = "#[test]\nfn t() -> anyhow::Result<()> {\n    decode(&v)?;\n    Ok(())\n}\n";
        assert!(
            findings(src).is_empty(),
            "a Result-returning test uses `?` for failure, got: {:?}",
            findings(src)
        );
    }

    #[test]
    fn brace_in_string_literal_does_not_truncate_body() {
        let src = "#[test]\nfn t() {\n    let s = \"}\";\n    assert_eq!(real(), 42);\n}\n";
        assert!(
            !findings(src)
                .iter()
                .any(|f| f.message.contains("NO assertion")),
            "a `}}` inside a string literal must not truncate the body, got: {:?}",
            findings(src)
        );
    }

    #[test]
    fn comment_brace_does_not_truncate_body() {
        let src =
            "#[test]\nfn t() {\n    // closing } in a comment\n    assert_eq!(real(), 42);\n}\n";
        assert!(
            !findings(src)
                .iter()
                .any(|f| f.message.contains("NO assertion")),
            "a `}}` inside a line comment must not truncate the body, got: {:?}",
            findings(src)
        );
    }
}
