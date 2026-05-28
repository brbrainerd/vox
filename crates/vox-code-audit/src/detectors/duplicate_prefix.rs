use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects identifiers that contain a duplicated prefix segment, e.g. `user_user_id`.
pub struct DuplicatePrefixDetector {
    /// Matches word-boundary-delimited identifiers with underscores
    ident_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for DuplicatePrefixDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DuplicatePrefixDetector {
    pub fn new() -> Self {
        Self {
            ident_pattern: Regex::new(r"\b(\w+(?:_\w+)+)\b").expect("valid regex"),
            supported_langs: vec![Language::Vox, Language::Rust, Language::TypeScript],
        }
    }

    /// Returns true if the identifier has a duplicated first segment, e.g. `user_user_id`.
    fn has_duplicate_prefix(ident: &str) -> bool {
        let parts: Vec<&str> = ident.splitn(3, '_').collect();
        // Need at least two segments: prefix_prefix[_rest]
        if parts.len() < 2 {
            return false;
        }
        parts[0] == parts[1]
    }
}

impl DetectionRule for DuplicatePrefixDetector {
    fn id(&self) -> &'static str {
        "style/duplicate-prefix-name"
    }

    fn name(&self) -> &'static str {
        "Duplicate Prefix Name Detector"
    }

    fn description(&self) -> &'static str {
        "Detects identifiers with a duplicated prefix segment such as `user_user_id`, likely a copy-paste error."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::STYLE_DUPLICATE_PREFIX_NAME)
    }

    fn explain(&self) -> &'static str {
        "Identifiers like `user_user_id` or `tasks_tasks` typically result from copy-paste errors \
        where a prefix was duplicated. Review the identifier and remove the duplicate segment. \
        In rare cases (e.g. a join table FK), the repetition may be intentional.\n\n\
        Bad:  let user_user_id = ...;\n\
        Good: let user_id = ...;"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — identifiers with duplicated prefix segments\n\
             let user_user_id = req.user.id;   // bad: user_user_id\n\
             let order_order_ref = order.ref;  // bad: order_order_ref\n\
             \n\
             // FIX — remove the duplicate prefix\n\
             let user_id   = req.user.id;\n\
             let order_ref = order.ref;",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
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

            for m in self.ident_pattern.find_iter(line) {
                let matched = m.as_str();
                if !Self::has_duplicate_prefix(matched) {
                    continue;
                }
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: format!(
                        "Identifier `{matched}` contains a duplicated prefix segment — likely a copy-paste error."
                    ),
                    suggestion: Some(
                        "Remove the duplicate prefix: `user_user_id` → `user_id`.".into(),
                    ),
                    alternatives: vec![
                        "Keep as-is if intentional (e.g., join table foreign key)".into(),
                    ],
                    rationale: None,
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{lang}")), code.to_string())
    }

    #[test]
    fn fires_on_user_user_id() {
        let d = DuplicatePrefixDetector::new();
        let f = source("rs", "let user_user_id = 42;");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on user_user_id");
        assert!(findings[0].message.contains("user_user_id"));
    }

    #[test]
    fn does_not_fire_on_user_id() {
        let d = DuplicatePrefixDetector::new();
        let f = source("rs", "let user_id = 42;");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "user_id should not fire");
    }

    #[test]
    fn fires_on_tasks_tasks() {
        let d = DuplicatePrefixDetector::new();
        let f = source("vox", "let tasks_tasks = [];");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on tasks_tasks");
    }

    #[test]
    fn skips_comment_lines() {
        let d = DuplicatePrefixDetector::new();
        let f = source("rs", "// user_user_id is bad style");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should not fire");
    }
}
