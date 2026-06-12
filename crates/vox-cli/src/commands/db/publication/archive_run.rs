use super::*;
use anyhow::{Context, Result};

/// Orchestrate the archive pipeline end-to-end for one publication.
///
/// Composes existing, already-tested pieces:
///   * `scientia_discovery::manifest_completion_report` — required-field gate.
///   * `archive_run::plan_archive_run` — pure step planner / blocker source.
///   * digest-bound approval count (`count_publication_approvers_for_digest`) —
///     the same approval source `publication-approve` writes; archive requires
///     at least one recorded approver.
///   * `scholarly::submit_with_adapter(manifest, "zenodo")` — the live Zenodo
///     adapter. NOTE: that adapter is **monolithic**: a single `submit` performs
///     deposit-draft + staging upload + (optional) publish internally, driven by
///     `VOX_ZENODO_*` env flags. So the planner's granular steps are a *preview*;
///     execution makes one Zenodo call and maps its receipt onto the coarse
///     `zenodo_*` steps. `--production`/`--publish` are surfaced into those env
///     flags before the call.
///   * `software_heritage::request_save` + `merge_swh_into_metadata_json` — code
///     archive, only when an origin URL is present in metadata; otherwise skipped
///     with a note.
///
/// Sandbox is the default; `--production` flips to production Zenodo. A step
/// failure surfaces as structured JSON, never a panic. Returns `Ok` on a clean
/// blocker (prints `{ "blocked": true, ... }`).
pub async fn publication_archive_run(
    publication_id: &str,
    production: bool,
    publish: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = publication_manifest_from_row(&row);

    // 1. Completion report (required-field gate).
    let completion = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);

    // 2. Approval state — same source `publication-approve` writes.
    let approver_count = db
        .count_publication_approvers_for_digest(publication_id, &row.content_sha3_256)
        .await?;
    let approved = approver_count >= 1;

    // 3. Plan.
    let plan = vox_publisher::archive_run::plan_archive_run(&completion, approved, publish);
    if let Some(blocker) = plan.first_blocker() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "blocked": true,
                "publication_id": publication_id,
                "blocker": blocker,
                "all_blockers": plan.blockers,
                "completeness_0_100": completion.completeness_0_100,
                "approver_count": approver_count,
            }))?
        );
        return Ok(());
    }

    // 4. Execute. The monolithic Zenodo adapter reads its sandbox/publish toggles
    //    from process env only, so surface `--production`/`--publish` there, then
    //    call it once.
    // SAFETY: `set_var` is `unsafe` under Rust 2024 because a concurrent reader on
    //    another thread is UB. This is a one-shot CLI command: these two vars are
    //    written here and read only by the Zenodo adapter invoked on the next line
    //    of THIS task, before any other scientia work runs in-process. No other
    //    code path reads VOX_ZENODO_SANDBOX / VOX_ZENODO_PUBLISH_DEPOSITION
    //    concurrently during a `publication-archive-run` invocation.
    #[allow(unsafe_code)]
    unsafe {
        if production {
            std::env::remove_var("VOX_ZENODO_SANDBOX");
        } else {
            std::env::set_var("VOX_ZENODO_SANDBOX", "1");
        }
        if publish {
            std::env::set_var("VOX_ZENODO_PUBLISH_DEPOSITION", "1");
        } else {
            std::env::remove_var("VOX_ZENODO_PUBLISH_DEPOSITION");
        }
    }

    let mut executed_steps: Vec<&'static str> = Vec::new();
    let mut manifest = manifest;

    // --- Zenodo (monolithic: draft + upload + optional publish) ---
    let zenodo_result =
        match vox_publisher::scholarly::submit_with_adapter(&manifest, "zenodo").await {
            Ok(receipt) => {
                executed_steps.push("zenodo_deposit_draft");
                executed_steps.push("zenodo_upload_staging");
                if publish {
                    executed_steps.push("zenodo_publish");
                }
                serde_json::json!({
                    "ok": true,
                    "adapter": receipt.adapter,
                    "external_submission_id": receipt.external_submission_id,
                    "status": receipt.status,
                    "response_fingerprint": receipt.response_fingerprint,
                    "metadata_json": receipt.metadata_json,
                })
            }
            Err(e) => serde_json::json!({
                "ok": false,
                "error": e.to_string(),
            }),
        };

    // --- Software Heritage (only when an origin URL is present) ---
    let origin_url: Option<String> = row
        .metadata_json
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .as_ref()
        .and_then(|v| v.get("scientia"))
        .and_then(|s| s.get("reproducibility"))
        .and_then(|r| r.get("code_repository_url"))
        .and_then(|u| u.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);

    let swh_result = if let Some(url) = origin_url {
        let token: Option<String> =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSwhidApiToken)
                .expose()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        let client = vox_http_client::client();
        match vox_publisher::scholarly::software_heritage::request_save(
            &client,
            &url,
            token.as_deref(),
        )
        .await
        {
            Ok(status) => {
                executed_steps.push("software_heritage_save");
                // Merge SWHID into the manifest metadata so a later receipt carries it.
                if let Ok(new_meta) =
                    vox_publisher::scholarly::software_heritage::merge_swh_into_metadata_json(
                        manifest.metadata_json.as_deref(),
                        &status,
                        &url,
                    )
                {
                    manifest.metadata_json = Some(new_meta);
                }
                serde_json::json!({
                    "ok": true,
                    "origin_url": url,
                    "request_status": status.request_status,
                    "task_status": status.task_status,
                    "snapshot_swhid": status.snapshot_swhid,
                })
            }
            Err(e) => serde_json::json!({
                "ok": false,
                "origin_url": url,
                "error": e.to_string(),
            }),
        }
    } else {
        serde_json::json!({
            "skipped": true,
            "reason": "no metadata_json.scientia.reproducibility.code_repository_url; run publication-autofill / publication-archive-code first",
        })
    };

    // --- Record receipt: persist any SWH metadata merge + append an audit event. ---
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
    .await
    .context("persist manifest after archive run")?;
    executed_steps.push("record_receipt");

    db.append_publication_status_event(
        publication_id,
        "scientia_archive_run",
        Some(
            &serde_json::json!({
                "production": production,
                "publish": publish,
                "executed_steps": executed_steps,
                "zenodo_ok": zenodo_result.get("ok").and_then(serde_json::Value::as_bool),
                "swh": swh_result,
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
            "planned_steps": plan.step_names(),
            "executed_steps": executed_steps,
            "zenodo": zenodo_result,
            "swh": swh_result,
            "production": production,
            "digest": digest,
        }))?
    );
    Ok(())
}
