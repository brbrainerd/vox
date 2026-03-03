use std::sync::Arc;
use vox_db::VoxDb;
use vox_runtime::llm::{llm_embed, LlmConfig};
use vox_runtime::{ActivityOptions, ActivityResult};

/// Service for generating and storing embeddings in VoxDb.
pub struct EmbeddingService {
    db: Arc<VoxDb>,
    config: LlmConfig,
}

impl EmbeddingService {
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

        let id = self.db
            .store()
            .store_embedding(source_type, source_id, &self.config.model, &vector, metadata, None)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(id)
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
}
