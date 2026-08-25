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
    // Conservative: if it looks like an app surface (has decorators, or the
    // post-hard-error-flip bare keywords they were flipped to, that belong
    // in module-position only), don't treat as script.
    let app_markers = [
        "@page",
        "@query",
        "@mutation",
        "@server",
        "@component",
        "@table",
        "@form",
        "@push",
    ];
    let has_at_marker = app_markers.iter().any(|m| source.contains(m));
    // `workflow`/`activity`/`actor` never had an `@`-prefixed spelling (unlike
    // table/query/mutation/server, which moved from `@endpoint(kind: ...)` /
    // `@table` to bare keywords on 2026-06-30) and were missing here entirely,
    // so a `.vox` file containing nothing else recognizable -- a bare
    // `workflow`/`activity`/`actor` decl, e.g. under `@distributed_train`,
    // whose grammar requires a `workflow` line to immediately follow -- was
    // silently misclassified as a script and routed through VoxScript-mode
    // parsing instead of full-module parsing.
    // Mirrors the union of `vox_language_surface::DECLARATION_KEYWORDS` and
    // `WEB_REACTIVE_KEYWORDS` (the actor/workflow/activity/component subset and
    // the table/query/mutation/server/tool/resource/form subset) plus `routes`,
    // which lives in neither list (recognized positionally, no lexer token) —
    // duplicated here per the Defactor policy (crate-edges is CI-gated and a
    // new vox-cli -> vox-language-surface edge needs a user-authorized
    // exception) rather than taking a new crate dependency for ~15 literals.
    let decl_keywords = [
        "table ",
        "query ",
        "mutation ",
        "server ",
        "component ",
        "routes ",
        "routes{",
        "workflow ",
        "activity ",
        "actor ",
        "tool ",
        "resource ",
    ];
    let has_decl_keyword = source.lines().any(|line| {
        decl_keywords
            .iter()
            .any(|k| line.trim_start().starts_with(k))
    });
    !(has_at_marker || has_decl_keyword)
}

/// Lex, parse, and type-check `file`; fail the process if any error-level diagnostic is reported.
///
/// When the user passes global `--json`, [`crate::apply_global_opts`] sets `VOX_CLI_GLOBAL_JSON=1`;
/// diagnostics are printed as JSON to stdout (parse failures already use JSON when `json` is true).
pub async fn run(args: &CheckArgs) -> Result<()> {
    let file = &args.file;
    let json =
        args.output_format == "json" || args.for_llm || crate::pipeline::global_json_enabled();

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
    crate::pipeline::print_diagnostics_with_mode(&result, file, json, args.human_diagnostics);
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

#[cfg(test)]
mod is_script_like_tests {
    use super::is_script_like;

    /// The bug this guards: `workflow`/`activity`/`actor` never had an
    /// `@`-prefixed spelling and were missing from `decl_keywords` entirely,
    /// so a file containing nothing else recognizable was silently routed
    /// through VoxScript-mode parsing instead of full-module parsing --
    /// `vox check` failed to parse valid module-level source with no
    /// rendered error, even though the underlying parser (and its own unit
    /// tests) handled it correctly all along.
    #[test]
    fn bare_workflow_decl_is_not_script_like() {
        assert!(!is_script_like(
            "workflow Train() to Unit {\n    return Unit\n}\n"
        ));
    }

    #[test]
    fn bare_activity_decl_is_not_script_like() {
        assert!(!is_script_like(
            "activity DoWork() to Unit {\n    return Unit\n}\n"
        ));
    }

    #[test]
    fn bare_actor_decl_is_not_script_like() {
        assert!(!is_script_like(
            "actor Counter {\n    state n: int = 0\n}\n"
        ));
    }

    #[test]
    fn distributed_train_workflow_is_not_script_like() {
        assert!(!is_script_like(
            "@distributed_train(strategy = \"data_parallel\", peers = 4)\nworkflow Train() to Unit {\n    return Unit\n}\n"
        ));
    }

    #[test]
    fn actual_script_is_still_script_like() {
        assert!(is_script_like("print(\"hello\")\nlet x = 1 + 2\n"));
    }
}
