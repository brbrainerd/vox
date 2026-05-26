use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use std::fs;

/// Detects workspace drift in `Cargo.toml` files:
/// 1. Sub-crates using `path = ` instead of `workspace = true`.
/// 2. Root `Cargo.toml` missing definitions for physical `crates/*` directories.
pub struct WorkspaceDriftDetector;

impl Default for WorkspaceDriftDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceDriftDetector {
    pub fn new() -> Self {
        Self
    }
}

impl DetectionRule for WorkspaceDriftDetector {
    fn id(&self) -> &'static str {
        "arch/workspace_drift"
    }

    fn name(&self) -> &'static str {
        "Workspace Drift Detector"
    }

    fn description(&self) -> &'static str {
        "Enforces `workspace = true` inheritance for all sub-crate dependencies (path and version) and detects unregistered orphan crates in the workspace root."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "# VIOLATION — sub-crate Cargo.toml using path/version directly (workspace drift)\n\
             [dependencies]\n\
             serde = { version = \"1\", features = [\"derive\"] }  # bad: duplicates root version\n\
             tokio = { path = \"../../tokio\" }                   # bad: use workspace = true\n\
             \n\
             # FIX — inherit from workspace\n\
             [dependencies]\n\
             serde = { workspace = true }\n\
             tokio = { workspace = true }",
        )
    }

    fn languages(&self) -> &[Language] {
        // We evaluate Cargo.toml layout files in the context of TOESTUB
        &[Language::Rust, Language::Vox, Language::TypeScript]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let file_name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if file_name != "Cargo.toml" {
            return findings;
        }

        let is_root = file.content.contains("[workspace.dependencies]")
            || file.content.contains("[workspace]");

        if !is_root {
            // Check for sprawl in sub-crates
            for (i, line) in file.content.lines().enumerate() {
                let trimmed = line.trim();
                // If it's a comment, skip
                if trimmed.starts_with('#') {
                    continue;
                }

                // Exception: workspace-hack is allowed to have its own versions as a build-bridge.
                if trimmed.contains("workspace-hack") {
                    continue;
                }

                // 1. Detect path = ... sprawl
                if trimmed.contains("path =") && trimmed.contains('{') && trimmed.contains('}') {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: None,
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        file: file.path.clone(),
                        line: i + 1,
                        column: 0,
                        message: "Sub-crate dependency specifies `path =` inline instead of inheriting from workspace.".to_string(),
                        suggestion: Some("Add the dependency to the root Cargo.toml [workspace.dependencies] and use `{ workspace = true }` here.".to_string()),
                        alternatives: vec![],
                        rationale: None,
                        context: trimmed.to_string(),
                        confidence: None,
                        evidence: None,
                    });
                }

                // 2. Detect version = "..." sprawl
                // This catches `uuid = { version = "1.0" }` or `version = "0.1.0"` (if not .workspace)
                if trimmed.contains("version = \"") || (trimmed.contains("version = '")) {
                    // Allow [package] version if it's not marked .workspace?
                    // No, AGENTS.md implies we use version.workspace = true everywhere.
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: None,
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        file: file.path.clone(),
                        line: i + 1,
                        column: 0,
                        message: "Sub-crate dependency specifies an explicit version string instead of inheriting from workspace.".to_string(),
                        suggestion: Some("Add the version to root Cargo.toml [workspace.dependencies] and use `{ workspace = true }` or `version.workspace = true` here.".to_string()),
                        alternatives: vec![],
                        rationale: None,
                        context: trimmed.to_string(),
                        confidence: None,
                        evidence: None,
                    });
                }

                // 3. Detect edition drift: enforce Rust 2024
                if trimmed.starts_with("edition =") {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: None,
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        file: file.path.clone(),
                        line: i + 1,
                        column: 0,
                        message: "Sub-crate specifies an explicit edition instead of inheriting from workspace.".to_string(),
                        suggestion: Some("Use `edition.workspace = true` and define `edition = \"2024\"` in the root Cargo.toml.".to_string()),
                        alternatives: vec![],
                        rationale: None,
                        context: trimmed.to_string(),
                        confidence: None,
                        evidence: None,
                    });
                }
            }
        } else {
            // It's the root Cargo.toml. Check for orphan crates in the `crates/` directory.
            if let Some(parent) = file.path.parent() {
                let crates_dir = parent.join("crates");
                if crates_dir.is_dir()
                    && let Ok(entries) = fs::read_dir(crates_dir)
                {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            // Check if this dir has a Cargo.toml and is not registered in the root Cargo.toml
                            let crate_cargo = path.join("Cargo.toml");
                            if crate_cargo.is_file()
                                && let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
                            {
                                // Simple string search for the crate name in the root Cargo.toml
                                if !file.content.contains(dir_name) {
                                    findings.push(Finding {
                                                rule_id: self.id().to_string(),
                                                diagnostic_id: None,
                                                rule_name: self.name().to_string(),
                                                severity: self.severity(),
                                                file: file.path.clone(),
                                                line: 1, // Global to the file
                                                column: 0,
                                                message: format!("Orphan crate detected: `crates/{}` is not registered in root Cargo.toml", dir_name),
                                                suggestion: Some(format!("Add `{x} = {{ path = \"crates/{x}\" }}` to [workspace.dependencies]", x = dir_name)),
                                                alternatives: vec![],
                                                rationale: None,
                                                context: String::new(),
                                                confidence: None,
                                                evidence: None,
                                            });
                                }
                            }
                        }
                    }
                }
            }

            // Enforce Rust 2024 in the root workspace
            if file.content.contains("edition = \"2021\"")
                || file.content.contains("edition = '2021'")
                || !file.content.contains("edition = \"2024\"")
            {
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: None,
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: 1,
                    column: 0,
                    message: "Root Cargo.toml must specify `edition = \"2024\"`.".to_string(),
                    suggestion: Some(
                        "Change edition to \"2024\" in the `[workspace.package]` section."
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
