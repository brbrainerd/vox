//! Post-task quality validation using TOESTUB.
//!
//! This module is only compiled when the `toestub-gate` feature is enabled.
//! It runs the TOESTUB analysis engine on files that an agent just modified,
//! checking for AI coding anti-patterns before the task is considered complete.

use std::path::PathBuf;

use vox_toestub::{Severity, ToestubConfig, ToestubEngine};

use crate::types::AgentTask;
use std::process::Command;

/// Run TOESTUB validation on the files in a completed task's manifest.
///
/// Returns the number of findings at or above the `error` severity level.
/// If the count is > 0, the task should be considered failed (quality gate not passed).
pub fn post_task_validate(task: &AgentTask) -> ValidationResult {
    let write_files: Vec<PathBuf> = task.write_files().into_iter().cloned().collect();

    if write_files.is_empty() {
        return ValidationResult {
            passed: true,
            error_count: 0,
            warning_count: 0,
            report: String::new(),
        };
    }

    // Build TOESTUB config scoped to just the task's files
    let config = ToestubConfig {
        roots: write_files
            .iter()
            .filter_map(|p| p.parent().map(|pp| pp.to_path_buf()))
            .collect(),
        min_severity: Severity::Warning,
        suggest_fixes: true,
        ..Default::default()
    };

    let engine = ToestubEngine::new(config);
    let (result, output) = engine.run_and_report();
    let summary = result.summary();

    let mut passed = summary.error == 0 && summary.critical == 0;
    let mut combined_report = if !passed { output } else { String::new() };
    let mut total_errors = summary.error + summary.critical;

    // LSP Integration
    for file_path in &write_files {
        if file_path.extension().and_then(|e| e.to_str()) == Some("vox") {
            if let Ok(text) = std::fs::read_to_string(file_path) {
                let diagnostics = vox_lsp::validate_document(&text);
                let errors: Vec<_> = diagnostics
                    .into_iter()
                    .filter(|d| {
                        matches!(
                            d.severity,
                            Some(tower_lsp_server::ls_types::DiagnosticSeverity::ERROR)
                        )
                    })
                    .collect();

                if !errors.is_empty() {
                    passed = false;
                    total_errors += errors.len();
                    combined_report
                        .push_str(&format!("\nLSP Errors in {}:\n", file_path.display()));
                    for e in errors {
                        combined_report.push_str(&format!(
                            "- line {}: {}\n",
                            e.range.start.line + 1,
                            e.message
                        ));
                    }
                }
            }
        }
    }

    // Cargo Test failures (if touching Rust source code)
    let touches_rust = write_files.iter().any(|p| {
        p.extension().and_then(|e| e.to_str()) == Some("rs")
            || p.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml")
    });
    if touches_rust {
        let output = Command::new("cargo")
            .arg("test")
            .arg("--workspace")
            .arg("--")
            .arg("--test-threads=1")
            .output();

        if let Ok(cmd_out) = output {
            if !cmd_out.status.success() {
                passed = false;
                total_errors += 1;
                combined_report.push_str("\nCargo test failed:\n");
                combined_report.push_str(&String::from_utf8_lossy(&cmd_out.stderr));
                combined_report.push_str(&String::from_utf8_lossy(&cmd_out.stdout));
            }
        } else {
            passed = false;
            total_errors += 1;
            combined_report
                .push_str("\ncargo test failed to execute (cargo not found or execution error).\n");
        }
    }

    ValidationResult {
        passed,
        error_count: total_errors,
        warning_count: summary.warning,
        report: combined_report,
    }
}

/// Check whether a validation result passes the quality gate.
pub fn quality_gate(result: &ValidationResult) -> bool {
    result.passed
}

/// Result of a post-task TOESTUB validation.
#[derive(Debug)]
pub struct ValidationResult {
    /// Whether the quality gate passed (no errors or critical findings).
    pub passed: bool,
    /// Number of error-level or critical findings.
    pub error_count: usize,
    /// Number of warning-level findings.
    pub warning_count: usize,
    /// Formatted report (only populated if gate failed).
    pub report: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TaskId, TaskPriority};

    #[test]
    fn empty_manifest_passes() {
        let task = AgentTask::new(
            TaskId(1),
            "no-op task",
            TaskPriority::Normal,
            vec![], // no files
        );
        let result = post_task_validate(&task);
        assert!(result.passed);
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn quality_gate_logic() {
        let pass = ValidationResult {
            passed: true,
            error_count: 0,
            warning_count: 2,
            report: String::new(),
        };
        assert!(quality_gate(&pass));

        let fail = ValidationResult {
            passed: false,
            error_count: 1,
            warning_count: 0,
            report: "found stub".to_string(),
        };
        assert!(!quality_gate(&fail));
    }
}
