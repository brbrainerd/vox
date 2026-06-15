// The single home for every LLM/AI setting key. Band A = provider/endpoint/model/
// tuning/budget surface. Band B (orchestrator routing/capability/selection) is a
// separate plan and intentionally NOT registered here yet.
//
// Seeded from docs/superpowers/specs/llm-config-key-manifest.md + the vox-secrets
// SPECS_LLM canonical_env set. Secret values never live here — only `secret: true`.

/// Non-secret, FlatToml, UserPreference key (the common shape).
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

/// Secret (Clavis-backed, EnvOnly, no display default).
macro_rules! secret_key {
    ($env:literal, $label:literal) => {
        LlmConfigKey {
            env: $env,
            default: "",
            kind: Kind::String,
            group: Group::ModelsAndEndpoints,
            class: ConfigClass::UserPreference,
            label: $label,
            hint: "Resolved via Clavis; never written to config.toml",
            options: &[],
            secret: true,
            persistence: Persistence::EnvOnly,
        }
    };
}

/// VoxConfig-tier (sectioned) key: persisted via `VoxConfig`, default owned by
/// `VoxConfig::default()` (registry `default` left empty to avoid drift).
macro_rules! vc_key {
    ($env:literal, $kind:ident, $group:ident, $label:literal, $hint:literal) => {
        LlmConfigKey {
            env: $env,
            default: "",
            kind: Kind::$kind,
            group: Group::$group,
            class: ConfigClass::UserPreference,
            label: $label,
            hint: $hint,
            options: &[],
            secret: false,
            persistence: Persistence::VoxConfig,
        }
    };
}

