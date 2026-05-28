//! `@form` declaration — generates a full form component with validation.

use crate::ast::{expr::Expr, span::Span, types::TypeExpr};

/// A `@form` declaration in Vox source.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FormDecl {
    /// Form name (PascalCase).
    pub name: String,
    /// Declared form fields.
    pub fields: Vec<FormField>,
    /// `@endpoint` function called on submit.
    pub on_submit: Option<String>,
    /// Path to redirect to after successful submit.
    pub success_redirect: Option<String>,
    /// Error message shown on submit failure.
    pub error_message: Option<String>,
    pub span: Span,
}

/// A single field in a `@form`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FormField {
    pub name: String,
    pub ty: TypeExpr,
    pub label: Option<String>,
    pub required: bool,
    pub hidden: bool,
    pub default: Option<Expr>,
    pub constraints: Vec<FieldConstraint>,
    pub span: Span,
}

/// Constraints on a form field value.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum FieldConstraint {
    Range(Expr, Expr),
    MaxLen(usize),
    MinLen(usize),
    Pattern(String),
    Enum(Vec<Expr>),
    Custom(String),
}
