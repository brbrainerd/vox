//! Deterministic path indexing over workspace inventory (token overlap ranking).

use std::path::Path;

use serde_json::Value;

use crate::policy::SearchPolicy;

use super::discover_files_for_intent;

/// Ranked file hits for `intent`, suitable for checkpoint / retrieval manifests.
#[must_use]
pub fn index_repo_paths_for_tokens(
    repo_root: &Path,
    intent: &str,
    limit: usize,
    policy: &SearchPolicy,
) -> Vec<Value> {
    discover_files_for_intent(repo_root, intent, limit, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn indexer_matches_discover_paths() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join("idx_hit.rs"), b"x").expect("write");
        let policy = SearchPolicy::default();
        let a = index_repo_paths_for_tokens(dir.path(), "idx hit", 5, &policy);
        let b = super::discover_files_for_intent(dir.path(), "idx hit", 5, &policy);
        assert_eq!(a, b);
    }
}
