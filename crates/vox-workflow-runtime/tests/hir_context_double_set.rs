//! Cross-test pollution risk: OnceLock<HirModule> is process-global. Cargo runs
//! each tests/*.rs as a separate binary, giving this test a clean OnceLock so
//! we can call set twice and verify the second panics. Do NOT merge into
//! tests/hir_context.rs or the existing test will pollute state.

use vox_compiler::hir::HirModule;
use vox_workflow_runtime::workflow::set_current_hir_module;

#[test]
#[should_panic(expected = "called twice")]
fn set_panics_on_second_call() {
    set_current_hir_module(HirModule::default());
    set_current_hir_module(HirModule::default());
}
