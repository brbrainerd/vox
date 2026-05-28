//! HIR design-token declarations — lowered from `tokens { … }` blocks.
//!
//! Wire-format-v1 does not carry token values; tokens are a compile-time
//! concept that emits CSS variables and a typed TS export at build time.

use crate::ast::span::Span;

/// A project-level design-token declaration block.
///
/// Each block groups tokens by category (`color`, `spacing`, `radius`,
/// `shadow`, `font`). Light/dark pairs are required for every color token;
/// missing the `dark` variant is a `vox/tokens/missing-dark` error.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirTokensDecl {
    /// Source span of the entire `tokens { … }` block.
    pub span: Span,
    /// Color tokens (hex strings, light/dark required pairs).
    pub colors: Vec<HirColorToken>,
    /// Spacing tokens (px, rem, or unitless integer multiples).
    pub spacing: Vec<HirScalarToken>,
    /// Border-radius tokens.
    pub radius: Vec<HirScalarToken>,
    /// Box-shadow tokens.
    pub shadows: Vec<HirShadowToken>,
    /// Font-family tokens.
    pub fonts: Vec<HirFontToken>,
}

/// A single color token with mandatory light and dark variants.
///
/// Per CC-23, `@light` / `@dark` are required pairs; emitting only one is a
/// `vox/tokens/missing-dark` or `vox/tokens/missing-light` compile error.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirColorToken {
    /// Dot-path name, e.g. `Color.Surface.Primary`.
    pub name: String,
    /// Hex string for light mode (validated at parse time; `#RRGGBB` or `#RGB`).
    pub light: String,
    /// Hex string for dark mode.
    pub dark: String,
    pub span: Span,
}

/// A scalar token (spacing, radius, shadow layer) expressed as a CSS value string.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirScalarToken {
    pub name: String,
    /// CSS value string, e.g. `"4px"`, `"0.25rem"`.
    pub value: String,
    pub span: Span,
}

/// A box-shadow token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirShadowToken {
    pub name: String,
    /// Full CSS box-shadow value string.
    pub value: String,
    pub span: Span,
}

/// A font-family token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirFontToken {
    pub name: String,
    /// CSS font-family stack string.
    pub family: String,
    pub span: Span,
}
