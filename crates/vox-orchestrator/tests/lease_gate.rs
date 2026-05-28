//! P0-T3: lease-gate local fallback against mesh_exec_leases.

use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::a2a::dispatch::lease_gate::{LeaseGateError, check_before_local_fallback};

#[tokio::test]
async fn no_lease_allows_local_fallback() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let res = check_before_local_fallback(&db, "task:42", "node-A", 1_000).await;
    assert!(
        res.is_ok(),
        "no lease should allow local fallback; got {res:?}"
    );
}

#[tokio::test]
async fn unexpired_remote_lease_blocks_local_fallback() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.exec_lease_grant("lease-1", "task:42", "task:42", "node-B", 1_000, 60_000)
        .await
        .expect("grant");
    let err = check_before_local_fallback(&db, "task:42", "node-A", 5_000)
        .await
        .expect_err("should block");
    match err {
        LeaseGateError::HeldByRemote { holder_node_id, .. } => {
            assert_eq!(holder_node_id, "node-B");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn expired_remote_lease_allows_local_fallback() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.exec_lease_grant("lease-2", "task:42", "task:42", "node-B", 1_000, 1_500)
        .await
        .expect("grant");
    // now_ms=5_000 > expires_at=1_500: expired
    let res = check_before_local_fallback(&db, "task:42", "node-A", 5_000).await;
    assert!(res.is_ok(), "expired remote lease should allow fallback");
}

#[tokio::test]
async fn local_node_lease_is_not_blocking() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.exec_lease_grant("lease-3", "task:42", "task:42", "node-A", 1_000, 60_000)
        .await
        .expect("grant");
    // Self-held lease: node-A checking against node-A → OK
    let res = check_before_local_fallback(&db, "task:42", "node-A", 5_000).await;
    assert!(res.is_ok(), "self-held lease must not block self");
}
