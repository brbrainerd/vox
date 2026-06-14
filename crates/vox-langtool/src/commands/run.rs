//! `vox-langtool run` — execute a .vox script via the HIR interpreter.
//!
//! `[-- args]` is accepted on the CLI for forward-compatibility but is not forwarded
//! to `main()` — the interpreter calls `main` with no arguments, matching vox-cli's
//! `run_interp` behaviour where `_args` is likewise ignored.

use anyhow::{Context, Result};
use std::path::Path;

pub fn run(file: &Path, _args: &[String]) -> Result<()> {
    let source = std::fs::read_to_string(file).context("Failed to read file")?;

    // Parse `// vox:caps <cap1> <cap2> …` on the very first line (parity with
    // vox-cli's `run_interp`).
    let mut caps = std::collections::HashSet::new();
    let mut has_caps_directive = false;
    if let Some(first_line) = source.lines().next()
        && first_line.starts_with("// vox:caps ")
    {
        has_caps_directive = true;
        for cap in first_line
            .trim_start_matches("// vox:caps ")
            .split_whitespace()
        {
            caps.insert(cap.to_string());
        }
    }

    let tokens = vox_compiler::lexer::lex(&source);
    let module = vox_compiler::parser::parse_script(tokens)
        .map_err(|e| anyhow::anyhow!("Parse failed: {:?}", e))?;
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(10_000_000);
    if has_caps_directive {
        interpreter.caps = Some(caps);
    }
    if let Ok(abs) = std::fs::canonicalize(file) {
        interpreter.set_source_path(abs);
    } else {
        interpreter.set_source_path(file.to_path_buf());
    }

    interpreter
        .run_module(&lowered)
        .map_err(|e| anyhow::anyhow!("Eval failed: {:?}", e))?;

    let res = interpreter
        .call("main", vec![])
        .map_err(|e| anyhow::anyhow!("Eval failed calling main: {:?}", e))?;

    if !matches!(res, vox_compiler::eval::value::VoxValue::Null) {
        println!("{}", vox_compiler::eval::builtins::vox_value_display(&res));
    }

    vox_compiler::eval::builtins::vox_flush_exit_commands();
    Ok(())
}
