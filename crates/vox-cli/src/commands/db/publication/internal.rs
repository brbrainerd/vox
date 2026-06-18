use super::*;
use anyhow::{Context, Result};

pub(crate) fn publication_item_from_manifest(
    row: &vox_db::PublicationManifestRow,
) -> Result<vox_publisher::types::UnifiedNewsItem> {
    vox_publisher::switching::unified_news_item_from_manifest_parts(
        &row.publication_id,
        &row.title,
        &row.author,
        &row.body_markdown,
        row.metadata_json.as_deref(),
    )
}
pub(super) fn publication_manifest_from_row(
    row: &vox_db::PublicationManifestRow,
) -> vox_publisher::publication::PublicationManifest {
    vox_publisher::publication::PublicationManifest {
        publication_id: row.publication_id.clone(),
        content_type: row.content_type.clone(),
        source_ref: row.source_ref.clone(),
        title: row.title.clone(),
        author: row.author.clone(),
        abstract_text: row.abstract_text.clone(),
        body_markdown: row.body_markdown.clone(),
        citations_json: row.citations_json.clone(),
        metadata_json: row.metadata_json.clone(),
    }
}
pub(super) async fn publication_attention_inputs_for_row(
    db: &vox_db::VoxDb,
    row: &vox_db::PublicationManifestRow,
    item: &vox_publisher::types::UnifiedNewsItem,
) -> Result<vox_publisher::publication_preflight::PreflightAttentionInputs> {
    let dual = db
        .has_dual_publication_approval_for_digest(
            row.publication_id.as_str(),
            row.content_sha3_256.as_str(),
        )
        .await?;
    let gate = vox_publisher::gate::evaluate_publish_gate(
        vox_publisher::gate::publish_gate_inputs_for_cli(false, true, dual, item),
    );
    Ok(vox_publisher::publication_preflight::PreflightAttentionInputs { gate: Some(gate) })
}
pub(super) async fn publication_preflight_report_for_row(
    db: &vox_db::VoxDb,
    row: &vox_db::PublicationManifestRow,
    manifest: &vox_publisher::publication::PublicationManifest,
    profile: vox_publisher::publication_preflight::PreflightProfile,
    with_worthiness: bool,
) -> Result<vox_publisher::publication_preflight::PreflightReport> {
    let item = publication_item_from_manifest(row)?;
    let attention = publication_attention_inputs_for_row(db, row, &item).await?;
    if with_worthiness {
        let root = vox_repository::resolve_repo_root_for_ci();
        let manifest =
            crate::commands::scientia_worthiness_enrich::enrich_manifest_for_worthiness_preflight(
                manifest.clone(),
                db,
                &root,
                None,
            )
            .await?;
        let contract_path =
            root.join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH);
        let yaml = read_utf8_path_capped(&contract_path).with_context(|| {
            format!(
                "read worthiness contract {} (repo root discovery required)",
                contract_path.display()
            )
        })?;
        let contract = vox_publisher::publication_worthiness::load_contract_from_str(&yaml)?;
        vox_publisher::publication_worthiness::validate_contract_invariants(&contract)?;
        let scientia_h =
            vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&root);
        let status_events = db
            .list_publication_status_events(row.publication_id.as_str())
            .await?;
        let status_snapshots: Vec<
            vox_publisher::publication_preflight::PublicationStatusEventSnapshot,
        > = status_events
            .into_iter()
            .map(
                |ev| vox_publisher::publication_preflight::PublicationStatusEventSnapshot {
                    status: ev.status,
                    detail_json: ev.detail_json,
                },
            )
            .collect();
        Ok(
            vox_publisher::publication_preflight::run_preflight_with_worthiness_attention_heuristics(
                &manifest,
                profile,
                &contract,
                Some(attention),
                &scientia_h,
                Some(&status_snapshots),
            ),
        )
    } else {
        Ok(
            vox_publisher::publication_preflight::run_preflight_with_attention(
                manifest,
                profile,
                Some(attention),
            ),
        )
    }
}
pub(super) fn cli_social_worthiness_enforce() -> bool {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialWorthinessEnforce)
        .expose()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
pub(super) fn cli_social_worthiness_score_min() -> f64 {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialWorthinessScoreMin)
        .expose()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.85)
}
pub(super) fn publisher_config_from_env(
    dry_run: bool,
    worthiness_score: Option<f64>,
) -> vox_publisher::PublisherConfig {
    let mut cfg = vox_publisher::PublisherConfig::from_operator_environment(
        dry_run,
        Some(vox_repository::resolve_repo_root_for_ci()),
        vox_publisher::NewsSiteConfig::from_default_with_operator_env(),
    );
    cfg.worthiness_score = worthiness_score;
    cfg
}
