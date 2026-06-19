//! Read-only graphify corpus-health command for the GUI.
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::Path;
use vox_config::graphify::{
    CorpusStatus, assess_corpus_status, load_graphify_corpora, resolve_ttl_days,
};

#[derive(Debug, Serialize)]
pub struct GraphifyStatusPayload {
    pub default_corpus_id: String,
    pub corpora: Vec<CorpusStatus>,
}

/// Pure: assemble corpus statuses for a repo. Injecting `head_sha`/`now` keeps it deterministic.
pub fn build_status_payload(
    repo_root: &Path,
    head_sha: Option<&str>,
    now: DateTime<Utc>,
) -> Result<GraphifyStatusPayload, String> {
    let reg = load_graphify_corpora(repo_root).map_err(|e| e.to_string())?;
    let ttl = resolve_ttl_days(reg.ttl_days_default);
    let corpora = reg
        .corpora
        .iter()
        .map(|c| assess_corpus_status(repo_root, c, head_sha, now, ttl))
        .collect();
    Ok(GraphifyStatusPayload {
        default_corpus_id: reg.default_corpus_id,
        corpora,
    })
}

#[tauri::command]
pub async fn vox_graphify_status() -> Result<GraphifyStatusPayload, String> {
    let repo_root = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    build_status_payload(&repo_root, head.as_deref(), Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_lists_corpora_with_freshness() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("contracts/retrieval");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("graphify-corpora.v1.yaml"),
            "default_corpus_id: repo-code-graph\nttl_days_default: 30\ncorpora:\n  - id: repo-code-graph\n    title: Repo\n    scope_path: \".\"\n    graph_path: \".vox/cache/graphify/repo-code-graph/graph.json\"\n    manifest_path: \".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json\"\n",
        )
        .unwrap();
        let payload = build_status_payload(tmp.path(), Some("abc"), Utc::now()).unwrap();
        assert_eq!(payload.default_corpus_id, "repo-code-graph");
        assert_eq!(payload.corpora.len(), 1);
        // No graph on disk → stale with graph_missing.
        assert!(!payload.corpora[0].is_fresh);
        assert!(payload.corpora[0].stale_reasons.iter().any(|r| r == "graph_missing"));
    }
}
