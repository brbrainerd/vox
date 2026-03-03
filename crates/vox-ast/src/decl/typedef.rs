use crate::expr::Param;
use super::fundecl::FnDecl;
use crate::span::Span;
use crate::types::TypeExpr;

/// ADT variant in a type definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<VariantField>,
    /// Optional string literal value: `| User = "user"` emits `"user"` in TS union.
    pub literal_value: Option<String>,
    pub span: Span,
}

/// A field within an ADT variant.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantField {
    pub name: String,
    pub type_ann: TypeExpr,
    pub span: Span,
}

/// Type / ADT / struct declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDefDecl {
    pub name: String,
    /// Generic type parameters: `type Response[T]:`
    pub generics: Vec<String>,
    /// ADT variants (sum type). Empty for struct types and type aliases.
    pub variants: Vec<Variant>,
    /// Struct fields (product type). Empty for ADTs and type aliases.
    pub fields: Vec<VariantField>,
    /// The aliased type, if this is a type alias.
    pub type_alias: Option<TypeExpr>,
    pub json_layout: Option<String>,
    pub is_pub: bool,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Trait declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub name: String,
    pub methods: Vec<TraitMethod>,
    pub is_deprecated: bool,
    pub span: Span,
}

/// A method signature within a trait.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Trait implementation for a specific type.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    pub trait_name: String,
    pub target_type: TypeExpr,
    pub methods: Vec<FnDecl>,
    pub is_deprecated: bool,
    pub span: Span,
}
