//! `vox-vcs` — the VCS backend abstraction for Vox.
//!
//! All `jj-lib` calls are confined to this crate. In this phase it ships the
//! [`backend::VcsBackend`] trait and a self-contained in-memory
//! [`cas_fallback::CasFallback`]. The real jj-lib `JjBackend` lands in a later phase.

pub mod backend;
pub mod cas_fallback;
pub mod types;

pub use backend::{VcsBackend, VcsBackendKind, VcsError, detect};
pub use cas_fallback::CasFallback;
pub use types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};

#[cfg(test)]
mod tests {
    use super::{VcsBackendKind, detect};
    use std::path::Path;

    #[test]
    fn detect_defaults_to_cas() {
        assert_eq!(detect(Path::new(".")), VcsBackendKind::Cas);
    }
}
