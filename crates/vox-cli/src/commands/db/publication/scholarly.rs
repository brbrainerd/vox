use super::*;
use crate::commands::db_cli::ScholarlyVenueExt;
use anyhow::Result;

pub async fn publication_zenodo_metadata(publication_id: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = vox_publisher::publication::PublicationManifest {
        publication_id: row.publication_id.clone(),
        content_type: row.content_type.clone(),
        source_ref: row.source_ref.clone(),
        title: row.title.clone(),
        author: row.author.clone(),
        abstract_text: row.abstract_text.clone(),
        body_markdown: row.body_markdown.clone(),
        citations_json: row.citations_json.clone(),
        metadata_json: row.metadata_json.clone(),
    };
    let z = vox_publisher::zenodo_metadata::zenodo_deposition_metadata(&manifest);
    println!("{}", serde_json::to_string_pretty(&z)?);
    Ok(())
}
/// Print merged OpenReview submit profile JSON (invitation, signature, readers, API base; no HTTP).
pub async fn publication_openreview_profile(publication_id: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = vox_publisher::publication::PublicationManifest {
        publication_id: row.publication_id.clone(),
        content_type: row.content_type.clone(),
        source_ref: row.source_ref.clone(),
        title: row.title.clone(),
        author: row.author.clone(),
        abstract_text: row.abstract_text.clone(),
        body_markdown: row.body_markdown.clone(),
        citations_json: row.citations_json.clone(),
        metadata_json: row.metadata_json.clone(),
    };
    let profile = vox_publisher::scholarly::export_openreview_submit_profile(&manifest)?;
    println!("{}", serde_json::to_string_pretty(&profile)?);
    Ok(())
}
/// Write [`vox_publisher::submission`] staging files for an existing manifest (by id).
pub async fn publication_scholarly_staging_export(
    publication_id: &str,
    output_dir: &std::path::Path,
    venue: vox_publisher::submission::ScholarlyVenue,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = vox_publisher::publication::PublicationManifest {
        publication_id: row.publication_id.clone(),
        content_type: row.content_type.clone(),
        source_ref: row.source_ref.clone(),
        title: row.title.clone(),
        author: row.author.clone(),
        abstract_text: row.abstract_text.clone(),
        body_markdown: row.body_markdown.clone(),
        citations_json: row.citations_json.clone(),
        metadata_json: row.metadata_json.clone(),
    };
    let written = vox_publisher::submission::write_scholarly_staging(&manifest, venue, output_dir)?;
    vox_publisher::submission::validate_scholarly_staging(output_dir, venue, &manifest).map_err(
        |findings: Vec<vox_publisher::submission::validation::ValidationFinding>| {
            let msg: String = findings
                .iter()
                .map(|f| format!("{}: {}", f.code, f.message))
                .collect::<Vec<_>>()
                .join("; ");
            anyhow::anyhow!("staging validation failed: {msg}")
        },
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "publication_id": publication_id,
            "output_dir": output_dir,
            "venue": venue.as_str(),
            "written": written,
        }))?
    );
    Ok(())
}
/// One-shot scholarly pipeline: local preflight, dual-approval gate, optional staging export, then digest-bound submit.
pub async fn publication_scholarly_pipeline_run(
    publication_id: &str,
    preflight_profile: vox_publisher::publication_preflight::PreflightProfile,
    dry_run: bool,
    staging_output_dir: Option<&std::path::Path>,
    venue: Option<ScholarlyVenueCli>,
    adapter: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = vox_publisher::publication::PublicationManifest {
        publication_id: row.publication_id.clone(),
        content_type: row.content_type.clone(),
        source_ref: row.source_ref.clone(),
        title: row.title.clone(),
        author: row.author.clone(),
        abstract_text: row.abstract_text.clone(),
        body_markdown: row.body_markdown.clone(),
        citations_json: row.citations_json.clone(),
        metadata_json: row.metadata_json.clone(),
    };
    let report =
        publication_preflight_report_for_row(&db, &row, &manifest, preflight_profile, false)
            .await?;
    if !report.ok {
        anyhow::bail!(
            "scholarly pipeline preflight failed (readiness {}):\n{}",
            report.readiness_score,
            serde_json::to_string_pretty(&report)?
        );
    }
    let digest = row.content_sha3_256.clone();
    let dual = db
        .has_dual_publication_approval_for_digest(publication_id, &digest)
        .await?;
    if !dual {
        anyhow::bail!(
            "scholarly pipeline requires two distinct digest-bound approvers before staging export / submit"
        );
    }
    let mut stages: Vec<String> = vec!["preflight_ok".into(), "dual_approval_ok".into()];

    match (venue, staging_output_dir) {
        (Some(v), Some(out)) => {
            if dry_run {
                stages.push(format!(
                    "staging_skipped_dry_run venue={} dir={}",
                    v.to_venue().as_str(),
                    out.display()
                ));
            } else {
                publication_scholarly_staging_export(publication_id, out, v.to_venue()).await?;
                stages.push("staging_exported".into());
            }
        }
        (None, Some(_)) => {
            anyhow::bail!("--staging-output-dir requires --venue");
        }
        (Some(_), None) => {
            anyhow::bail!("--venue requires --staging-output-dir (or omit both)");
        }
        (None, None) => {}
    }

    if dry_run {
        let doc = serde_json::json!({
            "dry_run": true,
            "publication_id": publication_id,
            "digest": digest,
            "stages": stages,
            "preflight_report": report,
        });
        if json_output {
            println!("{}", serde_json::to_string(&doc)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&doc)?);
        }
        return Ok(());
    }

    let receipt = vox_publisher::scholarly_external_jobs::publication_scholarly_submit_with_ledger(
        &db,
        publication_id,
        adapter,
    )
    .await?;
    let doc = serde_json::json!({
        "pipeline_completed": true,
        "publication_id": publication_id,
        "digest": digest,
        "stages": stages,
        "submission": {
            "adapter": receipt.adapter,
            "external_submission_id": receipt.external_submission_id,
            "status": receipt.status,
        }
    });
    if json_output {
        println!("{}", serde_json::to_string(&doc)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    }
    Ok(())
}
/// Record one digest-bound publication approval.
pub async fn publication_approve(publication_id: &str, approver: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(manifest) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let approver = approver.trim();
    if approver.is_empty() {
        anyhow::bail!("approver must not be empty");
    }
    db.record_publication_approval_for_digest(publication_id, &manifest.content_sha3_256, approver)
        .await?;
    let count = db
        .count_publication_approvers_for_digest(publication_id, &manifest.content_sha3_256)
        .await?;
    if count >= 2 {
        db.set_publication_state(publication_id, "approved", None)
            .await?;
    }
    println!(
        "Recorded approval for '{}' digest={} distinct_approvers={}",
        publication_id, manifest.content_sha3_256, count
    );
    Ok(())
}
/// Submit to the scholarly adapter (`--adapter` or `VOX_SCHOLARLY_ADAPTER`; default `local_ledger`).
pub async fn publication_submit_local(publication_id: &str, adapter: Option<&str>) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let receipt = vox_publisher::scholarly_external_jobs::publication_scholarly_submit_with_ledger(
        &db,
        publication_id,
        adapter,
    )
    .await?;
    println!(
        "Submitted '{}' via {} as {} ({})",
        publication_id, receipt.adapter, receipt.external_submission_id, receipt.status
    );
    Ok(())
}
/// Show publication state and scholarly submission rows.
pub async fn publication_status(publication_id: &str, with_worthiness: bool) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = publication_manifest_from_row(&row);
    let preflight_profile = vox_publisher::publication_preflight::PreflightProfile::Default;
    let preflight_report = publication_preflight_report_for_row(
        &db,
        &row,
        &manifest,
        preflight_profile,
        with_worthiness,
    )
    .await?;
    let operator_status_surface_v1 =
        vox_publisher::publication_preflight::operator_status_surface_v1(
            publication_id,
            preflight_profile,
            &preflight_report,
        );
    let approvals = db
        .count_publication_approvers_for_digest(publication_id, &row.content_sha3_256)
        .await?;
    let submissions = db.list_scholarly_submissions(publication_id).await?;
    let media_assets = db.list_publication_media_assets(publication_id).await?;
    let attempts = db.list_publication_attempts(publication_id).await?;
    let status_events = db.list_publication_status_events(publication_id).await?;
    let evidence =
        vox_publisher::scientia_evidence::parse_scientia_evidence(row.metadata_json.as_deref());
    let evidence_fallback = vox_publisher::scientia_evidence::ScientiaEvidenceContext::default();
    let evidence_ref = evidence.as_ref().unwrap_or(&evidence_fallback);
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
    let discovery_rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
        publication_id,
        row.source_ref.as_deref(),
        evidence_ref,
        &scientia_h,
        None,
    );
    let manifest_completion =
        vox_publisher::scientia_discovery::manifest_completion_report(&manifest);
    let evidence_complete = Some(
        vox_publisher::scientia_discovery::evidence_completeness_score(evidence_ref, &scientia_h),
    );
    let transform_preview = vox_publisher::scientia_discovery::destination_transform_previews(
        &manifest,
        evidence.as_ref(),
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "publication_id": row.publication_id,
            "content_type": row.content_type,
            "state": row.state,
            "digest": row.content_sha3_256,
            "version": row.version,
            "approvals_for_digest": approvals,
            "preflight_report": preflight_report,
            "operator_status_surface_v1": operator_status_surface_v1,
            "discovery_rank": discovery_rank,
            "manifest_completion": manifest_completion,
            "evidence_completeness_0_100": evidence_complete,
            "transform_preview": transform_preview,
            "scholarly_submissions": submissions,
            "media_assets": media_assets,
            "publication_attempts": attempts,
            "publication_status_events": status_events,
        }))?
    );
    Ok(())
}
