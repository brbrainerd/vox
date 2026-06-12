//! DB-backed exposure ledger over the `discovery_state` table. Mirrors the
//! `crates/vox-gamify/src/db/counters.rs` connection+breaker pattern.

use anyhow::Result;
use turso::params;
use vox_db::Codex;

use super::fsrs::{self, MemoryState, Recall};

/// One materialized ledger row.
#[derive(Debug, Clone)]
pub struct DiscoveryRow {
    pub seen_count: u32,
    pub used_count: u32,
    pub last_seen_ms: i64,
    pub last_used_ms: i64,
    pub dwell_ms_total: i64,
    pub fsrs_stability: f64,
    pub fsrs_difficulty: f64,
    pub fsrs_due_ms: i64,
}

/// Fetch the current row for (user, action), if any.
pub async fn get(db: &Codex, user_id: &str, action_id: &str) -> Result<Option<DiscoveryRow>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT seen_count, used_count, last_seen_ms, last_used_ms, dwell_ms_total, \
             fsrs_stability, fsrs_difficulty, fsrs_due_ms \
             FROM discovery_state WHERE user_id=?1 AND action_id=?2",
            params![user_id, action_id],
        )
        .await?;
    match rows.next().await? {
        None => Ok(None),
        Some(r) => Ok(Some(DiscoveryRow {
            seen_count: r.get::<i64>(0).unwrap_or(0).max(0) as u32,
            used_count: r.get::<i64>(1).unwrap_or(0).max(0) as u32,
            last_seen_ms: r.get::<i64>(2).unwrap_or(0),
            last_used_ms: r.get::<i64>(3).unwrap_or(0),
            dwell_ms_total: r.get::<i64>(4).unwrap_or(0),
            fsrs_stability: r.get::<f64>(5).unwrap_or(0.0),
            fsrs_difficulty: r.get::<f64>(6).unwrap_or(0.0),
            fsrs_due_ms: r.get::<i64>(7).unwrap_or(0),
        })),
    }
}

/// Record an exposure. `recall` distinguishes seen-vs-used; `dwell_ms` adds to the
/// running dwell total (pass 0 for `Used`). Updates the FSRS memory state.
pub async fn record(
    db: &Codex,
    user_id: &str,
    action_id: &str,
    recall: Recall,
    now_ms: i64,
    dwell_ms: i64,
) -> Result<()> {
    let prev = get(db, user_id, action_id).await?.map(|r| MemoryState {
        stability: r.fsrs_stability,
        difficulty: r.fsrs_difficulty,
        due_ms: r.fsrs_due_ms,
    });
    let next = fsrs::update(prev, recall, now_ms);
    let (seen_inc, used_inc) = match recall {
        Recall::Seen => (1_i64, 0_i64),
        Recall::Used => (0_i64, 1_i64),
    };
    let (last_seen, last_used) = match recall {
        Recall::Seen => (now_ms, 0),
        Recall::Used => (0, now_ms),
    };
    let (uid, aid) = (user_id.to_string(), action_id.to_string());
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO discovery_state \
                 (user_id, action_id, seen_count, used_count, last_seen_ms, last_used_ms, \
                  dwell_ms_total, fsrs_stability, fsrs_difficulty, fsrs_due_ms) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
                 ON CONFLICT(user_id, action_id) DO UPDATE SET \
                   seen_count=seen_count+?3, \
                   used_count=used_count+?4, \
                   last_seen_ms=MAX(last_seen_ms,?5), \
                   last_used_ms=MAX(last_used_ms,?6), \
                   dwell_ms_total=dwell_ms_total+?7, \
                   fsrs_stability=?8, fsrs_difficulty=?9, fsrs_due_ms=?10",
                params![
                    uid.as_str(),
                    aid.as_str(),
                    seen_inc,
                    used_inc,
                    last_seen,
                    last_used,
                    dwell_ms,
                    next.stability,
                    next.difficulty,
                    next.due_ms
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Action ids whose FSRS due time is at or before `now_ms`, soonest-first, capped.
pub async fn due_action_ids(
    db: &Codex,
    user_id: &str,
    now_ms: i64,
    limit: u32,
) -> Result<Vec<String>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT action_id FROM discovery_state \
             WHERE user_id=?1 AND fsrs_due_ms<=?2 AND fsrs_due_ms>0 \
             ORDER BY fsrs_due_ms ASC LIMIT ?3",
            params![user_id, now_ms, limit as i64],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        if let Ok(id) = r.get::<String>(0) {
            out.push(id);
        }
    }
    Ok(out)
}
