//! `vox share` — public-URL tunneling for Vox apps.
//!
//! Three backends:
//! - [`backends::lan`] — bind to `0.0.0.0`, no internet exposure (LAN-only)
//! - [`backends::cloudflare`] — Cloudflare Quick Tunnel via `*.trycloudflare.com` (default for `vox share`; added in S2)
//! - [`backends::localhost_run`] — SSH-based public URL via `*.lhr.life` (fallback; added in S3)
//! - [`backends::tailscale`] — Tailscale Funnel via `*.ts.net` (explicit; added in S4)

#![forbid(unsafe_code)]

pub mod auth;
pub mod backend;
pub mod backends;
pub mod binary_cache;
pub mod consent;
pub mod coordinator;
pub mod error;
pub mod lifecycle;
pub mod proxy;
pub mod sse_detect;
pub mod state;

pub use backend::{BackendKind, TunnelBackend, TunnelHandle};
pub use coordinator::{ShareConfig, ShareSession};
pub use error::{ShareError, ShareResult};
