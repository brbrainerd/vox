use crate::ingest::InboundItem;
use vox_db::VoxDb;
use vox_search::embeddings::EmbeddingService;

pub struct IngestDeduplicator<'a> {
    db: &'a VoxDb,
    embedder: Option<&'a EmbeddingService>,
}

impl<'a> IngestDeduplicator<'a> {
    pub fn new(db: &'a VoxDb, embedder: Option<&'a EmbeddingService>) -> Self {
        Self { db, embedder }
    }

    pub async fn is_duplicate(&self, item: &InboundItem, threshold: f32) -> anyhow::Result<bool> {
        let Some(embedder) = self.embedder else {
            return Ok(false); // Can't deduplicate without embedder
        };

        let text_to_embed = item.abstract_text.as_deref().unwrap_or(&item.title);
        if text_to_embed.is_empty() {
            return Ok(false);
        }

        let query_vector = match embedder.embed_query(text_to_embed).await {
            Ok(v) => v,
            Err(_) => return Ok(false),
        };

        // Use VoxDb similarity search targeting our new table
        let matches = self
            .db
            .search_similar_embeddings(&query_vector, Some("scientia_external_intelligence"), 5)
            .await?;

        for (_entry, score) in matches {
            if score >= threshold {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
