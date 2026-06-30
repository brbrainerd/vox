use anyhow::Result;

use vox_skill_discovery::{
    MinedOp, OpMiningOptions, arg_keys, mine_repeated_operations, render_json, render_terminal,
};

/// `vox skill suggest` — mine recurring operation procedures into advisory candidates.
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
    let rendered = match format {
        "json" => render_json(&candidates)?,
        _ => render_terminal(&candidates),
    };
    println!("{rendered}");
    Ok(())
}
