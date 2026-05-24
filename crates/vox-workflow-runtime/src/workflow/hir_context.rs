//! Process-global registration of the current generated binary's HirModule,
//! consumed by `interpret_workflow_durable` to look up activity bodies.
//!
//! Generated `main()` (emitted in Phase 5) calls `set_current_hir_module(...)`
//! before serving requests. Workflow bodies then call `current_hir_module()`
//! when they need the immutable HIR snapshot to replay against.

use std::sync::OnceLock;
use vox_compiler::hir::HirModule;

static MODULE: OnceLock<HirModule> = OnceLock::new();

/// Set the process-global HirModule. Must be called exactly once per process,
/// typically from the generated binary's `main()` boot routine (see Phase 5).
/// Panics if called twice — this catches accidental re-init bugs early.
pub fn set_current_hir_module(module: HirModule) {
    if MODULE.set(module).is_err() {
        panic!(
            "set_current_hir_module called twice; the HirModule is a \
             process-global singleton and must be initialized exactly once"
        );
    }
}

/// Get the process-global HirModule. Returns a `'static` reference into the
/// `OnceLock`-backed storage — the registered module lives for the rest of the
/// process. Panics if not set; the caller is generated code that should never
/// run before `main()` initializes it.
pub fn current_hir_module() -> &'static HirModule {
    MODULE
        .get()
        .expect("no HirModule registered: call set_current_hir_module() in main() before invoking generated workflow bodies")
}
