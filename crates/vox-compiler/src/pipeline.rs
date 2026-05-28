//! Unified compiler pipeline orchestrator.
//!
//! Provides a single entry point (`run_frontend`) that runs the full
//! lex → parse → typecheck → HIR validation pass and returns structured
//! results.

use crate::ast::decl::Module;
use crate::hir::HirModule;
use crate::hir::lower::LowerConfig;
use crate::typeck::Diagnostic;
use crate::typeck::diagnostics::{TypeckSeverity, VoxCompilerDiagnosticPayload};
use anyhow::Result;

// ADR-041 (2026-05-23) supersedes ADR-028. The reserved-keyword gate that rejected
// `@scheduled`, `@durable`, `workflow`, and `activity` at the source-text level has been
// removed: the durable runtime now ships for the supported subset (ADR-019 / ADR-021), and
// these keywords are part of the stable public grammar. The historical scanner and its
// `test_reject_*_adr028` regression tests were deleted at the same time.

/// Options for the unified compiler pipeline.
#[derive(Debug, Clone, Default)]
pub struct PipelineOptions {
    pub lower_config: LowerConfig,
    pub script_mode: bool,
}

/// The result of running the frontend pipeline.
pub struct FrontendResult {
    pub module: Module,
    pub hir: HirModule,
    pub diagnostics: Vec<Diagnostic>,
    pub source: String,
}

impl FrontendResult {
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == TypeckSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == TypeckSeverity::Warning)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

/// Run the frontend pipeline on a source string.
pub fn run_frontend_str(source: &str, file_path: &str) -> Result<FrontendResult> {
    run_frontend_str_with_options(source, file_path, &PipelineOptions::default())
}

pub fn run_frontend_str_with_options(
    source: &str,
    file_path: &str,
    options: &PipelineOptions,
) -> Result<FrontendResult> {
    // 1. Lex
    let tokens = crate::lexer::lex(source);

    // 1.5. Prevent Syntactic Configurability (K-Complexity Guard)
    for spanned in &tokens {
        if let crate::lexer::token::Token::Ident(ref name) = spanned.token
            && (name == "macro_rules" || name == "macro")
        {
            let diag = Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: "SyntacticConfigurabilityNotAllowed: Vox is strictly constrained. Do not use macros or custom syntactic configurability. Use vox-skills for extended actions.".to_string(),
                    span: crate::ast::span::Span::new(spanned.span.start, spanned.span.end),
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec!["Rewrite using standard syntax and route out-of-band logic through MCP skills.".to_string()],
                    category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                    code: Some("E091".to_string()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                };
            return Ok(FrontendResult {
                module: crate::ast::decl::Module {
                    declarations: vec![],
                    span: crate::ast::span::Span::new(0, 0),
                },
                hir: crate::hir::HirModule::default(),
                diagnostics: vec![diag],
                source: source.to_owned(),
            });
        }
    }

    // 2. Parse
    let module_res = if options.script_mode {
        crate::parser::parse_script(tokens.clone())
    } else {
        crate::parser::parse(tokens.clone())
    };
    let module = module_res
        .map_err(|errors| anyhow::anyhow!("Parsing failed with {} error(s)", errors.len()))?;

    // 3. Lower to HIR + structural validation
    let mut hir = crate::hir::lower::lower_module_with_config(&module, &options.lower_config);

    // 3.5. Resolve intra-project local-file imports — inline pub fn / pub
    // type bodies from imported `.vox` files into the main HIR. This makes
    // `--mode script` (Rust codegen) honor `import "./foo.vox"` without
    // any runtime resolver: by the time codegen sees the HIR, the imported
    // function bodies are part of `hir.functions`.
    //
    // The eval path (`Interpreter::resolve_local_file_import`) and the
    // typeck path (`resolve_imported_pubs_into_env`) both do their own
    // resolution at their respective phases; this is the codegen side of
    // the same coin. All three are gated on a known source-file path.
    let typeck_path = if file_path.is_empty() {
        None
    } else {
        Some(std::path::Path::new(file_path))
    };
    if let Some(path) = typeck_path {
        let mut visited: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();
        if let Ok(canon) = std::fs::canonicalize(path) {
            visited.insert(canon);
        }
        let import_paths: Vec<(String, Option<String>)> = hir
            .imports
            .iter()
            .filter_map(|imp| {
                imp.local_file_path
                    .as_ref()
                    .map(|p| (p.clone(), imp.local_file_alias.clone()))
            })
            .collect();
        for (rel, alias) in import_paths {
            inline_imported_decls(&mut hir, &rel, path, alias.as_deref(), &mut visited);
        }
    }

    // 4. Type-check HIR (populates inferred types). When we have a real file
    // path on disk, route through `_with_path` so intra-project local-file
    // imports can be eagerly resolved into the type environment.
    let mut diagnostics =
        crate::typeck::typecheck_hir_module_with_path(source, &mut hir, typeck_path);

    // 5. Deprecated Usage Detector (Item 16, @deprecated)
    for line in source.lines() {
        let line_start_byte = (line.as_ptr() as usize).saturating_sub(source.as_ptr() as usize);
        if line.trim_start().starts_with("@deprecated") {
            let start = line_start_byte + line.find("@deprecated").unwrap_or(0);
            diagnostics.push(Diagnostic {
                severity: TypeckSeverity::Warning,
                message: "Found @deprecated annotation. Consider removing this obsolete code."
                    .to_string(),
                span: crate::ast::span::Span::new(start, start + 11),
                expected_type: None,
                found_type: None,
                context: None,
                suggestions: vec![
                    "Refactor dependents and remove this deprecated item.".to_string(),
                ],
                category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                code: Some("W092".to_string()),
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                ast_node_kind: None,
            });
        }

        let jsx_leaks = ["className=", "onClick=", "onChange=", "onSubmit="];
        for leak in jsx_leaks {
            if let Some(idx) = line.find(leak) {
                let start = line_start_byte + idx;
                let attr = leak.trim_end_matches('=');
                let mut vox_attr = attr.to_lowercase();
                if vox_attr.starts_with("on") {
                    vox_attr = format!("on:{}", &vox_attr[2..]);
                }
                if vox_attr == "classname" {
                    vox_attr = "class".to_string();
                }
                diagnostics.push(Diagnostic {
                    severity: TypeckSeverity::Warning,
                    message: format!("Raw JSX '{}' leaks into Vox source (Item 16).", attr),
                    span: crate::ast::span::Span::new(start, start + leak.len()),
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec![format!(
                        "Use Vox-native syntax: '{}=' instead of '{}='.",
                        vox_attr, attr
                    )],
                    category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                    code: Some("W093".to_string()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                });
            }
        }
    }

    for e in crate::hir::validate_module(&hir) {
        diagnostics.push(Diagnostic::hir_invariant(
            e.message,
            e.span,
            source,
            e.correction_hint,
        ));
    }

    Ok(FrontendResult {
        module,
        hir,
        diagnostics,
        source: source.to_owned(),
    })
}

pub fn format_diagnostics_json(result: &FrontendResult, file_path: &str) -> String {
    let output: Vec<VoxCompilerDiagnosticPayload> = result
        .diagnostics
        .iter()
        .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(d, file_path, &result.source))
        .collect();
    serde_json::to_string_pretty(&output).unwrap_or_default()
}

