//! `vox-langtool build` — emit Rust script source via codegen.

use anyhow::{Context, Result};
use std::path::Path;
use vox_compiler::pipeline::{PipelineOptions, run_frontend_str_with_options};

pub fn run(file: &Path, out_dir: &Path) -> Result<()> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file.display()))?;

    let options = PipelineOptions {
        script_mode: crate::is_script_like(&source),
        ..PipelineOptions::default()
    };

    let result = run_frontend_str_with_options(&source, &file.to_string_lossy(), &options)?;

    if result.has_errors() {
        for diag in &result.diagnostics {
            eprintln!("{:?}: {}", diag.severity, diag.message);
        }
        anyhow::bail!("Build failed with {} error(s)", result.error_count());
    }

    let package_name = file.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
        anyhow::anyhow!("Cannot derive package name from path: {}", file.display())
    })?;

    let codegen_out = vox_codegen::codegen_rust::generate_script(&result.hir, package_name, None)
        .map_err(|e| anyhow::anyhow!("Codegen failed: {e}"))?;

    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create out-dir {}", out_dir.display()))?;

    for (filename, content) in &codegen_out.files {
        let path = out_dir.join(filename);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  wrote {}", path.display());
    }

    println!("Build complete -> {}", out_dir.display());
    Ok(())
}
