//! Unified compiler pipeline orchestrator.
//!
//! Provides a single entry point (`run_frontend`) that runs the full
//! lex → parse → typecheck → HIR validation pass and returns structured
//! results.

use crate::ast::decl::Module;
use crate::hir::HirModule;
use crate::hir::lower::LowerConfig;
use crate::typeck::Diagnostic;
use crate::typeck::diagnostics::{
    DiagnosticCategory, TypeckSeverity, VoxCompilerDiagnosticPayload,
};
use anyhow::Result;

/// ADR-028 (revised 2026-05-19): emit an error diagnostic for each still-reserved durability
/// keyword found in `source`.
///
/// **History.** Originally `@scheduled`, `@durable`, `workflow`, and `activity` were all
/// reserved-but-rejected pending a durability runtime. As of 2026-05-19, **`workflow` and
/// `activity` are part of the public grammar** — the `vox-workflow-runtime` crate provides
/// `DurablePromise`, journal-backed replay, and the `interpret_workflow_durable` interpreter
/// that the codegen targets via `durability_lower.rs::emit_workflow_body`. The parser, HIR,
/// lowering, and codegen have always been wired; only this text-level gate was holding them
/// closed.
///
/// **Still reserved.** `@scheduled` and `@durable` remain reserved — they target a future
/// "decorator on a plain fn" surface that the runtime doesn't model yet. Use a `workflow`
/// declaration instead until they land.
fn check_adr028_reserved_keywords(source: &str) -> Vec<Diagnostic> {
    // (pattern, keyword_label, error_code, identifier_boundary)
    // `identifier_boundary` is true when the pattern is a bare keyword that could appear inside
    // a longer identifier; for those we additionally require the byte immediately after the
    // match to NOT continue the identifier (alpha/digit/underscore). Decorator forms like
    // `@scheduled` use `@` as a leading sentinel and don't need it.
    const RESERVED: &[(&str, &str, &str, bool)] = &[
        ("@scheduled", "@scheduled", "E028", false),
        ("@durable", "@durable", "E028", false),
    ];

    let mut diags = Vec::new();
    for (pattern, label, code, ident_boundary) in RESERVED {
        let Some(offset) =
            find_keyword_outside_comments_and_strings(source, pattern, *ident_boundary)
        else {
            continue;
        };
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "{} is not yet implemented and has been reserved for a future release (ADR-028). \
                     Remove this declaration or replace it with a plain `fn`.",
                label
            ),
            span: crate::ast::span::Span::new(offset, offset + pattern.len()),
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![format!(
                "Replace `{}` with a plain `fn` declaration.",
                label
            )],
            category: DiagnosticCategory::Parse,
            code: Some(code.to_string()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
    }
    diags
}

/// Detect retired-form decorators in source text and emit an actionable
/// diagnostic suggesting the canonical replacement.
///
/// **Why text-level.** `@server`, `@query`, `@mutation`, `@health`,
/// `@metric` are not lexer tokens — the Logos lexer silently drops the
/// `@` byte and the parser sees an orphan `Ident("server")` which trips
/// the unhelpful "Unexpected token at top level: server" path. A pre-parse
/// text scan turns that into a precise migration hint.
///
/// Each entry pairs the retired form with its canonical replacement:
///
/// | Retired | Canonical |
/// |---|---|
/// | `@server fn` | `@endpoint(kind: server) fn` |
/// | `@query fn` | `@endpoint(kind: query) fn` |
/// | `@mutation fn` | `@endpoint(kind: mutation) fn` |
/// | `@health fn` | plain `fn` (use a route + healthcheck client) |
/// | `@metric fn` | plain `fn` (emit a metric from the body) |
fn check_retired_decorators(source: &str) -> Vec<Diagnostic> {
    // (pattern, retired_label, canonical_replacement, error_code)
    const RETIRED: &[(&str, &str, &str, &str)] = &[
        (
            "@server",
            "@server",
            "@endpoint(kind: server)",
            "E040",
        ),
        (
            "@query",
            "@query",
            "@endpoint(kind: query)",
            "E040",
        ),
        (
            "@mutation",
            "@mutation",
            "@endpoint(kind: mutation)",
            "E040",
        ),
        (
            "@health",
            "@health",
            "plain `fn` (wire a healthcheck route by hand)",
            "E040",
        ),
        (
            "@metric",
            "@metric",
            "plain `fn` (emit a metric from the body)",
            "E040",
        ),
    ];

    let mut diags = Vec::new();
    for (pattern, label, canonical, code) in RETIRED {
        // Decorator forms always carry the `@` leading sentinel, so no
        // identifier-boundary check is needed.
        let Some(offset) =
            find_keyword_outside_comments_and_strings(source, pattern, false)
        else {
            continue;
        };
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "`{label}` is retired. Replace with `{canonical}`. See AGENTS.md §Retired Surfaces.",
            ),
            span: crate::ast::span::Span::new(offset, offset + pattern.len()),
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![format!("Replace `{label}` with `{canonical}`.")],
            category: DiagnosticCategory::Parse,
            code: Some(code.to_string()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        });
    }
    diags
}

