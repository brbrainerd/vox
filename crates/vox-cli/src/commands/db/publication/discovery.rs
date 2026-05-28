use super::*;
use anyhow::{Context, Result};

/// Rank publication manifests for SCIENTIA discovery (deterministic; no LLM).
pub async fn publication_discovery_scan(
    content_type: Option<&str>,
    state: Option<&str>,
    limit: i64,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let rows = db
        .list_publication_manifests(content_type, state, limit)
        .await?;
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
    let mut candidates: Vec<serde_json::Value> = Vec::new();
    for row in rows {
        let evidence =
            vox_publisher::scientia_evidence::parse_scientia_evidence(row.metadata_json.as_deref())
                .unwrap_or_default();
        let rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
            row.publication_id.as_str(),
            row.source_ref.as_deref(),
            &evidence,
            &scientia_h,
            None,
        );
        candidates.push(serde_json::json!({
            "publication_id": row.publication_id,
            "content_type": row.content_type,
            "state": row.state,
            "updated_at_ms": row.updated_at_ms,
            "rank": rank,
        }));
    }
    candidates.sort_by(|a, b| {
        let sa = a["rank"]["rank_score"].as_u64().unwrap_or(0);
        let sb = b["rank"]["rank_score"].as_u64().unwrap_or(0);
        sb.cmp(&sa)
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_kind": "scientia_discovery_scan",
            "candidates": candidates,
        }))?
    );
    Ok(())
}
/// Machine explanation + completion + previews for one publication id.
pub async fn publication_discovery_explain(publication_id: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = publication_manifest_from_row(&row);
    let evidence =
        vox_publisher::scientia_evidence::parse_scientia_evidence(row.metadata_json.as_deref())
            .unwrap_or_default();
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
    let novelty_bundle = vox_publisher::scientia_prior_art::parse_novelty_bundle_from_metadata_json(
        row.metadata_json.as_deref(),
    );
    let overlap_for_rank = novelty_bundle.as_ref().map(|b| {
        vox_publisher::scientia_finding_ledger::novelty_overlap_blend_01(b, &scientia_h) as f32
    });
    let mut rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
        publication_id,
        row.source_ref.as_deref(),
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
    let completion = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);
    let preview = vox_publisher::scientia_discovery::destination_transform_previews(
        &manifest,
        Some(&evidence),
    );
    let impact_readership_projection = novelty_bundle.as_ref().map(|b| {
        vox_publisher::scientia_finding_ledger::impact_readership_projection_v1(b, &scientia_h)
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "publication_id": publication_id,
            "discovery_rank": rank,
            "novelty_evidence_bundle": novelty_bundle,
            "manifest_completion": completion,
            "evidence_completeness_0_100": vox_publisher::scientia_discovery::evidence_completeness_score(&evidence, &scientia_h),
            "transform_preview": preview,
            "impact_readership_projection": impact_readership_projection,
        }))?
    );
    Ok(())
}
/// Destination transform preview JSON only (no DB writes).
pub async fn publication_transform_preview(publication_id: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    let manifest = publication_manifest_from_row(&row);
    let evidence =
        vox_publisher::scientia_evidence::parse_scientia_evidence(row.metadata_json.as_deref());
    let preview = vox_publisher::scientia_discovery::destination_transform_previews(
        &manifest,
        evidence.as_ref(),
    );
    println!("{}", serde_json::to_string_pretty(&preview)?);
    Ok(())
}
pub(super) fn merge_novelty_bundle_into_metadata_json_str(
    metadata_json: Option<&str>,
    bundle: &vox_publisher::scientia_finding_ledger::NoveltyEvidenceBundleV1,
) -> Result<String> {
    let mut root: serde_json::Value =
        if let Some(raw) = metadata_json.map(str::trim).filter(|s| !s.is_empty()) {
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
    root[vox_publisher::scientia_evidence::METADATA_KEY_SCIENTIA_NOVELTY_BUNDLE] =
        serde_json::to_value(bundle).context("novelty bundle serde")?;
    Ok(serde_json::to_string(&root)?)
}
/// Fetch OpenAlex / Crossref / Semantic Scholar prior art for a stored manifest; optional `--persist-metadata` merges `scientia_novelty_bundle` and recomputes digest.
pub async fn publication_novelty_fetch(
    publication_id: &str,
    offline: bool,
    persist_metadata: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let Some(row) = db.get_publication_manifest(publication_id).await? else {
        anyhow::bail!("publication not found: {publication_id}");
    };
    if row.content_type != "scientia" {
        anyhow::bail!(
            "publication-novelty-fetch is intended for content_type `scientia` (got `{}`)",
            row.content_type
        );
    }
    let candidate_id = vox_publisher::scientia_finding_ledger::default_candidate_id(publication_id);
    let query = vox_publisher::scientia_prior_art::PriorArtQuery {
        title: row.title.clone(),
        abstract_text: row.abstract_text.clone(),
    };
    let client = vox_http_client::client();
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
    let bundle = vox_publisher::scientia_prior_art::fetch_prior_art_federated(
        &client,
        &candidate_id,
        &query,
        vec![],
        vox_publisher::scientia_prior_art::PriorArtFetchOptions::default(),
        offline,
        &scientia_h,
    )
    .await
    .context("prior-art federated fetch")?;

    if persist_metadata {
        let mut manifest = publication_manifest_from_row(&row);
        manifest.metadata_json = Some(merge_novelty_bundle_into_metadata_json_str(
            manifest.metadata_json.as_deref(),
            &bundle,
        )?);
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
            "scientia_novelty_bundle_updated",
            Some(
                &serde_json::json!({ "bundle_id": bundle.bundle_id, "digest": digest }).to_string(),
            ),
        )
        .await?;
    }

    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}

