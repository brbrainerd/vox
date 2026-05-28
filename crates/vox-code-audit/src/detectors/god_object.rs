use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detects "God Objects" — files or entities that are too large or have too many responsibilities.
pub struct GodObjectDetector {
    /// Soft cap for method count.
    pub max_methods: usize,
    /// Hard cap for lines.
    pub hard_max_lines: usize,
    pub warn_max_lines: usize,
    pub info_max_lines: usize,
}

impl Default for GodObjectDetector {
    fn default() -> Self {
        Self {
            hard_max_lines: 500,
            warn_max_lines: 400,
            info_max_lines: 300,
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

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — single struct with 600+ lines and 15+ methods (God Object)\n\
             pub struct AppManager {\n\
             \x20   // auth, payments, notifications, reporting, users, config ... all here\n\
             }\n\
             // (600 lines of mixed responsibilities)\n\
             \n\
             // FIX — split by responsibility into focused structs\n\
             pub struct AuthManager { ... }      // only auth\n\
             pub struct PaymentProcessor { ... } // only payments\n\
             pub struct NotificationService { ... } // only notifications",
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

        let nonblank_lines = file.lines.iter().filter(|l| !l.trim().is_empty()).count();

        // Check file size using non-blank lines with the 3-tier system.
        if nonblank_lines > self.hard_max_lines {
            findings.push(Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: None,
                rule_name: self.name().to_string(),
                severity: Severity::Error,
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "File is too large ({} non-blank lines). Hard maximum allowed is {}.",
                    nonblank_lines, self.hard_max_lines
                ),
                suggestion: Some(
                    "Break down the domain logic into sub-modules immediately.".to_string(),
                ),
                alternatives: vec![],
                rationale: None,
                context: file.context_around(1, 2),
                confidence: None,
                evidence: None,
            });
        } else if nonblank_lines > self.warn_max_lines {
            findings.push(Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: None,
                rule_name: self.name().to_string(),
                severity: Severity::Warning,
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "File is too large ({} non-blank lines/Warning Threshold >{}). Flagged for low-density MENS context.",
                    nonblank_lines, self.warn_max_lines
                ),
                suggestion: Some(
                    "Consider decomposing this file before it hits the 500-line hard block.".to_string(),
                ),
                alternatives: vec![],
                rationale: None,
                context: file.context_around(1, 2),
                confidence: None,
                evidence: None,
            });
        } else if nonblank_lines > self.info_max_lines {
            findings.push(Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: None,
                rule_name: self.name().to_string(),
                severity: Severity::Info,
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "File is growing large ({} non-blank lines / Soft Limit >{}).",
                    nonblank_lines, self.info_max_lines
                ),
                suggestion: Some(
                    "Consider trait extraction early to avoid refactoring later.".to_string(),
                ),
                alternatives: vec![],
                rationale: None,
                context: file.context_around(1, 2),
                confidence: None,
                evidence: None,
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

        if count > self.max_methods {
            // Rough heuristic for density
            findings.push(Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: None,
                rule_name: self.name().to_string(),
                severity: Severity::Error,
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "High method/entity density detected (~{} entities vs max {}).",
                    count, self.max_methods
                ),
                suggestion: Some(
                    "Consider decomposing large structs into multiple traits.".to_string(),
                ),
                alternatives: vec![],
                rationale: None,
                context: String::new(),
                confidence: None,
                evidence: None,
            });
        }

        findings
    }
}
