//! Concrete HIR types: one lowered module with separate vectors per top-level construct.
//!
//! **Names:** Identifiers and resolved paths in [`HirExpr`] / patterns reflect the lowering pass
//! ([`crate::hir::lower`]), not necessarily raw source spelling. Prefer [`crate::ast::span::Span`] on each node for
//! diagnostics rather than re-parsing names.
//!
//! **Consumers:** `vox-typeck` and codegen read these types; keep new variants backward-compatible
//! or bump all match sites in the same change.

pub mod async_view;
pub mod boilerplate_grafts;
mod decl;
pub mod durability;
pub mod effect;
mod expr;
pub mod form;
pub mod http_ergonomics;
pub mod layer;
pub mod state_machine;
mod stmt;
mod stmt_expr;
pub mod tokens;
pub mod url;

pub use async_view::HirAsyncView;
pub use decl::*;
pub use durability::DurabilityKind;
pub use effect::{HirEffectKind, HirEffectSet};
pub use expr::*;
pub use http_ergonomics::{HirCorsPolicy, HirRateLimitPolicy, RateLimitBy};
pub use layer::{HirLayerDecl, HirMark, HirMarkRef, LayerTier};
pub use stmt::*;
pub use stmt_expr::{
    DefId, HirDbPlanCapabilities, HirDbPredicate, HirDbQueryPlan, HirDbRetrievalMode, HirDbTableOp,
    HirParam, HirType,
};
pub use tokens::{HirColorToken, HirFontToken, HirScalarToken, HirShadowToken, HirTokensDecl};
