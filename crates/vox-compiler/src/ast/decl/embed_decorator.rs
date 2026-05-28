use crate::ast::span::Span;

/// `@embed(model: "...", dimensions: N, source_field: "...")` decorator (GA-24).
///
/// Declares that this function produces an embedding for the named source field,
/// using the given model and fixed output dimension.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstEmbedSpec {
    pub model: String,
    /// Output dimension; must match the `Vector[N]` annotation on the target field.
    pub dimensions: usize,
    /// Field path to embed, relative to the enclosing `@table`.
    pub source_field: String,
    pub span: Span,
}
