//! Embedding calls used by hybrid memory retrieval and `vox-db`-backed vector search.

use std::sync::Arc;

use vox_actor_runtime::llm::{LlmConfig, llm_embed};
use vox_actor_runtime::{ActivityOptions, ActivityResult};
use vox_db::VoxDb;

/// Generate query vectors and optionally persist rows into `embeddings`.
pub struct EmbeddingService {
    db: Arc<VoxDb>,
    config: LlmConfig,
}

impl EmbeddingService {
    /// New service bound to a [`vox_db::VoxDb`] handle and provider configuration.
    pub fn new(db: Arc<VoxDb>, config: LlmConfig) -> Self {
        Self { db, config }
    }

    /// Generate an embedding for text and store it in the database.
    pub async fn embed_and_store(
        &self,
        source_type: &str,
        source_id: &str,
        text: &str,
        metadata: Option<&str>,
    ) -> Result<i64, String> {
        let options = ActivityOptions::default();
        let vector = match llm_embed(&options, text, self.config.clone()).await {
            ActivityResult::Ok(Ok(v)) => v,
            ActivityResult::Ok(Err(e)) => return Err(format!("Embedding API error: {}", e)),
            ActivityResult::Failed(e) => return Err(format!("Embedding failed: {}", e)),
            ActivityResult::Cancelled => return Err("Embedding cancelled".to_string()),
        };

        self.db
            .store_embedding(
                source_type,
                source_id,
                &self.config.model,
                &vector,
                metadata,
                None,
            )
            .await
            .map_err(|e| format!("Database error: {}", e))
    }

    /// Generate a vector for a search query.
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>, String> {
        let options = ActivityOptions::default();
        match llm_embed(&options, query, self.config.clone()).await {
            ActivityResult::Ok(Ok(v)) => Ok(v),
            ActivityResult::Ok(Err(e)) => Err(format!("Embedding API error: {}", e)),
            ActivityResult::Failed(e) => Err(format!("Embedding failed: {}", e)),
            ActivityResult::Cancelled => Err("Embedding cancelled".to_string()),
        }
    }

    /// Underlying model configuration (for telemetry).
    #[must_use]
    pub fn llm_config(&self) -> &LlmConfig {
        &self.config
    }

    /// Shared database handle.
    #[must_use]
    pub fn db(&self) -> &Arc<VoxDb> {
        &self.db
    }
}
