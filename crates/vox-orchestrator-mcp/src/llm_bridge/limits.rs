//! Central caps for MCP HTTP LLM calls (avoid scattered literals).

/// Max output tokens passed to OpenRouter / Gemini-style HTTP APIs from MCP tools.
pub(crate) const HTTP_MAX_OUTPUT_TOKENS_CAP: u64 = 8192;

/// Timeout for Ollama `GET /api/tags` probe.
pub(crate) const OLLAMA_PROBE_TIMEOUT_SECS: u64 = 2;

/// Reuse successful Ollama probe for this duration (per process).
pub(crate) const OLLAMA_PROBE_CACHE_TTL_SECS: u64 = 30;

/// Timeout for VoxLocal `GET /health` probe.
pub(crate) const VOX_LOCAL_PROBE_TIMEOUT_SECS: u64 = 1;

/// Reuse successful VoxLocal probe for this duration (per process).
pub(crate) const VOX_LOCAL_PROBE_CACHE_TTL_SECS: u64 = 30;

/// Emit Track-E default-decision events for all limits in this module.
/// Guarded by a `OnceLock` — fires at most once per process.
pub(crate) fn emit_default_decisions_once() {
    use std::sync::OnceLock;
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        vox_telemetry::record_default_decision!("llm_output_token_cap", "8k_tokens", "default");
        vox_telemetry::record_default_decision!("ollama_probe_timeout", "2_secs", "default");
        vox_telemetry::record_default_decision!("ollama_probe_cache_ttl", "30_secs", "default");
    });
}
