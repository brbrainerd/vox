use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detects poor file organization, such as too many definitions in lib.rs or pure type dumps.
pub struct FileOrganizationDetector {
    pub max_lib_defs: usize,
    pub max_type_dump_lines: usize,
}

impl Default for FileOrganizationDetector {
    fn default() -> Self {
        Self {
            max_lib_defs: 3,
            max_type_dump_lines: 100,
        }
    }
}

impl DetectionRule for FileOrganizationDetector {
    fn id(&self) -> &'static str {
        "arch/organization"
    }

    fn name(&self) -> &'static str {
        "File Organization Detector"
    }

    fn description(&self) -> &'static str {
        "Identifies structural anti-patterns like bloated lib.rs files or unorganized type dumps."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        let file_name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // 1. Check for bloated lib.rs (Rust specific)
        if file.language == Language::Rust && file_name == "lib.rs" {
            let mut def_count = 0;
            for line in &file.lines {
                let trimmed = line.trim();
                if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") ||
                   trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") ||
                   trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ") ||
                   trimmed.starts_with("pub type ") || trimmed.starts_with("type ")
                {
                    def_count += 1;
                }
            }

            if def_count > self.max_lib_defs {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: Severity::Error,
                    file: file.path.clone(),
                    line: 1,
                    column: 0,
                    message: format!(
                        "lib.rs contains {} definitions, which exceeds the limit ({}). Move these to dedicated modules.",
                        def_count, self.max_lib_defs
                    ),
                    suggestion: Some("Extract structs, enums, and traits into separate files and re-export them from lib.rs.".to_string()),
                    context: String::new(),
                });
            }
        }

        // 2. Check for "Type Dumps"
        // Files that are mostly structs/enums but quite large, unless they are named appropriately.
        if file.lines.len() > self.max_type_dump_lines {
            let mut type_line_count = 0;
            for line in &file.lines {
                let trimmed = line.trim();
                if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") ||
                   trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") ||
                   trimmed.starts_with("impl ") || trimmed.starts_with("#[derive")
                {
                    type_line_count += 1;
                }
            }

            let type_ratio = type_line_count as f32 / file.lines.len() as f32;
            let is_allowed_name = file_name.contains("types") ||
                                 file_name.contains("params") ||
                                 file_name.contains("schema") ||
                                 file_name.contains("models");

            if type_ratio > 0.7 && !is_allowed_name {
                 findings.push(Finding {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: 1,
                    column: 0,
                    message: format!(
                        "File appears to be a large type-dump ({}% types) but is not in a 'types' or 'models' module.",
                        (type_ratio * 100.0) as u32
                    ),
                    suggestion: Some("Organize types into a 'types.rs' or 'models/' directory to maintain a clean logic-to-data ratio.".to_string()),
                    context: String::new(),
                });
            }
        }

        findings
    }
}
