//! Whitelisted [`ResearchEvent`] → `research_metrics` persistence (composition root).
//!
//! Live broadcast stays on [`super::emitter::BroadcastEmitter`]; this module mirrors a subset of
//! events into Tier‑B SQL analytics with catalog IDs aligned to `contracts/telemetry/events.v1.yaml`.

use vox_db::Codex;
use vox_research_events::ResearchEvent;

/// Telemetry catalog id for JSON payloads stored in `research_metrics.metadata_json` for bridged rows.
pub const TELEMETRY_CATALOG_ID_RESEARCH_EVENT_BRIDGE: &str = "research-event-bridge";

fn parse_session_row_id(session_id: &str) -> Option<i64> {
    let sid = session_id.trim();
    if sid.is_empty() || sid == "0" {
        return None;
    }
    sid.parse::<i64>().ok().filter(|&id| id > 0)
}

/// Fire-and-forget: persist whitelisted events when a Tokio runtime is available.
pub fn spawn_persist_research_event_for_metrics(db: Codex, event: ResearchEvent) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::debug!(
            target: "vox_orchestrator::research_metrics_bridge",
            "skip persist (no tokio runtime)"
        );
        return;
    };
    handle.spawn(async move {
        if let Err(err) = persist_research_event_metrics(&db, &event).await {
            tracing::warn!(
                target: "vox_orchestrator::research_metrics_bridge",
                error = %err,
                event_kind = ?event.kind(),
                "research_event_metrics_persist_failed"
            );
        }
    });
}

pub(crate) async fn persist_research_event_metrics(
    db: &Codex,
    event: &ResearchEvent,
) -> Result<(), String> {
    match event {
        ResearchEvent::TelemetryObservation {
            provider,
            metric_type,
            value,
            session_id,
            recorded_at_ms,
        } => {
            if !matches!(
                metric_type.as_str(),
                "research_started" | "sources_total" | "self_verification_reliability"
            ) {
                return Ok(());
            }
            let Some(sid) = parse_session_row_id(session_id) else {
                return Ok(());
            };
            let meta = serde_json::json!({
                "telemetry_catalog_id": TELEMETRY_CATALOG_ID_RESEARCH_EVENT_BRIDGE,
                "event_type": "TelemetryObservation",
                "provider": provider,
                "metric_type": metric_type,
                "recorded_at_ms": recorded_at_ms,
            });
            let meta_str = serde_json::to_string(&meta).map_err(|e| e.to_string())?;
            db.record_research_metric(sid, metric_type, *value, Some(&meta_str))
                .await
                .map_err(|e| e.to_string())?;
        }
        ResearchEvent::AggregateComputed {
            provider,
            metric_type,
            window_start_ms,
            window_end_ms,
            value,
            sample_count,
            session_id,
        } => {
            if !matches!(
                metric_type.as_str(),
                "citation_precision" | "retrieval_hit_rate"
            ) {
                return Ok(());
            }
            let Some(sid) = parse_session_row_id(session_id) else {
                return Ok(());
            };
            let meta = serde_json::json!({
                "telemetry_catalog_id": TELEMETRY_CATALOG_ID_RESEARCH_EVENT_BRIDGE,
                "event_type": "AggregateComputed",
                "provider": provider,
                "metric_type": metric_type,
                "window_start_ms": window_start_ms,
                "window_end_ms": window_end_ms,
                "sample_count": sample_count,
            });
            let meta_str = serde_json::to_string(&meta).map_err(|e| e.to_string())?;
            db.record_research_metric(sid, metric_type, *value, Some(&meta_str))
                .await
                .map_err(|e| e.to_string())?;
        }
        ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids,
            worthiness_score,
            session_id,
        } => {
            let Some(sid) = parse_session_row_id(session_id) else {
                return Ok(());
            };
            let meta = serde_json::json!({
                "telemetry_catalog_id": TELEMETRY_CATALOG_ID_RESEARCH_EVENT_BRIDGE,
                "event_type": "FindingCandidateProposed",
                "finding_id": finding_id,
                "claim_ids": claim_ids,
            });
            let meta_str = serde_json::to_string(&meta).map_err(|e| e.to_string())?;
            db.record_research_metric(
                sid,
                "scientia.finding_candidate_proposed",
                *worthiness_score,
                Some(&meta_str),
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        ResearchEvent::ClaimVerified {
            claim_id,
            verdict,
            confidence,
            verifier_model,
            session_id,
        } => {
            let Some(sid) = parse_session_row_id(session_id) else {
                return Ok(());
            };
            let meta = serde_json::json!({
                "telemetry_catalog_id": TELEMETRY_CATALOG_ID_RESEARCH_EVENT_BRIDGE,
                "event_type": "ClaimVerified",
                "claim_id": claim_id,
                "verdict": verdict,
                "confidence": confidence,
                "verifier_model": verifier_model,
            });
            let meta_str = serde_json::to_string(&meta).map_err(|e| e.to_string())?;
            db.record_research_metric(sid, "scientia.claim_verified", *confidence, Some(&meta_str))
                .await
                .map_err(|e| e.to_string())?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::DbConfig;
    use vox_db::VoxDb;
    use vox_research_events::ResearchEventKind;

    #[tokio::test]
    async fn bridge_persists_whitelisted_telemetry_observation() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
        let sid = db
            .create_research_session("bridge:test", "q")
            .await
            .expect("session");
        let evt = ResearchEvent::TelemetryObservation {
            provider: "unit".into(),
            metric_type: "research_started".into(),
            value: 1.0,
            session_id: sid.to_string(),
            recorded_at_ms: 1,
        };
        persist_research_event_metrics(&db, &evt)
            .await
            .expect("persist");
        let rows = db
            .list_research_metrics_by_session(&sid.to_string(), Some("research_started"), 5)
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2, Some(1.0));
    }

    #[tokio::test]
    async fn bridge_skips_unlisted_metric_type() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
        let sid = db
            .create_research_session("bridge:test2", "q")
            .await
            .expect("session");
        let evt = ResearchEvent::TelemetryObservation {
            provider: "unit".into(),
            metric_type: "noise_metric".into(),
            value: 2.0,
            session_id: sid.to_string(),
            recorded_at_ms: 1,
        };
        persist_research_event_metrics(&db, &evt)
            .await
            .expect("noop");
        let rows = db
            .list_research_metrics_by_session(&sid.to_string(), Some("noise_metric"), 5)
            .await
            .expect("list");
        assert!(rows.is_empty());
    }

    #[test]
    fn research_event_kind_helper_used() {
        let evt = ResearchEvent::TelemetryObservation {
            provider: "p".into(),
            metric_type: "research_started".into(),
            value: 1.0,
            session_id: "1".into(),
            recorded_at_ms: 0,
        };
        assert_eq!(evt.kind(), ResearchEventKind::TelemetryObservation);
    }
}
