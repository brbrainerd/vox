//! `vox check` — type-check only (no files written except logs to stderr).

use anyhow::Result;
use owo_colors::OwoColorize;
use vox_compiler::pipeline::PipelineOptions;

/// Lex, parse, and type-check `file`; fail the process if any error-level diagnostic is reported.
///
/// When the user passes global `--json`, [`crate::apply_global_opts`] sets `VOX_CLI_GLOBAL_JSON=1`;
/// diagnostics are printed as JSON to stdout (parse failures already use JSON when `json` is true).
use crate::cli_args::CheckArgs;

/// Heuristic for "this file is a script-style entry point" — uses `parse_script`
/// (which wraps top-level statements in a synthetic `fn main()`) instead of
/// strict `parse`. Files containing `@page`, `@endpoint`, or `@component`
/// decorators are definitely-not-scripts and use the strict path. Everything
/// else (including library files with no top-level statements) is safe to
/// route through `parse_script` — the synthetic-main wrapping is a no-op when
/// there are no top-level statements to wrap.
///
/// Without this, `vox check` and `vox run` diverge on script-style files like
/// [`scripts/scientia/atlas-draft.vox`](../../../../../scripts/scientia/atlas-draft.vox)
/// — `vox run` works (it uses `parse_script` itself); `vox check` errors with
/// "Unexpected token at top level". That divergence is itself a diagnostic-
/// parity defect (see [`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](../../../../../docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md) §5.4).
fn is_script_like(source: &str) -> bool {
    // Conservative: if it looks like an app surface (has decorators that
    // belong in module-position only), don't treat as script.
    let app_markers = [
        "@page",
        "@query",
        "@mutation",
        "@server",
        "@component",
        "@table",
        "@workflow",
    ];
    !app_markers.iter().any(|m| source.contains(m))
}

/// Lex, parse, and type-check `file`; fail the process if any error-level diagnostic is reported.
///
/// When the user passes global `--json`, [`crate::apply_global_opts`] sets `VOX_CLI_GLOBAL_JSON=1`;
/// diagnostics are printed as JSON to stdout (parse failures already use JSON when `json` is true).
pub async fn run(args: &CheckArgs) -> Result<()> {
    let file = &args.file;
    let json = args.output_format == "json"
        || args.for_llm
        || std::env::var("VOX_CLI_GLOBAL_JSON").ok().as_deref() == Some("1");

    if args.for_llm {
        let source = vox_bounded_fs::read_utf8_path_capped(file)?;
        let llm_json = crate::pipeline::format_check_for_llm_json(&source, file);
        println!("{}", llm_json);
        let envelope: serde_json::Value =
            serde_json::from_str(&llm_json).unwrap_or_else(|_| serde_json::json!({}));
        let error_count = envelope
            .get("error_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if error_count > 0 {
            anyhow::bail!("Check failed (--for-llm): {error_count} error-level diagnostic(s)");
        }
        let warning_count = envelope
            .get("warning_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if args.strict && warning_count > 0 {
            anyhow::bail!(
                "Check failed (--for-llm --strict): {warning_count} warning-level diagnostic(s) \
                 (warnings are errors in strict mode)"
            );
        }
        println!("Check passed (--for-llm) with {warning_count} warning(s)");
        return Ok(());
    }

    // Read once so we can both heuristic-classify and pass to the pipeline.
    let source = vox_bounded_fs::read_utf8_path_capped(file)?;
    let options = PipelineOptions {
        script_mode: is_script_like(&source),
        ..PipelineOptions::default()
    };
    let result = crate::pipeline::run_frontend_with_options(file, json, &options).await?;
    crate::pipeline::print_diagnostics(&result, file, json);
    let error_count = result.error_count();
    let warning_count = result.warning_count();

    if result.has_errors() {
        anyhow::bail!("Check failed with {error_count} error(s) and {warning_count} warning(s)");
    }

    if args.strict && result.has_warnings() {
        anyhow::bail!(
            "Check failed (--strict): {warning_count} warning(s) treated as error(s) \
             (use without --strict to allow warnings)"
        );
    }

    if args.emit_ir {
        let vox_ir =
            vox_codegen::vox_ir::lower::lower_hir_to_vox_ir(&result.hir, Some(&result.source));
        let json_ir = serde_json::to_string_pretty(&vox_ir)?;
        let mut ir_path = file.clone();
        ir_path.set_extension("vox-ir.json");
        std::fs::write(&ir_path, json_ir)?;
        println!("{} IR to {}", "Emitted".green(), ir_path.display());
    }

    #[cfg(feature = "extras-ludus")]
    {
        if vox_gamify::config_gate::is_enabled() {
            if let Ok(db) = crate::workspace_db::connect_cli_workspace_voxdb().await {
                let key = format!("vox-check:{}", file.display());
                vox_gamify::lsp_telemetry::after_cli_check_clean(&db, &key).await;
            }
        }
    }

    println!("Check passed with {warning_count} warning(s)");
    Ok(())
}
