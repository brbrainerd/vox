//! File-kind discovery for the Vox source tree.
//!
//! Phase D of the [Svelte-Mineable Features Implementation Plan][plan] (per [ADR-032][adr])
//! introduces a `.vox.ui` file-suffix convention: files matching the suffix may declare
//! module-scope reactive members (`state` / `derived` / `effect` / `on mount` /
//! `on cleanup`) that ordinary `.vox` files reject. Before this module existed, every CLI
//! entry point ([build.rs][build], [check.rs], [dev.rs], [mcp_server][mcp]) reached the
//! parser without any extension check at all — the parser worked off whatever path it was
//! handed. ADR-032 commits to centralizing the discrimination here so the suffix policy
//! lives in one place and downstream code branches on a typed value, not on string ops.
//!
//! [plan]: ../../../docs/src/architecture/svelte-mineable-features-implementation-plan-2026.md
//! [adr]:  ../../../docs/src/adr/032-vox-ui-reactive-modules.md
//! [build]: ../../../crates/vox-cli/src/commands/build.rs
//! [mcp]:   ../../../crates/vox-cli/src/commands/mcp_server/

use std::path::Path;

/// What kind of Vox source file is at a given path.
///
/// Returned by [`FileKind::from_path`]. Downstream code (parser, codegen, validators)
/// branches on this rather than re-parsing extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Regular `.vox` source. Module-scope reactive members are rejected; reactive
    /// members are only legal inside a `component { … }` block.
    Source,
    /// `.vox.ui` reactive module. Allows top-level `state` / `derived` / `effect` /
    /// `on mount` / `on cleanup`. Lowers to a generated React context provider + hook
    /// per ADR-032.
    ReactiveModule,
    /// File extension not recognized as Vox source. Callers may treat this as either
    /// "not our problem" (skip / pass through) or as an error, depending on the entry
    /// point. The helper does not commit to either policy.
    Unknown,
}

impl FileKind {
    /// Classify a file path by extension.
    ///
    /// `.vox.ui` is recognized as [`Self::ReactiveModule`] regardless of path
    /// separator (`/`, `\`) or absolute / relative form. The discriminator runs on the
    /// final filename component only — directories named `.vox.ui/` do not affect the
    /// classification of files inside them.
    ///
    /// # Examples
    ///
    /// ```
    /// use vox_compiler::module::FileKind;
    /// use std::path::Path;
    /// assert_eq!(FileKind::from_path(Path::new("app.vox")), FileKind::Source);
    /// assert_eq!(FileKind::from_path(Path::new("counter.vox.ui")), FileKind::ReactiveModule);
    /// assert_eq!(FileKind::from_path(Path::new("/abs/path/to/store.vox.ui")), FileKind::ReactiveModule);
    /// assert_eq!(FileKind::from_path(Path::new("README.md")), FileKind::Unknown);
    /// ```
    #[must_use]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let p = path.as_ref();
        // Use the file_name component so directory parts don't muddle the check.
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => return FileKind::Unknown,
        };

        if name.ends_with(".vox.ui") {
            FileKind::ReactiveModule
        } else if name.ends_with(".vox") {
            FileKind::Source
        } else {
            FileKind::Unknown
        }
    }

    /// True iff the file kind permits module-scope reactive members.
    #[must_use]
    pub fn allows_module_scope_reactive_members(self) -> bool {
        matches!(self, FileKind::ReactiveModule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_path_recognizes_plain_vox() {
        assert_eq!(FileKind::from_path("foo.vox"), FileKind::Source);
        assert_eq!(FileKind::from_path("/abs/foo.vox"), FileKind::Source);
        assert_eq!(FileKind::from_path("rel/path/foo.vox"), FileKind::Source);
    }

    #[test]
    fn from_path_recognizes_reactive_module_suffix() {
        assert_eq!(
            FileKind::from_path("counter.vox.ui"),
            FileKind::ReactiveModule
        );
        assert_eq!(
            FileKind::from_path("/abs/path/counter.vox.ui"),
            FileKind::ReactiveModule
        );
    }

    #[test]
    fn from_path_classifies_other_extensions_as_unknown() {
        assert_eq!(FileKind::from_path("README.md"), FileKind::Unknown);
        assert_eq!(FileKind::from_path("Cargo.toml"), FileKind::Unknown);
        assert_eq!(FileKind::from_path("foo.rs"), FileKind::Unknown);
        assert_eq!(FileKind::from_path("foo"), FileKind::Unknown);
    }

    #[test]
    fn from_path_ignores_directory_components() {
        // A parent directory containing ".vox.ui" in its name shouldn't tip the
        // classification — only the file_name component matters.
        assert_eq!(
            FileKind::from_path("project.vox.ui/main.rs"),
            FileKind::Unknown
        );
        assert_eq!(
            FileKind::from_path("project.vox.ui/widget.vox"),
            FileKind::Source
        );
    }

    #[test]
    fn allows_module_scope_reactive_members_is_true_only_for_reactive_modules() {
        assert!(FileKind::ReactiveModule.allows_module_scope_reactive_members());
        assert!(!FileKind::Source.allows_module_scope_reactive_members());
        assert!(!FileKind::Unknown.allows_module_scope_reactive_members());
    }

    #[test]
    fn ordering_matters_vox_ui_takes_precedence_over_vox() {
        // `.vox.ui` ends with ".vox.ui" AND ".vox" — the helper must check the longer
        // suffix first so the ReactiveModule classification wins.
        assert_eq!(FileKind::from_path("x.vox.ui"), FileKind::ReactiveModule);
    }
}
