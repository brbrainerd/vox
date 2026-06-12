//! vox-db-cached llm_embed implementation of [`vox_publisher::scientia_semantic::Embedder`].

use vox_actor_runtime::llm::{LlmConfig, llm_embed};
use vox_actor_runtime::{ActivityOptions, ActivityResult};
use vox_db::VoxDb;
use vox_publisher::scientia_semantic::embed_cache_key;

/// An [`Embedder`] that checks the `scientia_embedding_cache` table before
/// calling the LLM provider, and writes back on miss.
///
/// Constructed from a reference to an open [`VoxDb`] and a provider
/// configuration.  Matches how `vox-search`'s `EmbeddingService` is built —
/// `ActivityOptions::default()` (no retries) + the caller-supplied `LlmConfig`.
pub struct CachedLlmEmbedder<'a> {
    pub db: &'a VoxDb,
    pub config: LlmConfig,
    pub options: ActivityOptions,
}

impl<'a> CachedLlmEmbedder<'a> {
    /// Build from the env/secrets-resolved embedding config; `None` when no
    /// embedding provider is configured (callers then skip semantic enrichment).
    pub fn from_env(db: &'a VoxDb) -> Option<Self> {
        vox_search::embedding_env::embedding_config_from_env().map(|config| Self {
            db,
            config,
            options: ActivityOptions::default(),
        })
    }
}

#[async_trait::async_trait]
impl<'a> vox_publisher::scientia_semantic::Embedder for CachedLlmEmbedder<'a> {
    async fn embed(&self, text: &str) -> Option<Vec<f32>> {
        let key = embed_cache_key(text, &self.config.model);

        // 1. Cache hit?
        if let Ok(Some(vec)) = self.db.get_cached_embedding(&key).await {
            return Some(vec);
        }

        // 2. Call provider.
        let vec = match llm_embed(&self.options, text, self.config.clone()).await {
            ActivityResult::Ok(Ok(v)) => v,
            ActivityResult::Ok(Err(_)) | ActivityResult::Failed(_) | ActivityResult::Cancelled => {
                return None;
            }
        };

        // 3. Write-back (best-effort; ignore error so embed failure ≠ embed miss).
        let _ = self
            .db
            .put_cached_embedding(&key, &self.config.model, &vec)
            .await;

        Some(vec)
    }
}
