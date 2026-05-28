//! Shared [`tracing_subscriber`] initialization for Vox process entrypoints.
//!
//! Prefer these helpers over ad hoc `fmt().with_env_filter(...).try_init()` copies.

use tracing_subscriber::EnvFilter;

/// CLI preset: honor `RUST_LOG` when valid; otherwise default filter **`info`**.
pub fn try_init_cli_default_info_fallback() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Daemon/service preset: [`EnvFilter::from_default_env`] (unset ⇒ subscriber default levels).
pub fn try_init_from_default_env() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

/// Like [`try_init_from_default_env`] but writes logs to **stderr** (LSP and tools that reserve stdout).
pub fn try_init_from_default_env_stderr() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_init_can_be_called_twice_without_panic() {
        try_init_cli_default_info_fallback();
        try_init_cli_default_info_fallback();
    }

    #[test]
    fn env_only_init_can_be_called_twice_without_panic() {
        try_init_from_default_env();
        try_init_from_default_env();
    }

    #[test]
    fn stderr_env_init_can_be_called_twice_without_panic() {
        try_init_from_default_env_stderr();
        try_init_from_default_env_stderr();
    }
}
