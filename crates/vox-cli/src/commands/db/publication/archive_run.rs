use super::*;
use anyhow::{Context, Result};

const ARCHIVE_AUTOFILL_USER_ID: &str = "local-user";

const DUAL_APPROVAL_BLOCKER: &str = "archive run requires two distinct digest-bound approvers before Zenodo / Software Heritage network I/O (run publication-approve twice with distinct approvers)";

/// Exit non-zero when the archive plan carries blockers (prints JSON separately).
fn archive_run_blocker_result(plan: &vox_publisher::archive_run::ArchiveRunPlan) -> Result<()> {
    if let Some(blocker) = plan.first_blocker() {
        anyhow::bail!("archive run blocked: {blocker}");
    }
    Ok(())
}

fn detect_repo_license(repo_root: &std::path::Path) -> Option<String> {
    for name in &["LICENSE", "LICENSE.md", "LICENSE.txt"] {
        let path = repo_root.join(name);
        if let Ok(text) = std::fs::read_to_string(&path) {
            let text = &text[..text.len().min(4096)];
            let lower = text.to_lowercase();
            if lower.contains("apache") {
                return Some("Apache-2.0".into());
            }
            if lower.contains("mit license")
                || lower.contains("permission is hereby granted, free of charge")
            {
                return Some("MIT".into());
            }
        }
    }
    None
}

fn detect_workspace_version(repo_root: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(repo_root.join("Cargo.toml")).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version = ") {
            let version = trimmed
                .trim_start_matches("version = ")
                .trim()
                .trim_matches('"');
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    None
}

fn git_remote_origin() -> Option<String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let url = vox_git::read_only(&repo_root, &["remote", "get-url", "origin"]).ok()?;
    let url = url.trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

/// Compose autofill before archive gates: compute + apply in memory; persist when `persist`.
async fn compose_autofill_for_archive(
    db: &vox_db::VoxDb,
    row: &vox_db::PublicationManifestRow,
    manifest: &mut vox_publisher::publication::PublicationManifest,
    persist: bool,
) -> Result<bool> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let repo_license = detect_repo_license(&repo_root);
    let git_remote = git_remote_origin();
    let workspace_version = detect_workspace_version(&repo_root);

    let identity_row = db
        .get_user_identity(ARCHIVE_AUTOFILL_USER_ID)
        .await
        .ok()
        .flatten();
    let identity_view = identity_row.map(|r| vox_publisher::scientia_autofill::UserIdentityView {
        user_id: r.user_id,
        orcid_id: r.orcid_id,
    });

    let plan = vox_publisher::scientia_autofill::compute_autofill(
        manifest,
        identity_view.as_ref(),
        repo_license.as_deref(),
        git_remote.as_deref(),
        workspace_version.as_deref(),
    );
    if plan.fills.is_empty() {
        return Ok(false);
    }

    let new_meta = vox_publisher::scientia_autofill::apply_autofill(
        manifest.metadata_json.as_deref(),
        &mut manifest.abstract_text,
        &plan,
    )
    .context("archive autofill apply")?;
    manifest.metadata_json = Some(new_meta);

    if persist {
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
            &manifest.publication_id,
            "scientia_autofill_applied",
            Some(
                &serde_json::json!({ "fills": plan.fills.len(), "digest": digest, "source": "archive_run" })
                    .to_string(),
            ),
        )
        .await?;
    }

    Ok(true)
}

