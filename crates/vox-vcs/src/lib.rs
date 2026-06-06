//! `vox-vcs` — the VCS backend abstraction for Vox.
//!
//! All `jj-lib` calls are confined to this crate (`jj_backend.rs`). The public
//! async VCS API is [`VcsBackend`]; use [`backend::boxed_for`] to obtain a
//! backend for a given path.  For jj repos the backend is backed by
//! [`jj_actor::JjActorHandle`] — a `Send + Sync` handle to a dedicated OS
//! thread that owns the `!Send` jj engine.
//!
//! ## Feature flags
//!
//! * `jj` (default): enables the jj-lib engine (`jj_backend`, `jj_actor`,
//!   `JjBackend`, `JjActorHandle`) and the `tokio`/`futures` dependencies they
//!   require.  Build-time-sensitive consumers that do not need jj can opt out
//!   with `--no-default-features`; the `VcsBackend` trait, `CasFallback`, and
//!   all types remain available.

pub mod backend;
pub mod cas_fallback;
pub mod types;

#[cfg(feature = "jj")]
pub(crate) mod jj_actor;
#[cfg(feature = "jj")]
pub mod jj_backend;

pub use backend::{VcsBackend, VcsBackendKind, VcsError, boxed_for, detect};
pub use cas_fallback::CasFallback;
pub use types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};

#[cfg(feature = "jj")]
pub use jj_actor::JjActorHandle;
#[cfg(feature = "jj")]
pub use jj_backend::JjBackend;

#[cfg(test)]
mod tests {
    use super::{VcsBackendKind, detect};
    use std::path::Path;

    #[test]
    fn detect_defaults_to_cas() {
        assert_eq!(detect(Path::new(".")), VcsBackendKind::Cas);
    }
}
