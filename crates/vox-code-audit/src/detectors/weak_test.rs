//! Flags touch-test anti-patterns inside `#[test]` functions: tests that assert
//! nothing, tautological self-compares (`assert_eq!(x, x)`), and shallow-only
//! assertions (`.is_ok()`/`.is_some()`/`!is_empty()`) that pin no value or variant.
//! These are the "useless touch" tests the semantic-coverage initiative rejects.

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
                let asserts: Vec<String> = (start..end)
                    .map(|k| lines[k].trim().to_string())
                    .filter(|l| l.contains("assert") || l.contains("panic!"))
                    .collect();
                if asserts.is_empty() {
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
                    let only_shallow = asserts.iter().all(|a| {
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
        depth += lines[k].matches('{').count() as i32;
        if depth > 0 {
            seen = true;
        }
        depth -= lines[k].matches('}').count() as i32;
        if seen && depth <= 0 {
            return (start, (k + 1).min(lines.len()));
        }
        k += 1;
    }
    (start, lines.len())
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
}