/// Run the full check pipeline and return machine-readable diagnostics even on parse failure.
pub fn check_file(source: &str, file_path: &str) -> Vec<VoxCompilerDiagnosticPayload> {
    let tokens = crate::lexer::lex(source);

    // 1.5. Prevent Syntactic Configurability (K-Complexity Guard)
    for spanned in &tokens {
        if let crate::lexer::token::Token::Ident(ref name) = spanned.token
            && (name == "macro_rules" || name == "macro")
        {
            let diag = Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: "SyntacticConfigurabilityNotAllowed: Vox is strictly constrained. Do not use macros or custom syntactic configurability. Use vox-skills for extended actions.".to_string(),
                    span: crate::ast::span::Span::new(spanned.span.start, spanned.span.end),
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec!["Rewrite using standard syntax and route out-of-band logic through MCP skills.".to_string()],
                    category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                    code: Some("E091".to_string()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                };
            return vec![VoxCompilerDiagnosticPayload::from_diagnostic(
                &diag, file_path, source,
            )];
        }
    }

    match crate::parser::parse(tokens) {
        Ok(module) => {
            let mut hir = crate::hir::lower_module(&module);
            let mut diagnostics = crate::typeck::typecheck_hir_module(source, &mut hir);

            // 5. Deprecated Usage Detector (Item 16, @deprecated)
            for line in source.lines() {
                let line_start_byte =
                    (line.as_ptr() as usize).saturating_sub(source.as_ptr() as usize);
                if line.trim_start().starts_with("@deprecated") {
                    let start = line_start_byte + line.find("@deprecated").unwrap_or(0);
                    diagnostics.push(Diagnostic {
                        severity: TypeckSeverity::Warning,
                        message:
                            "Found @deprecated annotation. Consider removing this obsolete code."
                                .to_string(),
                        span: crate::ast::span::Span::new(start, start + 11),
                        expected_type: None,
                        found_type: None,
                        context: None,
                        suggestions: vec![
                            "Refactor dependents and remove this deprecated item.".to_string(),
                        ],
                        category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                        code: Some("W092".to_string()),
                        fixes: vec![],
                        line_col: None,
                        missing_cases: vec![],
                        ast_node_kind: None,
                    });
                }

                let jsx_leaks = ["className=", "onClick=", "onChange=", "onSubmit="];
                for leak in jsx_leaks {
                    if let Some(idx) = line.find(leak) {
                        let start = line_start_byte + idx;
                        let attr = leak.trim_end_matches('=');
                        let mut vox_attr = attr.to_lowercase();
                        if vox_attr.starts_with("on") {
                            vox_attr = format!("on:{}", &vox_attr[2..]);
                        }
                        if vox_attr == "classname" {
                            vox_attr = "class".to_string();
                        }
                        diagnostics.push(Diagnostic {
                            severity: TypeckSeverity::Warning,
                            message: format!("Raw JSX '{}' leaks into Vox source (Item 16).", attr),
                            span: crate::ast::span::Span::new(start, start + leak.len()),
                            expected_type: None,
                            found_type: None,
                            context: None,
                            suggestions: vec![format!(
                                "Use Vox-native syntax: '{}=' instead of '{}='.",
                                vox_attr, attr
                            )],
                            category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                            code: Some("W093".to_string()),
                            fixes: vec![],
                            line_col: None,
                            missing_cases: vec![],
                            ast_node_kind: None,
                        });
                    }
                }
            }

            for e in crate::hir::validate_module(&hir) {
                diagnostics.push(Diagnostic::hir_invariant(
                    e.message,
                    e.span,
                    source,
                    e.correction_hint,
                ));
            }
            diagnostics
                .into_iter()
                .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(&d, file_path, source))
                .collect()
        }
        Err(errors) => errors
            .into_iter()
            .map(|e| {
                let diag = Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: e.message,
                    span: e.span,
                    expected_type: None,
                    found_type: None,
                    context: None,
                    suggestions: vec![],
                    category: crate::typeck::diagnostics::DiagnosticCategory::Parse,
                    code: Some("E0001".to_string()),
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    ast_node_kind: None,
                };
                VoxCompilerDiagnosticPayload::from_diagnostic(&diag, file_path, source)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_macros_e091() {
        let source = "macro_rules! my_macro { () => {} }";
        let diagnostics = check_file(source, "test.vox");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].error_code, "E091".to_string());
        assert!(
            diagnostics[0]
                .message
                .contains("SyntacticConfigurabilityNotAllowed")
        );

        // Also test run_frontend_str
        let frontend_res = run_frontend_str(source, "test.vox").unwrap();
        assert_eq!(frontend_res.diagnostics.len(), 1);
        assert_eq!(frontend_res.diagnostics[0].code, Some("E091".to_string()));
    }

    // ADR-041 (supersedes ADR-028): `workflow`, `activity`, `actor`, `@scheduled`, and
    // `@durable` are public-grammar features backed by a real runtime. The frontend must
    // NOT emit an ADR-028-style reservation error for them. These tests pin the gate-lift.

    fn assert_no_adr028_reservation_error(diagnostics: &[VoxCompilerDiagnosticPayload]) {
        for d in diagnostics {
            assert!(
                !d.message.contains("reserved for a future release"),
                "ADR-028 reservation gate should be lifted (ADR-041); got: {:?}",
                d.message
            );
            assert_ne!(
                d.error_code.as_str(),
                "E028",
                "E028 reservation code must not appear after ADR-041; full diag: {:?}",
                d
            );
        }
    }

    #[test]
    fn test_accept_workflow_keyword_adr041() {
        let source = r#"workflow order(amount: int) to int { return amount }"#;
        let diagnostics = check_file(source, "test.vox");
        assert_no_adr028_reservation_error(&diagnostics);
    }

    #[test]
    fn test_accept_activity_keyword_adr041() {
        let source = r#"activity charge(amount: int) to int { return amount }"#;
        let diagnostics = check_file(source, "test.vox");
        assert_no_adr028_reservation_error(&diagnostics);
    }

    #[test]
    fn test_accept_workflow_plus_activity_adr041() {
        // Exercises the canonical golden shape: an activity used by a workflow.
        let source = r#"
activity charge(amount: int) to int { return amount }
workflow checkout(amount: int) to int { return charge(amount) }
"#;
        let diagnostics = check_file(source, "test.vox");
        assert_no_adr028_reservation_error(&diagnostics);
    }

    #[test]
    fn test_accept_scheduled_decorator_adr041() {
        let source = r#"@scheduled("1h") fn tick() to int { return 0 }"#;
        let diagnostics = check_file(source, "test.vox");
        assert_no_adr028_reservation_error(&diagnostics);
    }

    #[test]
    fn test_actor_still_compiles_adr041() {
        // actor is retained per ADR-041 — must produce zero errors at the frontend layer.
        let source = r#"actor Counter { on increment(n: int) to int { return n } }"#;
        let diagnostics = check_file(source, "test.vox");
        assert_no_adr028_reservation_error(&diagnostics);
        assert!(
            diagnostics
                .iter()
                .all(|d| d.severity != crate::typeck::diagnostics::TypeckSeverity::Error),
            "actor should still compile successfully (ADR-041 retains actor); errors: {:?}",
            diagnostics
                .iter()
                .filter(|d| d.severity == crate::typeck::diagnostics::TypeckSeverity::Error)
                .collect::<Vec<_>>()
        );
    }
}

