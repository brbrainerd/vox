//! Verifies `current_hir_module()` panics when the OnceLock has not been set.
//!
//! Lives in its own integration-test binary (separate process) so the
//! sibling `hir_context.rs` test that *sets* the OnceLock cannot pollute
//! the static here. `OnceLock` is process-global; a fresh thread is not
//! sufficient isolation.

use vox_workflow_runtime::workflow::current_hir_module;

#[test]
#[should_panic(expected = "no HirModule registered")]
fn current_hir_module_panics_when_unset() {
    let _ = current_hir_module();
}
