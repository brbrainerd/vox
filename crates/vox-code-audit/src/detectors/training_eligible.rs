use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `training_eligible: true` files that import from archive/deprecated/legacy paths.
///
/// When a file is marked `training_eligible: true` in its frontmatter, its imports should
/// not pull from paths that are known to be ineligible for training data (e.g. archived,
/// deprecated, or legacy modules). This is a heuristic check — it flags suspicious import
/// paths rather than doing a full graph analysis.
pub struct TrainingEligibleDetector {
    /// Matches `training_eligible: true` in the first 20 lines
    eligible_marker: Regex,
    /// Matches `training_eligible: false` in the first 20 lines
    ineligible_marker: Regex,
    /// Matches suspicious import paths containing legacy/archive/deprecated segments
    suspicious_import: Regex,
    supported_langs: Vec<Language>,
}

impl Default for TrainingEligibleDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TrainingEligibleDetector {
    pub fn new() -> Self {
        Self {
            eligible_marker: Regex::new(r"training_eligible\s*:\s*true").expect("valid regex"),
            ineligible_marker: Regex::new(r"training_eligible\s*:\s*false").expect("valid regex"),
            suspicious_import: Regex::new(
                r#"(?:^|\s)(?:use|import)\s+[^\s;]*(?:[/::]archive[/::)]|[/::]deprecated[/::)]|[/::]legacy[/::)]|_legacy\b|_deprecated\b)"#,
            )
            .expect("valid regex"),
            supported_langs: vec![Language::Rust, Language::Vox, Language::TypeScript],
        }
    }
}

impl DetectionRule for TrainingEligibleDetector {
    fn id(&self) -> &'static str {
        "corpus/training-ineligible-import"
    }

    fn name(&self) -> &'static str {
        "Training Ineligible Import Detector"
    }

    fn description(&self) -> &'static str {
        "Detects files marked `training_eligible: true` that import from archive, deprecated, or \
        legacy module paths, which are likely ineligible for training data."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::CORPUS_TRAINING_INELIGIBLE_IMPORT)
    }

    fn explain(&self) -> &'static str {
        "A file marked `training_eligible: true` imports from a path that appears to be archived, deprecated, or legacy, which is likely ineligible for corpus inclusion."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — file is training_eligible but imports from a deprecated path\n\
             // training_eligible: true\n\
             use crate::legacy::payment_processor::LegacyProcessor;  // bad: legacy import\n\
             use crate::deprecated::auth::OldAuthHandler;            // bad: deprecated import\n\
             \n\
             // FIX — either update imports to current paths or mark file ineligible\n\
             // training_eligible: true\n\
             use crate::payments::processor::Processor;  // current path\n\
             use crate::auth::handler::AuthHandler;      // current path",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !matches!(
            file.language,
            Language::Rust | Language::Vox | Language::TypeScript
        ) {
            return vec![];
        }

        let lines = &file.lines;
        let n = lines.len();

        // Check the first 20 lines for training_eligible markers
        let header_end = n.min(20);
        let mut is_eligible = false;
        let mut is_explicitly_ineligible = false;

        for line in &lines[..header_end] {
            if self.ineligible_marker.is_match(line) {
                is_explicitly_ineligible = true;
                break;
            }
            if self.eligible_marker.is_match(line) {
                is_eligible = true;
            }
        }

        // If the file is explicitly ineligible or has no training_eligible: true marker, skip
        if !is_eligible || is_explicitly_ineligible {
            return vec![];
        }

        let mut findings = Vec::new();

        for (i, line) in lines.iter().enumerate() {
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

            if let Some(m) = self.suspicious_import.find(line) {
                // Extract the import path for the message
                let import_snippet = line.trim().to_string();
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: format!(
                        "File is `training_eligible: true` but imports from a potentially ineligible path: `{import_snippet}`"
                    ),
                    suggestion: Some(
                        "Remove this import or mark the file `training_eligible: false` if it \
                        transitively depends on archived/deprecated/legacy modules.".to_string(),
                    ),
                    alternatives: vec![],
                    rationale: Some(
                        "Training corpus files must not pull in content from archived, deprecated, \
                        or legacy modules, which may contain outdated patterns or proprietary data \
                        excluded from the training corpus.".into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Low),
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

    fn source_rs(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.rs"), code.to_string())
    }

    fn source_vox(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_eligible_file_importing_archive() {
        let d = TrainingEligibleDetector::new();
        let code = "// training_eligible: true\nuse crate::archive::old_stuff;";
        let f = source_rs(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag archive import in eligible file"
        );
        assert!(findings[0].message.contains("archive"));
    }

    #[test]
    fn ignores_file_with_no_training_marker() {
        let d = TrainingEligibleDetector::new();
        let code = "use crate::archive::old_stuff;";
        let f = source_rs(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "no training_eligible marker means skip"
        );
    }

    #[test]
    fn ignores_ineligible_file() {
        let d = TrainingEligibleDetector::new();
        let code = "// training_eligible: false\nuse crate::archive::old_stuff;";
        let f = source_rs(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "training_eligible: false file should be skipped"
        );
    }

    #[test]
    fn flags_eligible_file_importing_deprecated() {
        let d = TrainingEligibleDetector::new();
        let code = "// training_eligible: true\nuse crate::utils::deprecated::helper;";
        let f = source_rs(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag deprecated import in eligible file"
        );
    }

    #[test]
    fn flags_eligible_vox_file_importing_legacy() {
        let d = TrainingEligibleDetector::new();
        let code = "# training_eligible: true\nuse core/legacy/old_module";
        let f = source_vox(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag legacy import in eligible vox file"
        );
    }

    #[test]
    fn ignores_normal_imports_in_eligible_file() {
        let d = TrainingEligibleDetector::new();
        let code = "// training_eligible: true\nuse crate::core::user::User;";
        let f = source_rs(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "normal imports should not fire");
    }
}
