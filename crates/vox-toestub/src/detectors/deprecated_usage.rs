use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects the presence of `@deprecated` annotations in Vox files.
///
/// Reminds the developer to remove obsolete code.
pub struct DeprecatedUsageDetector {
    deprecated_re: Regex,
}

impl Default for DeprecatedUsageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DeprecatedUsageDetector {
    pub fn new() -> Self {
        Self {
            deprecated_re: Regex::new(r"^\s*@deprecated").expect("valid"),
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
        "Flags the presence of @deprecated annotations to encourage removing obsolete code"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &[Language::Vox]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            if self.deprecated_re.is_match(line) {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: i + 1,
                    column: 0,
                    message: "Found @deprecated annotation. Consider removing this obsolete code."
                        .to_string(),
                    suggestion: Some(
                        "Refactor dependents and remove this deprecated item.".to_string(),
                    ),
                    context: file.context_around(i + 1, 1),
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

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn detects_deprecated_annotation() {
        let d = DeprecatedUsageDetector::new();
        let f = source("@deprecated\nfn old_stuff():\n    pass");
        let findings = d.detect(&f);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "deprecated-usage");
    }

    #[test]
    fn ignores_pure_annotation() {
        let d = DeprecatedUsageDetector::new();
        let f = source("@pure\nfn clean():\n    pass");
        let findings = d.detect(&f);
        assert!(findings.is_empty());
    }
}
