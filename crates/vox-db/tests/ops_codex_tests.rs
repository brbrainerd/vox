use tempfile::tempdir;
use vox_db::VoxDb;

#[tokio::test]
async fn test_skill_manifest_lifecycle() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let skill_id = "test_skill";
    let version = "1.0.0";
    let manifest = r#"{"name": "test"}"#;
    let md = "# Test Skill";

    store
        .publish_skill(skill_id, version, manifest, md)
        .await
        .unwrap();

    let retrieved = store
        .get_skill_manifest(skill_id)
        .await
        .unwrap()
        .expect("Manifest should exist");
    assert_eq!(retrieved.version, version);

    let list = store.list_skill_manifests().await.unwrap();
    assert!(list.iter().any(|m| m.id == skill_id));

    store.unpublish_skill(skill_id).await.unwrap();
    assert!(store.get_skill_manifest(skill_id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_codex_change_log_playback() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    store
        .append_codex_change("docs", Some("file"), Some("README.md"), "update", None)
        .await
        .unwrap();
    store
        .append_codex_change("docs", Some("file"), Some("TODO.md"), "create", None)
        .await
        .unwrap();

    let changes = store.list_codex_changes_since(None, 0, 10).await.unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].entity_id, Some("README.md".to_string()));
}

#[tokio::test]
async fn test_research_metrics_telemetry() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let session_id = "research_1";
    store
        .append_research_metric(session_id, "socrates_trust", Some(0.95), None)
        .await
        .unwrap();

    let metrics = store
        .list_research_metrics_by_type("socrates_trust", "research", 10)
        .await
        .unwrap();
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].0, session_id);
    assert_eq!(metrics[0].1, Some(0.95));
}

#[tokio::test]
async fn list_research_metrics_by_session_optional_metric_type() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("research_metrics_session.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    store
        .append_research_metric("bench:r1", "benchmark_event", Some(1.0), None)
        .await
        .unwrap();
    store
        .append_research_metric("bench:r1", "socrates_surface", None, Some("{}"))
        .await
        .unwrap();

    let all = store
        .list_research_metrics_by_session("bench:r1", None, 10)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    let filtered = store
        .list_research_metrics_by_session("bench:r1", Some("benchmark_event"), 10)
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0, "bench:r1");
    assert_eq!(filtered[0].1, "benchmark_event");
    assert_eq!(filtered[0].2, Some(1.0));
}

#[tokio::test]
async fn list_research_metrics_for_session_prefix_includes_created_at() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("research_metrics_prefix.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    store
        .append_research_metric("mens:r1", "entropy", Some(0.5), None)
        .await
        .unwrap();

    let rows = store
        .list_research_metrics_for_session_prefix("mens:", 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "mens:r1");
    assert_eq!(rows[0].1, "entropy");
    assert_eq!(rows[0].2, Some(0.5));
    assert!(
        rows[0].4.len() >= 8,
        "created_at should be non-empty: {:?}",
        rows[0].4
    );
}

#[tokio::test]
async fn test_endpoint_reliability_ewma() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let url = "https://api.openai.com";
    let model = "gpt-4";

    // First observation
    store
        .record_endpoint_observation(url, model, 0.0, 0.0, 0.0, false, false)
        .await
        .unwrap();

    // An error observation (infra failure)
    store
        .record_endpoint_observation(url, model, 0.0, 0.0, 1.0, true, false)
        .await
        .unwrap();

    let list = store.list_endpoint_reliability(10).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].endpoint_url, url);
    assert!(list[0].infra_failure_ewma > 0.0);
}
