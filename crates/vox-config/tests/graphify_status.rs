//! Graphify corpus registry + freshness assessment (`vox-config::graphify`).

use std::fs;
use std::path::Path;

use chrono::{TimeZone, Utc};
use vox_config::graphify::{
    GraphifyCorporaRegistry, assess_corpus_status, graph_stats_from_json, load_graphify_corpora,
};

fn write_minimal_registry(repo: &Path) {
    let dir = repo.join("contracts/retrieval");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("graphify-corpora.v1.yaml"),
        include_str!("../../../contracts/retrieval/graphify-corpora.v1.yaml"),
    )
    .unwrap();
}

fn corpus_by_id<'a>(
    reg: &'a GraphifyCorporaRegistry,
    id: &str,
) -> &'a vox_config::graphify::GraphifyCorpus {
    reg.corpora
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("corpus {id}"))
}

#[test]
fn load_graphify_corpora_reads_workspace_contract() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_registry(tmp.path());
    let reg = load_graphify_corpora(tmp.path()).expect("load");
    assert_eq!(reg.default_corpus_id, "repo-code-graph");
    assert_eq!(reg.ttl_days_default, 30);
    assert!(
        reg.corpora.len() >= 4,
        "expected at least 4 corpora, got {}",
        reg.corpora.len()
    );
    assert!(
        reg.corpora.iter().any(|c| c.id == "graphify-search-log"),
        "graphify-search-log must be present in registry"
    );
    assert!(reg.corpora.iter().any(|c| c.id == "vox-gui-surface"));
    assert!(
        reg.corpora.iter().any(|c| c.id == "vox-config-graph"),
        "vox-config-graph (non-GUI generality corpus) must be present"
    );
}

#[test]
fn assess_reports_graph_missing_when_file_absent() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_registry(tmp.path());
    let reg = load_graphify_corpora(tmp.path()).unwrap();
    let corpus = corpus_by_id(&reg, "repo-code-graph");
    let status = assess_corpus_status(
        tmp.path(),
        corpus,
        Some("deadbeef"),
        Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap(),
        30,
    );
    assert!(!status.graph_exists);
    assert!(!status.is_fresh);
    assert!(status.stale_reasons.iter().any(|r| r == "graph_missing"));
}

#[test]
fn assess_reports_graph_corrupt_when_file_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_registry(tmp.path());
    let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
    fs::create_dir_all(&graph_dir).unwrap();
    fs::write(graph_dir.join("graph.json"), r#"{"nodes": ["invalid":}"#).unwrap();
    let reg = load_graphify_corpora(tmp.path()).unwrap();
    let corpus = corpus_by_id(&reg, "repo-code-graph");
    let status = assess_corpus_status(
        tmp.path(),
        corpus,
        None,
        Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap(),
        30,
    );
    assert!(status.graph_exists);
    assert!(!status.is_fresh);
    assert!(status.stale_reasons.iter().any(|r| r == "graph_corrupt"));
}

#[test]
fn assess_fresh_when_graph_present_and_git_sha_matches() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_registry(tmp.path());
    let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
    fs::create_dir_all(&graph_dir).unwrap();
    fs::write(
        graph_dir.join("graph.json"),
        r#"{"nodes":[{"id":"a"}],"links":[{"source":"a","target":"b"}]}"#,
    )
    .unwrap();
    let built_at = "2026-06-15T10:00:00Z";
    fs::write(
        graph_dir.join(".graphify_manifest.v1.json"),
        format!(
            r#"{{"corpus_id":"repo-code-graph","built_at":"{built_at}","git_sha":"abc123","node_count":1,"edge_count":1}}"#
        ),
    )
    .unwrap();
    let reg = load_graphify_corpora(tmp.path()).unwrap();
    let corpus = corpus_by_id(&reg, "repo-code-graph");
    let status = assess_corpus_status(
        tmp.path(),
        corpus,
        Some("abc123"),
        Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap(),
        30,
    );
    assert!(status.graph_exists);
    assert!(
        status.is_fresh,
        "stale={:?} warn={:?}",
        status.stale_reasons, status.warnings
    );
    assert_eq!(status.node_count, Some(1));
    assert_eq!(status.edge_count, Some(1));
}

#[test]
fn assess_stale_on_git_drift() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_registry(tmp.path());
    let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
    fs::create_dir_all(&graph_dir).unwrap();
    fs::write(graph_dir.join("graph.json"), r#"{"nodes":[],"links":[]}"#).unwrap();
    fs::write(
        graph_dir.join(".graphify_manifest.v1.json"),
        r#"{"corpus_id":"repo-code-graph","built_at":"2026-06-15T10:00:00Z","git_sha":"oldsha"}"#,
    )
    .unwrap();
    let reg = load_graphify_corpora(tmp.path()).unwrap();
    let corpus = corpus_by_id(&reg, "repo-code-graph");
    let status = assess_corpus_status(
        tmp.path(),
        corpus,
        Some("newsha"),
        Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap(),
        30,
    );
    assert!(!status.is_fresh);
    assert!(status.stale_reasons.iter().any(|r| r == "git_drift"));
}

