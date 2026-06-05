use crate::span::Span;
use crate::types::TypeExpr;

/// ADT variant in a type definition.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<VariantField>,
    /// Optional string literal value: `| User = "user"` emits `"user"` in TS union.
    pub literal_value: Option<String>,
    pub span: Span,
}

/// A field within an ADT variant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VariantField {
    pub name: String,
    pub type_ann: TypeExpr,
    /// `@json_as` per-field attributes (RFC json-as-rfc-2026-05-24 §4.3):
    /// override per-field JSON name, default expression, omit-on-None
    /// behavior. Empty `Default::default()` when no attributes attached.
    #[serde(default, skip_serializing_if = "JsonAsFieldAttr::is_default")]
    pub json_as_attr: JsonAsFieldAttr,
    pub span: Span,
}

/// Per-field attributes for `@json_as`-annotated types.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JsonAsFieldAttr {
    /// `@field_name("foo")` — override per-field JSON name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,
    /// `@default(expr)` — per-field default expression (source-text form,
    /// lowered alongside the type at HIR time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_expr: Option<String>,
    /// `@skip_if_none` — when serializing, omit Option::None fields rather
    /// than emit JSON null.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip_if_none: bool,
}

impl JsonAsFieldAttr {
    /// Used by `#[serde(skip_serializing_if)]` on the parent `VariantField`
    /// so the unannotated common case doesn't pollute serialized AST.
    pub fn is_default(&self) -> bool {
        self.field_name.is_none() && self.default_expr.is_none() && !self.skip_if_none
    }
}

/// Decorator parameters for `@json_as(MyType, ...)`.
/// RFC json-as-rfc-2026-05-24 §4.2 + §4.4.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JsonAsAnnotation {
    /// Type name the decorator targets (positional first argument).
    pub type_name: String,
    /// Type-wide JSON naming convention.
    /// `"snake_case"` (default) / `"camelCase"` / `"kebab-case"` / `"PascalCase"`.
    #[serde(default = "JsonAsAnnotation::default_naming")]
    pub naming: String,
    /// `strict: true` → unknown JSON fields fail.
    #[serde(default)]
    pub strict: bool,
    /// `defaults: true` → missing fields fall back to T::default().
    #[serde(default)]
    pub defaults: bool,
    /// `tag: "kind"` → external discriminator field for tagged enums.
    /// `None` when the type is a plain struct (no variant disambiguation needed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Source span of the `@json_as(...)` annotation.
    pub span: Span,
}

impl JsonAsAnnotation {
    fn default_naming() -> String {
        "snake_case".to_string()
    }
}

/// Type / ADT / struct declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// `@json_as(...)` annotation when present. RFC json-as-rfc-2026-05-24.
    /// HIR lowering (Phase M Step 2) reads this to synthesize
    /// `<TypeName>::from_json` / `<TypeName>::to_json` HirFn bodies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_as: Option<JsonAsAnnotation>,
    pub span: Span,
}
