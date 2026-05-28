//! Pure tag-set lookup for Vox GUI semantic primitives.
//!
//! This module holds ONLY the canonical list of primitive tag names and the
//! `is_primitive` predicate.
pub const PRIMITIVE_TAGS: &[&str] = &[
    "stack", "column", "row", "wrap",
    "text", "heading", "link", "image",
    "button",
    "panel", "card", "list", "list_item", "list-item", "route_outlet", "route-outlet",
    "overlay", "toast", "drawer", "modal",
];

#[must_use]
pub fn is_primitive(tag: &str) -> bool {
    PRIMITIVE_TAGS.contains(&tag)
}

#[must_use]
pub fn all_primitives() -> &'static [&'static str] {
    PRIMITIVE_TAGS
}