#[test]
fn graph_stats_accepts_links_or_edges_key() {
    let with_links = serde_json::json!({"nodes": [{}, {}], "links": [{}]});
    let (n, e) = graph_stats_from_json(&with_links).unwrap();
    assert_eq!(n, 2);
    assert_eq!(e, 1);
    let with_edges = serde_json::json!({"nodes": [{}], "edges": [{}, {}]});
    let (n2, e2) = graph_stats_from_json(&with_edges).unwrap();
    assert_eq!(n2, 1);
    assert_eq!(e2, 2);
}

#[test]
fn virtual_corpus_is_always_fresh() {
    // No graph file written — virtual corpora must not check disk.
    let tmp = tempfile::tempdir().unwrap();
    let corpus = vox_config::graphify::GraphifyCorpus {
        id: "graphify-search-log".to_string(),
        title: "Search hit log".to_string(),
        scope_path: ".".to_string(),
        graph_path: "nonexistent/graph.json".to_string(),
        manifest_path: "nonexistent/.graphify_manifest.v1.json".to_string(),
        extraction_mode: None,
        default_for_intents: vec![],
        is_virtual: true,
        source_root: None,
    };
    let status = vox_config::graphify::assess_corpus_status(
        tmp.path(),
        &corpus,
        None,
        chrono::Utc::now(),
        30,
    );
    assert!(status.is_fresh, "virtual corpus must always be fresh");
    assert!(
        status.stale_reasons.is_empty(),
        "no stale reasons: {:?}",
        status.stale_reasons
    );
    assert!(
        status.warnings.contains(&"virtual_corpus".to_string()),
        "warnings must contain 'virtual_corpus'"
    );
}

#[test]
fn lexical_lag_detected_when_sha_mismatch() {
    use vox_config::graphify::{GraphifyManifest, lexical_lag_stale_reason};
    let manifest = GraphifyManifest {
        graph_json_sha256: Some("abc123".to_string()),
        lexical_ingest_sha256: Some("different456".to_string()),
        ..GraphifyManifest::default()
    };
    assert_eq!(
        lexical_lag_stale_reason(&manifest),
        Some("lexical_lag".to_string())
    );
}

#[test]
fn no_lexical_lag_when_sha_matches() {
    use vox_config::graphify::{GraphifyManifest, lexical_lag_stale_reason};
    let manifest = GraphifyManifest {
        graph_json_sha256: Some("abc123".to_string()),
        lexical_ingest_sha256: Some("abc123".to_string()),
        ..GraphifyManifest::default()
    };
    assert_eq!(lexical_lag_stale_reason(&manifest), None);
}

#[test]
fn no_lexical_lag_when_ingest_sha_absent() {
    use vox_config::graphify::{GraphifyManifest, lexical_lag_stale_reason};
    // Not yet ingested — we don't call this a lag.
    let manifest = GraphifyManifest {
        graph_json_sha256: Some("abc123".to_string()),
        lexical_ingest_sha256: None,
        ..GraphifyManifest::default()
    };
    assert_eq!(lexical_lag_stale_reason(&manifest), None);
}

#[test]
fn assess_reports_lexical_lag_when_manifest_sha_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_registry(tmp.path());
    let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
    fs::create_dir_all(&graph_dir).unwrap();
    fs::write(
        graph_dir.join("graph.json"),
        r#"{"nodes":[{"id":"a"}],"links":[{"source":"a","target":"b"}]}"#,
    )
    .unwrap();
    let built_at = "2026-06-15T10:00:00Z";
    fs::write(
        graph_dir.join(".graphify_manifest.v1.json"),
        format!(
            r#"{{"corpus_id":"repo-code-graph","built_at":"{built_at}","git_sha":"abc123","node_count":1,"edge_count":1,"graph_json_sha256":"sha-g","lexical_ingest_sha256":"sha-i"}}"#
        ),
    )
    .unwrap();
    let reg = load_graphify_corpora(tmp.path()).unwrap();
    let corpus = corpus_by_id(&reg, "repo-code-graph");
    let status = assess_corpus_status(
        tmp.path(),
        corpus,
        Some("abc123"),
        Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap(),
        30,
    );
    assert!(!status.is_fresh);
    assert!(
        status.stale_reasons.iter().any(|r| r == "lexical_lag"),
        "stale reasons were: {:?}",
        status.stale_reasons
    );
}

#[test]
fn test_resolve_ttl_days_with_env_override() {
    use vox_config::graphify::resolve_ttl_days;

    // Test default fallback
    unsafe {
        std::env::remove_var("VOX_GRAPHIFY_TTL_DAYS");
    }
    assert_eq!(resolve_ttl_days(30), 30);
    assert_eq!(resolve_ttl_days(15), 15);

    // Test env override
    unsafe {
        std::env::set_var("VOX_GRAPHIFY_TTL_DAYS", "10");
    }
    assert_eq!(resolve_ttl_days(30), 10);

    // Test invalid env override falls back
    unsafe {
        std::env::set_var("VOX_GRAPHIFY_TTL_DAYS", "invalid");
    }
    assert_eq!(resolve_ttl_days(30), 30);

    unsafe {
        std::env::remove_var("VOX_GRAPHIFY_TTL_DAYS");
    }
}
