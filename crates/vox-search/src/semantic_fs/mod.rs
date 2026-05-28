//! Semantic filesystem bridge: intent-oriented path discovery over repo inventory search.
//!
//! This is not a kernel-level semantic FS; it layers retrieval-style ranking on workspace paths.

mod indexer;
mod intent_ops;

pub use indexer::index_repo_paths_for_tokens;
pub use intent_ops::retrieve_evidence_for_intent;

use std::path::Path;

use serde_json::{Value, json};

use crate::execution::repo_path_search;
use crate::policy::SearchPolicy;

/// Ranked file paths for a natural-language `intent` string (token overlap over inventory).
pub fn discover_files_for_intent(
    repo_root: &Path,
    intent: &str,
    limit: usize,
    policy: &SearchPolicy,
) -> Vec<Value> {
    repo_path_search(repo_root, intent, limit, policy)
        .into_iter()
        .map(|hit| {
            json!({
                "path": hit.source,
                "score": hit.score,
                "snippet": hit.snippet,
                "evidence_source": format!("{:?}", hit.evidence_source),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovers_by_token_overlap() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join("agentos_hint.rs"), b"fn main() {}").expect("write");
        let policy = SearchPolicy::default();
        let hits = discover_files_for_intent(dir.path(), "agentos hint", 8, &policy);
        assert!(
            hits.iter()
                .any(|h| h["path"].as_str().unwrap_or("").contains("agentos_hint")),
            "{hits:?}"
        );
    }
}
