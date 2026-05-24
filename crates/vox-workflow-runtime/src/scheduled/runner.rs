//! Persistent scheduler runner.
//!
//! One tokio task drives a set of per-function `tokio::time::Instant`
//! deadlines, fires callbacks, and updates DB state. The DB row's
//! `next_due_at_ms` (wall-clock) is the **crash-recovery anchor** — at boot
//! the runner reads DB rows and computes Instant deadlines as
//! `Instant::now() + max(0, next_due_at_ms - wall_now())`. After each fire
//! the deadline advances by `interval`. The DB is updated in the same step
//! so a process crash leaves the next scheduled wall-clock moment intact.
//!
//! Why Instants for in-loop scheduling rather than wall-clock polling:
//! `tokio::time::advance` (used in tests) moves the virtual clock but not
//! `SystemTime::now()`. Driving fires off `Instant` deadlines makes the
//! runner deterministic under `tokio::test(start_paused = true)` while
//! still anchoring durable state to wall-clock for crash recovery.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, OnceCell, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use vox_db::VoxDb;

/// Async callback fired by the scheduler when a `@scheduled` function comes
/// due. Returns `anyhow::Result<()>`; an `Err` is logged via [`tracing`] but
/// does not pause the timer — the next interval is still scheduled.
pub type Callback =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync>;

struct Entry {
    interval: Duration,
    cb: Callback,
}

#[derive(Default)]
struct Registry {
    entries: HashMap<String, Entry>,
}

/// Process-global callback registry. Phase 5 `main_boot` codegen registers
/// every `@scheduled` function at startup; the runner reads from here when
/// computing deadlines and dispatching fires.
static REGISTRY: OnceCell<Arc<Mutex<Registry>>> = OnceCell::const_new();

async fn registry() -> Arc<Mutex<Registry>> {
    REGISTRY
        .get_or_init(|| async { Arc::new(Mutex::new(Registry::default())) })
        .await
        .clone()
}

fn wall_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Register a `@scheduled` function:
/// 1. Upsert its `scheduled_runs` row (preserving `next_due_at_ms` if the
///    row already exists from a prior boot).
/// 2. Store the callback + interval in the process-global registry.
pub async fn register(
    name: &str,
    interval: Duration,
    cb: Callback,
    db: Arc<VoxDb>,
) -> anyhow::Result<()> {
    let interval_ms = i64::try_from(interval.as_millis()).unwrap_or(i64::MAX);
    db.upsert_scheduled_run(name, interval_ms).await?;
    let reg = registry().await;
    reg.lock().await.entries.insert(
        name.to_string(),
        Entry {
            interval,
            cb: cb.clone(),
        },
    );
    Ok(())
}

/// Spawn the scheduler tokio task. Returns a [`ScheduledHandle`] used to
/// shut the task down on graceful exit.
pub async fn start(db: Arc<VoxDb>) -> anyhow::Result<ScheduledHandle> {
    let (tx, mut rx) = oneshot::channel::<()>();
    let reg = registry().await;

    // Compute initial Instant deadlines from DB wall-clock state.
    let mut deadlines: HashMap<String, Instant> = HashMap::new();
    let rows = db.scheduled_runs_due_now().await.unwrap_or_default();
    let now_ms = wall_now_ms();
    let now_inst = Instant::now();
    for row in rows {
        // Due-now rows: fire promptly.
        let delta_ms = (row.next_due_at_ms - now_ms).max(0) as u64;
        deadlines.insert(
            row.function_name,
            now_inst + Duration::from_millis(delta_ms),
        );
    }
    // Also seed deadlines from registered entries (covers freshly upserted
    // rows that didn't appear in due_now because next_due_at_ms is in the
    // future). For each registered entry not yet in deadlines, assume the
    // upsert set next_due_at_ms = now + interval.
    {
        let r = reg.lock().await;
        for (name, entry) in r.entries.iter() {
            deadlines
                .entry(name.clone())
                .or_insert_with(|| now_inst + entry.interval);
        }
    }

    let task = tokio::spawn(async move {
        loop {
            // Compute earliest deadline; if none, sleep a default tick.
            let sleep_until = deadlines
                .values()
                .min()
                .copied()
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(1));
            tokio::select! {
                _ = &mut rx => break,
                _ = tokio::time::sleep_until(sleep_until) => {
                    let fire_at = Instant::now();
                    // Snapshot due names.
                    let due_names: Vec<String> = deadlines
                        .iter()
                        .filter(|(_, d)| **d <= fire_at)
                        .map(|(n, _)| n.clone())
                        .collect();
                    for name in due_names {
                        let (cb, interval) = {
                            let r = reg.lock().await;
                            match r.entries.get(&name) {
                                Some(e) => (e.cb.clone(), e.interval),
                                None => {
                                    // Row exists in registry-state but
                                    // callback was removed; drop deadline.
                                    deadlines.remove(&name);
                                    continue;
                                }
                            }
                        };
                        // Catch-up semantics: if the deadline is far in
                        // the past (process was paused / clock jumped),
                        // fire once per missed interval. This also keeps
                        // the test deterministic under
                        // `tokio::time::advance` (one big jump fires
                        // multiple intervals at once).
                        loop {
                            let due = deadlines.get(&name).copied();
                            let Some(deadline) = due else { break };
                            if deadline > Instant::now() {
                                break;
                            }
                            let run_id = uuid::Uuid::new_v4().to_string();
                            if let Err(e) = db.scheduled_runs_mark_started(&name, &run_id).await {
                                tracing::warn!(error = %e, name = %name, "scheduled_runs_mark_started failed");
                            }
                            let result = cb().await;
                            if let Err(e) = &result {
                                tracing::warn!(error = %e, name = %name, "@scheduled callback returned error");
                            }
                            if let Err(e) = db
                                .scheduled_runs_mark_completed(&name, &run_id, result.is_ok())
                                .await
                            {
                                tracing::warn!(error = %e, name = %name, "scheduled_runs_mark_completed failed");
                            }
                            // Advance deadline by exactly one interval —
                            // grid-aligned, not Instant::now()-based — so
                            // catch-up fires the correct count.
                            deadlines.insert(name.clone(), deadline + interval);
                        }
                    }
                }
            }
        }
    });
    Ok(ScheduledHandle {
        shutdown_tx: tx,
        task,
    })
}

/// Handle returned by [`start`]; call [`ScheduledHandle::shutdown`] to stop
/// the runner cleanly.
pub struct ScheduledHandle {
    shutdown_tx: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl ScheduledHandle {
    /// Send a shutdown signal and await the runner task.
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.task.await;
    }
}
