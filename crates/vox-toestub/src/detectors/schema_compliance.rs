use std::path::PathBuf;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use serde::Deserialize;

/// Verifies that files are in locations authorized by vox-schema.json.
pub struct SchemaComplianceDetector {
    pub schema_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Schema {
    crates: std::collections::HashMap<String, CrateConfig>,
}

#[derive(Debug, Deserialize)]
struct CrateConfig {
    path_pattern: String,
}

impl SchemaComplianceDetector {
    pub fn new(schema_path: Option<PathBuf>) -> Self {
        Self { schema_path }
    }

    fn load_schema(&self) -> Option<Schema> {
        let path = self.schema_path.as_ref()?;
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl DetectionRule for SchemaComplianceDetector {
    fn id(&self) -> &'static str {
        "arch/schema_compliance"
    }

    fn name(&self) -> &'static str {
        "Schema Compliance Detector"
    }

    fn description(&self) -> &'static str {
        "Verifies that files are located in paths authorized by vox-schema.json."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript, Language::Python, Language::Vox]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        let schema = match self.load_schema() {
            Some(s) => s,
            None => return findings,
        };

        // Find which crate this file belongs to by checking its path
        // For Vox, crates are usually in crates/ or packages/
        let path_str = file.path.to_string_lossy().replace('\\', "/");

        let mut matched = false;

        for (_crate_name, config) in &schema.crates {
            // Check if the file is within the declared path_pattern of any crate
            let pattern_base = config.path_pattern.split("/**").next().unwrap_or(&config.path_pattern);

            // If the file path contains the crate's base path, it belongs to that crate
            if path_str.contains(pattern_base) {
                matched = true;
                break;
            }
        }

        // If it's in crates/ or packages/ but not declared in schema, or not matching any pattern
        if !matched && (path_str.contains("crates/") || path_str.contains("packages/")) {
             findings.push(Finding {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: self.severity(),
                file: file.path.clone(),
                line: 1,
                column: 0,
                message: format!(
                    "File '{}' is in a managed directory but not registered in vox-schema.json.",
                    path_str
                ),
                suggestion: Some("Register the crate in vox-schema.json or move the file to an authorized location.".to_string()),
                context: String::new(),
            });
        }

        findings
    }
}