/// Auto-publish `auto_draft_eligible` Scientia findings to the local RSS feed.
///
/// Scans publication manifests ranked by the Scientia discovery heuristics and
/// appends each `DiscoveryIntakeTier::StrongCandidate` finding as an RSS item in
/// `feed.xml`.  This is the **owned** channel (a local file), so no gate / dual
/// approval is required.  The insert is idempotent: items already present in the
/// feed (matched by GUID) are skipped silently.
///
/// # Arguments
/// - `content_type` — optional filter (e.g. `"scientia"`); default: all
/// - `feed_path_override` — override the path to `feed.xml`; default:
///   `<repo_root>/docs/src/feed.xml`
/// - `limit` — maximum candidates to scan
/// - `json` — emit a JSON summary of what was published
pub async fn publication_discovery_publish_rss(
    content_type: Option<&str>,
    feed_path_override: Option<&std::path::Path>,
    limit: i64,
    json: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let rows = db
        .list_publication_manifests(content_type, None, limit)
        .await?;
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let scientia_h =
        vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);

    // Resolve the RSS feed path: CLI override → env override → repo-root default.
    let site = if let Some(p) = feed_path_override {
        let mut s = vox_publisher::NewsSiteConfig::from_default_with_operator_env();
        s.rss_feed_path = p.to_path_buf();
        s
    } else {
        let mut s = vox_publisher::NewsSiteConfig::from_default_with_operator_env();
        // Resolve relative path against repo root so callers don't need to `cd` first.
        if s.rss_feed_path.is_relative() {
            s.rss_feed_path = repo_root.join(&s.rss_feed_path);
        }
        s
    };

    let mut published: Vec<serde_json::Value> = Vec::new();
    let mut skipped: Vec<serde_json::Value> = Vec::new();

    for row in rows {
        let evidence =
            vox_publisher::scientia_evidence::parse_scientia_evidence(row.metadata_json.as_deref())
                .unwrap_or_default();
        let rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
            row.publication_id.as_str(),
            row.source_ref.as_deref(),
            &evidence,
            &scientia_h,
            None,
        );

        if !rank.auto_draft_eligible {
            skipped.push(serde_json::json!({
                "publication_id": row.publication_id,
                "reason": "below_strong_candidate_threshold",
                "intake_tier": format!("{:?}", rank.intake_tier),
                "rank_score": rank.rank_score,
            }));
            continue;
        }

        // Build a UnifiedNewsItem from the manifest, forcing RSS syndication on.
        let item = match publication_item_from_manifest(&row) {
            Ok(mut it) => {
                it.syndication.rss = true;
                it
            }
            Err(e) => {
                tracing::warn!(
                    publication_id = row.publication_id.as_str(),
                    error = %e,
                    "Skipping: could not build UnifiedNewsItem"
                );
                skipped.push(serde_json::json!({
                    "publication_id": row.publication_id,
                    "reason": "item_build_error",
                    "error": e.to_string(),
                }));
                continue;
            }
        };

        match vox_publisher::adapters::rss::update_feed(&item, &site).await {
            Ok(()) => {
                tracing::info!(
                    publication_id = row.publication_id.as_str(),
                    "Scientia finding published to RSS feed."
                );
                published.push(serde_json::json!({
                    "publication_id": row.publication_id,
                    "title": item.title,
                    "intake_tier": format!("{:?}", rank.intake_tier),
                    "rank_score": rank.rank_score,
                }));
            }
            Err(e) => {
                tracing::error!(
                    publication_id = row.publication_id.as_str(),
                    error = %e,
                    "RSS feed update failed."
                );
                skipped.push(serde_json::json!({
                    "publication_id": row.publication_id,
                    "reason": "rss_update_error",
                    "error": e.to_string(),
                }));
            }
        }
    }

    let summary = serde_json::json!({
        "schema_kind": "scientia_discovery_publish_rss",
        "feed_path": site.rss_feed_path.display().to_string(),
        "published_count": published.len(),
        "skipped_count": skipped.len(),
        "published": published,
        "skipped": skipped,
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        let published_count = summary["published_count"].as_u64().unwrap_or(0);
        let skipped_count = summary["skipped_count"].as_u64().unwrap_or(0);
        println!(
            "Scientia → RSS: {} finding(s) published, {} skipped (feed: {})",
            published_count,
            skipped_count,
            site.rss_feed_path.display()
        );
        if let Some(items) = summary["published"].as_array() {
            for item in items {
                println!(
                    "  + {} — {}",
                    item["publication_id"].as_str().unwrap_or("?"),
                    item["title"].as_str().unwrap_or("?")
                );
            }
        }
    }

    Ok(())
}
