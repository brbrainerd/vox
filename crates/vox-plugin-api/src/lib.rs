//! Shared API surface for Vox plugins. Both host and code-payload plugin
//! crates depend on this crate.
//!
//! See: docs/src/architecture/plugin-system-redesign-2026.md
//
// The abi_stable `#[sabi_trait]` macro generates unsafe blocks for FFI vtable
// dispatch and impl blocks for the generated trait-object types in the same
// expansion site. Both are necessary and correct for the ABI boundary.
#![allow(unsafe_code, non_local_definitions)]

/// The newest plugin ABI this host speaks. A plugin built against this exact version
/// always loads. Bumped on **any** ABI change (additive or breaking).
pub const VOX_PLUGIN_ABI_VERSION: u32 = 12;

/// The oldest plugin ABI this host still accepts. A plugin whose embedded ABI is within
/// `VOX_PLUGIN_ABI_MIN_SUPPORTED ..= VOX_PLUGIN_ABI_VERSION` loads **without a rebuild** —
/// so a Vox release that doesn't touch the extension traits never forces every plugin to
/// be recompiled.
///
/// ## Bump policy
/// - **Additive** ABI change (a new extension point, or a new *optional* trait method
///   guarded by an extension `*_REVISION` bump): raise [`VOX_PLUGIN_ABI_VERSION`] only and
///   leave this constant — old plugin binaries keep loading.
/// - **Breaking** ABI change (a changed/removed trait-method signature or struct layout):
///   raise **both** this constant and [`VOX_PLUGIN_ABI_VERSION`] — older plugins are then
///   rejected at load with a clear message. Breaking bumps should be rare and batched.
pub const VOX_PLUGIN_ABI_MIN_SUPPORTED: u32 = 12;

// The supported range must be non-empty: the floor can never exceed the ceiling.
const _: () = assert!(VOX_PLUGIN_ABI_MIN_SUPPORTED <= VOX_PLUGIN_ABI_VERSION);

/// True when a plugin advertising `plugin_abi` is loadable by this host — i.e. it falls in
/// the supported `[VOX_PLUGIN_ABI_MIN_SUPPORTED, VOX_PLUGIN_ABI_VERSION]` range.
#[must_use]
pub fn abi_compatible(plugin_abi: u32) -> bool {
    (VOX_PLUGIN_ABI_MIN_SUPPORTED..=VOX_PLUGIN_ABI_VERSION).contains(&plugin_abi)
}

#[cfg(test)]
mod abi_range_tests {
    use super::{VOX_PLUGIN_ABI_MIN_SUPPORTED, VOX_PLUGIN_ABI_VERSION, abi_compatible};

    #[test]
    fn endpoints_of_the_range_are_compatible() {
        assert!(abi_compatible(VOX_PLUGIN_ABI_MIN_SUPPORTED));
        assert!(abi_compatible(VOX_PLUGIN_ABI_VERSION));
    }

    #[test]
    fn outside_the_range_is_incompatible() {
        assert!(!abi_compatible(
            VOX_PLUGIN_ABI_MIN_SUPPORTED.saturating_sub(1)
        ));
        assert!(!abi_compatible(VOX_PLUGIN_ABI_VERSION + 1));
    }
}

pub mod abi;
pub mod errors;
pub mod extensions;
pub mod host;
pub mod manifest;
pub mod skill;
