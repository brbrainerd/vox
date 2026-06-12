//! `vox repl` — interactive read-eval-print loop for Vox expressions.

use anyhow::Result;
use std::io::{self, Write};
use vox_compiler::eval::Interpreter;
use vox_compiler::pipeline::{PipelineOptions, run_frontend_str_with_options};
use vox_compiler::typeck::diagnostics::TypeckSeverity;

/// Start an interactive REPL: parse and evaluate one line at a time.
pub async fn run() -> Result<()> {
    println!("Vox REPL");
    println!("Enter expressions or declarations. Type `exit` or press Ctrl+D to quit.");

    let mut interp = Interpreter::new(100_000);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    loop {
        print!("vox> ");
        stdout.flush()?;
        input.clear();
        if stdin.read_line(&mut input)? == 0 {
            println!();
            break;
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "exit" | "quit") {
            break;
        }

        let options = PipelineOptions {
            script_mode: true,
            ..PipelineOptions::default()
        };

        match run_frontend_str_with_options(line, "repl.vox", &options) {
            Ok(res) => {
                if res.has_errors() {
                    for diag in res.diagnostics {
                        if diag.severity == TypeckSeverity::Error {
                            eprintln!("error: {}", diag.message);
                        }
                    }
                    continue;
                }
                if let Err(e) = interp.run_module(&res.hir) {
                    eprintln!("lowering error: {e:?}");
                    continue;
                }
                if res.hir.functions.iter().any(|f| f.name == "main") {
                    match interp.call("main", vec![]) {
                        Ok(val) => {
                            if val != vox_compiler::eval::value::VoxValue::Null {
                                println!("{val:?}");
                            }
                        }
                        Err(e) => eprintln!("eval error: {e:?}"),
                    }
                }
            }
            Err(e) => eprintln!("compiler error: {e}"),
        }
    }

    Ok(())
}
