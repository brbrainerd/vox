//! Shared local-first retrieval execution for MCP, orchestrator, CLI, and A2A bridges.
//!
//! - [`policy::SearchPolicy`] — versioned tunables + `VOX_SEARCH_*` env overrides.
//! - [`context::SearchRuntimeContext`] — repo root, optional VoxDb handle, memory paths.
//! - [`execution::execute_search_plan`] — memory hybrid + knowledge + chunks + repo inventory.
//! - [`bundle::run_search_with_verification`] — automatic verification pass.
//! - Optional **Tantivy** (`tantivy-lexical`) and **Qdrant** (`qdrant-vector`) backends.

pub mod a2a_contract;
pub mod bundle;
pub mod context;
pub mod crag;
pub mod duckduckgo;
pub mod embedding_env;
pub mod embeddings;
pub mod evaluation;
pub mod execution;
pub mod ingest;
mod memory_cache;
pub mod memory_hybrid;
pub mod policy;
pub mod research;
mod rrf;
#[cfg(feature = "web-scrape")]
pub mod scraper;
pub mod searxng;
mod searxng_defaults;
/// Intent-oriented repo path discovery (AgentOS semantic filesystem bridge).
pub mod semantic_fs;
pub mod symbol_proximity;
mod tavily_budget;
pub mod web_dispatcher;

#[cfg(feature = "tantivy-lexical")]
pub mod lexical_tantivy;
#[cfg(feature = "tavily")]
pub mod tavily;
#[cfg(feature = "qdrant-vector")]
pub mod vector_qdrant;

pub use a2a_contract::{A2ARetrievalRefinement, A2ARetrievalRequest, A2ARetrievalResponse};
pub use bundle::{
    RetrievalTriggerMode, diagnostics_value, run_search_with_verification, search_plan_value,
};
pub use context::SearchRuntimeContext;
pub use embedding_env::embedding_config_from_env;
pub use embeddings::EmbeddingService;
// Re-export `LlmConfig` so plugins (and other consumers that already depend on
// `vox-search` for `EmbeddingService`) do not need to pull in `vox-actor-runtime`
// just to construct an embedding configuration. Plugin crates must not depend
// on the core runtime; this re-export is the supported surface.
pub use evaluation::{SearchBenchmarkQuery, SearchEvalReport, default_doc_nav_queries};
pub use execution::{
    LexicalMemoryFallback, SearchExecution, execute_search_plan, repo_path_search,
};
pub use ingest::ingest_markdown_tree;
pub use memory_hybrid::{HybridSearchHit, MemorySearchEngine};
pub use policy::{SEARCH_POLICY_DEFAULT_VERSION, SearchPolicy, SearchPolicyFeedback};
pub use tavily_budget::TavilySessionBudget;
pub use vox_actor_runtime::llm::LlmConfig;
