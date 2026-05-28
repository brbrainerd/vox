//! Registry of non-secret operator and tuning environment variables.
//!
//! This serves as the SSOT for CI guards to distinguish between intentional
//! tuning knobs and potential "secret-shaped" leaks that should have been
//! registered in `vox-secrets`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigClass {
    UserPreference,
    NodeLocal,
    Bootstrap,
    CiGate,
}

pub struct OperatorEnvSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub defaults: &'static str,
    pub config_class: ConfigClass,
}

pub const OPERATOR_TUNING_ENVS: &[OperatorEnvSpec] = &[
    OperatorEnvSpec {
        name: "VOX_DB_CIRCUIT_BREAKER",
        description: "Gate workflow durability writes under DB stress (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ORCH_LINEAGE_OFF",
        description: "Disable orchestration lineage persistence (1/true to disable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_WORKFLOW_JOURNAL_CODEX_OFF",
        description: "Disable Codex workflow journal append (1/true to disable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_DB_SYNC_INTEGRATION",
        description: "Opt-in to remote sync_for test gate (1/true only).",
        defaults: "0",

        config_class: ConfigClass::CiGate,
    },
    OperatorEnvSpec {
        name: "VOX_DB_EMBEDDED_REPLICA_INTEGRATION",
        description: "Opt-in to embedded-replica test gate (1/true only).",
        defaults: "0",

        config_class: ConfigClass::CiGate,
    },
    OperatorEnvSpec {
        name: "vox_populi::inference_PROFILE",
        description: "Inference strategy (cloud, mobile, lan, desktop).",
        defaults: "desktop_ollama",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "POPULI_URL",
        description: "Local Populi base URL.",
        defaults: "http://localhost:11434",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "OLLAMA_URL",
        description: "Ollama base URL (fallback for POPULI_URL).",
        defaults: "http://localhost:11434",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "HF_CHAT_MODEL",
        description: "Preferred HF router model ID.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "OPENROUTER_CHAT_MODEL",
        description: "Preferred OpenRouter model ID.",
        defaults: "auto",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "HF_DEDICATED_CHAT_URL",
        description: "Pinned HF Inference Endpoint URL.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "HF_DEDICATED_CHAT_MODEL",
        description: "Model ID for dedicated HF endpoint.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_PROFILE",
        description: "Secrets resolution profile (ci, prod, hardcut, dev).",
        defaults: "dev",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_AUTO_PREFER_VAULT",
        description: "Prefer vault over Infisical/env in Auto mode.",
        defaults: "false",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_AUTO_VAULT",
        description: "Mirror signal for secrets vault presence.",
        defaults: "",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_VAULT_URL",
        description: "Cloudless vault URL.",
        defaults: "",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_VAULT_PATH",
        description: "Cloudless vault file path.",
        defaults: ".vox/clavis_vault.db",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_BACKEND",
        description: "Explicit secrets backend (env_only, infisical, vault, vox_cloud, auto).",
        defaults: "auto",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SCHOLARLY_ADAPTER",
        description: "Selects scholarly publisher backend (zenodo, openreview, arxiv_assist).",
        defaults: "zenodo",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SCHOLARLY_JOB_LOCK_OWNER",
        description: "Owner ID for distributed scholarly ingestion locks.",
        defaults: "local",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SCIENTIA_CROSSREF_MAILTO",
        description: "Email for Crossref API polite pool (required).",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ACCOUNT_ID",
        description: "Vox account ID (used for cost attribution and mesh identity).",
        defaults: "default_dev",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_NEWS_SITE_BASE_URL",
        description: "Base URL for syndication site generation.",
        defaults: "http://localhost:3000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_NEWS_RSS_FEED_PATH",
        description: "Path to output generated RSS feed.",
        defaults: "public/rss.xml",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_NEWS_PUBLISH_ARMED",
        description: "Gate for actually pushing news to remote endpoints (1/true to arm).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SYNDICATION_TEMPLATE_PROFILE",
        description: "Profile name for syndication template selection.",
        defaults: "default",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ZENODO_STAGING_DIR",
        description: "Local staging directory for Zenodo uploads.",
        defaults: "tmp/zenodo_staging",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ZENODO_UPLOAD_ALLOWLIST",
        description: "Comma-separated extensions allowed for Zenodo upload.",
        defaults: "pdf,zip,json",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ZENODO_API_BASE",
        description: "Zenodo API base URL (e.g. sandbox vs prod).",
        defaults: "https://sandbox.zenodo.org/api",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ZENODO_HTTP_MAX_ATTEMPTS",
        description: "Zenodo API retry limit.",
        defaults: "3",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_OPENREVIEW_HTTP_MAX_ATTEMPTS",
        description: "OpenReview API retry limit.",
        defaults: "5",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_ORCH_METRICS_SINK",
        description: "Metrics ingestion endpoint for orchestrator telemetry.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_PUBLISHER_DRY_RUN",
        description: "Disable actual publication side-effects (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_BENCHMARK_TELEMETRY",
        description: "Enable benchmark event recording (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::CiGate,
    },
    OperatorEnvSpec {
        name: "VOX_SYNTAX_K_TELEMETRY",
        description: "Enable syntax-k benchmark recording (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::CiGate,
    },
    OperatorEnvSpec {
        name: "VOX_RUNTIME_LLM_MAX_RETRY",
        description: "Max retries for LLM inference failures.",
        defaults: "3",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_BM25_K1",
        description: "BM25 ranking saturation parameter k1.",
        defaults: "1.2",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_BM25_B",
        description: "BM25 ranking document length normalization parameter b.",
        defaults: "0.75",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_RRF_K",
        description: "Reciprocal Rank Fusion constant k.",
        defaults: "60",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_VERIFICATION_QUALITY_THRESHOLD",
        description: "Min confidence for search hit verification.",
        defaults: "0.7",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_OPENCLAW_URL",
        description: "OpenClaw API base URL.",
        defaults: "http://localhost:8000",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_OPENCLAW_WS_URL",
        description: "OpenClaw WebSocket endpoint.",
        defaults: "ws://localhost:8000/ws",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_OPENCLAW_SIDECAR_DISABLE",
        description: "Disable OpenClaw sidecar installation.",
        defaults: "false",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "GEMINI_MODEL",
        description: "Default model for Gemini provider.",
        defaults: "gemini-1.5-pro",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "OLLAMA_MODEL",
        description: "Default model for Ollama provider.",
        defaults: "llama3",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "OPENAI_MODEL",
        description: "Default model for OpenAI provider.",
        defaults: "gpt-4o",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "OPENROUTER_MODEL",
        description: "Default model for OpenRouter provider.",
        defaults: "auto",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_OPENAI_BASE_URL",
        description: "Custom base URL for OpenAI-compatible providers.",
        defaults: "https://api.openai.com/v1",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_POLICY_VERSION",
        description: "Enforce a specific search policy version.",
        defaults: "1",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MENS_TRAIN_JSONL_STRICT",
        description: "Enforce strict compiler recheck for training corpora.",
        defaults: "false",

        config_class: ConfigClass::CiGate,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_MEMORY_VECTOR_WEIGHT",
        description: "Vector weight in hybrid search fusion.",
        defaults: "0.55",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_REPO_MAX_FILES",
        description: "Max files to scan for repo inventory.",
        defaults: "20000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_REPO_SKIP_DIRS",
        description: "Comma-separated list of directories to skip in repo scan.",
        defaults: ".git,target,node_modules",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TAVILY_ENABLED",
        description: "Master switch for Tavily web search.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TAVILY_DEPTH",
        description: "Tavily search depth (basic/advanced).",
        defaults: "basic",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TAVILY_MAX_RESULTS",
        description: "Max results per Tavily search.",
        defaults: "5",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TAVILY_ON_EMPTY",
        description: "Auto-fire Tavily when local search is empty.",
        defaults: "true",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TAVILY_ON_WEAK",
        description: "Auto-fire Tavily when local evidence is weak.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TAVILY_BUDGET",
        description: "Safety budget for Tavily credits per session.",
        defaults: "50",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SEARXNG_URL",
        description: "SearXNG instance URL.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SEARXNG_MAX_RESULTS",
        description: "Max results from SearXNG.",
        defaults: "5",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SEARXNG_MAX_SCRAPE",
        description: "Max URLs to deep-scrape from SearXNG results.",
        defaults: "3",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SEARXNG_ENGINES",
        description: "Engines for SearXNG query.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SEARXNG_LANGUAGE",
        description: "Language for SearXNG query.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SCRAPER_TIMEOUT",
        description: "Timeout for web scraper in ms.",
        defaults: "5000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SCRAPER_ROBOTS_RESPECT",
        description: "Respect robots.txt in scraper.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_SCRAPER_MIN_DENSITY",
        description: "Min text density for scraped content.",
        defaults: "0.15",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_MAX_HOPS",
        description: "Max iterative search hops.",
        defaults: "3",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_TANTIVY_ROOT",
        description: "Root path for Tantivy indices.",
        defaults: "",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_PREFER_RRF",
        description: "Prefer RRF for search fusion.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_QDRANT_URL",
        description: "Qdrant API base URL.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_QDRANT_COLLECTION",
        description: "Qdrant collection name.",
        defaults: "vox_docs",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_QDRANT_VECTOR_NAME",
        description: "Named vector in Qdrant collection.",
        defaults: "",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SEARCH_DDG_FALLBACK_DISABLED",
        description: "Disable DuckDuckGo fallback when SearXNG fails.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_RATE_LIMIT_MAX_REQUESTS",
        description: "Max requests per rate-limit window.",
        defaults: "100",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_RATE_LIMIT_WINDOW_SECONDS",
        description: "Duration of rate-limit window in seconds.",
        defaults: "60",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_UNIFIED_ROUTING",
        description: "Enable experimental unified routing across agents.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_EXE",
        description: "Path to the vox executable for self-forwarding.",
        defaults: "vox",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_SCHOLA_FORWARD",
        description: "Enable SCHOLA training forwarding.",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_SCHOLA_TRAIN_IN_PROCESS",
        description: "Run SCHOLA training in the main process.",
        defaults: "true",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_EMBEDDING_MODEL",
        description: "Default model for text embeddings.",
        defaults: "text-embedding-3-small",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "OPENAI_BASE_URL",
        description: "Custom base URL for OpenAI-compatible providers (legacy).",
        defaults: "https://api.openai.com/v1",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "RUST_LOG",
        description: "Standard Rust logging filter.",
        defaults: "info",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "HOSTNAME",
        description: "System hostname (auto-detected).",
        defaults: "localhost",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_DEVICE_CLASS",
        description: "Device tier for mesh coordination (mobile, workstation, cluster).",
        defaults: "workstation",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_ALLOW_UNAUTHENTICATED",
        description: "Gate for allowing unauthenticated runtime requests (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_CHROME_EXECUTABLE",
        description: "Path to Chrome/Chromium executable for scraping.",
        defaults: "",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_CLOUDLESS_DB_PATH",
        description: "Mirror signal for vault path.",
        defaults: "",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "POPULI_MAX_TOKENS",
        description: "Max tokens for Populi local inference.",
        defaults: "4096",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "POPULI_MODEL",
        description: "Model ID for Populi local inference.",
        defaults: "llama3",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "POPULI_TEMPERATURE",
        description: "Temperature for Populi local inference.",
        defaults: "0.7",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_EXEC_LEASE_STORE_PATH",
        description: "Path to the mesh execution lease store.",
        defaults: ".vox/mesh_leases.db",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_CODEX_TELEMETRY",
        description: "Enable mesh telemetry recording to Codex (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_A2A_MAX_MESSAGES",
        description: "Max A2A messages retained in memory per peer.",
        defaults: "1000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_BOOTSTRAP_EXPIRES_UNIX_MS",
        description: "Expiration timestamp for bootstrap tokens.",
        defaults: "0",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_SCOPE_ID",
        description: "Isolation scope ID for the mesh network.",
        defaults: "global",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_SERVER_STALE_PRUNE_MS",
        description: "Interval for pruning stale mesh servers.",
        defaults: "300000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_MAX_STALE_MS",
        description: "Maximum staleness for mesh nodes in monitoring.",
        defaults: "600000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "COMPUTERNAME",
        description: "Windows system computer name (auto-detected).",
        defaults: "localhost",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_EXEC_POLICY",
        description: "Execution policy for mesh tasks (local_only, prefer_remote, remote_only).",
        defaults: "local_only",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_NODE_ID",
        description: "Unique identifier for this mesh node.",
        defaults: "default_node",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_REPLAY_PERSIST",
        description: "Enable persistence for mesh message replay (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_REPLAY_STATE_PATH",
        description: "Path to mesh replay state file.",
        defaults: ".vox/mesh_replay.bin",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_A2A_LEASE_MS",
        description: "Lease duration for A2A message locks.",
        defaults: "30000",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_HTTP_MAX_BODY_BYTES",
        description: "Max body size for mesh HTTP transport.",
        defaults: "104857600",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_A2A_STORE_PATH",
        description: "Path to the A2A message store.",
        defaults: ".vox/mesh_a2a.db",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_DISPATCH_STORE_PATH",
        description: "Path to the task dispatch store.",
        defaults: ".vox/mesh_dispatch.db",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_GPU_MODEL",
        description: "Manually specify the GPU model for training (e.g. RTX4080).",
        defaults: "",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_GPU_VRAM_MB",
        description: "Manually specify available VRAM in MB.",
        defaults: "0",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_RANK",
        description: "Rank of this node in a distributed training mesh.",
        defaults: "0",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_TRAIN",
        description: "Enable distributed mesh training (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_BASE_MODEL",
        description: "Path or ID of the base model for training/fine-tuning.",
        defaults: "",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_TRAIN_PROFILE",
        description: "Configuration profile for ML training (e.g. qlora_v1).",
        defaults: "default",

        config_class: ConfigClass::UserPreference,
    },
    OperatorEnvSpec {
        name: "VOX_VRAM_OVERRIDE_GB",
        description: "Override detected VRAM for memory-intensive ops (GB).",
        defaults: "0",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_MESH_REGISTRY_PATH",
        description: "Path to the mesh node registry file.",
        defaults: ".vox/mesh_nodes.json",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "CARGO_HOME",
        description: "Standard Cargo home directory.",
        defaults: "~/.cargo",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "HOME",
        description: "Standard Unix home directory.",
        defaults: "~",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "USERPROFILE",
        description: "Standard Windows user profile directory.",
        defaults: "C:\\Users\\Default",

        config_class: ConfigClass::NodeLocal,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_CUTOVER_PHASE",
        description: "Secrets cutover phase controls (shadow, enforce, decommission).",
        defaults: "shadow",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_SECRETS_MIGRATION_PHASE",
        description: "Secrets migration phase controls (legacy_pre, shadow, dual).",
        defaults: "legacy_pre",

        config_class: ConfigClass::Bootstrap,
    },
    OperatorEnvSpec {
        name: "VOX_DB_MVCC",
        description: "Enable experimental MVCC for SQLite (1/true to enable).",
        defaults: "false",

        config_class: ConfigClass::UserPreference,
    },
];

#[must_use]
pub fn all_operator_env_names() -> Vec<&'static str> {
    OPERATOR_TUNING_ENVS.iter().map(|e| e.name).collect()
}