/// Append published test-server URIs to `metadata_json.scientia.nanopub_uris`.
fn merge_nanopub_uris_into_metadata(
    metadata_json: Option<&str>,
    uris: &[String],
) -> Result<String> {
    if uris.is_empty() {
        return Ok(metadata_json
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("{}")
            .to_string());
    }
    let mut root: serde_json::Value = metadata_json
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str)
        .transpose()
        .context("parse metadata_json for nanopub merge")?
        .unwrap_or_else(|| serde_json::json!({}));
    let obj = root
        .as_object_mut()
        .context("metadata_json must be a JSON object")?;
    let scientia = obj
        .entry("scientia")
        .or_insert_with(|| serde_json::json!({}));
    let sci_obj = scientia
        .as_object_mut()
        .context("metadata_json.scientia must be an object")?;
    let mut existing: Vec<String> = sci_obj
        .get("nanopub_uris")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for uri in uris {
        if !existing.iter().any(|u| u == uri) {
            existing.push(uri.clone());
        }
    }
    sci_obj.insert(
        "nanopub_uris".into(),
        serde_json::Value::Array(
            existing
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    serde_json::to_string(&root).context("serialize metadata_json after nanopub merge")
}

/// Publish locally-built nanopubs for approved claims (test server only).
async fn publish_approved_local_nanopubs(
    db: &vox_db::VoxDb,
    publication_id: &str,
) -> Result<Vec<serde_json::Value>> {
    use vox_scientia::review_flow::{
        approval_for, nanopub_publish_test_server, publication_session_id,
    };

    let session_id = publication_session_id(publication_id);
    let claims = db
        .list_publication_claims(session_id)
        .await
        .context("list publication claims for nanopub archive step")?;
    let user_id = vox_config::paths::local_user_id();
    let mut published = Vec::new();
    for claim in claims {
        let claim_id = claim.claim_id;
        if db
            .count_scientia_nanopubs_for_claim(claim_id)
            .await
            .unwrap_or(0)
            == 0
        {
            continue;
        }
        let token = match approval_for(db, publication_id, claim_id).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let uri = nanopub_publish_test_server(db, publication_id, claim_id, &token, &user_id, None)
            .await
            .with_context(|| format!("publish nanopub for claim {claim_id}"))?;
        published.push(serde_json::json!({
            "claim_id": claim_id,
            "uri": uri,
        }));
    }
    Ok(published)
}

/// Orchestrate the archive pipeline end-to-end for one publication.
///
/// Composes existing, already-tested pieces:
///   * `scientia_discovery::manifest_completion_report` — required-field gate.
///   * `archive_run::plan_archive_run` — pure step planner / blocker source.
///   * digest-bound dual approval (`has_dual_publication_approval_for_digest`) —
///     the same gate as `publication-scholarly-pipeline-run`; archive requires
///     two distinct digest-bound approvers before any network I/O.
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
/// failure surfaces as structured JSON, never a panic. Blockers exit non-zero
/// after printing `{ "blocked": true, ... }`.
pub async fn publication_archive_run(
    publication_id: &str,
    production: bool,
    publish: bool,
    dry_run: bool,
    publish_nanopub_test_server: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let mut manifest = publication_manifest_from_row(&row);

    // 0. Compose autofill (in-memory for dry-run; persisted otherwise).
    let _autofill_applied =
        compose_autofill_for_archive(&db, &row, &mut manifest, !dry_run).await?;

    // 1. Completion report (required-field gate) after autofill compose.
    let completion = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);

    // 2. Dual approval — same digest-bound gate as scholarly submit.
    let digest = manifest.content_sha3_256();
    let dual = db
        .has_dual_publication_approval_for_digest(publication_id, &digest)
        .await?;
    let approver_count = db
        .count_publication_approvers_for_digest(publication_id, &digest)
        .await?;

    // 3. Plan.
    let include_nanopub = publish_nanopub_test_server
        && std::env::var("VOX_NANOPUB_TEST_SERVER")
            .map(|v| v == "1")
            .unwrap_or(false);
    let plan =
        vox_publisher::archive_run::plan_archive_run(&completion, dual, publish, include_nanopub);
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
                "dual_approval": dual,
                "dry_run": dry_run,
            }))?
        );
        archive_run_blocker_result(&plan)?;
    }

    if dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "dry_run": true,
                "publication_id": publication_id,
                "planned_steps": plan.step_names(),
                "completeness_0_100": completion.completeness_0_100,
                "approver_count": approver_count,
                "dual_approval": dual,
                "production": production,
                "publish": publish,
                "nanopub_test_server": include_nanopub,
                "human_gated": true,
                "note": "No network I/O in dry-run; run without --dry-run after dual publication-approve.",
            }))?
        );
        return Ok(());
    }

    if !dual {
        anyhow::bail!(DUAL_APPROVAL_BLOCKER);
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
    let origin_url: Option<String> = manifest
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

    // --- Nanopub test server (human-gated: approval token + env var) ---
    let nanopub_result = if include_nanopub {
        match publish_approved_local_nanopubs(&db, publication_id).await {
            Ok(published) if !published.is_empty() => {
                executed_steps.push("nanopub_test_server_publish");
                let uris: Vec<String> = published
                    .iter()
                    .filter_map(|v| {
                        v.get("uri")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .collect();
                if let Ok(new_meta) =
                    merge_nanopub_uris_into_metadata(manifest.metadata_json.as_deref(), &uris)
                {
                    manifest.metadata_json = Some(new_meta);
                }
                serde_json::json!({ "ok": true, "published": published })
            }
            Ok(_) => serde_json::json!({
                "skipped": true,
                "reason": "no approved local nanopubs; run publication-nanopub-build after claim review",
            }),
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        }
    } else {
        serde_json::json!({
            "skipped": true,
            "reason": "pass --publish-nanopub-test-server with VOX_NANOPUB_TEST_SERVER=1",
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
                "nanopub": nanopub_result,
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
            "nanopub": nanopub_result,
            "production": production,
            "digest": digest,
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_publisher::archive_run::plan_archive_run;
    use vox_publisher::scientia_discovery::ManifestCompletionReport;

    #[test]
    fn merge_nanopub_uris_dedups_and_appends() {
        let base = r#"{"scientia":{"nanopub_uris":["https://w3id.org/np/existing"]}}"#;
        let merged = merge_nanopub_uris_into_metadata(
            Some(base),
            &[
                "https://w3id.org/np/existing".into(),
                "https://w3id.org/np/new".into(),
            ],
        )
        .expect("merge");
        let v: serde_json::Value = serde_json::from_str(&merged).expect("parse");
        let uris = v["scientia"]["nanopub_uris"].as_array().unwrap();
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn archive_run_blocker_returns_error_not_ok() {
        let mut report = ManifestCompletionReport {
            completeness_0_100: 50,
            required_missing: vec!["publication_date".into()],
            inferred_ok: vec![],
            human_only_pending: vec![],
            field_provenance: vec![],
        };
        let plan = plan_archive_run(&report, true, false, false);
        assert!(plan.first_blocker().is_some());
        let err = archive_run_blocker_result(&plan).unwrap_err();
        assert!(err.to_string().contains("archive run blocked"), "{err}");
        assert!(err.to_string().contains("publication_date"), "{err}");

        report.required_missing.clear();
        let clean = plan_archive_run(&report, true, false, false);
        archive_run_blocker_result(&clean).expect("no blockers");
    }
}
