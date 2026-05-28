//! Best-effort AgentOS telemetry into `research_metrics` (when `VoxDb` is attached).

use vox_db::research_metrics_contract::{
    METRIC_TYPE_AGENTOS_GUARDRAIL_DENY, TelemetryWriteOptions,
};
use vox_orchestrator::agentos::guardrail_kernel::GuardrailDenyDetail;

/// Persist guardrail deny — failures are logged only (never blocks the deny response).
pub(crate) async fn record_guardrail_deny_best_effort(
    db: Option<&std::sync::Arc<vox_db::VoxDb>>,
    repository_id: &str,
    detail: &GuardrailDenyDetail,
) {
    let Some(db) = db else {
        return;
    };
    let session_key = TelemetryWriteOptions::new(repository_id).session_mcp();
    let metadata = serde_json::json!({
        "tool": detail.tool,
        "reason": detail.reason,
        "risk_score": detail.risk_score,
    });
    let metadata_str = match serde_json::to_string(&metadata) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "agentos guardrail deny metadata serialize failed");
            return;
        }
    };
    match db
        .research_metric_append_linked(
            &session_key,
            METRIC_TYPE_AGENTOS_GUARDRAIL_DENY,
            Some(f64::from(detail.risk_score)),
            Some(metadata_str.as_str()),
            repository_id,
        )
        .await
    {
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "agentos guardrail deny row append failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::{DbConfig, VoxDb};

    #[tokio::test]
    async fn append_guardrail_deny_metric_round_trips_on_memory_db() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let arc = std::sync::Arc::new(db);
        let detail = GuardrailDenyDetail {
            tool: "vox_run_shell".into(),
            reason: "test deny".into(),
            risk_score: 99,
        };
        record_guardrail_deny_best_effort(Some(&arc), "repo-test", &detail).await;
        let rows = arc
            .list_research_metrics_by_type(METRIC_TYPE_AGENTOS_GUARDRAIL_DENY, "mcp:repo-test", 5)
            .await
            .expect("list metrics");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "mcp:repo-test");
        assert_eq!(rows[0].1, Some(99.0));
        let meta = rows[0].2.as_deref().expect("metadata_json");
        assert!(meta.contains("vox_run_shell"));
    }
}
