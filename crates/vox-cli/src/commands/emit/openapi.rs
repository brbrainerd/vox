//! `vox emit openapi` — standalone OpenAPI 3.1 JSON from a single Vox source file.
//!
//! Uses [`vox_codegen::codegen_ts::openapi_emit::generate_openapi`] (the same function
//! that `vox emit client` invokes as part of the Library bundle) to produce a canonical
//! OpenAPI 3.1 spec as a standalone file — no TypeScript, no npm package directory.
//!
//! Exit codes:
//! - 0: spec written successfully.
//! - non-zero: parse/typecheck failure or I/O error (error message on stderr).

use crate::cli_args::EmitOpenapiArgs;
use anyhow::{Context, Result};
use std::path::Path;

pub async fn run(args: &EmitOpenapiArgs) -> Result<()> {
    let file = Path::new(&args.file);

    // ── 1. Run the compiler frontend (parse → typecheck → HIR) ───────────────
    let frontend = crate::pipeline::run_frontend(file, false).await?;
    crate::pipeline::print_diagnostics(&frontend, file, false);
    if frontend.has_errors() {
        anyhow::bail!(
            "vox emit openapi: frontend failed with {} error(s)",
            frontend.error_count()
        );
    }

    // ── 2. Generate OpenAPI 3.1 JSON ─────────────────────────────────────────
    let json = vox_codegen::codegen_ts::openapi_emit::generate_openapi(
        &frontend.hir,
        &args.package_name,
        &args.package_version,
    );

    // ── 3. Write to --out path ────────────────────────────────────────────────
    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output directory {}", parent.display()))?;
        }
    }
    std::fs::write(&args.out, &json)
        .with_context(|| format!("write OpenAPI spec to {}", args.out.display()))?;

    eprintln!(
        "OpenAPI 3.1 spec written to {} ({} bytes)",
        args.out.display(),
        json.len()
    );
    Ok(())
}
