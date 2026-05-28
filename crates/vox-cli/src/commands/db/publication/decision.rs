use super::*;
use crate::commands::db_cli::ArxivHandoffStageExt;
use anyhow::{Context, Result};

/// Preflight + worthiness + discovery rank with optional live prior-art refresh (stdout JSON).
pub async fn publication_decision_explain(
    publication_id: &str,
    live_prior_art: bool,
    offline: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
    let mut manifest = publication_manifest_from_row(&row);
    if live_prior_art {
        if manifest.content_type != "scientia" {
            anyhow::bail!("--live-prior-art requires content_type `scientia`");
        }
        let candidate_id =
            vox_publisher::scientia_finding_ledger::default_candidate_id(publication_id);
        let query = vox_publisher::scientia_prior_art::PriorArtQuery {
            title: manifest.title.clone(),
            abstract_text: manifest.abstract_text.clone(),
        };
        let client = vox_http_client::client();
        let bundle = vox_publisher::scientia_prior_art::fetch_prior_art_federated(
            &client,
            &candidate_id,
            &query,
            vec![],
            vox_publisher::scientia_prior_art::PriorArtFetchOptions::default(),
            offline,
            &scientia_h,
        )
        .await?;
        manifest.metadata_json = Some(merge_novelty_bundle_into_metadata_json_str(
            manifest.metadata_json.as_deref(),
            &bundle,
        )?);
    }
    manifest =
        crate::commands::scientia_worthiness_enrich::enrich_manifest_for_worthiness_preflight(
            manifest, &db, &repo_root, None,
        )
        .await?;

    let contract_yaml = read_utf8_path_capped(
        &repo_root.join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH),
    )?;
    let contract = vox_publisher::publication_worthiness::load_contract_from_str(&contract_yaml)
        .context("worthiness yaml")?;
    vox_publisher::publication_worthiness::validate_contract_invariants(&contract)?;

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
        publication_id,
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

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "publication_id": publication_id,
            "preflight_readiness_score": report.readiness_score,
            "worthiness": report.worthiness,
            "discovery_rank": rank,
            "preflight_findings": report.findings,
            "impact_readership_projection": impact_readership_projection,
        }))?
    );
    Ok(())
}
/// Prior-art fetch + finding-candidate ledger row + decision snapshot (stdout JSON; does not persist unless `publication-novelty-fetch --persist-metadata` is used separately).
pub async fn publication_novelty_happy_path(publication_id: &str, offline: bool) -> Result<()> {
    let t0 = Instant::now();
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    if row.content_type != "scientia" {
        anyhow::bail!("publication-novelty-happy-path requires content_type `scientia`");
    }
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
    let candidate_id = vox_publisher::scientia_finding_ledger::default_candidate_id(publication_id);
    let query = vox_publisher::scientia_prior_art::PriorArtQuery {
        title: row.title.clone(),
        abstract_text: row.abstract_text.clone(),
    };
    let client = vox_http_client::client();
    let bundle = vox_publisher::scientia_prior_art::fetch_prior_art_federated(
        &client,
        &candidate_id,
        &query,
        vec![],
        vox_publisher::scientia_prior_art::PriorArtFetchOptions::default(),
        offline,
        &scientia_h,
    )
    .await?;

    let mut manifest = publication_manifest_from_row(&row);
    manifest.metadata_json = Some(merge_novelty_bundle_into_metadata_json_str(
        manifest.metadata_json.as_deref(),
        &bundle,
    )?);
    manifest =
        crate::commands::scientia_worthiness_enrich::enrich_manifest_for_worthiness_preflight(
            manifest, &db, &repo_root, None,
        )
        .await?;

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
        publication_id,
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
        Some(publication_id),
        Some(row.title.clone()),
        signals,
        publication_id,
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

    let contract_yaml = read_utf8_path_capped(
        &repo_root.join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH),
    )?;
    let contract = vox_publisher::publication_worthiness::load_contract_from_str(&contract_yaml)?;
    vox_publisher::publication_worthiness::validate_contract_invariants(&contract)?;
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
        publication_id,
        &candidate_id,
        &bundle,
        decision_latency_ms,
        offline,
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

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_kind": "scientia_novelty_happy_path",
            "finding_candidate": candidate,
            "novelty_evidence_bundle": bundle,
            "discovery_rank": rank,
            "worthiness": report.worthiness,
            "preflight_readiness_score": report.readiness_score,
            "calibration_telemetry": calibration,
            "impact_readership_projection": impact_readership_projection,
        }))?
    );
    Ok(())
}
/// Record an operator milestone for the arXiv-assist workflow (append-only audit in `publication_status_events`).
pub async fn publication_arxiv_handoff_record(
    publication_id: &str,
    stage: ArxivHandoffStageCli,
    operator: Option<&str>,
    note: Option<&str>,
    arxiv_id: Option<&str>,
) -> Result<()> {
    let publication_id = publication_id.trim();
    if publication_id.is_empty() {
        anyhow::bail!("publication_id must not be empty");
    }
    if matches!(stage, ArxivHandoffStageCli::Published)
        && arxiv_id.map(str::trim).filter(|s| !s.is_empty()).is_none()
    {
        anyhow::bail!("--arxiv-id is required when --stage published");
    }
    let db = vox_db::VoxDb::connect_default().await?;
    if db.get_publication_manifest(publication_id).await?.is_none() {
        anyhow::bail!("publication not found: {publication_id}");
    }
    let status = format!("arxiv_handoff:{}", stage.slug());
    let op_trim = operator.map(str::trim).filter(|s| !s.is_empty());
    let note_trim = note.map(str::trim).filter(|s| !s.is_empty());
    let arxiv_trim = arxiv_id.map(str::trim).filter(|s| !s.is_empty());
    let detail = serde_json::json!({
        "schema_version": 1_u32,
        "workflow": "arxiv_operator_assist",
        "stage": stage.slug(),
        "operator": op_trim,
        "note": note_trim,
        "arxiv_id": arxiv_trim,
    });
    db.append_publication_status_event(publication_id, &status, Some(&detail.to_string()))
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "recorded": true,
            "publication_id": publication_id,
            "status": status,
            "detail": detail,
        }))?
    );
    Ok(())
}
