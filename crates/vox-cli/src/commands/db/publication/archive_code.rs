use super::*;
use anyhow::{Context, Result};
use std::time::Duration;

const POLL_MAX_WAIT: Duration = Duration::from_secs(5 * 60);
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Archive the publication's code repository via Software Heritage Save Code Now.
///
/// If `origin_url` is `None` it is read from
/// `metadata_json.scientia.reproducibility.code_repository_url`.
/// On success the manifest's `metadata_json` is updated with `scientia.swh_save`
/// (and `scientia.swhid` when a snapshot SWHID is returned) and persisted via
/// digest recompute + upsert.
pub async fn publication_archive_code(
    publication_id: &str,
    origin_url: Option<&str>,
    wait: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };

    // Resolve origin URL
    let resolved_url: String = if let Some(u) = origin_url.filter(|s| !s.trim().is_empty()) {
        u.to_string()
    } else {
        // Try metadata_json.scientia.reproducibility.code_repository_url
        let meta: Option<serde_json::Value> = row
            .metadata_json
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| serde_json::from_str(s).ok());
        let from_meta = meta
            .as_ref()
            .and_then(|v| v.get("scientia"))
            .and_then(|s| s.get("reproducibility"))
            .and_then(|r| r.get("code_repository_url"))
            .and_then(|u| u.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);
        from_meta.ok_or_else(|| {
            anyhow::anyhow!(
                "no origin_url supplied and \
                 metadata_json.scientia.reproducibility.code_repository_url is absent; \
                 run `vox db publication-autofill --publication-id {publication_id} --apply` first"
            )
        })?
    };

    // Build HTTP client
    let client = vox_http_client::client_builder()
        .user_agent("vox-publisher/scientia-archive")
        .build()
        .context("build http client for Software Heritage")?;

    // Resolve optional bearer token (VOX_SWHID_API_TOKEN)
    let token: Option<String> =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSwhidApiToken)
            .expose()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

    // POST save request
    let initial_status = vox_publisher::scholarly::software_heritage::request_save(
        &client,
        &resolved_url,
        token.as_deref(),
    )
    .await
    .context("Software Heritage request_save")?;

    let final_status = if wait {
        vox_publisher::scholarly::software_heritage::poll_save_status(
            &client,
            &resolved_url,
            token.as_deref(),
            POLL_MAX_WAIT,
            POLL_INTERVAL,
        )
        .await
        .context("Software Heritage poll_save_status")?
    } else {
        initial_status
    };

    // Merge into manifest metadata_json
    let mut manifest = publication_manifest_from_row(&row);
    let new_meta = vox_publisher::scholarly::software_heritage::merge_swh_into_metadata_json(
        manifest.metadata_json.as_deref(),
        &final_status,
        &resolved_url,
    )
    .context("merge SWH status into metadata_json")?;
    manifest.metadata_json = Some(new_meta);

    let digest = manifest.content_sha3_256();
    db.upsert_publication_manifest(vox_db::PublicationManifestParams {
        publication_id: &manifest.publication_id,
        content_type: &manifest.content_type,
        source_ref: manifest.source_ref.as_deref(),
        title: &manifest.title,
        author: &manifest.author,
        abstract_text: manifest.abstract_text.as_deref(),
        body_markdown: &manifest.body_markdown,
        citations_json: manifest.citations_json.as_deref(),
        metadata_json: manifest.metadata_json.as_deref(),
        revision_history_json: row.revision_history_json.as_deref(),
        content_sha3_256: &digest,
        state: row.state.as_str(),
    })
    .await?;

    db.append_publication_status_event(
        publication_id,
        "scientia_swh_save_requested",
        Some(
            &serde_json::json!({
                "origin_url": resolved_url,
                "request_status": final_status.request_status,
                "task_status": final_status.task_status,
                "snapshot_swhid": final_status.snapshot_swhid,
                "digest": digest,
            })
            .to_string(),
        ),
    )
    .await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "publication_id": publication_id,
            "origin_url": resolved_url,
            "status": {
                "request_status": final_status.request_status,
                "task_status": final_status.task_status,
            },
            "swhid": final_status.snapshot_swhid,
            "digest": digest,
        }))?
    );
    Ok(())
}