pub const LLM_CONFIG_KEYS: &[LlmConfigKey] = &[
    // ---- Endpoints (non-secret) -----------------------------------------------------
    key!("OPENROUTER_BASE_URL", "https://openrouter.ai/api", Url, ModelsAndEndpoints, "OpenRouter base URL", "OpenAI-compatible OpenRouter endpoint"),
    key!("VOX_OPENAI_BASE_URL", "https://api.openai.com/v1", Url, ModelsAndEndpoints, "OpenAI base URL", "OpenAI-compatible cloud endpoint"),
    key!("POPULI_URL", "http://localhost:11434", Url, ModelsAndEndpoints, "Local Ollama base URL", "Local Ollama / Populi HTTP endpoint"),
    key!("OLLAMA_URL", "http://localhost:11434", Url, ModelsAndEndpoints, "Ollama URL", "Local Ollama base URL (fallback)"),
    key!("OLLAMA_HOST", "", Url, ModelsAndEndpoints, "Ollama host", "Ollama host override"),
    key!("HF_DEDICATED_CHAT_URL", "", Url, ModelsAndEndpoints, "HF dedicated chat URL", "Pinned Hugging Face Inference Endpoint chat URL"),
    key!("VOX_HF_ROUTER_CHAT_COMPLETIONS_URL", "https://router.huggingface.co/v1/chat/completions", Url, ModelsAndEndpoints, "HF router chat URL", "HF Inference Providers router chat completions URL"),
    key!("VOX_GROQ_CHAT_COMPLETIONS_URL", "", Url, ModelsAndEndpoints, "Groq chat URL", "Groq chat completions endpoint"),
    key!("VOX_CEREBRAS_CHAT_COMPLETIONS_URL", "", Url, ModelsAndEndpoints, "Cerebras chat URL", "Cerebras chat completions endpoint"),
    key!("VOX_MISTRAL_CHAT_COMPLETIONS_URL", "", Url, ModelsAndEndpoints, "Mistral chat URL", "Mistral chat completions endpoint"),
    key!("VOX_DEEPSEEK_CHAT_COMPLETIONS_URL", "", Url, ModelsAndEndpoints, "DeepSeek chat URL", "DeepSeek chat completions endpoint"),
    key!("VOX_SAMBANOVA_CHAT_COMPLETIONS_URL", "", Url, ModelsAndEndpoints, "SambaNova chat URL", "SambaNova chat completions endpoint"),
    key!("VOX_ANTHROPIC_CHAT_COMPLETIONS_URL", "", Url, ModelsAndEndpoints, "Anthropic chat URL", "Anthropic chat completions endpoint"),
    // ---- Inference profile (enum) ---------------------------------------------------
    LlmConfigKey {
        env: "vox_populi::inference_PROFILE",
        default: "desktop_ollama",
        kind: Kind::Enum,
        group: Group::ModelsAndEndpoints,
        class: ConfigClass::UserPreference,
        label: "Inference profile",
        hint: "Where chat/completion traffic runs",
        options: &["desktop_ollama", "cloud_openai_compatible", "mobile_litert", "mobile_coreml", "lan_gateway"],
        secret: false,
        persistence: Persistence::FlatToml,
    },
    // ---- Model preferences (non-secret) ---------------------------------------------
    key!("OPENROUTER_CHAT_MODEL", "", String, ModelsAndEndpoints, "OpenRouter chat model", "Preferred OpenRouter chat model id"),
    key!("OPENROUTER_MODEL", "", String, ModelsAndEndpoints, "OpenRouter model", "Default OpenRouter model id"),
    key!("HF_CHAT_MODEL", "", String, ModelsAndEndpoints, "HF chat model", "HF router chat model id"),
    key!("HF_DEDICATED_CHAT_MODEL", "", String, ModelsAndEndpoints, "HF dedicated model", "Model id for dedicated HF endpoint"),
    key!("OLLAMA_MODEL", "", String, ModelsAndEndpoints, "Ollama model", "Default Ollama model tag"),
    key!("TOGETHER_FINETUNE_MODEL", "", String, ModelsAndEndpoints, "Together finetune model", "Together finetune model id"),
    // ---- OpenRouter attribution (non-secret) ----------------------------------------
    key!("OPENROUTER_HTTP_REFERER", "", String, ModelsAndEndpoints, "OpenRouter HTTP-Referer", "Attribution Referer header for OpenRouter"),
    key!("OPENROUTER_APP_TITLE", "", String, ModelsAndEndpoints, "OpenRouter app title", "X-Title attribution header for OpenRouter"),
    key!("OPENROUTER_ROUTE_HINT", "", String, ModelsAndEndpoints, "OpenRouter route hint", "Route hint (price/quality/fallback) for OpenRouter auto"),
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
    // ---- Provider API keys (secret, Clavis) -----------------------------------------
    secret_key!("OPENROUTER_API_KEY", "OpenRouter API key"),
    secret_key!("OPENAI_API_KEY", "OpenAI API key"),
    secret_key!("ANTHROPIC_API_KEY", "Anthropic API key"),
    secret_key!("GEMINI_API_KEY", "Gemini API key"),
    secret_key!("GROQ_API_KEY", "Groq API key"),
    secret_key!("MISTRAL_API_KEY", "Mistral API key"),
    secret_key!("DEEPSEEK_API_KEY", "DeepSeek API key"),
    secret_key!("SAMBANOVA_API_KEY", "SambaNova API key"),
    secret_key!("CEREBRAS_API_KEY", "Cerebras API key"),
    secret_key!("TOGETHER_API_KEY", "Together AI API key"),
    secret_key!("RUNPOD_API_KEY", "RunPod API key"),
    secret_key!("VAST_API_KEY", "Vast.ai API key"),
    secret_key!("CUSTOM_OPENAI_API_KEY", "Custom OpenAI API key"),
    secret_key!("OPENCLAW_API_KEY", "OpenClaw API key"),
    secret_key!("OPENCLAW_TOKEN", "OpenClaw token"),
    secret_key!("HF_TOKEN", "Hugging Face token"),
    // ---- VoxConfig-tier (sectioned ~/.vox/config.toml; default owned by VoxConfig) ---
    // `default` left empty here: the display default for these comes from
    // `VoxConfig::default()` at read time so it can't drift from the struct.
    vc_key!("model", String, General, "Default model", "Model id used when no per-call override is set"),
    vc_key!("daily_budget_usd", Float, General, "Daily budget (USD)", "Soft cap on spend per day"),
    vc_key!("per_session_budget_usd", Float, General, "Per-session budget (USD)", "Soft cap on spend per session"),
    vc_key!("data_dir", Path, Training, "Training data dir", "Directory for MENS training data ([train].data_dir)"),
    vc_key!("db_url", String, General, "Database URL", "Optional external database URL (blank = local default)"),
    vc_key!("train_epochs", Int, Training, "Training epochs", "Default epochs for MENS training runs"),
    vc_key!("train_batch_size", Int, Training, "Training batch size", "Default batch size for MENS training runs"),
];
