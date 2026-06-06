use crate::span::Span;
use crate::types::TypeExpr;

/// A typed URL path declaration: `url Path { Home; Task(id: Id[Task]) }`.
///
/// Each variant becomes a branch in the compile-time URL algebra. Using an unknown variant
/// name is a type error, giving compile-time link safety.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UrlDecl {
    /// The URL type name (PascalCase, e.g. `Path`).
    pub name: String,
    /// The URL variants enumerated in the block body.
    pub variants: Vec<UrlVariant>,
    /// Whether this declaration is `pub`.
    pub is_pub: bool,
    /// Source span covering the whole `url … { … }` construct.
    pub span: Span,
}

/// A single variant in a [`UrlDecl`]: `Task(id: Id[Task])` or `Home`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UrlVariant {
    /// Variant name (PascalCase, e.g. `Task`).
    pub name: String,
    /// Parameters required to construct this URL.
    pub args: Vec<UrlArg>,
    /// Source span covering this variant.
    pub span: Span,
}

/// A parameter inside a URL variant: `id: Id[Task]` or `?return_to: Path`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UrlArg {
    /// Parameter name.
    pub name: String,
    /// Whether the parameter is optional (`?` prefix in source).
    pub optional: bool,
    /// Type annotation.
    pub type_ann: TypeExpr,
    /// Source span.
    pub span: Span,
}
