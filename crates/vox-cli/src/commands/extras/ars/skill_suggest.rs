use anyhow::Result;

use vox_db::NewSkillCandidate;
use vox_skill_discovery::{
    MinedOp, OpMiningOptions, arg_keys, mine_repeated_operations, render_json, render_terminal,
};

/// Persist mined candidates to `skill_candidates` (Task 3.2). Best-effort: a
/// write failure is logged and does not stop the command from still printing
/// its advisory output — this path is advisory, never fatal.
async fn persist_candidates(db: &vox_db::Codex, candidates: &[vox_skill_discovery::Candidate]) {
    for c in candidates {
        let name = c
            .draft_frontmatter
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("{:?}", c.kind));
        let raw_json = match serde_json::to_string(c) {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error = %e, "failed to serialize mined candidate; skipping persist");
                continue;
            }
        };
        if let Err(e) = db
            .insert_skill_candidate(&NewSkillCandidate {
                candidate_name: name,
                source: "op_miner".to_string(),
                raw_json,
            })
            .await
        {
            tracing::debug!(error = %e, "failed to persist mined skill candidate");
        }
    }
}

/// `vox skill suggest` — mine recurring operation procedures into advisory candidates,
/// and persist each one to `skill_candidates` for later review/promotion (Task 3.2).
///
/// `Codex` is a type alias for `VoxDb`, so `connect_default()` yields the handle on
/// which `list_recent_operations` (an `impl VoxDb` method) is callable directly —
/// the same pattern the other ars handlers use.
pub async fn skill_suggest(limit: i64, format: &str) -> Result<()> {
    let db = match vox_db::Codex::connect_default().await {
        Ok(db) => db,
        Err(_) => {
            println!("No operations captured yet (operation capture disabled or DB unavailable).");
            return Ok(());
        }
    };
    // Degrade gracefully: an un-migrated DB (no `agent_operations` table) or any
    // read error means there is simply nothing to mine yet — advisory, never fatal.
    let rows = match db.list_recent_operations(limit).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::debug!(error = %e, "list_recent_operations failed; treating as no operations");
            println!("No operations captured yet.");
            return Ok(());
        }
    };
    if rows.is_empty() {
        println!("No operations captured yet.");
        return Ok(());
    }
    // Map DB rows -> MinedOp, dropping rows with no session_id.
    let ops: Vec<MinedOp> = rows
        .into_iter()
        .filter_map(|r| {
            r.session_id.map(|sid| MinedOp {
                ts_ms: r.ts_ms,
                session_id: sid,
                tool_name: r.tool_name,
                arg_keys: arg_keys(&r.args_redacted),
            })
        })
        .collect();
    let candidates = mine_repeated_operations(&ops, &OpMiningOptions::default());
    persist_candidates(&db, &candidates).await;
    let rendered = match format {
        "json" => render_json(&candidates)?,
        _ => render_terminal(&candidates),
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::{DbConfig, VoxDb};
    use vox_skill_discovery::MinedOp;

    /// The `vox skill suggest` invocation path (this module's `skill_suggest`)
    /// mines candidates and then calls `persist_candidates`, which is the
    /// piece Task 3.2 adds. Exercise `persist_candidates` directly against an
    /// in-memory DB with real miner output, so a regression that stops the
    /// invocation path from persisting is caught here rather than only via a
    /// full CLI + on-disk DB integration test.
    #[tokio::test]
    async fn persist_candidates_writes_mined_output_to_skill_candidates() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");

        // Build ops the same way `skill_suggest` maps DB rows -> MinedOp, then
        // mine them with the same options, exactly mirroring the invocation path.
        // Needs >= min_occurrences (3) total across >= min_distinct_sessions (2)
        // sessions for the default `OpMiningOptions`, so repeat the sequence
        // twice in s1 and once in s2.
        let mut ops = Vec::new();
        let push_seq = |ops: &mut Vec<MinedOp>, session: &str, base_ts: i64| {
            for (i, tool) in ["Read", "Edit", "Bash"].iter().enumerate() {
                ops.push(MinedOp {
                    ts_ms: base_ts + i as i64,
                    session_id: session.to_string(),
                    tool_name: tool.to_string(),
                    arg_keys: vec![],
                });
            }
        };
        push_seq(&mut ops, "s1", 0);
        push_seq(&mut ops, "s1", 10);
        push_seq(&mut ops, "s2", 0);
        let candidates = mine_repeated_operations(&ops, &OpMiningOptions::default());
        assert!(
            !candidates.is_empty(),
            "fixture should mine at least one candidate"
        );

        persist_candidates(&db, &candidates).await;

        let pending = db
            .list_pending_skill_candidates()
            .await
            .expect("list pending");
        assert_eq!(
            pending.len(),
            candidates.len(),
            "every mined candidate must be persisted as a pending skill_candidates row"
        );
        assert!(pending.iter().all(|p| p.source == "op_miner"));
        assert!(pending.iter().all(|p| p.status == "pending"));
        assert!(
            pending
                .iter()
                .any(|p| p.raw_json.contains("RepeatedOperations"))
        );
    }
}
