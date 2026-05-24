use tempfile::tempdir;
use turso::params;
use vox_db::VoxDb;

#[tokio::test]
async fn retention_ms_count_and_prune_agent_session_events() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("retention_ms.db");
    let store = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    store
        .create_session("s_ret", "agent_r", None, None)
        .await
        .unwrap();
    store
        .append_session_event("s_ret", "evt", "{}")
        .await
        .unwrap();

    let none_old = store
        .retention_count_older_than_ms_cutoff("agent_session_events", "created_at_ms", 0)
        .await
        .unwrap();
    assert_eq!(none_old, 0, "live timestamps must not be < 0");

    store
        .connection()
        .execute(
            "UPDATE agent_session_events SET created_at_ms = 50 WHERE session_id = ?1",
            params!["s_ret"],
        )
        .await
        .unwrap();

    assert_eq!(
        store
            .retention_count_older_than_ms_cutoff("agent_session_events", "created_at_ms", 500)
            .await
            .unwrap(),
        1
    );

    let deleted = store
        .retention_delete_ms_older_than_chunk("agent_session_events", "created_at_ms", 500, 10)
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    assert_eq!(
        store
            .retention_count_older_than_ms_cutoff("agent_session_events", "created_at_ms", 500)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn retention_count_and_delete_expires_lt_now() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("retention_expires.db");
    let store = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    store
        .connection()
        .execute(
            "CREATE TABLE ret_expires_fixture (id INTEGER PRIMARY KEY, expires_at TEXT)",
            (),
        )
        .await
        .unwrap();

    store
        .connection()
        .execute(
            "INSERT INTO ret_expires_fixture (id, expires_at) VALUES (1, '2000-01-01 00:00:00'), (2, '2099-01-01 00:00:00'), (3, NULL)",
            (),
        )
        .await
        .unwrap();

    assert_eq!(
        store
            .retention_count_expires_lt_now("ret_expires_fixture", "expires_at")
            .await
            .unwrap(),
        1
    );

    let deleted = store
        .retention_delete_expires_lt_now("ret_expires_fixture", "expires_at")
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    assert_eq!(
        store
            .retention_count_expires_lt_now("ret_expires_fixture", "expires_at")
            .await
            .unwrap(),
        0
    );
}
