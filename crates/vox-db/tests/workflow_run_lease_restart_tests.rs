#![allow(missing_docs)]
//! T1.4 item 5: "crash while a workflow run holds a lease -> lease is expired
//! or cleanly re-acquired, never resurrected as live."
//!
//! `workflow_run_log.lease_owner`/`lease_until_ms` is a time-based state, not
//! a boot-time snapshot: `try_claim_workflow_run_lease`'s own WHERE clause
//! (`lease_owner IS NULL OR lease_until_ms IS NULL OR lease_until_ms < ?4 OR
//! lease_owner = ?2`) re-checks expiry against `now` on every claim attempt,
//! with no separate "resurrect from snapshot" boot path anywhere reachable
//! from orchestrator init. These tests are the concrete verification for
//! that finding: a lease that expired while the original holder was dead
//! (simulated by advancing past `lease_until_ms` without ever calling
//! `record_workflow_run_completed`/`cancelled`) is claimable by a new owner
//! exactly as if the DB had just booted fresh — there is no code path that
//! could resurrect it as still-held.

use vox_db::VoxDb;

async fn db() -> VoxDb {
    VoxDb::open_memory().await.expect("open_memory")
}

/// A lease held by a since-crashed owner, with `lease_until_ms` already in
/// the past, is claimable by a different owner — it is never resurrected as
/// still live just because the row still has the old `lease_owner` value.
#[tokio::test]
async fn expired_lease_from_dead_owner_is_reclaimable_not_resurrected() {
    let cs = db().await;
    cs.record_workflow_run_started("run-1", "wf-lease-test", 3)
        .await
        .expect("start run");

    // Original owner claims with a lease that expires almost immediately.
    let claimed = cs
        .try_claim_workflow_run_lease("run-1", "owner-A-crashed", 1)
        .await
        .expect("claim");
    assert!(claimed, "first claim on a fresh run must succeed");

    // Simulate the owner crashing: nothing ever renews or releases the
    // lease. Wait past its 1ms TTL — this is the "restart" moment: on a real
    // daemon restart, init_db/boot never touches workflow_run_log at all
    // (verified: no code path reads it during orchestrator startup), so the
    // row is exactly as the crashed process left it — an expired lease, not
    // a live one.
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    // A different owner (standing in for the restarted daemon / a peer)
    // must be able to claim the now-expired lease. If leases were ever
    // resurrected as still-held from some boot snapshot, this would fail.
    let reclaimed = cs
        .try_claim_workflow_run_lease("run-1", "owner-B-after-restart", 60_000)
        .await
        .expect("reclaim");
    assert!(
        reclaimed,
        "an expired lease from a crashed owner must be cleanly re-acquirable \
         by a new owner, never resurrected as still held by the dead one"
    );
}

/// A lease that has NOT expired (the original owner is still alive and
/// renewing, or simply hasn't hit its TTL yet) must NOT be claimable by a
/// different owner — the flip side of the precedence rule: only expiry
/// releases a lease, not merely "another process asked."
#[tokio::test]
async fn live_lease_is_not_claimable_by_a_different_owner() {
    let cs = db().await;
    cs.record_workflow_run_started("run-2", "wf-lease-test", 1)
        .await
        .expect("start run");

    let claimed = cs
        .try_claim_workflow_run_lease("run-2", "owner-A-alive", 60_000)
        .await
        .expect("claim");
    assert!(claimed);

    // A different owner tries immediately, well before the 60s TTL expires.
    let stolen = cs
        .try_claim_workflow_run_lease("run-2", "owner-C-rival", 60_000)
        .await
        .expect("claim attempt");
    assert!(
        !stolen,
        "a live (unexpired) lease must not be claimable by a different owner"
    );

    // The original owner can still renew its own lease (lease_owner = ?2 branch).
    let renewed = cs
        .try_claim_workflow_run_lease("run-2", "owner-A-alive", 60_000)
        .await
        .expect("renew");
    assert!(renewed, "the current lease owner must be able to renew its own lease");
}
