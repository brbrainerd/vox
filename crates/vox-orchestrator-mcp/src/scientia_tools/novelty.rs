//! MCP: prior-art fetch + decision explain (parity with `vox db publication-novelty-*`).

use crate::params::ToolResult;
use crate::server_state::ServerState;
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::Instant;

use super::common::{REM_PUBLICATION_ID, REM_SCIENTIA_DB, publication_manifest_from_row};

fn merge_novelty_bundle_into_metadata_json_str(
    metadata_json: Option<&str>,
    bundle: &vox_publisher::scientia_finding_ledger::NoveltyEvidenceBundleV1,
) -> Result<String, String> {
    let mut root: serde_json::Value =
        if let Some(raw) = metadata_json.map(str::trim).filter(|s| !s.is_empty()) {
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
    root[vox_publisher::scientia_evidence::METADATA_KEY_SCIENTIA_NOVELTY_BUNDLE] =
        serde_json::to_value(bundle).map_err(|e| e.to_string())?;
    serde_json::to_string(&root).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationNoveltyFetchParams {
    pub publication_id: String,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub persist_metadata: bool,
}

pub async fn vox_scientia_publication_novelty_fetch(
    state: &ServerState,
    params: VoxScientiaPublicationNoveltyFetchParams,
) -> String {
    let Some(db) = &state.db else {
        return super::common::no_voxdb_tool_string();
    };
    let pid = params.publication_id.trim();
    let row = match db.get_publication_manifest(pid).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let Some(row) = row else {
        return ToolResult::<String>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json();
    };
    if row.content_type != "scientia" {
        return ToolResult::<()>::err_with_remediation(
            format!(
                "publication-novelty-fetch requires content_type scientia (got `{}`)",
                row.content_type
            ),
            REM_PUBLICATION_ID,
        )
        .to_json();
    }
    let candidate_id = vox_publisher::scientia_finding_ledger::default_candidate_id(pid);
    let query = vox_publisher::scientia_prior_art::PriorArtQuery {
        title: row.title.clone(),
        abstract_text: row.abstract_text.clone(),
    };
    let client = vox_http_client::client();
    let scientia_h = vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(
        &state.repository.root,
    );
    let bundle = match vox_publisher::scientia_prior_art::fetch_prior_art_federated(
        &client,
        &candidate_id,
        &query,
        vec![],
        vox_publisher::scientia_prior_art::PriorArtFetchOptions::default(),
        params.offline,
        &scientia_h,
    )
    .await
    {
        Ok(b) => b,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("prior-art fetch: {e:#}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };

    if params.persist_metadata {
        let mut manifest = publication_manifest_from_row(&row);
        let meta = match merge_novelty_bundle_into_metadata_json_str(
            manifest.metadata_json.as_deref(),
            &bundle,
        ) {
            Ok(s) => s,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("metadata merge: {e}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };
        manifest.metadata_json = Some(meta);
        let digest = manifest.content_sha3_256();
        if let Err(e) = db
            .upsert_publication_manifest(vox_db::PublicationManifestParams {
                publication_id: &manifest.publication_id,
                content_type: &manifest.content_type,
                source_ref: manifest.source_ref.as_deref(),
                title: &manifest.title,
                author: &manifest.author,
                abstract_text: manifest.abstract_text.as_deref(),
                body_markdown: &manifest.body_markdown,
                citations_json: manifest.citations_json.as_deref(),
                metadata_json: manifest.metadata_json.as_deref(),
                revision_history_json: None,
                content_sha3_256: &digest,
                state: row.state.as_str(),
            })
            .await
        {
            return ToolResult::<String>::err_with_remediation(
                format!("DB upsert: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
        let _ = db
            .append_publication_status_event(
                pid,
                "scientia_novelty_bundle_updated",
                Some(
                    &serde_json::json!({ "bundle_id": bundle.bundle_id, "digest": digest })
                        .to_string(),
                ),
            )
            .await;
    }

    ToolResult::ok(serde_json::json!({
        "schema_kind": "vox_scientia_publication_novelty_fetch",
        "publication_id": pid,
        "novelty_evidence_bundle": bundle,
        "persisted": params.persist_metadata,
    }))
    .to_json()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationDecisionExplainParams {
    pub publication_id: String,
    #[serde(default)]
    pub live_prior_art: bool,
    #[serde(default)]
    pub offline: bool,
}

pub async fn vox_scientia_publication_decision_explain(
    state: &ServerState,
    params: VoxScientiaPublicationDecisionExplainParams,
) -> String {
    let Some(db) = &state.db else {
        return super::common::no_voxdb_tool_string();
    };
    let pid = params.publication_id.trim();
    let row = match db.get_publication_manifest(pid).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let Some(row) = row else {
        return ToolResult::<String>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json();
    };
    let mut manifest = publication_manifest_from_row(&row);
    let scientia_h = vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(
        &state.repository.root,
    );
    if params.live_prior_art {
        if manifest.content_type != "scientia" {
            return ToolResult::<()>::err_with_remediation(
                "--live-prior-art requires content_type scientia",
                REM_PUBLICATION_ID,
            )
            .to_json();
        }
        let candidate_id = vox_publisher::scientia_finding_ledger::default_candidate_id(pid);
        let query = vox_publisher::scientia_prior_art::PriorArtQuery {
            title: manifest.title.clone(),
            abstract_text: manifest.abstract_text.clone(),
        };
        let client = vox_http_client::client();
        let bundle = match vox_publisher::scientia_prior_art::fetch_prior_art_federated(
            &client,
            &candidate_id,
            &query,
            vec![],
            vox_publisher::scientia_prior_art::PriorArtFetchOptions::default(),
            params.offline,
            &scientia_h,
        )
        .await
        {
            Ok(b) => b,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("prior-art fetch: {e:#}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };
        let meta = match merge_novelty_bundle_into_metadata_json_str(
            manifest.metadata_json.as_deref(),
            &bundle,
        ) {
            Ok(s) => s,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("metadata merge: {e}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };
        manifest.metadata_json = Some(meta);
    }

    manifest =
        match vox_publisher::scientia_worthiness_enrich::enrich_manifest_socrates_and_sidecars(
            manifest,
            db,
            &state.repository.root,
            None,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("worthiness enrich: {e:#}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };

    let contract_path = state
        .repository
        .root
        .join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH);
    let contract_yaml = match std::fs::read_to_string(&contract_path) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("read worthiness contract {}: {e}", contract_path.display()),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let contract =
        match vox_publisher::publication_worthiness::load_contract_from_str(&contract_yaml) {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("parse worthiness: {e:#}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };
    if let Err(e) = vox_publisher::publication_worthiness::validate_contract_invariants(&contract) {
        return ToolResult::<String>::err_with_remediation(
            format!("worthiness invariants: {e:#}"),
            REM_SCIENTIA_DB,
        )
        .to_json();
    }

    let report = vox_publisher::publication_preflight::run_preflight_with_worthiness_heuristics(
        &manifest,
        vox_publisher::publication_preflight::PreflightProfile::Default,
        &contract,
        &scientia_h,
    );
    let evidence = vox_publisher::scientia_evidence::parse_scientia_evidence(
        manifest.metadata_json.as_deref(),
    )
    .unwrap_or_default();
    let novelty_bundle = vox_publisher::scientia_prior_art::parse_novelty_bundle_from_metadata_json(
        manifest.metadata_json.as_deref(),
    );
    let overlap_for_rank = novelty_bundle.as_ref().map(|b| {
        vox_publisher::scientia_finding_ledger::novelty_overlap_blend_01(b, &scientia_h) as f32
    });
    let mut rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
        pid,
        manifest.source_ref.as_deref(),
        &evidence,
        &scientia_h,
        overlap_for_rank,
    );
    if let Some(ref b) = novelty_bundle {
        vox_publisher::scientia_discovery::merge_novelty_overlap_into_rank(
            &mut rank,
            b,
            &scientia_h,
        );
    }
    let impact_readership_projection =
        vox_publisher::scientia_prior_art::parse_novelty_bundle_from_metadata_json(
            manifest.metadata_json.as_deref(),
        )
        .map(|b| {
            vox_publisher::scientia_finding_ledger::impact_readership_projection_v1(&b, &scientia_h)
        });

    ToolResult::ok(serde_json::json!({
        "schema_kind": "vox_scientia_publication_decision_explain",
        "publication_id": pid,
        "preflight_readiness_score": report.readiness_score,
        "worthiness": report.worthiness,
        "discovery_rank": rank,
        "preflight_findings": report.findings,
        "impact_readership_projection": impact_readership_projection,
    }))
    .to_json()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationNoveltyHappyPathParams {
    pub publication_id: String,
    #[serde(default)]
    pub offline: bool,
}

pub async fn vox_scientia_publication_novelty_happy_path(
    state: &ServerState,
    params: VoxScientiaPublicationNoveltyHappyPathParams,
) -> String {
    let t0 = Instant::now();
    let Some(db) = &state.db else {
        return super::common::no_voxdb_tool_string();
    };
    let pid = params.publication_id.trim();
    let row = match db.get_publication_manifest(pid).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let Some(row) = row else {
        return ToolResult::<String>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json();
    };
    if row.content_type != "scientia" {
        return ToolResult::<()>::err_with_remediation(
            "publication-novelty-happy-path requires content_type scientia",
            REM_PUBLICATION_ID,
        )
        .to_json();
    }

    let scientia_h = vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(
        &state.repository.root,
    );
    let candidate_id = vox_publisher::scientia_finding_ledger::default_candidate_id(pid);
    let query = vox_publisher::scientia_prior_art::PriorArtQuery {
        title: row.title.clone(),
        abstract_text: row.abstract_text.clone(),
    };
    let client = vox_http_client::client();
    let bundle = match vox_publisher::scientia_prior_art::fetch_prior_art_federated(
        &client,
        &candidate_id,
        &query,
        vec![],
        vox_publisher::scientia_prior_art::PriorArtFetchOptions::default(),
        params.offline,
        &scientia_h,
    )
    .await
    {
        Ok(b) => b,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("prior-art fetch: {e:#}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };

    let mut manifest = publication_manifest_from_row(&row);
    let meta = match merge_novelty_bundle_into_metadata_json_str(
        manifest.metadata_json.as_deref(),
        &bundle,
    ) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("metadata merge: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    manifest.metadata_json = Some(meta);
    manifest =
        match vox_publisher::scientia_worthiness_enrich::enrich_manifest_socrates_and_sidecars(
            manifest,
            db,
            &state.repository.root,
            None,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("worthiness enrich: {e:#}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };

    let evidence = vox_publisher::scientia_evidence::parse_scientia_evidence(
        manifest.metadata_json.as_deref(),
    )
    .unwrap_or_default();
    let signals = if evidence.discovery_signals.is_empty() {
        vox_publisher::scientia_evidence::infer_discovery_signals(
            manifest.source_ref.as_deref(),
            &evidence,
        )
    } else {
        evidence.discovery_signals.clone()
    };
    let overlap_for_rank =
        vox_publisher::scientia_finding_ledger::novelty_overlap_blend_01(&bundle, &scientia_h)
            as f32;
    let mut rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
        pid,
        manifest.source_ref.as_deref(),
        &evidence,
        &scientia_h,
        Some(overlap_for_rank),
    );
    vox_publisher::scientia_discovery::merge_novelty_overlap_into_rank(
        &mut rank,
        &bundle,
        &scientia_h,
    );
    let now = vox_publisher::scientia_prior_art::now_unix_ms_strict();
    let mut candidate = vox_publisher::scientia_finding_ledger::build_finding_candidate(
        Some(pid),
        Some(row.title.clone()),
        signals,
        pid,
        rank.strong_signal_count,
        rank.supporting_signal_count,
        rank.informational_signal_count,
        rank.rank_score,
        rank.intake_tier == vox_publisher::scientia_discovery::DiscoveryIntakeTier::LowSignal,
        !rank.conflicts.is_empty(),
        now,
        &scientia_h,
    );
    candidate.novelty_evidence_bundle_id = Some(bundle.bundle_id.clone());

    let contract_path = state
        .repository
        .root
        .join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH);
    let contract_yaml = match std::fs::read_to_string(&contract_path) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("read worthiness contract {}: {e}", contract_path.display()),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let contract =
        match vox_publisher::publication_worthiness::load_contract_from_str(&contract_yaml) {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("parse worthiness: {e:#}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        };
    if let Err(e) = vox_publisher::publication_worthiness::validate_contract_invariants(&contract) {
        return ToolResult::<String>::err_with_remediation(
            format!("worthiness invariants: {e:#}"),
            REM_SCIENTIA_DB,
        )
        .to_json();
    }

    let report = vox_publisher::publication_preflight::run_preflight_with_worthiness_heuristics(
        &manifest,
        vox_publisher::publication_preflight::PreflightProfile::Default,
        &contract,
        &scientia_h,
    );

    let decision_latency_ms = t0.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let (worthiness_decision, worthiness_score, hard_metrics_ok) = match report.worthiness.as_ref()
    {
        Some(w) => (
            serde_json::to_value(w.decision)
                .ok()
                .and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s),
                    _ => None,
                })
                .unwrap_or_else(|| "unknown".to_string()),
            w.worthiness_score,
            w.hard_metrics_ok,
        ),
        None => ("unknown".to_string(), 0.0, false),
    };
    let calibration = vox_publisher::scientia_finding_ledger::novelty_decision_calibration_v1(
        pid,
        &candidate_id,
        &bundle,
        decision_latency_ms,
        params.offline,
        &worthiness_decision,
        worthiness_score,
        hard_metrics_ok,
        rank.prior_art_max_lexical_overlap,
    );
    let impact_readership_projection =
        vox_publisher::scientia_finding_ledger::impact_readership_projection_v1(
            &bundle,
            &scientia_h,
        );

    ToolResult::ok(serde_json::json!({
        "schema_kind": "vox_scientia_publication_novelty_happy_path",
        "finding_candidate": candidate,
        "novelty_evidence_bundle": bundle,
        "discovery_rank": rank,
        "worthiness": report.worthiness,
        "preflight_readiness_score": report.readiness_score,
        "calibration_telemetry": calibration,
        "impact_readership_projection": impact_readership_projection,
    }))
    .to_json()
}
