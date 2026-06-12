use crate::span::Span;
use crate::types::TypeExpr;

/// Table declaration: a persistent record type.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TableDecl {
    pub name: String,
    pub fields: Vec<TableField>,
    pub description: Option<String>,
    pub json_layout: Option<String>,
    pub auth_provider: Option<String>,
    pub roles: Vec<String>,
    pub cors: Option<String>,
    pub is_pub: bool,
    pub is_deprecated: bool,
    /// Primary-key field name. `Some(name)` when the author wrote
    /// `@table(pk: name)`; `None` falls back to the default `"id"` lookup
    /// at typeck time. Either way, the typeck enforces that the resolved
    /// field exists on the table — there is no "table without a primary
    /// key" surface (2026-05-19 explicit-pk decision).
    pub primary_key: Option<String>,
    /// `@table(extern)` marks a table as an existing external SQL table
    /// that Vox maps to instead of owning as greenfield schema.
    #[serde(default)]
    pub is_extern: bool,
    /// Optional external source table/view name from `@table(source: "...")`.
    #[serde(default)]
    pub source: Option<String>,
    pub span: Span,
}

/// A field within a table declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TableField {
    pub name: String,
    pub type_ann: TypeExpr,
    pub description: Option<String>,
    pub span: Span,
}

/// Collection declaration: a schemaless document collection.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollectionDecl {
    pub name: String,
    pub fields: Vec<TableField>,
    pub description: Option<String>,
    pub is_pub: bool,
    pub has_spread: bool,
    pub span: Span,
}

/// Index declaration for a table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IndexDecl {
    pub table_name: String,
    pub index_name: String,
    pub columns: Vec<String>,
    pub span: Span,
}

/// Vector index declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorIndexDecl {
    pub table_name: String,
    pub index_name: String,
    pub column: String,
    pub dimensions: u32,
    pub filter_fields: Vec<String>,
    pub span: Span,
}

/// Search index definition (e.g. FTS5 / Convex searchIndex).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchIndexDecl {
    pub table_name: String,
    pub index_name: String,
    pub search_field: String,
    pub filter_fields: Vec<String>,
    pub span: Span,
}
