//! Phase 0 SSOT acceptance test: two daemons + three agents + contention.

use std::path::Path;
use vox_db::{DbConfig, VoxDb};
use vox_orchestrator_queue::locks::leader::{LeaderRole, LockLeaderElection};
use vox_orchestrator_queue::locks::{FileLockManager, LockKind};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn two_daemons_no_double_write_under_contention() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let repo = "repo-1";

    let elect_a = LockLeaderElection::new(db.clone(), "node-A", repo);
    let elect_b = LockLeaderElection::new(db.clone(), "node-B", repo);

    let role_a = elect_a.try_become_leader().await.unwrap();
    let role_b = elect_b.try_become_leader().await.unwrap();
    assert!(matches!(role_a, LeaderRole::Leader { .. }));
    assert!(matches!(role_b, LeaderRole::Follower { .. }));

    let mgr_a = FileLockManager::with_db(db.clone(), "node-A", repo);
    // Agent 1 acquires with write-through to DB so it survives hydration.
    mgr_a
        .try_acquire_persisted(Path::new("src/main.rs"), AgentId(1), LockKind::Exclusive)
        .await
        .expect("agent 1 wins");
    let res2 = mgr_a
        .try_acquire_persisted(Path::new("src/main.rs"), AgentId(2), LockKind::Exclusive)
        .await;
    assert!(res2.is_err(), "agent 2 must lose: {res2:?}");
    let res3 = mgr_a
        .try_acquire_persisted(Path::new("src/main.rs"), AgentId(3), LockKind::Exclusive)
        .await;
    assert!(res3.is_err(), "agent 3 must lose: {res3:?}");

    // Replay after kill-9 of leader: drop in-memory state and rehydrate from DB.
    drop(mgr_a);
    let mgr_a2 = FileLockManager::with_db(db.clone(), "node-A", repo);
    mgr_a2.hydrate_from_db().await.unwrap();
    assert!(mgr_a2.is_locked(Path::new("src/main.rs")));
    let (holder, kind) = mgr_a2.holder(Path::new("src/main.rs")).expect("holder");
    assert_eq!(holder, AgentId(1));
    assert_eq!(kind, LockKind::Exclusive);
}
