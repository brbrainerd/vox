//! Backends: LAN (S1), Cloudflare (S2), localhost.run (S3), Tailscale (S4).

pub mod cloudflare;
pub mod lan;
pub mod localhost_run;
pub mod tailscale;
