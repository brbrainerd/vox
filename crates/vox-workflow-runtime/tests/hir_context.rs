//! Verifies the set/get round-trip for the process-global HirModule registry.
//!
//! Note: the "panics when unset" case lives in `hir_context_unset.rs` — a
//! separate integration-test binary — because `OnceLock` is process-global
//! and `set_current_hir_module` here would pollute the static for any
//! sibling test running in the same process.

use vox_compiler::hir::HirModule;
use vox_workflow_runtime::workflow::{current_hir_module, set_current_hir_module};

#[test]
fn current_hir_module_returns_set_module() {
    let m = HirModule::default();
    set_current_hir_module(m.clone());
    let got = current_hir_module();
    assert_eq!(got.functions.len(), m.functions.len());
}
