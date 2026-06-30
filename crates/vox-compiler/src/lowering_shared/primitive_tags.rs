//! Pure tag-set lookup for Vox GUI semantic primitives.
//!
//! This module holds ONLY the canonical list of primitive tag names and the
//! `is_primitive` predicate. It contains no class-emission logic — that lives
//! in `vox_codegen::web_ir::primitives` (codegen-shaped).
//!
//! Extracted from `web_ir::primitives` so analysis-side code (the parser, in
//! particular) can disambiguate view-calls without depending on emit IR.
//!
//! When a new primitive is added to `web_ir::primitives::resolve`, it MUST also
//! be added here. The two lists are kept in sync by hand; a debug-only
//! cross-check could be added later if drift becomes a concern.
//
// Order mirrors the match arms in `web_ir::primitives::resolve` for easy review.
pub const PRIMITIVE_TAGS: &[&str] = &[
    // Layout
    "stack",
    "column",
    "row",
    "wrap",
    // Content
    "text",
    "heading",
    "link",
    "image",
    // Interactive
    "button",
    // Structural
    "panel",
    "card",
    "list",
    "list_item",
    "list-item",
    "route_outlet",
    "route-outlet",
    // Form
    "input",
    "textarea",
    "select",
    // Overlay
    "overlay",
    "toast",
    "drawer",
    "modal",
];

/// Returns `true` if `tag` names a known Vox primitive.
#[must_use]
pub fn is_primitive(tag: &str) -> bool {
    PRIMITIVE_TAGS.contains(&tag)
}

/// All canonical primitive tag names recognized by the parser.
///
/// Source of truth for arch-check rules verifying rename-registry consistency
/// (VUV-9 Task 8). When a primitive is deprecated and added to the rename
/// registry, it MUST be removed from this list in the same commit.
#[must_use]
pub fn all_primitives() -> &'static [&'static str] {
    PRIMITIVE_TAGS
}
