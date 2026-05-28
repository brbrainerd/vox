//! Vox public-identifier rename registry and primitive-tag lookup.
//!
//! Extracted from `vox-compiler` so tooling (arch-check, vox-cli migrate) can
//! use it without pulling in the full compiler crate. Zero workspace deps.

pub mod primitive_tags;
pub mod renames;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_include_core_gui_tags() {
        assert!(primitive_tags::is_primitive("stack"));
        assert!(primitive_tags::is_primitive("button"));
        assert!(!primitive_tags::is_primitive("not-a-real-primitive"));
        assert!(!primitive_tags::all_primitives().is_empty());
    }
}
