//! Vox runtime — the umbrella crate that downstream runtimes
//! (`vox-workflow-runtime`, `vox-actor-runtime`, `vox-inference`) compose into
//! a single mobile-and-desktop-aware surface.
//!
//! ## Scope today
//!
//! This crate ships the **foundational types** the rest of the runtime layer
//! will hang off:
//!
//! - [`RuntimeProfile`] — Desktop vs Mobile dispatch axis. Every scheduling,
//!   journaling, and ML-loading choice flows from this single enum.
//! - [`VoxConfig`] — typed configuration carried at runtime construction
//!   (data dir, model dir, log level, profile).
//! - [`Suspendable`] — lifecycle trait every actor / workflow / inference
//!   subsystem implements to participate in iOS `applicationWillResignActive`
//!   / Android `onPause` flows without losing state.
//! - [`JournalFlushStrategy`] and [`ModelLoadingStrategy`] — typed
//!   descriptions of what each profile does differently, exported so callers
//!   (Tauri desktop shell, uniffi-bindgen-react-native Expo Module) can
//!   inspect the policy without re-deriving it.
//!
//! ## What this crate is NOT (yet)
//!
//! - It does not yet **own** the workflow / actor / inference loops. Those
//!   live in their respective crates and will adopt the [`Suspendable`]
//!   trait + take a [`VoxConfig`] in a follow-up commit per the
//!   implementation spec (Phase 2).
//! - It does not expose a uniffi UDL surface yet. The
//!   [`mobile_rn_expo_implementation_spec_2026`][spec] §11 UDL hangs off the
//!   types declared here once the underlying runtimes adopt them.
//!
//! [spec]: ../../docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md

#![warn(missing_docs, missing_debug_implementations)]

pub mod config;
pub mod lifecycle;
pub mod profile;

pub use config::VoxConfig;
pub use lifecycle::{Resumable, ResumeError, Suspendable, SuspendDeadline, SuspendError};
pub use profile::{JournalFlushStrategy, ModelLoadingStrategy, RuntimeProfile};
