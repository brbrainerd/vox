use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use serde::Deserialize;
use std::path::PathBuf;

/// Verifies that files are in locations authorized by vox-schema.json.
pub struct SchemaComplianceDetector {
    /// Path to `vox-schema.json` (or compatible); [`None`] disables the rule (always clean).
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
    /// Creates a detector; pass [`None`] to skip disk I/O and emit no findings from this rule.
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

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — file placed outside its authorized path_pattern in vox-schema.json\n\
             // vox-schema.json declares:\n\
             //   \"vox-payments\": { \"path_pattern\": \"crates/vox-payments/src/**\" }\n\
             // but file is at: crates/shared/src/payment_handler.rs  ← wrong location!\n\
             \n\
             // FIX — move the file to its authorized location\n\
             // crates/vox-payments/src/payment_handler.rs  ← matches path_pattern",
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
        let schema = match self.load_schema() {
            Some(s) => s,
            None => return findings,
        };

        // Find which crate this file belongs to by checking its path
        // For Vox, crates are usually in crates/ or packages/
        let path_str = file.path.to_string_lossy().replace('\\', "/");

        let mut matched = false;

        for config in schema.crates.values() {
            // Check if the file is within the declared path_pattern of any crate
            let pattern_base = config
                .path_pattern
                .split("/**")
                .next()
                .unwrap_or(&config.path_pattern);

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
                diagnostic_id: None,
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
