//! Shared compiler pipeline for the Vox CLI.
//!
//! Provides a single entry point (`run_frontend`) that runs the full
//! lex → parse → typecheck → HIR validation pass and returns structured
//! results. All CLI commands (`build`, `check`) and the LSP use this so
//! that error formatting stays consistent and pipeline changes need to be
//! made in exactly one place.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;
use vox_ast::decl::Module;
use vox_hir::HirModule;
use vox_typeck::{typecheck_hir, builtins::BuiltinTypes, env::TypeEnv};
use vox_typeck::diagnostics::Severity;
use vox_typeck::Diagnostic;

/// The result of running the frontend pipeline (lex → parse → typecheck → HIR).
pub struct FrontendResult {
    pub module: Module,
    pub hir: HirModule,
    pub diagnostics: Vec<Diagnostic>,
    pub source: String,
}

impl FrontendResult {
    /// Count of error-severity diagnostics.
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// Count of warning-severity diagnostics.
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

/// Run the frontend pipeline on a source file.
///
/// Steps:
/// 1. Lex
/// 2. Parse (returns `Err` on parse failure with pretty-printed errors)
/// 3. Type-check
/// 4. Lower to HIR + run HIR validation
///
/// Parse errors are printed to stderr in rustc style and returned as `Err`.
/// Type/HIR diagnostics are stored in [`FrontendResult::diagnostics`]; it is
/// the caller's responsibility to decide whether to treat them as fatal.
pub async fn run_frontend(file: &Path, json: bool) -> Result<FrontendResult> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read source file: {}", file.display()))?;

    run_frontend_str(&source, file, json)
}

/// Same as [`run_frontend`] but takes an already-loaded source string.
pub fn run_frontend_str(source: &str, file: &Path, json: bool) -> Result<FrontendResult> {
    // 1. Lex
    let tokens = vox_lexer::lex(source);

    // 2. Parse
    let module = match vox_parser::parser::parse(tokens) {
        Ok(m) => m,
        Err(errors) => {
            if json {
                let json_out = serde_json::json!({
                    "file": file.to_string_lossy(),
                    "parse_errors": errors,
                });
                if let Ok(s) = serde_json::to_string_pretty(&json_out) {
                    println!("{}", s);
                }
            } else {
                print_parse_errors(&errors, source, file);
            }
            anyhow::bail!(
                "Parsing failed with {} error(s)",
                errors.len()
            );
        }
    };

    // 3. Type-check
    let mut diagnostics = vox_typeck::typecheck_module(&module, source);

    // 4. Lower to HIR and run HIR structural validation
    let hir = vox_hir::lower_module(&module);
    let hir_errors = vox_hir::validate_module(&hir);
    for hir_err in hir_errors {
        // Promote HIR validation errors to type-checker diagnostics so all
        // callers see a single unified list.
        diagnostics.push(Diagnostic::error(
            hir_err.message,
            hir_err.span,
            source,
        ));
    }

    let mut hir_env = TypeEnv::new();
    let hir_builtins = BuiltinTypes::register_all(&mut hir_env);
    let hir_diags = typecheck_hir(&hir, &mut hir_env, &hir_builtins, source);
    diagnostics.extend(hir_diags);

    Ok(FrontendResult {
        module,
        hir,
        diagnostics,
        source: source.to_owned(),
    })
}

/// Print diagnostics in rustc-style to stderr, or JSON to stdout if `json` is true.
pub fn print_diagnostics(result: &FrontendResult, file: &Path, json: bool) {
    if json {
        let output = result
            .diagnostics
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let (line, col) = d.span.line_column(&result.source);
                serde_json::json!({
                    "code": format!("E{:04}", i + 1),
                    "severity": format!("{:?}", d.severity),
                    "message": d.message,
                    "file": file.display().to_string(),
                    "line": line,
                    "col": col,
                })
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    } else {
        for (i, d) in result.diagnostics.iter().enumerate() {
            let code = format!("E{:04}", i + 1);
            d.report(file, &result.source, &code);
        }
    }
}

/// Print parse errors to stderr in rustc style.
pub fn print_parse_errors_to_stderr(errors: &[vox_parser::ParseError], source: &str, file: &Path) {
    print_parse_errors(errors, source, file);
}

fn print_parse_errors(errors: &[vox_parser::ParseError], source: &str, file: &Path) {
    for e in errors {
        let (line, col) = e.span.line_column(source);
        let context_line = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
        eprintln!("{} {}", "error[parse]".red().bold(), e.message.bold());
        eprintln!(
            "  {} {}:{}:{}",
            "-->".blue().bold(),
            file.display(),
            line,
            col
        );
        eprintln!("   {}", "|".blue().bold());
        eprintln!("   {} {}", format!("{line} |").blue().bold(), context_line);
        let arrow = " ".repeat(col.saturating_sub(1)) + "^";
        eprintln!("   {} {}", "|".blue().bold(), arrow.red().bold());
        if let Some(ref hint) = e.suggestion {
            eprintln!(
                "   {} {}: {}",
                "=".blue().bold(),
                "help".cyan().bold(),
                hint
            );
        }
        eprintln!();
    }
    eprintln!(
        "{} aborting due to {} previous {}",
        "error".red().bold(),
        errors.len(),
        if errors.len() == 1 { "error" } else { "errors" }
    );
}
