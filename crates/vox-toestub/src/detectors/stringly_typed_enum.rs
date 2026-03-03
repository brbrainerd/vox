use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects stringly-typed enum patterns in Vox files where a proper ADT should be used.
///
/// Catches patterns like:
///   `frame: String  // "gain" | "loss"`
///   `role: String # "user" | "assistant"`
///
/// These are a code smell: the comment describes an exhaustive set of values,
/// which means the field should use a Vox ADT (`type Frame = | Gain | Loss`)
/// instead of a bare `String` with a comment as the only type safety.
pub struct StringlyTypedEnumDetector {
    /// Matches `: String` (or `: str`) followed by a comment listing `"a" | "b"` alternatives.
    pattern: Regex,
}

impl Default for StringlyTypedEnumDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StringlyTypedEnumDetector {
    pub fn new() -> Self {
        Self {
            // Matches lines like:  `field: String  // "x" | "y"` or `field: str  # "x" | "y"`
            // The key signal is a String/str type annotation followed by a comment containing
            // quoted alternatives separated by `|`.
            pattern: Regex::new(
                r#":\s*(?:String|str)\s*,?\s*(?://|#)\s*"[^"]+"\s*\|"#,
            )
            .expect("valid stringly-typed enum regex"),
        }
    }
}

impl DetectionRule for StringlyTypedEnumDetector {
    fn id(&self) -> &'static str {
        "stringly-typed-enum"
    }
    fn name(&self) -> &'static str {
        "Stringly-Typed Enum Detector"
    }
    fn description(&self) -> &'static str {
        "Detects String fields with comments listing enum alternatives — should be a Vox ADT"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Vox, Language::Rust, Language::TypeScript, Language::Python]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if self.pattern.is_match(line) {
                // Extract the field name for a better message
                let field_name = line
                    .trim()
                    .split(':')
                    .next()
                    .unwrap_or("field")
                    .trim();

                findings.push(Finding {
                    rule_id: "stringly-typed-enum".to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: format!(
                        "'{}' uses String with a comment listing alternatives — define a Vox ADT instead",
                        field_name
                    ),
                    suggestion: Some(format!(
                        "Replace `{}: String` with a proper ADT type. For example:\n  type {} = | ... | ...\nThis enables exhaustive `match` checking and eliminates stringly-typed bugs.",
                        field_name,
                        capitalize_first(field_name)
                    )),
                    context: file.context_around(line_num, 1),
                });
            }
        }

        findings
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
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
    fn detects_string_with_pipe_comment() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source(r#"  frame: String // "gain" | "loss""#);
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("frame"));
        assert!(findings[0].message.contains("ADT"));
    }

    #[test]
    fn detects_str_with_hash_comment() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source(r#"  role: str # "user" | "assistant""#);
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_string_without_enum_comment() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source("  name: String");
        let findings = d.detect(&f);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_proper_adt_usage() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source("  frame: Frame");
        let findings = d.detect(&f);
        assert!(findings.is_empty());
    }

    #[test]
    fn detects_in_rust_files_too() {
        let d = StringlyTypedEnumDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            r#"    pub role: String, // "user" | "assistant""#.to_string(),
        );
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
    }
}
