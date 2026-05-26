use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects invalid or conflicting `syntax_version` declarations in Vox source files.
///
/// Vox files may declare their expected syntax version via a header comment such as
/// `// syntax_version = "0.5"`. This detector flags:
/// - Versions not matching the canonical `\d+\.\d+` format
/// - Multiple `syntax_version` declarations with different values in the same file
pub struct SyntaxVersionDetector {
    /// Matches `// syntax_version = "X.Y"` or `# syntax_version = "X.Y"`
    version_decl: Regex,
    /// Validates the canonical semver-like version format `\d+\.\d+`
    valid_version: Regex,
    supported_langs: Vec<Language>,
}

impl Default for SyntaxVersionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxVersionDetector {
    pub fn new() -> Self {
        Self {
            version_decl: Regex::new(r#"(?://|#)\s*syntax_version\s*=\s*"([^"]*)""#)
                .expect("valid regex"),
            valid_version: Regex::new(r"^\d+\.\d+$").expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for SyntaxVersionDetector {
    fn id(&self) -> &'static str {
        "syntax/version-mismatch"
    }

    fn name(&self) -> &'static str {
        "Syntax Version Mismatch Detector"
    }

    fn description(&self) -> &'static str {
        "Detects Vox source files with invalid `syntax_version` header format or conflicting \
        version declarations."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::SYNTAX_VERSION_MISMATCH)
    }

    fn explain(&self) -> &'static str {
        "Vox source files may declare `// syntax_version = \"X.Y\"` at the top; the version must be in `\\d+\\.\\d+` format and must not conflict if declared more than once."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION 1 — invalid version format (must be digits.digits)\n\
             // syntax_version = \"v0.6\"   // bad: has 'v' prefix\n\
             // syntax_version = \"latest\" // bad: not a version number\n\
             \n\
             // VIOLATION 2 — conflicting declarations in the same file\n\
             // syntax_version = \"0.5\"\n\
             // ... (more code) ...\n\
             // syntax_version = \"0.6\"    // bad: conflicts with first declaration\n\
             \n\
             // FIX — single declaration at the top with correct format\n\
             // syntax_version = \"0.6\"\n\
             fn my_function() { ... }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox {
            return vec![];
        }

        let mut findings = Vec::new();
        let mut seen_versions: Vec<(usize, String)> = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if let Some(caps) = self.version_decl.captures(line) {
                let version = caps.get(1).map_or("", |m| m.as_str());
                let m = self.version_decl.find(line).unwrap();

                // Check for invalid format
                if !self.valid_version.is_match(version) {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: line_num,
                        column: m.start() + 1,
                        message: format!(
                            "`syntax_version = \"{version}\"` is not in the required `MAJOR.MINOR` format (e.g. `\"0.5\"`)."
                        ),
                        suggestion: Some(
                            "Use a version string matching `\\d+\\.\\d+`, e.g. `\"0.5\"`.".to_string(),
                        ),
                        alternatives: vec![],
                        rationale: Some(
                            "The `syntax_version` header must be in `MAJOR.MINOR` format so tooling \
                            can parse and compare it against the workspace Vox toolchain version.".into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::High),
                        evidence: None,
                    });
                } else {
                    // Valid format — check for conflicts with previously seen versions
                    for &(prev_line, ref prev_ver) in &seen_versions {
                        if prev_ver != version {
                            findings.push(Finding {
                                rule_id: self.id().to_string(),
                                diagnostic_id: self.diagnostic_id().map(str::to_string),
                                rule_name: self.name().to_string(),
                                severity: Severity::Warning,
                                file: file.path.clone(),
                                line: line_num,
                                column: m.start() + 1,
                                message: format!(
                                    "Conflicting `syntax_version` declarations: `\"{prev_ver}\"` (line {prev_line}) vs `\"{version}\"` (line {line_num})."
                                ),
                                suggestion: Some(
                                    "Remove one of the `syntax_version` declarations or ensure they agree.".to_string(),
                                ),
                                alternatives: vec![],
                                rationale: Some(
                                    "A file must declare a single consistent `syntax_version`. \
                                    Conflicting declarations indicate a merge error or copy-paste mistake.".into(),
                                ),
                                context: file.context_around(line_num, 2),
                                confidence: Some(FindingConfidence::High),
                                evidence: None,
                            });
                        }
                    }
                    seen_versions.push((line_num, version.to_string()));
                }
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
    fn flags_invalid_version_format() {
        let d = SyntaxVersionDetector::new();
        let f = source("// syntax_version = \"0.5beta\"\nfn foo() {}");
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should flag invalid version format");
        assert!(findings[0].message.contains("0.5beta"));
    }

    #[test]
    fn ignores_valid_version() {
        let d = SyntaxVersionDetector::new();
        let f = source("// syntax_version = \"0.5\"\nfn foo() {}");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "valid version format should not fire");
    }

    #[test]
    fn flags_conflicting_versions() {
        let d = SyntaxVersionDetector::new();
        let code = "// syntax_version = \"0.4\"\n// some comment\n// syntax_version = \"0.5\"\nfn foo() {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag conflicting version declarations"
        );
        assert!(findings[0].message.contains("Conflicting"));
    }

    #[test]
    fn ignores_duplicate_same_version() {
        let d = SyntaxVersionDetector::new();
        let code = "// syntax_version = \"0.5\"\n// syntax_version = \"0.5\"\nfn foo() {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "identical duplicate declarations should not fire"
        );
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = SyntaxVersionDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            "// syntax_version = \"0.5beta\"".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "rust files should be ignored");
    }
}
