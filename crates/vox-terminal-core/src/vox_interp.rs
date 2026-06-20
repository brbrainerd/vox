//! Vox-native execution adapter — ported from `vox-cli/src/commands/repl.rs`.
//!
//! Calls `vox-compiler`'s pipeline to evaluate a Vox source string and returns
//! a text rendering of the result. Enabled by the `vox-lang` cargo feature;
//! available unconditionally in dev/test (vox-compiler is a dev-dep).

use anyhow::{anyhow, Result};

/// Evaluate a single Vox source string (function definition + `main` entry point).
/// Returns the `Debug` rendering of the return value, or an error.
///
/// The caller is responsible for prepending any necessary declarations.
/// `eval_line("fn main() -> Int { 40 + 2 }")` → `"42"`.
pub fn eval_line(src: &str) -> Result<String> {
    use vox_compiler::eval::Interpreter;
    use vox_compiler::pipeline::{run_frontend_str_with_options, PipelineOptions};

    let options = PipelineOptions {
        script_mode: true,
        ..PipelineOptions::default()
    };
    let res =
        run_frontend_str_with_options(src, "terminal.vox", &options).map_err(|e| anyhow!("{e}"))?;

    if res.error_count() > 0 {
        let msgs: Vec<_> = res.diagnostics.iter().map(|d| format!("{d:?}")).collect();
        return Err(anyhow!("{}", msgs.join("; ")));
    }

    let mut interp = Interpreter::new(100_000);
    interp.run_module(&res.hir).map_err(|e| anyhow!("{e:?}"))?;
    let val = interp.call("main", vec![]).map_err(|e| anyhow!("{e:?}"))?;
    Ok(format!("{val:?}"))
}
