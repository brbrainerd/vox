// AI client HTTP/provider constants.

// ─── Constants ───────────────────────────────────────────

pub(crate) const POLLINATIONS_BASE: &str = "https://text.pollinations.ai/";
pub(crate) const OLLAMA_DEFAULT_URL: &str = vox_config::LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT;
pub(crate) const OLLAMA_DEFAULT_MODEL: &str = "codellama";
pub(crate) const GEMINI_DEFAULT_MODEL: &str = "gemini-2.5-flash";
pub(crate) const GEMINI_ENDPOINT_TEMPLATE: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent?key={KEY}";
pub(crate) const HTTP_TIMEOUT_SECS: u64 = 15;
pub(crate) const OLLAMA_PROBE_TIMEOUT_SECS: u64 = 2;

/// Free-tier OpenRouter models tried in order (most capable first).
/// All end with `:free` to guarantee zero cost.
pub(crate) const OPENROUTER_FREE_MODELS: &[&str] = &[
    "google/gemma-3-27b-it:free",
    "meta-llama/llama-3.3-70b-instruct:free",
    "qwen/qwen3-235b-a22b:free",
    "mistralai/mistral-7b-instruct:free",
    "microsoft/phi-3-mini-128k-instruct:free",
];
