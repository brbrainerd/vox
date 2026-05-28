//! `vox doctor --project <path>` — CR-L7 project-health check.
//!
//! Discovers every `.vox` file under `project_root` and compile-checks each
//! via `vox_compiler::pipeline::check_file`. A file passes when it produces
//! zero error-severity diagnostics. Aggregate outcome is `green` if every
//! file passes, `red` otherwise.
//!
//! This is the third leg of the CR-L7 `vox new → vox deploy → vox doctor`
//! integration test. The deploy step lands a Vox project on disk; this
//! check verifies the on-disk artifact is well-typed. Pre-existing
//! environment-check `vox doctor` modes (compile-target, build-perf,
//! scope, etc.) remain available without `--project`.
//!
//! Emits one [`vox_telemetry::types::DoctorProjectCheckEvent`] per run,
//! observable through any registered telemetry sink.

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
use vox_compiler::typeck::diagnostics::{TypeckSeverity, VoxCompilerDiagnosticPayload};
use vox_telemetry::record_event;
use vox_telemetry::types::{
    DoctorProjectCheckEvent, METRIC_TYPE_DOCTOR_PROJECT_CHECK, TelemetryEvent,
};
use walkdir::WalkDir;

/// Per-file result row in the structured report.
#[derive(Debug, Serialize)]
pub struct FileResult {
    pub path: String,
    pub pass: bool,
    pub error_count: u32,
    pub warning_count: u32,
}

/// Aggregate report shape emitted as JSON when `--json` is set.
#[derive(Debug, Serialize)]
pub struct ProjectHealthReport {
    pub schema_version: u32,
    pub project_root: String,
    pub files_total: u32,
    pub files_passing: u32,
    pub files_failing: u32,
    pub outcome: String,
    pub duration_seconds: f64,
    pub failing_files: Vec<FileResult>,
}

/// Entry point invoked by `vox doctor --project <path>`.
pub async fn run(project_root: &Path, json: bool) -> Result<()> {
    let started = Instant::now();
    let canonical_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());

    if !canonical_root.exists() {
        let event = DoctorProjectCheckEvent {
            project_root: canonical_root.display().to_string(),
            files_total: 0,
            files_passing: 0,
            files_failing: 0,
            outcome: "infra_error".to_string(),
            duration_seconds: started.elapsed().as_secs_f64(),
            repository_id: None,
        };
        emit(event);
        anyhow::bail!(
            "vox doctor --project: path does not exist: {}",
            canonical_root.display()
        );
    }

    let files = discover_vox_files(&canonical_root);
    let mut per_file: Vec<FileResult> = Vec::with_capacity(files.len());
    let mut files_passing = 0u32;
    let mut files_failing = 0u32;

    for path in &files {
        let result = check_one_file(path);
        if result.pass {
            files_passing += 1;
        } else {
            files_failing += 1;
        }
        per_file.push(result);
    }

    let outcome = if files_failing == 0 { "green" } else { "red" };
    let duration_seconds = started.elapsed().as_secs_f64();

    // Telemetry event regardless of output format.
    emit(DoctorProjectCheckEvent {
        project_root: canonical_root.display().to_string(),
        files_total: files.len() as u32,
        files_passing,
        files_failing,
        outcome: outcome.to_string(),
        duration_seconds,
        repository_id: None,
    });

    // Structured output: only failing files in the report (passing files are
    // implied by `files_total - files_failing`). Keeps payload bounded on
    // large projects.
    let failing_files: Vec<FileResult> = per_file.into_iter().filter(|r| !r.pass).collect();
    let report = ProjectHealthReport {
        schema_version: 1,
        project_root: canonical_root.display().to_string(),
        files_total: files.len() as u32,
        files_passing,
        files_failing,
        outcome: outcome.to_string(),
        duration_seconds,
        failing_files,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human_report(&report);
    }

    if files_failing > 0 {
        anyhow::bail!(
            "vox doctor --project: {} of {} file(s) failed compile-check",
            files_failing,
            files.len()
        );
    }
    Ok(())
}

