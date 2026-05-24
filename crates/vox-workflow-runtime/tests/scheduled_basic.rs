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
        std::time::Duration::from_secs(60),
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

    let handle = vox_workflow_runtime::scheduled::start(db.clone()).await.unwrap();

    tokio::time::advance(std::time::Duration::from_secs(180)).await;
    // Yield enough times for the runner's tokio::select! loop to make progress.
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    handle.shutdown().await;

    let runs = counter.load(Ordering::SeqCst);
    assert!(runs >= 2, "expected >=2 fires in 180s with 60s interval; got {runs}");
}
