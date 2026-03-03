use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detects "Sprawl" — unorganized directory structures, excessive file counts, or forbidden generic names.
pub struct SprawlDetector {
    pub max_files_per_dir: usize,
    pub forbidden_names: Vec<String>,
}

impl Default for SprawlDetector {
    fn default() -> Self {
        Self {
            max_files_per_dir: 20,
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

    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript, Language::Python, Language::Vox]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 1. Check for forbidden generic names
        let file_name = file.path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if self.forbidden_names.contains(&file_name.to_string()) {
            findings.push(Finding {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "Generic file name '{}' is forbidden. Use domain-specific naming.",
                    file_name
                ),
                suggestion: Some(format!("Rename '{}' to reflect its specific domain (e.g., 'git_ops.rs').", file_name)),
                context: String::new(),
            });
        }

        // 2. Directory count check (This is technically checked by the Engine/Scanner,
        // but as a rule, we can flag files sitting in over-populated directories).
        if let Some(parent) = file.path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let count = entries.count();
                if count > self.max_files_per_dir {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
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
                        suggestion: Some("Group these files into logical sub-directories (feature-slices).".to_string()),
                        context: String::new(),
                    });
                }
            }
        }

        findings
    }
}
