//! Phase 4.2: scheduler fires registered callbacks at the configured interval.
//! Uses tokio's start_paused = true to advance time deterministically.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use vox_db::{DbConfig, VoxDb};

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduled_function_fires_after_interval() {
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.unwrap());
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    vox_workflow_runtime::scheduled::register(
        "ticker",
        vox_config::timeouts::D_60S,
        Arc::new(move || {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }),
        db.clone(),
    )
    .await
    .unwrap();

    let handle = vox_workflow_runtime::scheduled::start(db.clone())
        .await
        .unwrap();

    tokio::time::advance(std::time::Duration::from_secs(180)).await;
    // Yield enough times for the runner's tokio::select! loop to make progress.
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    handle.shutdown().await;

    let runs = counter.load(Ordering::SeqCst);
    assert!(
        runs >= 2,
        "expected >=2 fires in 180s with 60s interval; got {runs}"
    );
}

/// ADR-041 §6(a) regression: on restart, the in-memory Instant deadline must
/// be derived from the DB row's `next_due_at_ms`, not `now + interval`.
///
/// Scenario: a `@scheduled("1h")` function is registered (upsert sets
/// `next_due_at_ms = now + 1h`). We simulate "50 minutes elapsed" by writing
/// `next_due_at_ms = wall_now + 10min` directly. We then start the scheduler.
/// Advancing virtual time by 15min must fire the callback at least once —
/// the runner should have seeded a 10-minute Instant deadline, not a fresh
/// 60-minute one.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduler_restart_preserves_partial_interval_wait() {
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.unwrap());
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    vox_workflow_runtime::scheduled::register(
        "long_ticker",
        vox_config::timeouts::D_3600S,
        Arc::new(move || {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }),
        db.clone(),
    )
    .await
    .unwrap();

    // Simulate a crash 50 minutes into the interval by overwriting the
    // persisted next_due_at_ms with `wall_now + 10min`.
    let now_ms: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let due_ms = now_ms + 10 * 60 * 1000;
    db.connection()
        .execute(
            "UPDATE scheduled_runs SET next_due_at_ms = ?1 WHERE function_name = ?2",
            (due_ms, "long_ticker".to_string()),
        )
        .await
        .expect("UPDATE scheduled_runs succeeds");

    let handle = vox_workflow_runtime::scheduled::start(db.clone())
        .await
        .unwrap();

    // Advance virtual time by 15 minutes. If the runner correctly seeded a
    // 10-minute deadline, the callback fires; if it fell back to `now +
    // interval` (60min), the counter stays at 0.
    tokio::time::advance(std::time::Duration::from_secs(15 * 60)).await;
    for _ in 0..40 {
        tokio::task::yield_now().await;
    }
    handle.shutdown().await;

    let runs = counter.load(Ordering::SeqCst);
    assert!(
        runs >= 1,
        "expected ≥1 fire within 15min of virtual time when persisted \
         next_due_at_ms says 10min from start; got {runs} \
         (regression of ADR-041 §6(a) — runner ignored persisted next_due_at_ms)"
    );
}
