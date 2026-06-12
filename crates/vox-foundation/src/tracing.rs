//! Shared [`tracing_subscriber`] initialization for Vox process entrypoints.
//!
//! Prefer these helpers over ad hoc `fmt().with_env_filter(...).try_init()` copies.

use tracing_subscriber::EnvFilter;

/// CLI preset: honor `RUST_LOG` when valid; otherwise default filter **`info`**.
pub fn try_init_cli_default_info_fallback() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
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

/// Optional OpenTelemetry OTLP layer for subscribers when the `otel` feature is enabled.
///
/// Reads `OTEL_EXPORTER_OTLP_ENDPOINT` (and standard OTEL env vars honored by the OTLP
/// exporter). Returns `None` when the endpoint is unset so callers can keep fmt-only init.
#[cfg(feature = "otel")]
pub fn init_otel_layer() -> Option<
    tracing_opentelemetry::OpenTelemetryLayer<
        tracing_subscriber::Registry,
        opentelemetry_sdk::trace::Tracer,
    >,
> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .is_none_or(|v| v.trim().is_empty())
    {
        return None;
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
        .ok()?;
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();
    let tracer = provider.tracer("vox");
    opentelemetry::global::set_tracer_provider(provider);
    Some(tracing_opentelemetry::layer().with_tracer(tracer))
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
