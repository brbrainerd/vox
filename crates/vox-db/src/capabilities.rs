//! Environment-driven knobs for Turso-backed search and retrieval.
//!
//! ANN / sqlite-vss integration is deferred; `embedding_candidate_cap` tunes the brute-force
//! embedding window without code changes.
//!
//! `probe_sqlite_capabilities` reads a handful of PRAGMAs once at runtime so routing / compilers
//! can pick FTS vs fallback paths without hard-coding build flavor.

/// Observed libSQL/SQLite behavior for strategy selection (best-effort; never panics on parse).
#[derive(Debug, Clone)]
pub struct SqliteProbeSnapshot {
    /// `PRAGMA journal_mode` first column (e.g. `wal`, `delete`).
    pub journal_mode: String,
    /// `PRAGMA foreign_keys` → `1` when enforced.
    pub foreign_keys_on: bool,
    /// Whether `sqlite_compileoption_used('ENABLE_FTS5')`-style check passed (best-effort).
    pub fts5_reported: bool,
}

/// Run lightweight PRAGMA probes on an open connection (safe for repeated calls).
pub async fn probe_sqlite_capabilities(
    conn: &turso::Connection,
) -> Result<SqliteProbeSnapshot, turso::Error> {
    let mut journal_mode = String::from("unknown");
    let mut rows = conn.query("PRAGMA journal_mode", ()).await?;
    if let Some(row) = rows.next().await? {
        let j: String = row.get(0)?;
        journal_mode = j;
    }

    let mut fk_on = false;
    let mut fk = conn.query("PRAGMA foreign_keys", ()).await?;
    if let Some(row) = fk.next().await? {
        let v: i64 = row.get(0)?;
        fk_on = v != 0;
    }

    let mut fts5 = false;
    if let Ok(mut o) = conn.query("PRAGMA compile_options", ()).await {
        while let Some(row) = o.next().await? {
            let opt: String = row.get(0)?;
            if opt.contains("FTS5") || opt == "ENABLE_FTS5" {
                fts5 = true;
                break;
            }
        }
    }

    Ok(SqliteProbeSnapshot {
        journal_mode,
        foreign_keys_on: fk_on,
        fts5_reported: fts5,
    })
}

/// Rows to scan before ranking = `limit * multiplier`, capped.
///
/// Override with `VOX_EMBEDDING_SEARCH_CANDIDATE_MULT` (integer ≥ 1). Default multiplier is passed
/// by the caller (e.g. `10` for `search_similar_embeddings`).
///
/// When `probe` reports **`fts5_reported == false`**, the effective multiplier is boosted (capped)
/// so brute-force embedding recall stays competitive on builds without FTS5 in compile options.
#[must_use]
pub fn embedding_candidate_cap(
    limit: i64,
    default_multiplier: i64,
    probe: Option<&SqliteProbeSnapshot>,
) -> i64 {
    let mut mult = vox_config::env_parse::resolve_config_u64(
        "VOX_EMBEDDING_SEARCH_CANDIDATE_MULT",
        default_multiplier as u64,
    ) as i64;
    if let Some(p) = probe {
        if !p.fts5_reported {
            mult = (mult * 2).min(30);
        }
    }
    let lim = limit.clamp(1, 500);
    (lim * mult).clamp(1, 50_000)
}
