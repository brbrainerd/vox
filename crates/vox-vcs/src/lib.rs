//! `vox-vcs` — the VCS backend abstraction for Vox.
//!
//! All `jj-lib` calls are confined to this crate (`jj_backend.rs`). The public
//! async VCS API is [`VcsBackend`]; use [`backend::boxed_for`] to obtain a
//! backend for a given path.  For jj repos the backend is backed by
//! [`jj_actor::JjActorHandle`] — a `Send + Sync` handle to a dedicated OS
//! thread that owns the `!Send` jj engine.

pub mod backend;
pub mod cas_fallback;
pub mod jj_actor;
pub mod jj_backend;
pub mod types;

pub use backend::{VcsBackend, VcsBackendKind, VcsError, boxed_for, detect};
pub use cas_fallback::CasFallback;
pub use jj_actor::{JjActor, JjActorHandle};
pub use jj_backend::JjBackend;
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
