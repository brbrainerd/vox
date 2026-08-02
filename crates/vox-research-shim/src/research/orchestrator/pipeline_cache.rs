use vox_db::Codex;

use super::super::types::ResearchQuery;
use super::super::types::ResearchResult;
use super::config::ResearchConfig;

/// Codex `list_memories_by_type` cache short-circuit for identical-ish queries.
pub(super) async fn research_cache_short_circuit(
    query: &ResearchQuery,
    db: &Codex,
    config: &ResearchConfig,
) -> Option<ResearchResult> {
    let key = research_cache_key(query);
    let now = current_unix_secs();
    let entries = db.list_memories_by_type("research_cache", 200).await.ok()?;
    entries
        .into_iter()
        .filter_map(|entry| serde_json::from_str::<ResearchCacheEntry>(&entry).ok())
        .find(|entry| {
            entry.key == key
                && now.saturating_sub(entry.created_at_unix_secs) <= config.cache_ttl_secs
        })
        .map(|entry| entry.result)
}

/// Persist a completed research result for future cache short-circuit lookups.
pub(super) async fn research_cache_store(
    query: &ResearchQuery,
    result: &ResearchResult,
    db: &Codex,
    _config: &ResearchConfig,
) {
    let key = research_cache_key(query);
    let node_id = format!("research_cache:{key}");
    let entry = ResearchCacheEntry {
        key: key.clone(),
        created_at_unix_secs: current_unix_secs(),
        result: result.clone(),
    };
    let Ok(content) = serde_json::to_string(&entry) else {
        tracing::warn!("research cache entry serialization failed");
        return;
    };
    // Must use `knowledge_nodes` — `research_cache_short_circuit` reads via
    // `list_memories_by_type`, not the episodic `memories` table.
    if let Err(e) = db
        .upsert_knowledge_node(&node_id, &key, &content, Some("research_cache"), None, None)
        .await
    {
        tracing::warn!(error = %e, "research cache store failed");
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ResearchCacheEntry {
    key: String,
    created_at_unix_secs: u64,
    result: ResearchResult,
}

fn research_cache_key(query: &ResearchQuery) -> String {
    let normalized_query = query
        .query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let raw = format!(
        "{}|{:?}|{}|{}",
        normalized_query, query.scope, query.max_sources, query.verify_claims
    );
    format!("{:016x}", super::helpers::fnv1a_hash(&raw))
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::types::{
        ResearchMetadata, ResearchScope, RetrievalDiagnostics, RoutingTier,
    };

    fn query(text: &str, scope: ResearchScope) -> ResearchQuery {
        ResearchQuery {
            query: text.to_string(),
            scope,
            max_sources: 8,
            persist_to_docs: false,
            verify_claims: true,
            site_scope: None,
        }
    }

    fn minimal_result() -> ResearchResult {
        ResearchResult {
            answer: "cached answer".to_string(),
            sources: vec![],
            citations: vec![],
            research_metadata: ResearchMetadata {
                session_id: 42,
                duration_ms: 1,
                provider: "test".to_string(),
                routing_tier: RoutingTier::Direct,
                confidence: 0.5,
                subquery_count: 1,
                source_count: 0,
                claim_verdicts: vec![],
                retrieval_diagnostics: RetrievalDiagnostics::default(),
                quality_score: 50,
                planner_degraded: false,
                competence: None,
                self_verification: None,
                citation_audit: None,
                corroboration_counts: vec![],
            },
        }
    }

    #[test]
    fn cache_key_changes_with_scope_and_normalizes_whitespace() {
        let a = research_cache_key(&query("  Deep   Research  ", ResearchScope::Web));
        let b = research_cache_key(&query("deep research", ResearchScope::Web));
        let c = research_cache_key(&query("deep research", ResearchScope::Local));

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn research_cache_entry_serde_roundtrip() {
        let q = query("deep research", ResearchScope::Web);
        let entry = ResearchCacheEntry {
            key: research_cache_key(&q),
            created_at_unix_secs: 1_700_000_000,
            result: minimal_result(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let decoded: ResearchCacheEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.key, entry.key);
        assert_eq!(decoded.created_at_unix_secs, entry.created_at_unix_secs);
        assert_eq!(decoded.result.answer, entry.result.answer);
    }
}
