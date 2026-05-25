use crate::rule_pack_detector::pack_rule;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use vox_rule_pack::CompiledRule;

/// Detects stringly-typed enum patterns where a proper ADT should be used.
///
/// Catches patterns like:
///   `frame: String  // "gain" | "loss"`
///   `role: String # "user" | "assistant"`
///
/// Pattern is sourced from the embedded rule pack (`stringly-typed-enum`).
pub struct StringlyTypedEnumDetector {
    rule: &'static CompiledRule,
}

impl Default for StringlyTypedEnumDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StringlyTypedEnumDetector {
    pub fn new() -> Self {
        Self {
            rule: pack_rule("stringly-typed-enum"),
        }
    }

    /// Byte index of the first `//` line comment **outside** string / raw-string literals.
    fn first_double_slash_outside_strings(line: &str) -> Option<usize> {
        let b = line.as_bytes();
        let mut i = 0usize;
        while i + 1 < b.len() {
            if b[i] == b'r' {
                let mut j = i + 1;
                let mut n_hash = 0usize;
                while j < b.len() && b[j] == b'#' {
                    n_hash += 1;
                    j += 1;
                }
                if j < b.len() && b[j] == b'"' {
                    j += 1;
                    'raw: while j < b.len() {
                        if b[j] == b'"' {
                            if n_hash == 0 {
                                j += 1;
                                break 'raw;
                            }
                            if j + n_hash < b.len() && (1..=n_hash).all(|k| b[j + k] == b'#') {
                                j += 1 + n_hash;
                                break 'raw;
                            }
                        }
                        j += 1;
                    }
                    i = j;
                    continue;
                }
            }
            if b[i] == b'"' {
                let mut j = i + 1;
                let mut esc = false;
                while j < b.len() {
                    if esc {
                        esc = false;
                        j += 1;
                        continue;
                    }
                    match b[j] {
                        b'\\' => {
                            esc = true;
                            j += 1;
                        }
                        b'"' => {
                            j += 1;
                            break;
                        }
                        _ => j += 1,
                    }
                }
                i = j;
                continue;
            }
            if b[i] == b'/' && b[i + 1] == b'/' {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// Mask `"..."` and `r#*` literals in `line` (used on the code prefix before `//`).
    fn mask_rust_strings_and_raw(line: &str) -> String {
        let b = line.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0usize;
        while i < b.len() {
            if b[i] == b'r' {
                let mut j = i + 1;
                let mut n_hash = 0usize;
                while j < b.len() && b[j] == b'#' {
                    n_hash += 1;
                    j += 1;
                }
                if j < b.len() && b[j] == b'"' {
                    j += 1;
                    'raw: while j < b.len() {
                        if b[j] == b'"' {
                            if n_hash == 0 {
                                j += 1;
                                break 'raw;
                            }
                            if j + n_hash < b.len() && (1..=n_hash).all(|k| b[j + k] == b'#') {
                                j += 1 + n_hash;
                                break 'raw;
                            }
                        }
                        j += 1;
                    }
                    out.extend(std::iter::repeat_n(b' ', j.saturating_sub(i)));
                    i = j;
                    continue;
                }
            }
            if b[i] == b'"' {
                let start = i;
                i += 1;
                let mut esc = false;
                while i < b.len() {
                    if esc {
                        esc = false;
                        i += 1;
                        continue;
                    }
                    match b[i] {
                        b'\\' => {
                            esc = true;
                            i += 1;
                        }
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                out.extend(std::iter::repeat_n(b' ', i.saturating_sub(start)));
                continue;
            }
            out.push(b[i]);
            i += 1;
        }
        String::from_utf8(out).unwrap_or_else(|_| line.to_string())
    }

    /// Hide Rust string / raw-string **code** so `r#"…"#` fixtures do not match; keep `// …` tail intact.
    fn rust_line_for_pattern_match(line: &str) -> String {
        let split = Self::first_double_slash_outside_strings(line).unwrap_or(line.len());
        let (head, tail) = line.split_at(split);
        format!("{}{}", Self::mask_rust_strings_and_raw(head), tail)
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
        &[
            Language::Vox,
            Language::Rust,
            Language::TypeScript,
            Language::Python,
        ]
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — stringly-typed field with enum alternatives in a comment\n\
             struct OrderStatus {\n\
             \x20   status: String,  // \"pending\" | \"confirmed\" | \"shipped\" | \"delivered\"\n\
             }\n\
             \n\
             // FIX — use a proper ADT/enum\n\
             enum OrderStatus {\n\
             \x20   Pending,\n\
             \x20   Confirmed,\n\
             \x20   Shipped,\n\
             \x20   Delivered,\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let re = self.rule.regex();
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed_start = line.trim_start();
            if trimmed_start.starts_with("///")
                || trimmed_start.starts_with("//!")
                || trimmed_start.starts_with('*')
            {
                continue;
            }

            let scan_line = if file.language == Language::Rust {
                Self::rust_line_for_pattern_match(line)
            } else {
                line.to_string()
            };

            if re.is_match(&scan_line) {
                let field_name = line.trim().split(':').next().unwrap_or("field").trim();

                findings.push(Finding {
                    rule_id: "stringly-typed-enum".to_string(),
                    diagnostic_id: None,
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
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: None,
                    evidence: None,
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
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("frame"));
        assert!(findings[0].message.contains("ADT"));
    }

    #[test]
    fn detects_str_with_hash_comment() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source(r#"  role: str # "user" | "assistant""#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_string_without_enum_comment() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source("  name: String");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_proper_adt_usage() {
        let d = StringlyTypedEnumDetector::new();
        let f = vox_source("  frame: Frame");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn detects_in_rust_files_too() {
        let d = StringlyTypedEnumDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            r#"    pub role: String, // "user" | "assistant""#.to_string(),
        );
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_vox_fixture_inside_rust_raw_string() {
        let d = StringlyTypedEnumDetector::new();
        let inner = r#"  frame: String // "gain" | "loss"#;
        let line = format!("    let _ = r#\"{inner}\"#;");
        let f = SourceFile::new(PathBuf::from("detectors/tests.rs"), line);
        assert!(d.detect(&f, None).is_empty());
    }

    /// Raw string with fewer closing `#` than opening used to index past EOF (`j + n_hash == len`).
    #[test]
    fn raw_scan_does_not_panic_on_short_closing_delimiter() {
        let line = r##"let _ = r##"x"#;"##;
        let _ = StringlyTypedEnumDetector::rust_line_for_pattern_match(line);
        let _ = StringlyTypedEnumDetector::first_double_slash_outside_strings(line);
    }
}