/// Inline pub fn / pub type-variant decls from an intra-project
/// `import "./foo.vox"` directive into the importing file's HIR.
///
/// Called by [`run_frontend_str_with_options`] before typecheck. Cycle-safe
/// via a per-invocation visited set of canonicalized paths. Silently no-ops
/// on file-read / parse failure — the typeck step that follows will surface
/// the resulting "unknown name" diagnostics, which is the right place for
/// the user-facing error.
///
/// **Why we inline at pipeline level rather than at HIR lowering:** the
/// lowering pass doesn't know the source-file path (it operates on an
/// already-parsed AST). The pipeline does have the path, and inlining
/// here means downstream consumers (typeck, codegen, code-audit detectors)
/// see one merged HIR with all required function bodies — no special
/// runtime resolver needed.
///
/// **Alias form** (`import "./foo.vox" as alias`): when `alias` is `Some`,
/// the imported decls are wrapped under a synthesized namespace-like prefix
/// so `alias.fn_name(...)` resolves. For v0.7 the simplest implementation
/// is to prefix the inlined function names with `<alias>__` and emit a
/// type-environment alias entry; this matches the eval-side Object-method
/// dispatch from intra-project-imports RFC §11. Codegen sees the prefixed
/// names directly. Bare form (no alias) inlines under the original names.
fn inline_imported_decls(
    hir: &mut crate::hir::HirModule,
    rel_path: &str,
    importer_path: &std::path::Path,
    alias: Option<&str>,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) {
    let base_dir = importer_path.parent().unwrap_or(std::path::Path::new("."));
    let joined = base_dir.join(rel_path);
    let canonical = match std::fs::canonicalize(&joined) {
        Ok(c) => c,
        Err(_) => return,
    };
    if !visited.insert(canonical.clone()) {
        return;
    }
    let source = match std::fs::read_to_string(&canonical) {
        Ok(s) => s,
        Err(_) => return,
    };
    let tokens = crate::lexer::lex(&source);
    let module = match crate::parser::parse_script(tokens) {
        Ok(m) => m,
        Err(_) => return,
    };
    let mut imported_hir = crate::hir::lower::lower_module(&module);

    // Recurse into the imported file's own local-file imports first so
    // transitive `pub` decls land in this HIR too.
    let import_paths: Vec<(String, Option<String>)> = imported_hir
        .imports
        .iter()
        .filter_map(|imp| {
            imp.local_file_path
                .as_ref()
                .map(|p| (p.clone(), imp.local_file_alias.clone()))
        })
        .collect();
    for (rel, sub_alias) in import_paths {
        inline_imported_decls(
            &mut imported_hir,
            &rel,
            &canonical,
            sub_alias.as_deref(),
            visited,
        );
    }

    // Move `pub` functions and `pub` types into the importer's HIR.
    // Importer-defined names with the same identifier always win (RFC §3
    // scope-merge), so we only insert when the importer doesn't already
    // have the name.
    let importer_fn_names: std::collections::HashSet<String> =
        hir.functions.iter().map(|f| f.name.clone()).collect();
    let importer_type_names: std::collections::HashSet<String> =
        hir.types.iter().map(|t| t.name.clone()).collect();

    for mut f in imported_hir.functions.into_iter().filter(|f| f.is_pub) {
        if let Some(prefix) = alias {
            f.name = format!("{}__{}", prefix, f.name);
        }
        if !importer_fn_names.contains(&f.name) {
            hir.functions.push(f);
        }
    }
    for t in imported_hir.types.into_iter().filter(|t| t.is_pub) {
        if alias.is_none() && !importer_type_names.contains(&t.name) {
            hir.types.push(t);
        }
        // Alias-form type imports — defer; aliased namespace constructors
        // would need a synthesized prefix that ripples into match patterns.
        // Eval-side alias dispatch handles it via Object lookup at runtime;
        // codegen-side alias type imports are a follow-on.
    }
}
