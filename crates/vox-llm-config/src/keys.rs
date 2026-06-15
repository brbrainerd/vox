// The single home for every LLM/AI setting key. Grows via Band-A Task 1.2
// (manifest-driven), gated by the parity tests in vox-config / vox-secrets / vox-gui.
//
// Seeded from docs/superpowers/specs/llm-config-key-manifest.md (Band-A rows).
// Secret values never live here — only `secret: true` + classification.

/// Convenience for the common non-secret UserPreference shape.
macro_rules! key {
    ($env:literal, $default:literal, $kind:ident, $group:ident, $label:literal, $hint:literal) => {
        LlmConfigKey {
            env: $env,
            default: $default,
            kind: Kind::$kind,
            group: Group::$group,
            class: ConfigClass::UserPreference,
            label: $label,
            hint: $hint,
            options: &[],
            secret: false,
            persistence: Persistence::FlatToml,
        }
    };
}

/// Convenience for a secret (Clavis-backed, EnvOnly, no default).
macro_rules! secret_key {
    ($env:literal, $label:literal, $hint:literal) => {
        LlmConfigKey {
            env: $env,
            default: "",
            kind: Kind::String,
            group: Group::ModelsAndEndpoints,
            class: ConfigClass::UserPreference,
            label: $label,
            hint: $hint,
            options: &[],
            secret: true,
            persistence: Persistence::EnvOnly,
        }
    };
}

pub const LLM_CONFIG_KEYS: &[LlmConfigKey] = &[
    // ---- Endpoints (non-secret) -----------------------------------------------------
    key!(
        "OPENROUTER_BASE_URL",
        "https://openrouter.ai/api",
        Url,
        ModelsAndEndpoints,
        "OpenRouter base URL",
        "OpenAI-compatible OpenRouter endpoint"
    ),
    key!(
        "VOX_OPENAI_BASE_URL",
        "https://api.openai.com/v1",
        Url,
        ModelsAndEndpoints,
        "OpenAI base URL",
        "OpenAI-compatible cloud endpoint"
    ),
    key!(
        "POPULI_URL",
        "http://localhost:11434",
        Url,
        ModelsAndEndpoints,
        "Local Ollama base URL",
        "Local Ollama / Populi HTTP endpoint"
    ),
    // ---- Inference profile (enum) ---------------------------------------------------
    LlmConfigKey {
        env: "vox_populi::inference_PROFILE",
        default: "desktop_ollama",
        kind: Kind::Enum,
        group: Group::ModelsAndEndpoints,
        class: ConfigClass::UserPreference,
        label: "Inference profile",
        hint: "Where chat/completion traffic runs",
        options: &[
            "desktop_ollama",
            "cloud_openai_compatible",
            "mobile_litert",
            "mobile_coreml",
            "lan_gateway",
        ],
        secret: false,
        persistence: Persistence::FlatToml,
    },
    // ---- Tuning (non-secret) --------------------------------------------------------
    key!("OLLAMA_TUNING_TEMPERATURE", "", Float, Tuning, "Ollama temperature", "Sampling temperature for local Ollama"),
    key!("OLLAMA_TUNING_TOP_P", "", Float, Tuning, "Ollama top-p", "Nucleus sampling cutoff for local Ollama"),
    key!("OLLAMA_TUNING_NUM_CTX", "", Int, Tuning, "Ollama context window", "num_ctx tokens for local Ollama"),
    key!("OPENAI_TUNING_TEMPERATURE", "", Float, Tuning, "OpenAI temperature", "Sampling temperature for OpenAI-compatible cloud"),
    key!("OPENAI_TUNING_TOP_P", "", Float, Tuning, "OpenAI top-p", "Nucleus sampling cutoff for OpenAI-compatible cloud"),
    key!("ANTHROPIC_TUNING_TEMPERATURE", "", Float, Tuning, "Anthropic temperature", "Sampling temperature for Anthropic"),
    key!("ANTHROPIC_TUNING_TOP_P", "", Float, Tuning, "Anthropic top-p", "Nucleus sampling cutoff for Anthropic"),
    key!("GEMINI_TUNING_TEMPERATURE", "", Float, Tuning, "Gemini temperature", "Sampling temperature for Gemini"),
    key!("GEMINI_TUNING_TOP_P", "", Float, Tuning, "Gemini top-p", "Nucleus sampling cutoff for Gemini"),
    key!("TOGETHER_TUNING_TEMPERATURE", "", Float, Tuning, "Together temperature", "Sampling temperature for Together AI"),
    key!("TOGETHER_TUNING_TOP_P", "", Float, Tuning, "Together top-p", "Nucleus sampling cutoff for Together AI"),
    // ---- Secrets (Clavis-backed) ----------------------------------------------------
    secret_key!("OPENROUTER_API_KEY", "OpenRouter API key", "Resolved via Clavis; never written to config.toml"),
    secret_key!("OPENAI_API_KEY", "OpenAI API key", "Resolved via Clavis; never written to config.toml"),
    secret_key!("ANTHROPIC_API_KEY", "Anthropic API key", "Resolved via Clavis; never written to config.toml"),
    secret_key!("GEMINI_API_KEY", "Gemini API key", "Resolved via Clavis; never written to config.toml"),
    secret_key!("HF_TOKEN", "Hugging Face token", "Resolved via Clavis; never written to config.toml"),
];
