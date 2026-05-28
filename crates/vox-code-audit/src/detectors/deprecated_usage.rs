use crate::rule_pack_detector::{RulePackDetector, make_finding, pack_rule};
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use vox_rule_pack::CompiledRule;

/// Detects `@deprecated` annotations in Vox files and raw JSX leakage (e.g. `className=`).
///
/// Patterns are sourced from the embedded rule pack (`deprecated-usage`, `raw-jsx-leakage`).
/// The JSX rule iterates all matches per line and provides dynamic per-attribute suggestions.
pub struct DeprecatedUsageDetector {
    deprecated_detector: RulePackDetector,
    jsx_rule: &'static CompiledRule,
}

impl Default for DeprecatedUsageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DeprecatedUsageDetector {
    pub fn new() -> Self {
        Self {
            deprecated_detector: RulePackDetector::for_ids(&["deprecated-usage"]),
            jsx_rule: pack_rule("raw-jsx-leakage"),
        }
    }
}

impl DetectionRule for DeprecatedUsageDetector {
    fn id(&self) -> &'static str {
        "deprecated-usage"
    }

    fn name(&self) -> &'static str {
        "Deprecated Usage Detector"
    }

    fn description(&self) -> &'static str {
        "Flags @deprecated annotations and raw JSX attribute leakage in Vox source"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION 1 — @deprecated annotation left in shipped code\n\
             @deprecated\n\
             fn old_payment_flow(amount: f64) { ... }\n\
             \n\
             // VIOLATION 2 — raw JSX attributes leak into Vox source\n\
             <div className=\"card\" onClick={handler}>\n\
             \n\
             // FIX 2 — use Vox-native attribute syntax\n\
             <div class=\"card\" on:click={handler}>",
        )
    }

    fn languages(&self) -> &[Language] {
        &[Language::Vox]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = self.deprecated_detector.detect_file(file);

        // JSX: find every occurrence per line (a line may have several JSX attributes).
        let re = self.jsx_rule.regex();
        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            for mat in re.find_iter(line) {
                let attr = mat.as_str().trim_end_matches('=');
                let mut vox_attr = attr.to_lowercase();
                if vox_attr.starts_with("on") {
                    vox_attr = format!("on:{}", &vox_attr[2..]);
                }
                if vox_attr == "classname" {
                    vox_attr = "class".to_string();
                }
                let mut f = make_finding(self.jsx_rule, file, line_num);
                f.message = format!("Raw JSX '{}' leaks into Vox source (Item 16).", attr);
                f.suggestion = Some(format!(
                    "Use Vox-native syntax: '{}=' instead of '{}='.",
                    vox_attr, attr
                ));
                findings.push(f);
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn detects_deprecated_annotation() {
        let d = DeprecatedUsageDetector::new();
        let f = source("@deprecated\nfn old_stuff():\n    pass");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "deprecated-usage");
    }

    #[test]
    fn detects_jsx_leakage() {
        let d = DeprecatedUsageDetector::new();
        let f = source("<div className=\"hello\" onClick={handler}>");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule_id, "raw-jsx-leakage");
        assert_eq!(
            findings[0].suggestion.as_deref(),
            Some("Use Vox-native syntax: 'class=' instead of 'className='.")
        );
    }

    #[test]
    fn ignores_pure_annotation() {
        let d = DeprecatedUsageDetector::new();
        let f = source("@pure\nfn clean():\n    pass");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }
}
