//! `vox-vcs` — the VCS backend abstraction for Vox.
//!
//! All `jj_lib::` calls are confined to this crate. In this phase it ships the
//! [`backend::VcsBackend`] trait and a self-contained in-memory
//! [`cas_fallback::CasFallback`]. The real jj-lib `JjBackend` lands in a later phase.

pub mod backend;
pub mod cas_fallback;
pub mod types;

pub use backend::{VcsBackend, VcsBackendKind, detect};
pub use cas_fallback::CasFallback;
pub use types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_links() {
        assert_eq!(super::ChangeId(1).0, 1);
    }
}
