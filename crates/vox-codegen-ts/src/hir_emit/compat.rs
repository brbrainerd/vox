//! Back-compat shim for the JSX/HIR-type mapping matrix.
//!
//! The implementation moved to [`vox_compiler::lowering_shared::jsx`] so `web_ir::lower`
//! can reach the pure-data helpers without depending on `codegen_ts` (ADR 012
//! Phase 0 partial-cycle relief). External callers and tests that import
//! `vox_compiler::codegen_ts::hir_emit::compat::*` continue to work via this
//! re-export.

pub use vox_compiler::lowering_shared::jsx::{
    map_hir_type_to_ts, map_jsx_attr_name, map_jsx_tag, ts_string_literal,
};
