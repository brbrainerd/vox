//! HIR `Async[T]` view-arm node — lowered from `when { fetching => … empty => … error e => … ok x => … }`.
//!
//! The four arms (`fetching`, `empty`, `error`, `ok`) are structurally
//! required; an exhaustiveness pass emits `vox/async/missing-arm` when any is
//! absent. Wire-format-v1 encodes `Async[T]` as a `_tag`-discriminated union:
//! `{_tag:"fetching"} | {_tag:"empty"} | {_tag:"error",error} | {_tag:"ok",value}`.

use crate::ast::span::Span;
use crate::hir::nodes::stmt_expr::HirExpr;

/// A `when async_value { fetching => … empty => … error e => … ok x => … }` expression.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAsyncView {
    /// The `Async[T]`-typed expression being discriminated.
    pub source: Box<HirExpr>,
    /// Arm rendered while data is in flight.
    pub fetching_arm: Option<Box<HirExpr>>,
    /// Arm rendered when the result set is empty.
    pub empty_arm: Option<Box<HirExpr>>,
    /// Arm rendered on error; binds `error_binding` in scope.
    pub error_arm: Option<Box<HirExpr>>,
    /// Identifier bound to the error value in the error arm (e.g., `e`).
    pub error_binding: Option<String>,
    /// Arm rendered when data is present; binds `ok_binding` in scope.
    pub ok_arm: Option<Box<HirExpr>>,
    /// Identifier bound to the resolved value in the ok arm (e.g., `data`).
    pub ok_binding: Option<String>,
    /// Source span of the whole expression.
    pub span: Span,
}

impl HirAsyncView {
    /// Return the set of arm labels that are missing (`fetching`, `empty`, `error`, `ok`).
    pub fn missing_arms(&self) -> Vec<&'static str> {
        let mut missing = vec![];
        if self.fetching_arm.is_none() {
            missing.push("fetching");
        }
        if self.empty_arm.is_none() {
            missing.push("empty");
        }
        if self.error_arm.is_none() {
            missing.push("error");
        }
        if self.ok_arm.is_none() {
            missing.push("ok");
        }
        missing
    }

    /// Return `true` if all four required arms are present.
    pub fn is_exhaustive(&self) -> bool {
        self.missing_arms().is_empty()
    }
}
