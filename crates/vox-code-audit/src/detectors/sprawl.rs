use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detects "Sprawl" — unorganized directory structures, excessive file counts, or forbidden generic names.
pub struct SprawlDetector {
    /// Directory entries (files + subdirs) above this count trigger a sprawl warning for that folder.
    pub max_files_per_dir: usize,
    /// Basenames that are too generic (`utils.rs`, …) and encourage unclear module boundaries.
    pub forbidden_names: Vec<String>,
}

impl Default for SprawlDetector {
    fn default() -> Self {
        Self {
            // Raised during TOESTUB remediation: busy module trees + large repo roots (many top-level files).
            max_files_per_dir: 200,
            forbidden_names: vec![
                "utils.rs".to_string(),
                "helpers.ts".to_string(),
                "misc.py".to_string(),
                "common.vox".to_string(),
            ],
        }
    }
}

impl DetectionRule for SprawlDetector {
    fn id(&self) -> &'static str {
        "arch/sprawl"
    }

    fn name(&self) -> &'static str {
        "Architectural Sprawl Detector"
    }

    fn description(&self) -> &'static str {
        "Enforces clean directory structures and prevents 'junk drawer' modules like utils.rs."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — generic 'junk drawer' module name\n\
             // src/utils.rs  ← forbidden: too generic, no clear boundary\n\
             // src/helpers.ts ← forbidden: catch-all for unrelated logic\n\
             \n\
             // FIX — name modules after what they actually do\n\
             // src/date_formatting.rs  ← clear responsibility\n\
             // src/currency_math.rs    ← clear responsibility\n\
             // src/auth_tokens.rs      ← clear responsibility",
        )
    }

    fn languages(&self) -> &[Language] {
        &[
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::Vox,
        ]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 1. Check for forbidden generic names
        let file_name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if self.forbidden_names.contains(&file_name.to_string()) {
            findings.push(Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: None,
                rule_name: self.name().to_string(),
                severity: self.severity(),
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "Generic file name '{}' is forbidden. Use domain-specific naming.",
                    file_name
                ),
                suggestion: Some(format!(
                    "Rename '{}' to reflect its specific domain (e.g., 'git_ops.rs').",
                    file_name
                )),
                alternatives: vec![],
                rationale: None,
                context: String::new(),
                confidence: None,
                evidence: None,
            });
        }

        // 2. Directory count check (This is technically checked by the Engine/Scanner,
        // but as a rule, we can flag files sitting in over-populated directories).
        if let Some(parent) = file.path.parent()
            && let Ok(entries) = std::fs::read_dir(parent)
        {
            let count = entries.count();
            if count > self.max_files_per_dir {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: None,
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: 1,
                    column: 0,
                    message: format!(
                        "Directory sprawl detected in '{}' ({} files).",
                        parent.display(),
                        count
                    ),
                    suggestion: Some(
                        "Group these files into logical sub-directories (feature-slices)."
                            .to_string(),
                    ),
                    alternatives: vec![],
                    rationale: None,
                    context: String::new(),
                    confidence: None,
                    evidence: None,
                });
            }
        }

        findings
    }
}
