//! Optional mesh kudos for SCIENTIA research milestones (never gates publication).

use vox_db::Codex;
use vox_mesh_types::kudos::{CreditJobRequest, RewardPrimitive};

/// Local operator identity used for desktop SCIENTIA kudos rows.
pub const LOCAL_SCIENTIA_USER_ID: &str = "local-user";

/// Local node id for non-mesh SCIENTIA completions.
pub const LOCAL_SCIENTIA_NODE_ID: &str = "local-scientia";

/// Fixed kudos amount for one completed research session (non-fungible accounting unit).
pub const RESEARCH_SESSION_COMPLETE_KUDOS: u64 = 1;

/// Credit mesh kudos when a research session completes. No-op when Ludus/gamify is disabled.
pub async fn emit_research_session_complete_kudos(
    db: &Codex,
    session_id: i64,
) -> Result<(), String> {
    if !crate::config_gate::is_enabled() {
        return Ok(());
    }
    let meta = serde_json::json!({
        "event": "research_session_complete",
        "session_id": session_id,
    });
    let meta_str = serde_json::to_string(&meta).map_err(|e| e.to_string())?;
    db.credit_kudos(&CreditJobRequest {
        vox_user_id: LOCAL_SCIENTIA_USER_ID.to_string(),
        node_id: LOCAL_SCIENTIA_NODE_ID.to_string(),
        primitive: RewardPrimitive::DocsContribution,
        amount: RESEARCH_SESSION_COMPLETE_KUDOS,
        task_id: Some(format!("research-session:{session_id}")),
        metadata_json: Some(meta_str),
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)] // Rust 2024: env set/remove in single-threaded tests.

    use super::*;
    use vox_db::{DbConfig, VoxDb};

    #[tokio::test]
    async fn research_session_kudos_respects_gamify_gate() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");

        unsafe {
            std::env::set_var("VOX_LUDUS_EMERGENCY_OFF", "1");
        }
        emit_research_session_complete_kudos(&db, 42)
            .await
            .expect("noop when disabled");
        let disabled_count = db
            .count_kudos_for_user(LOCAL_SCIENTIA_USER_ID)
            .await
            .expect("count");
        assert_eq!(disabled_count, 0);

        unsafe {
            std::env::remove_var("VOX_LUDUS_EMERGENCY_OFF");
        }
        if !crate::config_gate::is_enabled() {
            return;
        }
        emit_research_session_complete_kudos(&db, 99)
            .await
            .expect("credit kudos");
        let enabled_count = db
            .count_kudos_for_user(LOCAL_SCIENTIA_USER_ID)
            .await
            .expect("count");
        assert_eq!(enabled_count, 1);
    }
}
