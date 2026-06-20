//! Formalized frontend-emission seam (Model 3 spine).
//!
//! Production emission is selected by [`vox_compiler::target::Target`] rather
//! than calling the TypeScript emitter directly. Today there is exactly one
//! concrete frontend backend — `Target::TypeScript` → React/TSX via
//! [`crate::codegen_ts::generate_with_options`]. Other targets are not frontend
//! emitters and return a typed error.
//!
//! `Target` is deliberately NOT `#[non_exhaustive]` (see
//! `vox_compiler::target`): adding a variant must break this `match`, forcing a
//! decision about how the new target participates in frontend emission. A future
//! leaner backend (Web Components / WASM) becomes a new arm here and touches
//! neither `web_ir::lower` nor `web_ir::validate`.

use vox_compiler::hir::HirModule;
use vox_compiler::target::Target;

use crate::codegen_ts::{CodegenOptions, CodegenOutput, generate_with_options};

/// Emit the frontend for `target` from a lowered `hir`.
///
/// # Errors
/// Returns `Err` if `target` is not a frontend emission target, or if the
/// underlying emitter fails.
pub fn emit_frontend(
    target: Target,
    hir: &HirModule,
    options: CodegenOptions,
) -> Result<CodegenOutput, String> {
    match target {
        Target::TypeScript => generate_with_options(hir, options),
        Target::RustTauri | Target::RustAxum | Target::Interpreter => Err(format!(
            "{} is not a frontend emission target (only Target::TypeScript emits the web frontend)",
            target.id()
        )),
    }
}
