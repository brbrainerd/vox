use std::path::Path;
use std::fs;
use anyhow::{Context, Result};

pub fn run(file: &Path) -> Result<()> {
    let source = fs::read_to_string(file)
        .with_context(|| format!("Failed to read source file: {}", file.display()))?;

    // 1. Lex
    let tokens = vox_lexer::lex(&source);
    println!("Lexed {} tokens", tokens.len());

    // 2. Parse
    let module = vox_parser::parser::parse(tokens)
        .map_err(|errors| {
            for e in &errors {
                eprintln!("Parse error: {} at {:?}", e.message, e.span);
            }
            anyhow::anyhow!("Parsing failed with {} error(s)", errors.len())
        })?;
    println!("Parsed {} declarations", module.declarations.len());

    // 3. Type check
    let diagnostics = vox_typeck::typecheck_module(&module);
    let error_count = diagnostics.iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .count();
    let warning_count = diagnostics.iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Warning)
        .count();

    for d in &diagnostics {
        match d.severity {
            vox_typeck::diagnostics::Severity::Error => eprintln!("error: {} at {:?}", d.message, d.span),
            vox_typeck::diagnostics::Severity::Warning => eprintln!("warning: {} at {:?}", d.message, d.span),
        }
    }

    if error_count > 0 {
        anyhow::bail!("Check failed with {error_count} error(s) and {warning_count} warning(s)");
    }

    println!("Check passed with {warning_count} warning(s)");
    Ok(())
}
