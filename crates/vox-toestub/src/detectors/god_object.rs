use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detects "God Objects" — files or entities that are too large or have too many responsibilities.
pub struct GodObjectDetector {
    pub max_lines: usize,
    pub max_methods: usize,
}

impl Default for GodObjectDetector {
    fn default() -> Self {
        Self {
            max_lines: 500,
            max_methods: 12,
        }
    }
}

impl DetectionRule for GodObjectDetector {
    fn id(&self) -> &'static str {
        "arch/god_object"
    }

    fn name(&self) -> &'static str {
        "God Object Detector"
    }

    fn description(&self) -> &'static str {
        "Detects files or structs that are too large and should be decomposed into traits or smaller modules."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript, Language::Python, Language::Vox]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Check file line count
        if file.lines.len() > self.max_lines {
            findings.push(Finding {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "File is too large ({} lines). Maximum allowed is {}.",
                    file.lines.len(),
                    self.max_lines
                ),
                suggestion: Some("Refactor this file into smaller sub-modules or extract logic into traits.".to_string()),
                context: file.context_around(1, 2),
            });
        }

        // Logic for method counting would require tree-sitter or regex analysis.
        // For now, we use a simple heuristic counting 'pub fn' in Rust or 'async function' in TS.
        let method_patterns = match file.language {
            Language::Rust => vec!["pub fn ", "fn ", "impl "],
            Language::TypeScript => vec!["function ", "export const ", "class "],
            _ => vec![],
        };

        let mut count = 0;
        for line in &file.lines {
            for pattern in &method_patterns {
                if line.contains(pattern) {
                    count += 1;
                    break;
                }
            }
        }

        if count > self.max_methods * 2 { // Rough heuristic for density
             findings.push(Finding {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: Severity::Warning,
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "High method/entity density detected (~{} entities).",
                    count
                ),
                suggestion: Some("Consider decomposing large structs into multiple traits.".to_string()),
                context: String::new(),
            });
        }

        findings
    }
}
