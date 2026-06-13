//! `vox-langtool check` — type-check a .vox file.

use anyhow::Result;
use std::path::Path;
use vox_compiler::pipeline::{PipelineOptions, run_frontend_str_with_options};
use vox_compiler::typeck::diagnostics::TypeckSeverity;

pub fn run(file: &Path) -> Result<()> {
    let source = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", file.display(), e))?;

    let options = PipelineOptions {
        script_mode: crate::is_script_like(&source),
        ..PipelineOptions::default()
    };

    let result = run_frontend_str_with_options(&source, &file.to_string_lossy(), &options)?;

    for diag in &result.diagnostics {
        let level = match diag.severity {
            TypeckSeverity::Error => "error",
            TypeckSeverity::Warning => "warning",
        };
        eprintln!("{}: {}", level, diag.message);
    }

    let error_count = result.error_count();
    let warning_count = result.warning_count();

    if result.has_errors() {
        anyhow::bail!(
            "Check failed with {} error(s) and {} warning(s)",
            error_count,
            warning_count
        );
    }

    println!("Check passed with {} warning(s)", warning_count);
    Ok(())
}
