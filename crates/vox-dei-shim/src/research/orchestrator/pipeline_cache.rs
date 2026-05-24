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

#[derive(serde::Deserialize)]
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
    use crate::dei_shim::research::types::ResearchScope;

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

    #[test]
    fn cache_key_changes_with_scope_and_normalizes_whitespace() {
        let a = research_cache_key(&query("  Deep   Research  ", ResearchScope::Web));
        let b = research_cache_key(&query("deep research", ResearchScope::Web));
        let c = research_cache_key(&query("deep research", ResearchScope::Local));

        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
