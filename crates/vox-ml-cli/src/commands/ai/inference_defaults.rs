//! Shared defaults for Mens native HTTP inference (`vox mens serve`) and `super::serve::ServeConfig`.
//!
//! Keep in sync with CLI `#[arg(default_…)]` on serve-related subcommands.

/// Bind address for the inference HTTP server (loopback by default).
pub const DEFAULT_INFERENCE_HOST: &str = "127.0.0.1";
/// TCP port for the inference HTTP server.
pub const DEFAULT_INFERENCE_PORT: u16 = 11434;
/// Max new tokens per `/v1/completions`-style request unless overridden.
pub const DEFAULT_INFERENCE_MAX_TOKENS: usize = 256;
/// Sampling temperature (0.0 = greedy).
pub const DEFAULT_INFERENCE_TEMPERATURE: f32 = 0.7;
