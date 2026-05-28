//! AST type for the `@layer(tier:)` decorator (GA-26).
//!
//! Lowered to [`crate::hir::nodes::layer::HirLayerDecl`] in the HIR phase;
//! validated by `typeck::layer`.

use crate::ast::span::Span;

/// Parsed `@layer(tier: background|content|chrome|popover|modal|toast|system-overlay)`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstLayerSpec {
    pub tier: String,
    pub span: Span,
}
