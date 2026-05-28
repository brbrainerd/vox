use tempfile::tempdir;
use vox_db::VoxDb;

#[tokio::test]
async fn test_agent_session_lifecycle() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let session_id = "sess_123";
    let agent_id = "agent_alpha";

    // Create
    store
        .create_session(session_id, agent_id, None, Some("initial task"))
        .await
        .unwrap();

    // Close
    store.close_session(session_id, "completed").await.unwrap();
}

#[tokio::test]
async fn test_llm_interaction_logging() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let session_id = "sess_456";
    store
        .create_session(session_id, "agent_beta", None, None)
        .await
        .unwrap();

    let interaction_id = store
        .log_interaction(
            session_id,
            Some("user_789"),
            "What is Vox?",
            "Vox is an AI-native language.",
            "v1.0",
            Some(150),
            Some(42),
            Some(42),
        )
        .await
        .unwrap();

    assert!(interaction_id > 0);

    let feedback_id = store
        .submit_feedback(
            interaction_id,
            Some("user_789"),
            Some(5),
            "positive",
            None,
            None,
        )
        .await
        .unwrap();

    assert!(feedback_id > 0);
}

#[tokio::test]
async fn test_agent_reliability_scoring() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store: VoxDb = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let agent_id = "trusty_agent";

    // Record success
    store
        .record_task_reliability_observation(agent_id, true)
        .await
        .unwrap();

    let scores = store.list_agent_reliability().await.unwrap();
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0].0, agent_id);
    assert!(scores[0].1 > 0.6); // (1+1)/(1+0+2) = 2/3 approx 0.66

    let one = store.get_agent_reliability(agent_id).await.unwrap();
    assert!(one.is_some_and(|r| (r - scores[0].1).abs() < 1e-9));
    assert!(
        store
            .get_agent_reliability("missing")
            .await
            .unwrap()
            .is_none()
    );
}
