//! Intent-shaped evidence bundles for orchestrator and MCP bridges.

use std::path::Path;

use serde_json::{Value, json};

use crate::policy::SearchPolicy;

use super::discover_files_for_intent;

/// JSON envelope: intent string plus bounded path hits (deterministic given inventory).
#[must_use]
pub fn retrieve_evidence_for_intent(
    repo_root: &Path,
    intent: &str,
    limit: usize,
    policy: &SearchPolicy,
) -> Value {
    let hits = discover_files_for_intent(repo_root, intent, limit, policy);
    json!({
        "intent": intent,
        "limit": limit,
        "hits": hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn bundle_includes_hits_array() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join("intent_ops.rs"), b"fn x() {}").expect("write");
        let policy = SearchPolicy::default();
        let v = retrieve_evidence_for_intent(dir.path(), "intent ops", 8, &policy);
        assert_eq!(v["intent"], "intent ops");
        let hits = v["hits"].as_array().expect("hits array");
        assert!(
            hits.iter()
                .any(|h| h["path"].as_str().unwrap_or("").contains("intent_ops")),
            "{hits:?}"
        );
    }
}