fn discover_vox_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir(e.path()))
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|x| x == "vox") {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    out
}

/// Skip walking into directories that aren't part of the project's Vox
/// source (build outputs, dependency caches, version control, archives).
fn is_skipped_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(
        name,
        "target" | "node_modules" | ".git" | ".cargo" | "dist" | "build" | "archive"
    )
}

fn check_one_file(path: &Path) -> FileResult {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => {
            return FileResult {
                path: path.display().to_string(),
                pass: false,
                error_count: 1,
                warning_count: 0,
            };
        }
    };
    let diags = vox_compiler::pipeline::check_file(&source, &path.to_string_lossy());
    let (error_count, warning_count) = count_severities(&diags);
    FileResult {
        path: path.display().to_string(),
        pass: error_count == 0,
        error_count,
        warning_count,
    }
}

fn count_severities(diags: &[VoxCompilerDiagnosticPayload]) -> (u32, u32) {
    let mut errors = 0u32;
    let mut warnings = 0u32;
    for d in diags {
        match d.severity {
            TypeckSeverity::Error => errors += 1,
            TypeckSeverity::Warning => warnings += 1,
        }
    }
    (errors, warnings)
}

fn emit(event: DoctorProjectCheckEvent) {
    let outer = TelemetryEvent::DoctorProjectCheck(event);
    record_event!(&outer);
    // metric_type is documented by the consumer-side sink; the constant is
    // re-exported here to keep the surface discoverable.
    let _ = METRIC_TYPE_DOCTOR_PROJECT_CHECK;
}

fn print_human_report(report: &ProjectHealthReport) {
    let status = if report.outcome == "green" {
        "GREEN"
    } else {
        "RED"
    };
    println!(
        "vox doctor --project {} → {} ({}/{} files clean, {:.2}s)",
        report.project_root,
        status,
        report.files_passing,
        report.files_total,
        report.duration_seconds,
    );
    if !report.failing_files.is_empty() {
        println!();
        println!("Failing files:");
        for f in &report.failing_files {
            println!(
                "  ✗ {} ({} error(s), {} warning(s))",
                f.path, f.error_count, f.warning_count
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[tokio::test]
    async fn green_when_all_files_compile_clean() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("src/main.vox"),
            "fn add(a: int, b: int) to int { return a + b }\n",
        );
        write(
            &tmp.path().join("src/util.vox"),
            "fn neg(n: int) to int { return -n }\n",
        );
        // Should not bail.
        let result = run(tmp.path(), true).await;
        assert!(result.is_ok(), "expected green: {result:?}");
    }

    #[tokio::test]
    async fn red_when_a_file_fails_to_compile() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("src/good.vox"),
            "fn ok() to int { return 1 }\n",
        );
        write(
            &tmp.path().join("src/bad.vox"),
            "this is not vox source ###\n",
        );
        let result = run(tmp.path(), true).await;
        assert!(result.is_err(), "expected red on bad fixture");
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("failed compile-check"), "got: {msg}");
    }

    #[tokio::test]
    async fn missing_project_root_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let nope = tmp.path().join("does-not-exist");
        let result = run(&nope, true).await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn empty_project_is_green() {
        let tmp = tempfile::tempdir().unwrap();
        // No .vox files at all.
        let result = run(tmp.path(), true).await;
        assert!(result.is_ok(), "empty project should be vacuously green");
    }

    #[tokio::test]
    async fn skipped_dirs_are_not_walked() {
        let tmp = tempfile::tempdir().unwrap();
        // Real source.
        write(
            &tmp.path().join("src/ok.vox"),
            "fn id(n: int) to int { return n }\n",
        );
        // Broken file under target/ should be ignored.
        write(
            &tmp.path().join("target/debris.vox"),
            "this is broken vox ###\n",
        );
        let result = run(tmp.path(), true).await;
        assert!(
            result.is_ok(),
            "target/ should be skipped despite containing broken .vox: {result:?}"
        );
    }
}
