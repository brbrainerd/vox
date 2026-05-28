//! Ingest workflow exported as an rlib entry point so that `vox-cli` can call
//! it without taking a direct dependency on `vox-scientia-ingest`.

use anyhow::Result;
use std::sync::Arc;
use vox_db::VoxDb;
use vox_scientia::ingest::{FeedCrawler, IngestDeduplicator};
// `LlmConfig` is re-exported by `vox-search` so plugins can avoid a direct
// `vox-actor-runtime` dependency (plugin boundary: plugins must not pull core
// runtime crates).
use vox_search::{EmbeddingService, LlmConfig};

/// Run one batch of Scientist RSS/Atom crawling and deduplication tick.
///
/// This function mirrors the logic previously embedded in
/// `vox-cli/src/commands/db/publication/ingest.rs` and is the canonical
/// dispatch target now that the CLI delegates via this rlib instead of
/// importing `vox-scientia-ingest` directly.
pub async fn ingest_tick(feed_id: Option<&str>, limit: usize) -> Result<()> {
    let db = VoxDb::connect_default().await?;
    let sources = db.list_feed_sources().await?;
    let filtered: Vec<_> = if let Some(id) = feed_id {
        sources.into_iter().filter(|s| s.id == id).collect()
    } else {
        sources
    };

    if filtered.is_empty() {
        println!("No matching feed sources found for ingest-tick.");
        return Ok(());
    }

    let db_arc = Arc::new(db);
    let embedding_config = LlmConfig::openai("text-embedding-3-small");
    let embedder = EmbeddingService::new(db_arc.clone(), embedding_config);

    let deduplicator = IngestDeduplicator::new(&db_arc, Some(&embedder));
    let crawler = FeedCrawler::new()?;

    for src in filtered {
        println!("Crawling feed '{}' ({}) ...", src.id, src.url);
        match crawler.crawl_url(&src.url, &src.source_kind).await {
            Ok(items) => {
                let mut count = 0;
                for item in items.into_iter().take(limit) {
                    if deduplicator.is_duplicate(&item, 0.85).await? {
                        continue;
                    }

                    db_arc
                        .upsert_external_intelligence_pending(
                            &item.source_url,
                            &item.source_kind,
                            &item.title,
                            item.abstract_text.as_deref(),
                        )
                        .await?;
                    count += 1;
                }
                println!("  -> Ingested {} new items from '{}'.", count, src.id);
            }
            Err(e) => {
                eprintln!("  !! Failed to crawl feed '{}': {}", src.id, e);
            }
        }
    }

    Ok(())
}
