use tempfile::tempdir;
use vox_db::{DbConfig, VoxDb};

/// B3 audit log: an approval is recorded `pending`, resolves to a terminal
/// status with a timestamp, appears in the recent list, and survives a restart.
#[tokio::test]
async fn hitl_approval_record_resolve_and_survive_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("vox.db").to_str().unwrap().to_string();

    {
        let db = VoxDb::connect(DbConfig::Local { path: path.clone() })
            .await
            .unwrap();
        db.hitl_approval_record("AP-000001", "vox_run_shell", "echo hi", 1000)
            .await
            .unwrap();

        let pending = db.hitl_approval_get("AP-000001").await.unwrap().expect("present");
        assert_eq!(pending.status, "pending");
        assert_eq!(pending.tool, "vox_run_shell");
        assert!(pending.resolved_at_ms.is_none());

        db.hitl_approval_resolve("AP-000001", "approved", 2000)
            .await
            .unwrap();

        let recent = db.hitl_approvals_recent(10).await.unwrap();
        assert!(recent.iter().any(|r| r.approval_id == "AP-000001"));
    }

    // Reopen: the audit row must survive restart with its resolved outcome.
    let db = VoxDb::connect(DbConfig::Local { path }).await.unwrap();
    let got = db
        .hitl_approval_get("AP-000001")
        .await
        .unwrap()
        .expect("survives restart");
    assert_eq!(got.status, "approved");
    assert_eq!(got.resolved_at_ms, Some(2000));
}
