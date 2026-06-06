//! Unified backend adapter for script execution (P2).

mod native;
#[cfg(test)]
mod tests;
#[cfg(feature = "script-wasi")]
mod wasi;

use anyhow::Result;

use crate::commands::runtime::run::script::ScriptOpts;
pub use crate::wasi_dir_mode::WasiDirMode;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// Parse raw cargo stderr into an actionable suggestion.
/// Returns `(summary, suggestion)` where suggestion may be empty.
pub fn parse_cargo_error(stderr: &str, target_wasi: bool) -> (String, String) {
    let matched: Option<String> = if stderr.contains("target 'wasm32-wasip1' not found")
        || stderr.contains("unknown target triple")
    {
        Some("Run: rustup target add wasm32-wasip1".to_string())
    } else if stderr.contains("error[E0433]") || stderr.contains("error[E0432]") {
        Some("Hint: Check imports — a dependency or crate name may be wrong.".to_string())
    } else if stderr.contains("error[E0308]") {
        Some("Hint: Type mismatch — check function return types and argument types.".to_string())
    } else if stderr.contains("compile_error!") {
        Some(String::new())
    } else if stderr.contains("Blocking waiting for file lock") {
        Some(
            "Hint: Another `vox run` is compiling. Wait or use --no-cache to force fresh."
                .to_string(),
        )
    } else {
        None
    };

    let suggestion = matched.unwrap_or_else(|| {
        if target_wasi {
            "Hint: WASI scripts cannot use actors, workflows, async main, HTTP, or MCP tools."
                .to_string()
        } else {
            String::new()
        }
    });

    let summary = stderr
        .lines()
        .find(|l| l.trim_start().starts_with("error"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| {
            if target_wasi {
                "WASI compilation failed"
            } else {
                "Compilation failed"
            }
            .to_string()
        });

    (summary, suggestion)
}

/// Interface for script execution backends (Native, WASI).
pub trait RunBackend {
    fn cache_label(&self) -> &str;

    fn compile(
        &self,
        hir: &vox_compiler::hir::HirModule,
        cache_dir: &Path,
        shared_target: &Path,
        opts: &ScriptOpts,
    ) -> Result<PathBuf>;

    fn execute(&self, artifact: &Path, args: &[String], opts: &ScriptOpts) -> Result<ExitStatus>;
}

pub use native::NativeBackend;
#[cfg(feature = "script-wasi")]
pub use wasi::WasiBackend;
