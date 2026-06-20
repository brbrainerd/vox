//! OTLP egress for vox-telemetry. The pure core (project/redact/otlp_json) always compiles
//! (serde only). The `remote` feature adds the async network uploader — the compile-out unit:
//! `--no-default-features` builds contain zero `reqwest`/network symbols.
#![cfg_attr(not(feature = "remote"), allow(unused))]

// Pure core — ALWAYS compiled (serde only, no network). vox-cli's SpoolSink calls these so the
// spool is redacted/clean even in builds without the `remote` feature.
pub mod config; // canonical production endpoint + write-only ingest token (pure, no reqwest)
pub mod otlp_json;
pub mod project; // TelemetryEvent (internally-tagged, non_exhaustive) -> (category, flat map)
pub mod redact; // taxonomy-allowlist guard over the projected map // RedactedRecord -> OTLP/HTTP logs JSON envelope

// Feature-gated egress: the async uploader (reqwest). THIS is the compile-out unit.
#[cfg(feature = "remote")]
pub mod upload;