/// Find the first occurrence of `pattern` in `source` that is NOT inside a `//` line comment,
/// `/* */` block comment, or a `"…"` string literal. Returns the byte offset of the match.
///
/// Needed because ADR-028's reserved-keyword scan runs at the source-text level (before parsing)
/// and would otherwise flag the word "workflow" appearing in a doc comment as a real declaration.
#[cfg(test)]
fn find_outside_comments_and_strings(source: &str, pattern: &str) -> Option<usize> {
    find_keyword_outside_comments_and_strings(source, pattern, false)
}

/// Same as `find_outside_comments_and_strings` but with optional identifier-boundary enforcement
/// for bare keyword matches. When `ident_boundary` is true, the byte immediately following the
/// match must NOT be an identifier-continuing character (alpha/digit/underscore), so substrings
/// inside longer identifiers (e.g. `workflow_handle`) don't trigger.
fn find_keyword_outside_comments_and_strings(
    source: &str,
    pattern: &str,
    ident_boundary: bool,
) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = 0usize;
    let plen = pattern.len();
    while i + plen <= bytes.len() {
        // Skip over `// …\n` line comments.
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Skip over `/* … */` block comments.
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        // Skip over `"…"` string literals (handle simple backslash escapes).
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            i = (i + 1).min(bytes.len());
            continue;
        }
        if &bytes[i..i + plen] == pattern.as_bytes() {
            if ident_boundary {
                let next = bytes.get(i + plen).copied().unwrap_or(0);
                let continues_ident = next.is_ascii_alphanumeric() || next == b'_';
                if continues_ident {
                    i += 1;
                    continue;
                }
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod adr028_comment_skip_tests {
    use super::find_outside_comments_and_strings;

    #[test]
    fn finds_keyword_outside_comment() {
        assert_eq!(
            find_outside_comments_and_strings("workflow Foo {}", "workflow "),
            Some(0)
        );
    }

    #[test]
    fn skips_keyword_in_line_comment() {
        // The bare word "workflow" inside a `//` comment must NOT be matched.
        assert_eq!(
            find_outside_comments_and_strings(
                "// workflow time-travel scrubber\nfn foo() {}",
                "workflow "
            ),
            None
        );
    }

    #[test]
    fn skips_keyword_in_block_comment() {
        assert_eq!(
            find_outside_comments_and_strings("/* see workflow doc */\nfn foo() {}", "workflow "),
            None
        );
    }

    #[test]
    fn skips_keyword_in_string_literal() {
        assert_eq!(
            find_outside_comments_and_strings(r#"let s = "workflow demo";"#, "workflow "),
            None
        );
    }

    #[test]
    fn finds_after_passing_comment() {
        let src = "// pre-amble mentioning workflow\nworkflow Real {}";
        let offset = find_outside_comments_and_strings(src, "workflow ").unwrap();
        // Must be the second occurrence (start of the `workflow Real` line), not the comment.
        assert!(
            offset > 30,
            "expected match past the comment, got offset {offset}"
        );
    }
}

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
pub fn run_frontend_str(source: &str, _file_path: &str) -> Result<FrontendResult> {
    run_frontend_str_with_options(source, _file_path, &PipelineOptions::default())
}

pub fn run_frontend_str_with_options(
    source: &str,
    _file_path: &str,
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

    // 1.6. ADR-028: reject reserved durability grammar keywords early.
    {
        let reserved_diags = check_adr028_reserved_keywords(source);
        if !reserved_diags.is_empty() {
            return Ok(FrontendResult {
                module: crate::ast::decl::Module {
                    declarations: vec![],
                    span: crate::ast::span::Span::new(0, 0),
                },
                hir: crate::hir::HirModule::default(),
                diagnostics: reserved_diags,
                source: source.to_owned(),
            });
        }
    }

    // 1.7. Retired-decorator scan: catch `@server`/`@query`/`@mutation`/
    // `@health`/`@metric` at the text level and emit an actionable
    // diagnostic suggesting the canonical replacement.
    {
        let retired_diags = check_retired_decorators(source);
        if !retired_diags.is_empty() {
            return Ok(FrontendResult {
                module: crate::ast::decl::Module {
                    declarations: vec![],
                    span: crate::ast::span::Span::new(0, 0),
                },
                hir: crate::hir::HirModule::default(),
                diagnostics: retired_diags,
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

    // 4. Type-check HIR (populates inferred types)
    let mut diagnostics = crate::typeck::typecheck_hir_module(source, &mut hir);

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

    // 1.6. ADR-028: reject reserved durability grammar keywords early.
    {
        let reserved_diags = check_adr028_reserved_keywords(source);
        if !reserved_diags.is_empty() {
            return reserved_diags
                .iter()
                .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(d, file_path, source))
                .collect();
        }
    }

    // 1.7. Retired-decorator scan (mirrors run_frontend_str path 1.7).
    {
        let retired_diags = check_retired_decorators(source);
        if !retired_diags.is_empty() {
            return retired_diags
                .iter()
                .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(d, file_path, source))
                .collect();
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

    /// Retired-decorator scan must emit an actionable migration hint
    /// instead of the bare "Unexpected token at top level" path that
    /// the parser used to produce when the lexer silently dropped `@`.
    #[test]
    fn retired_decorators_emit_actionable_migration_hint() {
        for (retired, canonical_fragment) in [
            ("@server", "@endpoint(kind: server)"),
            ("@query", "@endpoint(kind: query)"),
            ("@mutation", "@endpoint(kind: mutation)"),
            ("@health", "plain `fn`"),
            ("@metric", "plain `fn`"),
        ] {
            let source = format!("{retired} fn handler() to int {{ return 0 }}");
            let diagnostics = check_file(&source, "retired.vox");
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.message.contains(retired) && d.message.contains(canonical_fragment)),
                "retired {retired} should suggest {canonical_fragment}; got: {:?}",
                diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
            );
            assert!(
                diagnostics.iter().any(|d| d.error_code == "E040"),
                "retired-decorator diagnostic should carry code E040"
            );
        }
    }

    /// The retired-decorator scan must NOT fire when the form appears
    /// inside a string literal or a comment. Mirrors the ADR-028 comment
    /// skip tests.
    #[test]
    fn retired_decorators_are_ignored_inside_strings_and_comments() {
        let source = r#"
// Note: @server is retired — this is just a comment.
fn render() to str {
    return "documentation for @query is at /docs"
}
"#;
        let diagnostics = check_file(source, "no_retired.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "retired decorators inside strings/comments should not fire; got: {errors:?}"
        );
    }

    /// B3: `subscribe(Actor)` types as `Stream[T]` where `T` is the
    /// return type of the actor's `on broadcast(...)` handler. When the
    /// actor declares `on broadcast(...) to int`, the call should type
    /// as `Stream[int]` (not the `Stream[str]` v1.0 default fallback).
    #[test]
    fn subscribe_uses_actor_broadcast_return_type() {
        let source = r#"
actor Counter {
    on broadcast(n: int) to int { return n }
}

fn watch(c: Counter) to Stream[int] {
    return subscribe(c)
}
"#;
        let diagnostics = check_file(source, "subscribe_int.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "subscribe(Counter) where Counter.broadcast returns int should type as Stream[int]; got: {errors:?}"
        );
    }

    /// `let r: T = if cond { ctor_a } else { ctor_b }` should specialize
    /// each branch's polymorphic constructor against `T`. Without
    /// expected-type propagation through `if`, the polymorphic
    /// constructor's fresh type vars never bind and the unification
    /// against `T` fails downstream.
    #[test]
    fn if_expression_propagates_expected_to_branches() {
        let source = r#"
type MyErr = | NotFound | BadInput
fn classify(cond: bool) to Result[int, MyErr] {
    let r: Result[int, MyErr] = if cond { Ok(42) } else { Error(NotFound) }
    return r
}
"#;
        let diagnostics = check_file(source, "if_expected.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "if-expression with Result[T, MyErr] should specialize both branches; got: {errors:?}"
        );
    }

    /// `@endpoint(kind: stream, every: "<duration>")` accepts the
    /// `every:` argument for tick-on-schedule streaming endpoints.
    /// The interval string is preserved through HIR for downstream
    /// SSE-handler codegen.
    #[test]
    fn endpoint_stream_with_every_compiles_clean() {
        let source = r#"@endpoint(kind: stream, every: "1s") fn ticker() to int { return 0 }"#;
        let diagnostics = check_file(source, "stream.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "stream + every should compile clean; got: {errors:?}"
        );
    }

    /// `every:` on a non-stream endpoint must be rejected — it's a
    /// streaming-only directive.
    #[test]
    fn endpoint_every_rejected_on_non_stream() {
        let source = r#"@endpoint(kind: query, every: "1s") fn bad() to int { return 0 }"#;
        let diagnostics = check_file(source, "every_wrong.vox");
        let has_error = diagnostics
            .iter()
            .any(|d| matches!(d.severity, crate::typeck::diagnostics::TypeckSeverity::Error));
        assert!(
            has_error,
            "every: on a query endpoint should error; got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    /// `subscribe(ActorName)` is a builtin returning `Stream[str]` — the
    /// minimum surface needed for an actor-driven streaming endpoint.
    #[test]
    fn subscribe_builtin_returns_stream() {
        let source = r#"
actor ChatRoom { on broadcast(msg: str) to str { return msg } }
@endpoint(kind: stream) fn watch() to Stream[str] {
    return subscribe(ChatRoom)
}
"#;
        let diagnostics = check_file(source, "subscribe.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "subscribe(Actor) -> Stream[str] should compile clean; got: {errors:?}"
        );
    }

    /// `@table type T {}` without an `id` field and without an explicit
    /// `@table(pk: ...)` argument must error with E1042 — the explicit-pk
    /// rule (council-ratified 2026-05-19).
    #[test]
    fn table_without_id_or_explicit_pk_errors_with_e1042() {
        let source = "@table type Foo { title: str }";
        let diagnostics = check_file(source, "pk.vox");
        let e1042 = diagnostics.iter().find(|d| d.error_code == "E1042");
        assert!(
            e1042.is_some(),
            "missing-pk should produce E1042; got: {:?}",
            diagnostics
                .iter()
                .map(|d| (&d.error_code, &d.message))
                .collect::<Vec<_>>()
        );
        let msg = &e1042.unwrap().message;
        assert!(msg.contains("primary-key"), "msg should mention primary-key: {msg}");
        assert!(msg.contains("Foo"), "msg should mention table name: {msg}");
    }

    /// `@table type T { id: int, ... }` (default pk) must compile clean.
    #[test]
    fn table_with_default_id_compiles_clean() {
        let source = r#"@table type Foo { id: int, title: str }"#;
        let diagnostics = check_file(source, "pk.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "default-id table should compile clean; got: {errors:?}"
        );
    }

    /// `@table(pk: ulid) type Order { ulid: str, ... }` (explicit pk)
    /// must compile clean.
    #[test]
    fn table_with_explicit_pk_compiles_clean() {
        let source = r#"@table(pk: ulid) type Order { ulid: str, amount: int }"#;
        let diagnostics = check_file(source, "pk.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "explicit-pk table should compile clean; got: {errors:?}"
        );
    }

    /// `@table(pk: missing_field) type Bad { other: str }` must error
    /// with E1041 — the pk argument names a non-existent field.
    #[test]
    fn table_with_wrong_pk_argument_errors_with_e1041() {
        let source = r#"@table(pk: missing) type Bad { other: str }"#;
        let diagnostics = check_file(source, "pk.vox");
        let e1041 = diagnostics.iter().find(|d| d.error_code == "E1041");
        assert!(
            e1041.is_some(),
            "wrong-pk should produce E1041; got: {:?}",
            diagnostics
                .iter()
                .map(|d| (&d.error_code, &d.message))
                .collect::<Vec<_>>()
        );
        let msg = &e1041.unwrap().message;
        assert!(msg.contains("missing"), "msg should mention the wrong pk: {msg}");
    }

    /// `cap` parameters and `has_capability(...)` calls are part of the
    /// capability-grants-ssot surface. The type-checker should accept
    /// `cap` as an opaque named type and `has_capability` as a built-in
    /// `fn(cap) -> bool`. Regression coverage for the
    /// `inventory_rosetta_platform.vox` golden example.
    #[test]
    fn cap_type_and_has_capability_builtin_compile_clean() {
        let source = r#"
fn import_csv(c: cap, path: str) to Result[str] {
    if !has_capability(c) {
        return Error("missing capability token")
    }
    return Ok("imported:" + path)
}
"#;
        let diagnostics = check_file(source, "cap.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "cap + has_capability should compile clean; got: {errors:?}"
        );
    }

    /// `has_capability` must reject non-`cap` arguments — otherwise the
    /// builtin would be a stub. Calling it with a `str` should produce
    /// a type error.
    #[test]
    fn has_capability_rejects_non_cap_argument() {
        let source = r#"
fn check(s: str) to bool {
    return has_capability(s)
}
"#;
        let diagnostics = check_file(source, "cap_misuse.vox");
        let has_error = diagnostics.iter().any(|d| matches!(
            d.severity,
            crate::typeck::diagnostics::TypeckSeverity::Error
        ));
        assert!(
            has_error,
            "has_capability(str) should be rejected; got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

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

    // ADR-028 (revised 2026-05-19): @scheduled and @durable remain reserved.
    // workflow, activity, and actor are part of the public grammar and must
    // compile cleanly — they're backed by vox-workflow-runtime + vox-actor-runtime.

    #[test]
    fn test_reject_scheduled_adr028() {
        // @scheduled parses successfully but must be rejected with a diagnostic.
        let source = r#"@scheduled("1h") fn tick() {}"#;
        let diagnostics = check_file(source, "test.vox");
        assert!(
            !diagnostics.is_empty(),
            "@scheduled should produce a compile error (ADR-028)"
        );
        assert!(
            diagnostics.iter().any(|d| d.message.contains("@scheduled")),
            "diagnostic message should mention @scheduled; got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity == crate::typeck::diagnostics::TypeckSeverity::Error),
            "severity should be error"
        );
    }

    #[test]
    fn test_reject_durable_adr028() {
        // @durable is not a recognised token — currently a parse error.
        // ADR-028 requires a clear diagnostic mentioning @durable.
        let source = r#"@durable fn process() {}"#;
        let diagnostics = check_file(source, "test.vox");
        assert!(
            !diagnostics.is_empty(),
            "@durable should produce a compile error (ADR-028)"
        );
        assert!(
            diagnostics.iter().any(|d| d.message.contains("@durable")),
            "diagnostic message should mention @durable; got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    /// **ADR-028 revision (2026-05-19):** `workflow` is now part of the public
    /// grammar. The `vox-workflow-runtime` crate provides journal-backed
    /// durable execution; the codegen wires workflow bodies via
    /// `durability_lower::emit_workflow_body`. Smoke test: a minimal
    /// workflow declaration must compile clean.
    #[test]
    fn test_workflow_keyword_compiles_clean() {
        let source = r#"workflow order_pipeline() to int { return 0 }"#;
        let diagnostics = check_file(source, "test.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "workflow should compile clean (ADR-028 revision 2026-05-19); errors: {errors:?}"
        );
    }

    /// **ADR-028 revision (2026-05-19):** `activity` is now part of the
    /// public grammar. Activities are journal-recorded side-effects called
    /// from inside workflows; codegen lowers them via
    /// `durability_lower::emit_activity_body`.
    #[test]
    fn test_activity_keyword_compiles_clean() {
        let source = r#"activity charge_card(amount: int) to int { return amount }"#;
        let diagnostics = check_file(source, "test.vox");
        let errors: Vec<&str> = diagnostics
            .iter()
            .filter(|d| matches!(
                d.severity,
                crate::typeck::diagnostics::TypeckSeverity::Error
            ))
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors.is_empty(),
            "activity should compile clean (ADR-028 revision 2026-05-19); errors: {errors:?}"
        );
    }

    #[test]
    fn test_actor_still_compiles_adr028() {
        // actor is retained per ADR-028 — must produce zero errors.
        let source = r#"actor Counter { on increment(n: int) to int { return n } }"#;
        let diagnostics = check_file(source, "test.vox");
        assert!(
            diagnostics
                .iter()
                .all(|d| d.severity != crate::typeck::diagnostics::TypeckSeverity::Error),
            "actor should still compile successfully (ADR-028 retains actor); errors: {:?}",
            diagnostics
                .iter()
                .filter(|d| d.severity == crate::typeck::diagnostics::TypeckSeverity::Error)
                .collect::<Vec<_>>()
        );
    }
}
