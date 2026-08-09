//! Regression test: Warning-severity `ParseError` diagnostics produced during a
//! *successful* parse (tolerant `;` at statement boundaries, `->` return-type
//! deprecation, `==`/`!=` as `is`/`is not` aliases) must reach
//! `FrontendResult::diagnostics` — and therefore `vox check`'s warning count and
//! printed output — instead of being silently discarded on the `Ok(Module)` path.
//!
//! Before this fix, `Parser::parse_module`/`parse_module_script` only returned
//! diagnostics via `Err(Vec<ParseError>)`; a successful parse dropped
//! `self.errors` (including any Warning-severity entries) on the floor.

use std::path::Path;

use vox_cli::pipeline::run_frontend_str_with_options;
use vox_compiler::pipeline::PipelineOptions;

/// A tolerated trailing `;` at a script-mode statement boundary must surface as
/// exactly one warning, and must not fail the check.
#[test]
fn tolerated_semicolon_surfaces_as_warning_in_check() {
    let source = "let x = 5;\n";
    let options = PipelineOptions {
        script_mode: true,
        ..PipelineOptions::default()
    };
    let result = run_frontend_str_with_options(source, Path::new("test.vox"), false, &options)
        .expect("frontend should succeed on a merely-tolerated construct");

    assert!(
        !result.has_errors(),
        "expected no error-severity diagnostics, got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );

    assert_eq!(
        result.warning_count(),
        1,
        "expected exactly one warning diagnostic for the tolerated `;`, got diagnostics: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| (d.severity, d.message.clone()))
            .collect::<Vec<_>>()
    );

    let warning = result
        .diagnostics
        .iter()
        .find(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Warning)
        .expect("one warning diagnostic must be present");
    assert!(
        warning.message.to_lowercase().contains("semicolon"),
        "warning message should mention the tolerated semicolon, got: {}",
        warning.message
    );
}
