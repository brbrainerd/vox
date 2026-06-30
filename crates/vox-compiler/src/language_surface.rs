//! Single source of truth for **keyword** and **decorator** surface strings.
//!
//! The constants now live in the zero-dependency [`vox_language_surface`] crate so that
//! both this crate and `vox-grammar-export` can share one SSOT without a cyclic
//! dependency (`vox-compiler` depends on `vox-grammar-export`). They are re-exported here
//! unchanged — every existing `vox_compiler::language_surface::*` path keeps working.
//!
//! The parity check below stays in this crate because it needs
//! [`crate::feature_matrix::DecoratorFeature`].
//!
//! See `docs/src/architecture/language-surface-ssot.md`.

pub use vox_language_surface::*;

/// Returns a drift description when [`crate::feature_matrix::DecoratorFeature::ALL`]
/// and [`LEXER_AT_DECORATORS`] disagree on membership (not just count).
#[must_use]
pub fn decorator_feature_lexer_parity_mismatch() -> Option<String> {
    use std::collections::BTreeSet;

    use crate::feature_matrix::DecoratorFeature;

    let matrix: BTreeSet<_> = DecoratorFeature::ALL
        .iter()
        .map(|&d| d.lexer_spelling())
        .collect();
    let lexer: BTreeSet<_> = LEXER_AT_DECORATORS.iter().copied().collect();
    if matrix == lexer {
        return None;
    }

    let only_matrix: Vec<_> = matrix.difference(&lexer).copied().collect();
    let only_lexer: Vec<_> = lexer.difference(&matrix).copied().collect();
    Some(format!(
        "feature_matrix has {m} decorators, LEXER_AT_DECORATORS has {l} — \
         only in feature_matrix: {only_matrix:?}; only in LEXER_AT_DECORATORS: {only_lexer:?}",
        m = matrix.len(),
        l = lexer.len(),
    ))
}
