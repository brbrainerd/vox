//! Typed Scientia-domain read commands (research sessions + publication manifests).
//! Reads go through the shared [`GuiDbPool`] — no per-invoke connect.

use std::sync::Arc;

use tauri::Emitter;
use tauri::State;
use vox_db::VoxDb;

use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

#[derive(Debug, serde::Serialize)]
pub struct ResearchSessionDto {
    pub id: i64,
    pub status: String,
    pub query_text: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize, Default)]
pub struct ResearchClaimDto {
    pub claim_id: String,
    pub text: String,
    pub verdict: String,
    pub confidence: f64,
    pub resample_stability: f64,
    pub citation_urls: Vec<String>,
    pub corroboration_count: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct ResearchDetailDto {
    pub session: ResearchSessionDto,
    pub report_markdown: Option<String>,
    pub artifact_json: Option<String>,
    pub confidence_tier: Option<String>,
    pub claims: Vec<ResearchClaimDto>,
    pub source_count: Option<usize>,
    pub citation_precision: Option<f64>,
}

// ── Minimal mirror of `vox_research_shim::research::types::ResearchRunArtifact`
// (and friends) used only to parse `artifact_json` for the DTO above.
// `vox-gui` deliberately does NOT depend on `vox-research-shim` (a heavy
// LLM/HTTP-pulling crate not otherwise used by this crate) just to read a
// handful of already-serialized fields — these mirror structs cover only
// what `extract_research_summary` needs and tolerate unknown/missing fields
// via `#[serde(default)]`, so upstream additions never break parsing here.
mod artifact_mirror {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Default)]
    pub struct Citation {
        pub source_id: i64,
        pub url: String,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct EvidenceSpan {
        pub source_id: i64,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct Claim {
        pub text: String,
        pub claim_id: u64,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct ClaimVerdict {
        pub claim: Claim,
        pub verdict: String,
        pub confidence: f64,
        #[serde(default)]
        pub evidence_spans: Vec<EvidenceSpan>,
        #[serde(default)]
        pub resample_stability: f64,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct ResearchMetadata {
        pub routing_tier: String,
        #[serde(default)]
        pub source_count: usize,
        #[serde(default)]
        pub claim_verdicts: Vec<ClaimVerdict>,
        #[serde(default)]
        pub citation_audit: Option<CitationAudit>,
        /// `(claim_id, distinct_supporting_domain_count)` pairs computed by
        /// the pipeline's `compute_corroboration_counts` — see
        /// `vox_search::corroboration`.
        #[serde(default)]
        pub corroboration_counts: Vec<(u64, usize)>,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct CitationAudit {
        #[serde(default)]
        pub precision: f64,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct ResearchResult {
        #[serde(default)]
        pub citations: Vec<Citation>,
        pub research_metadata: ResearchMetadata,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct ResearchRunArtifact {
        pub result: ResearchResult,
    }
}

/// Extracted summary fields the ResearchView trust UI (headline banner +
/// claim accordion) needs, parsed out of an artifact's `artifact_json`.
#[derive(Debug, Default)]
pub struct ResearchSummary {
    pub confidence_tier: Option<String>,
    pub claims: Vec<ResearchClaimDto>,
    pub source_count: Option<usize>,
    pub citation_precision: Option<f64>,
}

/// `Verdict`'s wire form is snake_case (`"contested"`), but the GUI's
/// existing verdict vocabulary (`VerdictBadge`/`ClaimsView`) is
/// capitalized (`"Contested"`). Map to match rather than introduce a second
/// casing convention in the trust UI.
fn capitalize_verdict(wire: &str) -> String {
    match wire {
        "supported" => "Supported".to_string(),
        "contradicted" => "Contradicted".to_string(),
        "contested" => "Contested".to_string(),
        "unverified" => "Unverified".to_string(),
        other => other.to_string(),
    }
}

/// Parse `artifact_json` (the serialized `ResearchRunArtifact` written by the
/// research pipeline, see `vox-research-shim`'s `pipeline.rs`) into the
/// summary fields the GUI's trust UI needs. Fail-open: on parse failure,
/// returns an all-empty summary and logs a warning rather than erroring the
/// whole `get_research_session_detail` command — matches the pipeline's own
/// fail-open convention for degraded/legacy artifacts.
pub fn extract_research_summary(artifact_json: &str) -> ResearchSummary {
    use std::collections::HashMap;

    let artifact: artifact_mirror::ResearchRunArtifact = match serde_json::from_str(artifact_json)
    {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("failed to parse research artifact_json for trust UI: {e}");
            return ResearchSummary::default();
        }
    };
    let result = artifact.result;
    let meta = result.research_metadata;

    let url_by_source_id: HashMap<i64, String> = result
        .citations
        .iter()
        .map(|c| (c.source_id, c.url.clone()))
        .collect();
    let corroboration_by_claim_id: HashMap<u64, usize> =
        meta.corroboration_counts.into_iter().collect();

    let claims: Vec<ResearchClaimDto> = meta
        .claim_verdicts
        .into_iter()
        .map(|cv| {
            let mut citation_urls: Vec<String> = cv
                .evidence_spans
                .iter()
                .filter_map(|span| url_by_source_id.get(&span.source_id).cloned())
                .collect();
            citation_urls.dedup();
            let corroboration_count = corroboration_by_claim_id
                .get(&cv.claim.claim_id)
                .copied()
                .unwrap_or(0);
            ResearchClaimDto {
                claim_id: cv.claim.claim_id.to_string(),
                text: cv.claim.text,
                verdict: capitalize_verdict(&cv.verdict),
                confidence: cv.confidence,
                resample_stability: cv.resample_stability,
                citation_urls,
                corroboration_count,
            }
        })
        .collect();

    ResearchSummary {
        confidence_tier: Some(meta.routing_tier),
        claims,
        source_count: Some(meta.source_count),
        citation_precision: meta.citation_audit.map(|a| a.precision),
    }
}

#[tauri::command]
pub async fn list_research_sessions(
    pool: State<'_, GuiDbPool>,
    limit: Option<u32>,
) -> Result<Vec<ResearchSessionDto>, String> {
    let db = pool.handle()?;
    let rows = db
        .list_recent_research_sessions(limit.unwrap_or(20))
        .await
        .map_err(map_db_err)?;
    Ok(rows
        .iter()
        .map(|r| ResearchSessionDto {
            id: r.id,
            status: r.status.clone(),
            query_text: r.query_text.clone(),
            started_at_ms: r.started_at_ms,
            finished_at_ms: r.finished_at_ms,
        })
        .collect())
}

#[tauri::command]
pub async fn get_research_session_detail(
    pool: State<'_, GuiDbPool>,
    session_id: i64,
) -> Result<ResearchDetailDto, String> {
    let db = pool.handle()?;
    let s = db
        .get_research_session(session_id)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| format!("research session {session_id} not found"))?;
    let artifact = db
        .get_research_artifact(session_id)
        .await
        .map_err(map_db_err)?;
    let summary = artifact
        .as_ref()
        .map(|a| extract_research_summary(&a.artifact_json))
        .unwrap_or_default();
    Ok(ResearchDetailDto {
        session: ResearchSessionDto {
            id: s.id,
            status: s.status.clone(),
            query_text: s.query_text.clone(),
            started_at_ms: s.started_at_ms,
            finished_at_ms: s.finished_at_ms,
        },
        report_markdown: artifact.as_ref().map(|a| a.report_markdown.clone()),
        artifact_json: artifact.as_ref().map(|a| a.artifact_json.clone()),
        confidence_tier: summary.confidence_tier,
        claims: summary.claims,
        source_count: summary.source_count,
        citation_precision: summary.citation_precision,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct PublicationManifestDto {
    pub publication_id: String,
    pub content_type: String,
    pub state: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[tauri::command]
pub async fn list_publication_manifests(
    pool: State<'_, GuiDbPool>,
    limit: Option<u32>,
) -> Result<Vec<PublicationManifestDto>, String> {
    let db = pool.handle()?;
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, limit.unwrap_or(200) as i64)
        .await
        .map_err(map_db_err)?;
    Ok(manifests
        .iter()
        .map(|m| PublicationManifestDto {
            publication_id: m.publication_id.clone(),
            content_type: m.content_type.clone(),
            state: m.state.clone(),
            created_at_ms: m.created_at_ms,
            updated_at_ms: m.updated_at_ms,
        })
        .collect())
}

/// Assemble a `QueueSnapshot` (candidates, claims-pending, retraction queue,
/// stall detection) from the shared [`GuiDbPool`] — the native-command
/// equivalent of `vox scientia dashboard`, which the GUI previously shelled
/// out to via `execute_command`. That path opened a SEPARATE, fresh
/// `VoxDb::connect_default()` in a spawned subprocess while this app's own
/// pool already held the DB file open, producing "Locking error ... os error
/// 33 / SQLITE_BUSY" — the exact class of bug `GuiDbPool` exists to prevent
/// (see its module doc comment), just not yet applied to this surface.
#[tauri::command]
pub async fn scientia_dashboard_snapshot(
    pool: State<'_, GuiDbPool>,
) -> Result<vox_scientia::dashboard::QueueSnapshot, String> {
    use vox_scientia::dashboard::{
        CandidateRow, ClaimsPendingSummary, DashboardInputs, ReplyWindowEntry, build_queue_snapshot,
    };
    let db = pool.handle()?;
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, 200)
        .await
        .map_err(map_db_err)?;
    // Manifests carry no confidence signal; sourced as 0.0 (honest unknown) —
    // mirrors vox-cli's `scientia_phase_handlers::scientia_dashboard`.
    let candidates: Vec<CandidateRow> = manifests
        .iter()
        .map(|m| CandidateRow {
            candidate_id: m.publication_id.clone(),
            candidate_class: m.content_type.clone(),
            confidence: 0.0,
            state: m.state.clone(),
            created_at_ms: m.created_at_ms,
            updated_at_ms: m.updated_at_ms,
        })
        .collect();
    let retraction_queue = retraction_queue_from(&candidates);
    let counts = db
        .scientia_claims_pending_summary()
        .await
        .map_err(map_db_err)?;
    let claims_pending = ClaimsPendingSummary {
        verifiable: counts.verifiable.max(0) as u64,
        abstained: counts.abstained.max(0) as u64,
        extraction_running: counts.extraction_running.max(0) as u64,
    };
    let manifests_in_reply_window: Vec<ReplyWindowEntry> = Vec::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let inputs = DashboardInputs {
        candidates: &candidates,
        claims_pending,
        manifests_in_reply_window: &manifests_in_reply_window,
        retraction_queue: &retraction_queue,
        now_ms,
    };
    Ok(build_queue_snapshot(&inputs))
}

fn retraction_queue_from(candidates: &[vox_scientia::dashboard::CandidateRow]) -> Vec<String> {
    candidates
        .iter()
        .filter(|c| c.state == "retracted")
        .map(|c| c.candidate_id.clone())
        .collect()
}

/// Native-command equivalent of `vox scientia cost`, for the same reason as
/// [`scientia_dashboard_snapshot`] — avoids a second `VoxDb::connect_default()`
/// contending with this app's own already-open pool.
#[tauri::command]
pub async fn scientia_cost_rollup(
    pool: State<'_, GuiDbPool>,
) -> Result<vox_scientia::dashboard::cost::CostRollup, String> {
    use vox_scientia::dashboard::cost::{CostInputs, build_cost_rollup};
    let db = pool.handle()?;
    let (provider_rows, phase_rows, findings) = db
        .scientia_cost_raw_this_quarter()
        .await
        .map_err(map_db_err)?;
    let by_provider: Vec<(String, f64)> = provider_rows
        .into_iter()
        .map(|r| (r.provider, r.total_usd))
        .collect();
    let mut inputs = CostInputs {
        extraction_usd: 0.0,
        critic_usd: 0.0,
        novelty_retrieval_usd: 0.0,
        scholarly_submission_usd: 0.0,
        by_provider,
        findings_published_this_quarter: findings,
    };
    apply_phase_costs(
        &mut inputs,
        phase_rows.iter().map(|r| (r.phase.as_str(), r.total_usd)),
    );
    Ok(build_cost_rollup(&inputs))
}

/// Map `(pipeline_phase, usd)` rows onto the four `CostInputs` category
/// fields. Mirrors `vox-cli`'s `scientia_phase_handlers::apply_phase_costs`.
fn apply_phase_costs<'a>(
    inputs: &mut vox_scientia::dashboard::cost::CostInputs,
    phases: impl IntoIterator<Item = (&'a str, f64)>,
) {
    for (phase, usd) in phases {
        match phase {
            "extraction" => inputs.extraction_usd += usd,
            "critic" => inputs.critic_usd += usd,
            "novelty" => inputs.novelty_retrieval_usd += usd,
            "scholarly" => inputs.scholarly_submission_usd += usd,
            _ => { /* unknown phase: ignore (forward-compat) */ }
        }
    }
}

// ── Live Scientia-queue push bridge (F2) ─────────────────────────────────────

/// Tauri event channel carrying a lightweight "the Scientia queue changed" ping
/// to the UI. The payload is a compact signal object
/// (`{ signal: u64, manifest_count, research_count }`); on receipt the UI
/// refetches via the typed read commands above.
pub const SCIENTIA_QUEUE_EVENT: &str = "vox://scientia-queue";

/// Tauri event channel for newly-surfaced discovery inbox rows (mirrors the
/// orchestrator-mcp `scientia.discovery.surfaced` WS topic at the desktop boundary).
pub const SCIENTIA_DISCOVERY_SURFACED_EVENT: &str = "vox://scientia-discovery-surfaced";

/// Wire payload for [`SCIENTIA_DISCOVERY_SURFACED_EVENT`]: one inbox row.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoverySurfacedDto {
    pub id: i64,
    pub publication_id: String,
    pub surfaced_at_ms: i64,
    pub intake_tier: String,
    pub signal_codes: Vec<String>,
    pub origin: String,
}

fn discovery_inbox_origin(signal_codes: &[String]) -> &'static str {
    if signal_codes
        .iter()
        .any(|code| code.starts_with("research_pipeline."))
    {
        "research"
    } else {
        "commit_watcher"
    }
}

fn discovery_row_to_dto(row: &vox_db::DiscoveryInboxRow) -> DiscoverySurfacedDto {
    DiscoverySurfacedDto {
        id: row.id,
        publication_id: row.publication_id.clone(),
        surfaced_at_ms: row.surfaced_at_ms,
        intake_tier: row.intake_tier.clone(),
        signal_codes: row.signal_codes.clone(),
        origin: discovery_inbox_origin(&row.signal_codes).to_string(),
    }
}

/// How often the push bridge samples the DB for a change. The UI keeps its own
/// (longer) interval as a fallback, so this only governs push latency.
const SCIENTIA_POLL_INTERVAL_MS: u64 = crate::config::SCIENTIA_QUEUE_POLL_SECS * 1000;

/// Compute a compact change signal over the Scientia queue: a hash folded from
/// each publication manifest's `(publication_id, state, updated_at_ms)` plus each
/// research session's `(id, status, finished_at_ms)`. Any add / state transition
/// / timestamp bump flips the signal; a steady queue keeps it stable. Returns
/// `(signal, manifest_count, research_count)`.
async fn scientia_queue_signal(db: &vox_db::VoxDb) -> Result<(u64, usize, usize), String> {
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, 500)
        .await
        .map_err(map_db_err)?;
    let sessions = db
        .list_recent_research_sessions(200)
        .await
        .map_err(map_db_err)?;
    fn fnv1a_mix(mut acc: u64, bytes: &[u8]) -> u64 {
        for &b in bytes {
            acc ^= b as u64;
            acc = acc.wrapping_mul(0x00000100_000001B3);
        }
        acc
    }
    let mut acc: u64 = 0xcbf29ce484222325; // FNV offset basis
    for m in &manifests {
        acc = fnv1a_mix(acc, m.publication_id.as_bytes());
        acc = fnv1a_mix(acc, m.state.as_bytes());
        acc = fnv1a_mix(acc, &m.updated_at_ms.to_le_bytes());
    }
    for s in &sessions {
        acc = fnv1a_mix(acc, &s.id.to_le_bytes());
        acc = fnv1a_mix(acc, s.status.as_bytes());
        let ts_bytes = s.finished_at_ms.unwrap_or(-1_i64).to_le_bytes();
        acc = fnv1a_mix(acc, &ts_bytes);
    }
    Ok((acc, manifests.len(), sessions.len()))
}

/// Spawn a background task that watches the canonical DB for Scientia-queue
/// changes and emits a [`SCIENTIA_QUEUE_EVENT`] ping when the queue signal flips.
///
/// This is the Scientia analog of
/// [`spawn_orchestrator_status_stream`](crate::commands::orchestrator::spawn_orchestrator_status_stream),
/// adapted to a DB-backed surface: the Scientia queue is sourced from the
/// canonical DB via the typed read commands (not the daemon's status stream and
/// not the disabled HTTP gateway), so there is no daemon RPC to subscribe to.
/// Instead we poll a cheap change signal and push only on change — turning the
/// UI's interval refresh into event-driven refresh. Resilient by design: a DB
/// error is logged and retried on the next tick; the task never crashes the app.
// toestub-ignore(skeleton/untested-pub-api) — spawns a background DB-watch task bridging Scientia-queue changes to Tauri events; covered by integration
pub fn spawn_scientia_queue_stream(app_handle: tauri::AppHandle, db: Arc<VoxDb>) {
    tokio::spawn(async move {
        let mut last_signal: Option<u64> = None;
        loop {
            match scientia_queue_signal(&db).await {
                Ok((signal, manifest_count, research_count)) => {
                    if last_signal != Some(signal) {
                        last_signal = Some(signal);
                        let _ = app_handle.emit(
                            SCIENTIA_QUEUE_EVENT,
                            serde_json::json!({
                                "signal": signal,
                                "manifest_count": manifest_count,
                                "research_count": research_count,
                            }),
                        );
                    }
                }
                Err(e) => tracing::debug!("scientia queue signal failed: {e}"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(SCIENTIA_POLL_INTERVAL_MS)).await;
        }
    });
}

/// Spawn a background task that watches `scientia_discovery_inbox` for new row ids
/// and emits [`SCIENTIA_DISCOVERY_SURFACED_EVENT`] for each newly-surfaced candidate.
// toestub-ignore(skeleton/untested-pub-api) — spawns a background DB-watch task; covered by unit tests on diff logic
pub fn spawn_discovery_surfaced_stream(app_handle: tauri::AppHandle, db: Arc<VoxDb>) {
    tokio::spawn(async move {
        let mut last_max_id: i64 = db.max_discovery_inbox_id().await.unwrap_or(0);
        loop {
            match db.max_discovery_inbox_id().await {
                Ok(current_max) if current_max > last_max_id => {
                    match db.discoveries_since(last_max_id, 64).await {
                        Ok(rows) => {
                            for row in rows {
                                let dto = discovery_row_to_dto(&row);
                                let _ = app_handle.emit(SCIENTIA_DISCOVERY_SURFACED_EVENT, &dto);
                                last_max_id = last_max_id.max(row.id);
                            }
                            last_max_id = last_max_id.max(current_max);
                        }
                        Err(e) => {
                            tracing::debug!("discovery surfaced: discoveries_since failed: {e}")
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => tracing::debug!("discovery surfaced: max id failed: {e}"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(SCIENTIA_POLL_INTERVAL_MS)).await;
        }
    });
}

#[cfg(test)]
mod research_summary_tests {
    use super::extract_research_summary;

    fn fake_artifact_json() -> String {
        // Minimal shape matching `ResearchRunArtifact` -> `ResearchResult` ->
        // `ResearchMetadata` as serialized by vox-research-shim's pipeline.
        r#"{
            "schema_version": 1,
            "query": {},
            "plan": {},
            "report_markdown": "",
            "result": {
                "answer": "x",
                "sources": [],
                "citations": [
                    { "source_id": 1, "url": "https://example.com/a", "title": "A", "snippet": "", "confidence": 0.9 },
                    { "source_id": 2, "url": "https://example.org/b", "title": "B", "snippet": "", "confidence": 0.5 }
                ],
                "research_metadata": {
                    "session_id": 1,
                    "duration_ms": 10,
                    "provider": "test",
                    "routing_tier": "DeepResearch",
                    "confidence": 0.8,
                    "subquery_count": 1,
                    "source_count": 2,
                    "claim_verdicts": [
                        {
                            "claim": { "text": "The sky is blue.", "claim_id": 42, "is_numeric": false, "is_recent": false, "is_named_event": false },
                            "verdict": "supported",
                            "confidence": 0.95,
                            "supporting_count": 2,
                            "contradicting_count": 0,
                            "evidence_spans": [
                                { "source_id": 1, "span_start": 0, "span_end": 5, "text": "blue", "span_type": "supporting" },
                                { "source_id": 2, "span_start": 0, "span_end": 5, "text": "blue", "span_type": "supporting" }
                            ],
                            "resample_stability": 1.0
                        }
                    ],
                    "retrieval_diagnostics": {},
                    "quality_score": 80,
                    "planner_degraded": false,
                    "competence": null,
                    "self_verification": null,
                    "citation_audit": { "checked_citations": 2, "supported_citations": 2, "unsupported_citation_indices": [], "precision": 1.0, "supports": [] },
                    "corroboration_counts": [[42, 2]]
                }
            }
        }"#
        .to_string()
    }

    #[test]
    fn extracts_confidence_tier_claims_and_source_count() {
        let summary = extract_research_summary(&fake_artifact_json());
        assert_eq!(summary.confidence_tier.as_deref(), Some("DeepResearch"));
        assert_eq!(summary.source_count, Some(2));
        assert_eq!(summary.citation_precision, Some(1.0));
        assert_eq!(summary.claims.len(), 1);
        let claim = &summary.claims[0];
        assert_eq!(claim.claim_id, "42");
        assert_eq!(claim.text, "The sky is blue.");
        assert_eq!(claim.verdict, "Supported");
        assert!((claim.confidence - 0.95).abs() < f64::EPSILON);
        assert!((claim.resample_stability - 1.0).abs() < f64::EPSILON);
        assert_eq!(
            claim.citation_urls,
            vec![
                "https://example.com/a".to_string(),
                "https://example.org/b".to_string()
            ]
        );
        assert_eq!(claim.corroboration_count, 2);
    }

    #[test]
    fn fails_open_on_malformed_json() {
        let summary = extract_research_summary("not json");
        assert!(summary.confidence_tier.is_none());
        assert!(summary.claims.is_empty());
        assert!(summary.source_count.is_none());
    }
}

#[cfg(test)]
mod discovery_surfaced_tests {
    use super::*;

    #[test]
    fn discovery_origin_flags_research_pipeline_signals() {
        assert_eq!(
            discovery_inbox_origin(&["research_pipeline.supported_claims".into()]),
            "research"
        );
        assert_eq!(
            discovery_inbox_origin(&["perf_claim".into()]),
            "commit_watcher"
        );
    }
}

#[cfg(test)]
mod dashboard_snapshot_tests {
    use super::*;

    #[tokio::test]
    async fn dashboard_snapshot_over_pool_succeeds_on_an_empty_db() {
        // The pooled equivalent of vox-cli's own
        // `dashboard_snapshot_empty_inputs_succeeds` regression test — proves
        // this reads through GuiDbPool without erroring, which is the whole
        // point of the fix (no fresh VoxDb::connect_default() to contend
        // with the app's own already-open connection).
        use tauri::Manager;
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let snap = scientia_dashboard_snapshot(pool)
            .await
            .expect("empty DB yields a zeroed snapshot, not an error");
        assert_eq!(snap.candidates.total, 0);
        assert_eq!(snap.claims_pending.verifiable, 0);
        assert!(snap.retraction_queue.is_empty());
    }

    #[tokio::test]
    async fn cost_rollup_over_pool_succeeds_on_an_empty_db() {
        use tauri::Manager;
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let rollup = scientia_cost_rollup(pool)
            .await
            .expect("empty DB yields an all-zeros rollup, not an error");
        assert_eq!(rollup.this_quarter.total_usd, 0.0);
        assert!(rollup.by_provider.is_empty());
    }
}

#[cfg(test)]
mod tests {
    /// The signal hash is order-deterministic and sensitive to state changes:
    /// two folds over the same tuples match; a state change diverges. (Pure hash
    /// fold; no DB required.)
    #[test]
    fn queue_signal_fold_is_deterministic_and_state_sensitive() {
        fn fnv1a_mix(mut acc: u64, bytes: &[u8]) -> u64 {
            for &b in bytes {
                acc ^= b as u64;
                acc = acc.wrapping_mul(0x00000100_000001B3);
            }
            acc
        }
        fn fold(rows: &[(&str, &str, i64)]) -> u64 {
            let mut acc: u64 = 0xcbf29ce484222325;
            for (id, state, ts) in rows {
                acc = fnv1a_mix(acc, id.as_bytes());
                acc = fnv1a_mix(acc, state.as_bytes());
                acc = fnv1a_mix(acc, &ts.to_le_bytes());
            }
            acc
        }
        let a = fold(&[("pub-1", "draft", 100), ("pub-2", "approved", 200)]);
        let b = fold(&[("pub-1", "draft", 100), ("pub-2", "approved", 200)]);
        let c = fold(&[("pub-1", "approved", 100), ("pub-2", "approved", 200)]);
        assert_eq!(a, b, "same tuples -> same signal");
        assert_ne!(a, c, "a state transition flips the signal");
    }
}
