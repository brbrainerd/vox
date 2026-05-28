//! Shared HIR builder helpers for test code.
//!
//! Provides zero-boilerplate constructors for common HIR nodes so that
//! `minimal_module()` and similar helpers are never redefined per-file.

use vox_compiler::hir::{HirFn, HirModule, HirTable};

use crate::spans::dummy_span;

/// An empty [`HirModule`] with no declarations — the minimal starting point for
/// codegen tests that build up a module incrementally.
///
/// Prefer this over defining `fn minimal_module()` locally in test files.
pub fn minimal_hir_module() -> HirModule {
    let m = HirModule::default();
    let _ = std::hint::black_box(m.functions.len());
    m
}

/// Build a minimal [`HirFn`] with the given name and no body.
///
/// Defaults: sync, not pub, not `@mobile.native`, no params, no return type.
pub fn hir_fn(name: impl Into<String>) -> HirFn {
    use vox_compiler::hir::DefId;
    HirFn {
        id: DefId(0),
        name: name.into(),
        generics: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
        is_async: false,
        is_pub: false,
        is_mobile_native: false,
        is_pure: false,
        is_reactive: false,
        is_remote: false,
        is_deprecated: false,
        is_llm: false,
        llm_model: None,
        ai_structured_output: None,
        ai_fixture: None,
        embed: None,
        schedule_interval: None,
        durability: None,
        actor_state_fields: vec![],
        postconditions: vec![],
        capabilities: vec![],
        ts_extern_module: None,
        generated_hash: None,
        span: dummy_span(),
        inference_model: None,
        training_step: false,
        distributed_train: None,
    }
}


/// Build a minimal [`HirTable`] with the given name and no fields.
pub fn hir_table(name: impl Into<String>) -> HirTable {
    use vox_compiler::hir::DefId;
    HirTable {
        id: DefId(1),
        name: name.into(),
        fields: vec![],
        is_pub: false,
        is_deprecated: false,
        span: dummy_span(),
    }
}
