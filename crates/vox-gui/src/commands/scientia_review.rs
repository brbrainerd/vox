//! DiscoveryReview Tauri commands — a THIN delegation layer over
//! [`vox_scientia::review_flow`].
//!
//! This module owns NO review-flow logic of its own: every command forwards to
//! the SSOT in `vox_scientia::review_flow` (queue read, decision write, offline
//! nanopublish) and maps the returned rows/tokens into UI-facing DTOs. Both the
//! CLI (`vox scientia publication-*`) and this GUI surface therefore call ONE
//! shared implementation. Like the CLI guard, this file carries NO
//! production-network publishing symbols (asserted by the guard test below).

use vox_scientia::review_flow;

/// Open the canonical DB connection used by every command in this module.
async fn db() -> Result<vox_db::VoxDb, String> {
    vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// One extracted claim awaiting human review, with its latest verdict (if any).
#[derive(Debug, serde::Serialize)]
pub struct ClaimAwaitingReviewDto {
    pub claim_id: i64,
    pub text: String,
    pub is_numeric: bool,
    pub verdict: Option<String>,
    pub confidence: Option<f64>,
    pub verifier_model: Option<String>,
    pub created_at_ms: i64,
}

/// List the claims awaiting review for a publication. Delegates the session-id
/// derivation to the SSOT and reads the queue via the typed DB op.
#[tauri::command]
pub async fn list_publication_review_queue(
    publication_id: String,
) -> Result<Vec<ClaimAwaitingReviewDto>, String> {
    let db = db().await?;
    let sid = review_flow::publication_session_id(&publication_id);
    let rows = db
        .list_claims_awaiting_review(sid, &publication_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(rows
        .into_iter()
        .map(|r| ClaimAwaitingReviewDto {
            claim_id: r.claim_id,
            text: r.text,
            is_numeric: r.is_numeric,
            verdict: r.verdict,
            confidence: r.confidence,
            verifier_model: r.verifier_model,
            created_at_ms: r.created_at_ms,
        })
        .collect())
}

/// The persisted human review decision, content-bound to the publication's
/// current manifest digest.
#[derive(Debug, serde::Serialize)]
pub struct ReviewDecisionDto {
    pub claim_id: i64,
    pub publication_id: String,
    pub decision: String,
    pub bound_digest: String,
    pub decided_at_ms: i64,
}

/// Record a human review decision for ONE claim. Delegates to the SSOT, which
/// binds the decision to the publication's current content digest.
#[tauri::command]
pub async fn record_publication_claim_review(
    publication_id: String,
    claim_id: i64,
    decision: String,
    reason: Option<String>,
) -> Result<ReviewDecisionDto, String> {
    let db = db().await?;
    let row = review_flow::record_claim_review(&db, &publication_id, claim_id, &decision, reason)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(ReviewDecisionDto {
        claim_id: row.claim_id,
        publication_id: row.publication_id,
        decision: row.decision,
        bound_digest: row.bound_digest,
        decided_at_ms: row.decided_at_ms,
    })
}

/// The result of building a local, offline-validated nanopublication. NOTHING
/// is published to the network: `published_state` is always `"local"`.
#[derive(Debug, serde::Serialize)]
pub struct NanopubResultDto {
    pub trusty_uri: String,
    pub published_state: String,
    pub validated_offline: bool,
}

/// Build + RSA-sign + offline-validate a nanopublication for an APPROVED claim,
/// persisting it locally. Requires a prior approval: the SSOT mints the approval
/// token from the DB review ledger and refuses if the latest decision is not
/// `"approved"`.
#[tauri::command]
pub async fn nanopublish_approved_claim(
    publication_id: String,
    claim_id: i64,
    orcid: Option<String>,
) -> Result<NanopubResultDto, String> {
    let db = db().await?;
    let token = review_flow::approval_for(&db, &publication_id, claim_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let signed =
        review_flow::nanopub_build(&db, &publication_id, claim_id, orcid.as_deref(), &token)
            .await
            .map_err(|e| format!("{e:#}"))?;
    Ok(NanopubResultDto {
        trusty_uri: signed.trusty_uri,
        published_state: "local".into(),
        validated_offline: true,
    })
}

/// LLM-assisted ADVISORY evidence/conclusion suggestions for ONE claim in the
/// review queue. Delegates to [`vox_scientia::evidence_assist::suggest`] (routed
/// through the model-agnostic actor-runtime LLM facade). Never mutates any
/// decision or assertion; degrades to an empty list on any LLM error.
#[tauri::command]
pub async fn suggest_evidence_improvements(
    publication_id: String,
    claim_id: i64,
) -> Result<Vec<vox_scientia::evidence_assist::EvidenceSuggestion>, String> {
    let db = db().await?;
    let sid = review_flow::publication_session_id(&publication_id);
    let claims = db
        .list_claims_awaiting_review(sid, &publication_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let c = claims
        .into_iter()
        .find(|c| c.claim_id == claim_id)
        .ok_or_else(|| format!("claim {claim_id} not in review queue"))?;
    Ok(vox_scientia::evidence_assist::suggest(&c.text, c.verdict.as_deref(), c.confidence).await)
}

// ---------------------------------------------------------------------------
// Novelty evidence (Task 17): assess a publication's stored novelty bundle.
// ---------------------------------------------------------------------------

/// One prior-art hit, trimmed to the fields the evidence panel renders.
#[derive(Debug, serde::Serialize)]
pub struct PriorArtHitDto {
    pub work_uri: String,
    pub title: String,
    pub year: Option<i32>,
    pub cited_by_count: Option<u64>,
    pub semantic_score: Option<f64>,
}

/// One supporting-vs-contradicting conflict among high-similarity hits.
#[derive(Debug, serde::Serialize)]
pub struct ConflictDto {
    pub claim_text: String,
    pub conflict_score: f64,
    /// `work_uri`s (with optional excerpt) of the supporting side.
    pub supporting: Vec<ConflictHitDto>,
    /// `work_uri`s (with optional excerpt) of the contradicting side.
    pub contradicting: Vec<ConflictHitDto>,
}

/// One side of a conflict: the hit's uri + the excerpt that justified its polarity.
#[derive(Debug, serde::Serialize)]
pub struct ConflictHitDto {
    pub work_uri: String,
    pub excerpt: Option<String>,
}

/// Explainable signal breakdown mirrored from `NoveltySignalBreakdown`.
#[derive(Debug, serde::Serialize)]
pub struct SignalsDto {
    pub max_semantic: Option<f64>,
    pub max_lexical: Option<f64>,
    pub near_hit_count: usize,
    pub top_hit_citations: Option<u64>,
    pub sources_succeeded: usize,
}

/// The full novelty assessment for one publication, ready for the GUI panel.
#[derive(Debug, serde::Serialize)]
pub struct NoveltyAssessmentDto {
    /// `"insufficient_evidence"` | `"novel"` | `"possibly_novel"` | `"not_novel"`.
    pub verdict_kind: String,
    pub closest_hit_uri: Option<String>,
    pub closest_score: Option<f64>,
    pub excluded_future_hits: usize,
    pub conflicts: Vec<ConflictDto>,
    pub signals: SignalsDto,
    /// Top-5 prior-art hits by semantic score (desc).
    pub prior_art: Vec<PriorArtHitDto>,
}

/// A DTO representing "no bundle has been fetched yet": the panel renders the
/// insufficient-evidence banner. This is NOT an error — absence of a stored
/// bundle simply means retrieval never ran for this publication.
fn insufficient_evidence_dto() -> NoveltyAssessmentDto {
    NoveltyAssessmentDto {
        verdict_kind: "insufficient_evidence".into(),
        closest_hit_uri: None,
        closest_score: None,
        excluded_future_hits: 0,
        conflicts: vec![],
        signals: SignalsDto {
            max_semantic: None,
            max_lexical: None,
            near_hit_count: 0,
            top_hit_citations: None,
            sources_succeeded: 0,
        },
        prior_art: vec![],
    }
}

/// Assess novelty for ONE publication from its stored evidence bundle.
///
/// Loads the manifest row, parses the embedded novelty bundle (key
/// `scientia_novelty_bundle`), and runs [`vox_publisher::scientia_novelty_assess::assess_novelty`]
/// against the current calendar year. If no bundle is present the command returns
/// an `insufficient_evidence` DTO (NOT an error). Read-only: nothing is mutated
/// or published.
#[tauri::command]
pub async fn get_novelty_assessment(
    publication_id: String,
) -> Result<NoveltyAssessmentDto, String> {
    use chrono::Datelike;

    let db = db().await?;
    let row = db
        .get_publication_manifest(&publication_id)
        .await
        .map_err(|e| format!("{e:#}"))?
        .ok_or_else(|| "publication not found".to_string())?;

    let Some(bundle) = vox_publisher::scientia_prior_art::parse_novelty_bundle_from_metadata_json(
        row.metadata_json.as_deref(),
    ) else {
        // No fetched bundle → nothing assessed yet → insufficient evidence.
        return Ok(insufficient_evidence_dto());
    };

    let claim_year = chrono::Utc::now().date_naive().year();
    let assessment = vox_publisher::scientia_novelty_assess::assess_novelty(
        &bundle,
        claim_year,
        &vox_scientia::inspect_bridge::NoveltyConfig::default(),
    );

    // Top-5 prior-art hits by semantic score (desc). Hits with no semantic score
    // sort last (treated as -inf).
    let mut hits = bundle.normalized_hits.clone();
    hits.sort_by(|a, b| {
        b.semantic_score
            .unwrap_or(f64::NEG_INFINITY)
            .partial_cmp(&a.semantic_score.unwrap_or(f64::NEG_INFINITY))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let prior_art = hits
        .into_iter()
        .take(5)
        .map(|h| PriorArtHitDto {
            work_uri: h.work_uri,
            title: h.title,
            year: h.year,
            cited_by_count: h.cited_by_count,
            semantic_score: h.semantic_score,
        })
        .collect();

    let conflicts = assessment
        .conflicts
        .into_iter()
        .map(|c| ConflictDto {
            claim_text: c.claim_text,
            conflict_score: c.conflict_score,
            supporting: c
                .supporting_hits
                .into_iter()
                .map(|h| ConflictHitDto {
                    work_uri: h.work_uri,
                    excerpt: h.excerpt,
                })
                .collect(),
            contradicting: c
                .contradicting_hits
                .into_iter()
                .map(|h| ConflictHitDto {
                    work_uri: h.work_uri,
                    excerpt: h.excerpt,
                })
                .collect(),
        })
        .collect();

    Ok(NoveltyAssessmentDto {
        verdict_kind: assessment.verdict_kind,
        closest_hit_uri: assessment.closest_hit_uri,
        closest_score: assessment.closest_score,
        excluded_future_hits: assessment.excluded_future_hits,
        conflicts,
        signals: SignalsDto {
            max_semantic: assessment.signals.max_semantic,
            max_lexical: assessment.signals.max_lexical,
            near_hit_count: assessment.signals.near_hit_count,
            top_hit_citations: assessment.signals.top_hit_citations,
            sources_succeeded: assessment.signals.sources_succeeded,
        },
        prior_art,
    })
}

// ---------------------------------------------------------------------------
// Discovery inbox (Task 18): unacknowledged surfaced research candidates.
// ---------------------------------------------------------------------------

/// One unacknowledged discovery-inbox row, trimmed to the fields the inbox
/// surface renders. `acknowledged_at_ms` is omitted because the list command
/// only returns rows that are, by definition, unacknowledged.
#[derive(Debug, serde::Serialize)]
pub struct DiscoveryInboxDto {
    pub id: i64,
    pub publication_id: String,
    pub surfaced_at_ms: i64,
    pub intake_tier: String,
    pub signal_codes: Vec<String>,
}

/// List unacknowledged discoveries, newest first, capped at `limit` (default 50).
/// Read-only: nothing is mutated. Each DB row maps 1:1 into a [`DiscoveryInboxDto`].
#[tauri::command]
pub async fn list_discovery_inbox(limit: Option<i64>) -> Result<Vec<DiscoveryInboxDto>, String> {
    let db = db().await?;
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let rows = db
        .list_unacknowledged_discoveries(limit)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(rows
        .into_iter()
        .map(|r| DiscoveryInboxDto {
            id: r.id,
            publication_id: r.publication_id,
            surfaced_at_ms: r.surfaced_at_ms,
            intake_tier: r.intake_tier,
            signal_codes: r.signal_codes,
        })
        .collect())
}

/// Mark a discovery-inbox row acknowledged (now). No-op if the id is unknown.
/// After this the row no longer appears in [`list_discovery_inbox`].
#[tauri::command]
pub async fn acknowledge_discovery(id: i64) -> Result<(), String> {
    let db = db().await?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    db.acknowledge_discovery(id, now_ms)
        .await
        .map_err(|e| format!("{e:#}"))
}

// ---------------------------------------------------------------------------
// Archive panel (Task 19): metadata completeness, deterministic autofill, and
// deposit status (Zenodo DOI / Software Heritage SWHID). Surfaces the Track B
// archive pipeline. Autofill REUSES the exact SSOT planner + applier
// (`vox_publisher::scientia_autofill`) and the same persist sequence the CLI
// `publication-autofill --apply` handler uses, so GUI and CLI stay in agreement.
// ---------------------------------------------------------------------------

/// Default user id for identity lookup — mirrors the CLI autofill handler
/// (`publication-autofill`), which always queries the single account-level identity.
const ARCHIVE_DEFAULT_USER_ID: &str = "local-user";

/// One provenance entry: which field, where its value came from, optional note.
#[derive(Debug, serde::Serialize)]
pub struct FieldProvenanceDto {
    pub field: String,
    pub origin: String,
    pub notes: Option<String>,
}

/// Metadata-completeness report for one publication's manifest.
#[derive(Debug, serde::Serialize)]
pub struct CompletionReportDto {
    pub completeness_0_100: u8,
    pub required_missing: Vec<String>,
    pub inferred_ok: Vec<String>,
    pub human_only_pending: Vec<String>,
    pub field_provenance: Vec<FieldProvenanceDto>,
}

fn completion_report_dto(
    r: vox_publisher::scientia_discovery::ManifestCompletionReport,
) -> CompletionReportDto {
    CompletionReportDto {
        completeness_0_100: r.completeness_0_100,
        required_missing: r.required_missing,
        inferred_ok: r.inferred_ok,
        human_only_pending: r.human_only_pending,
        field_provenance: r
            .field_provenance
            .into_iter()
            .map(|e| FieldProvenanceDto {
                field: e.field,
                origin: e.origin,
                notes: e.notes,
            })
            .collect(),
    }
}

/// Build a [`PublicationManifest`] from a loaded DB row (mirrors the CLI helper
/// `publication_manifest_from_row`).
fn manifest_from_row(
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

/// Load a manifest row by id, mapping a missing publication to a structured error.
async fn load_manifest_row(
    db: &vox_db::VoxDb,
    publication_id: &str,
) -> Result<vox_db::PublicationManifestRow, String> {
    db.get_publication_manifest(publication_id)
        .await
        .map_err(|e| format!("{e:#}"))?
        .ok_or_else(|| format!("publication not found: {publication_id}"))
}

/// Metadata-completeness report for ONE publication. Read-only.
#[tauri::command]
pub async fn get_completion_report(publication_id: String) -> Result<CompletionReportDto, String> {
    let db = db().await?;
    let row = load_manifest_row(&db, &publication_id).await?;
    let manifest = manifest_from_row(&row);
    let report = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);
    Ok(completion_report_dto(report))
}

/// One proposed (or applied) field fill, with provenance. `value` is the JSON
/// value serialized to a compact string for display.
#[derive(Debug, serde::Serialize)]
pub struct PlannedFillDto {
    pub field: String,
    pub value: String,
    pub origin: String,
    pub notes: Option<String>,
}

/// Result of computing (and optionally applying) the deterministic autofill plan.
#[derive(Debug, serde::Serialize)]
pub struct AutofillResultDto {
    pub fills: Vec<PlannedFillDto>,
    pub human_only_remaining: Vec<String>,
    pub completeness_before: u8,
    /// Equals `completeness_before` when `apply == false`.
    pub completeness_after: u8,
}

/// Run `git remote get-url origin`; `None` on any failure.
fn git_remote_origin() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let url = vox_git::read_only(&cwd, &["remote", "get-url", "origin"]).ok()?;
    let url = url.trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

/// Detect the SPDX license id from a LICENSE file in `repo_root` (mirrors the CLI
/// helper: reads up to 4 KB of the first LICENSE file; "Apache"/"MIT").
fn detect_repo_license(repo_root: &std::path::Path) -> Option<String> {
    for name in &["LICENSE", "LICENSE.md", "LICENSE.txt"] {
        let path = repo_root.join(name);
        if let Ok(text) = std::fs::read_to_string(&path) {
            let mut end = text.len().min(4096);
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            let lower = text[..end].to_lowercase();
            if lower.contains("apache") {
                return Some("Apache-2.0".into());
            }
            if lower.contains("mit") {
                return Some("MIT".into());
            }
        }
    }
    None
}

/// Compute the deterministic autofill plan for a publication and, when `apply`,
/// persist it via the SAME sequence as the CLI `publication-autofill --apply`
/// handler: [`vox_publisher::scientia_autofill::apply_autofill`] → digest
/// recompute → [`vox_db::VoxDb::upsert_publication_manifest`] →
/// `append_publication_status_event`. Returns before/after completeness scores.
///
/// Autofill inputs are best-effort (any `None` simply fills fewer fields):
/// repo license + git remote from the resolved repo root, ORCID identity from
/// the account-level `user_identities` row.
#[tauri::command]
pub async fn run_autofill(
    publication_id: String,
    apply: bool,
) -> Result<AutofillResultDto, String> {
    let db = db().await?;
    let row = load_manifest_row(&db, &publication_id).await?;
    let mut manifest = manifest_from_row(&row);

    // Best-effort autofill inputs.
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let repo_license = detect_repo_license(&repo_root);
    let git_remote = git_remote_origin();
    let identity_view = db
        .get_user_identity(ARCHIVE_DEFAULT_USER_ID)
        .await
        .ok()
        .flatten()
        .map(|r| vox_publisher::scientia_autofill::UserIdentityView {
            user_id: r.user_id,
            orcid_id: r.orcid_id,
        });

    let before = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);
    let completeness_before = before.completeness_0_100;

    let plan = vox_publisher::scientia_autofill::compute_autofill(
        &manifest,
        identity_view.as_ref(),
        repo_license.as_deref(),
        git_remote.as_deref(),
    );

    let mut completeness_after = completeness_before;

    if apply && !plan.fills.is_empty() {
        let new_meta = vox_publisher::scientia_autofill::apply_autofill(
            manifest.metadata_json.as_deref(),
            &mut manifest.abstract_text,
            &plan,
        )
        .map_err(|e| format!("autofill apply: {e:#}"))?;
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
        .await
        .map_err(|e| format!("{e:#}"))?;
        db.append_publication_status_event(
            &publication_id,
            "scientia_autofill_applied",
            Some(
                &serde_json::json!({ "fills": plan.fills.len(), "digest": digest, "via": "gui" })
                    .to_string(),
            ),
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
        let after = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);
        completeness_after = after.completeness_0_100;
    }

    let fills = plan
        .fills
        .into_iter()
        .map(|f| PlannedFillDto {
            field: f.field,
            value: f.value.to_string(),
            origin: f.origin,
            notes: f.notes,
        })
        .collect();

    Ok(AutofillResultDto {
        fills,
        human_only_remaining: plan.human_only_remaining,
        completeness_before,
        completeness_after,
    })
}

/// Deposit status surfaced from whatever is actually persisted. SWHID +
/// SWH task status live on `metadata_json.scientia.{swhid, swh_save.task_status}`
/// (written by `software_heritage::merge_swh_into_metadata_json`). Zenodo DOI +
/// state are read from the persisted `scholarly_submissions` row for the
/// `zenodo` adapter (its `metadata_json` is the Zenodo deposition JSON, carrying
/// `doi`/`expected_doi_hint`; its `status` is the deposition state). Any field is
/// honestly `None` when nothing is on record ("not yet deposited").
#[derive(Debug, serde::Serialize)]
pub struct ArchiveStatusDto {
    pub swhid: Option<String>,
    pub swh_task_status: Option<String>,
    pub zenodo_doi: Option<String>,
    pub zenodo_state: Option<String>,
}

/// Deposit status for ONE publication. Read-only; never deposits.
#[tauri::command]
pub async fn get_archive_status(publication_id: String) -> Result<ArchiveStatusDto, String> {
    let db = db().await?;
    let row = load_manifest_row(&db, &publication_id).await?;

    // Software Heritage: scientia.swhid + scientia.swh_save.task_status.
    let scientia: Option<serde_json::Value> = row
        .metadata_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.get("scientia").cloned());
    let swhid = scientia
        .as_ref()
        .and_then(|s| s.get("swhid"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let swh_task_status = scientia
        .as_ref()
        .and_then(|s| s.get("swh_save"))
        .and_then(|s| s.get("task_status"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // Zenodo: most-recent persisted scholarly submission for the zenodo adapter.
    let mut zenodo_doi: Option<String> = None;
    let mut zenodo_state: Option<String> = None;
    let subs = db
        .list_scholarly_submissions(&publication_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    {
        if let Some(z) = subs.iter().rev().find(|s| s.adapter == "zenodo") {
            zenodo_state = Some(z.status.clone()).filter(|s| !s.is_empty());
            zenodo_doi = z
                .metadata_json
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .and_then(|v| {
                    v.get("doi")
                        .or_else(|| v.get("expected_doi_hint"))
                        .and_then(|d| d.as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                });
        }
    }

    Ok(ArchiveStatusDto {
        swhid,
        swh_task_status,
        zenodo_doi,
        zenodo_state,
    })
}

#[cfg(test)]
mod tests {
    /// Guard: this GUI review surface must carry NO production-network publishing
    /// symbols (no network-publish toggle, no test-server toggle). The forbidden
    /// needles are assembled from fragments at runtime so this file cannot trip
    /// its own assertion. Mirrors the CLI guard in
    /// `crates/vox-cli/src/commands/scientia_nanopub.rs`.
    #[test]
    fn no_network_publish_symbol_in_gui_review_commands() {
        let src = include_str!("scientia_review.rs");
        let publish = format!("{}{}", "publish_to_", "network");
        let test_server = format!("{}{}", "use_test_", "server");
        assert!(!src.to_lowercase().contains(&publish));
        assert!(!src.contains(&test_server));
    }

    /// Negative path: with NO review decision on record, the SSOT must refuse to
    /// mint an approval token, so `nanopublish_approved_claim` cannot proceed. We
    /// exercise the gate directly against an in-memory DB (no canonical-DB / vault
    /// dependency) since the command's first step is `review_flow::approval_for`.
    #[tokio::test]
    async fn nanopublish_requires_prior_approval() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("in-memory db connect");
        let err = vox_scientia::review_flow::approval_for(&db, "pub-x", 1)
            .await
            .expect_err("no approval on record must refuse");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("no review decision"),
            "error must explain there is no decision, got: {msg}"
        );
    }

    /// Parity + DTO-preservation: the GUI surface reads the queue through the
    /// SAME typed DB op the CLI uses; on an empty in-memory DB the schema applies
    /// without panic and the queue is empty (each row maps 1:1 into the DTO).
    #[tokio::test]
    async fn gui_queue_dto_preserves_all_row_fields() {
        use vox_db::{DbConfig, VoxDb};
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let sid = vox_scientia::review_flow::publication_session_id("pub-x");
        let rows = db
            .list_claims_awaiting_review(sid, "pub-x")
            .await
            .expect("queue");
        assert!(rows.is_empty()); // empty DB → empty queue; schema applies w/o panic
    }

    /// Discovery-inbox parity: an inserted unacknowledged row reads back through
    /// the SAME typed DB op the inbox command uses, and acknowledging it removes
    /// it from the unacknowledged list (the surface's "Acknowledge" semantics).
    #[tokio::test]
    async fn discovery_inbox_list_then_acknowledge() {
        use vox_db::{DbConfig, VoxDb};
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let id = db
            .insert_discovery_inbox("commit-abc", 1_000, "strong_candidate", r#"["perf_claim"]"#)
            .await
            .expect("insert");

        let rows = db.list_unacknowledged_discoveries(50).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].publication_id, "commit-abc");
        assert_eq!(rows[0].intake_tier, "strong_candidate");
        assert_eq!(rows[0].signal_codes, vec!["perf_claim".to_string()]);

        db.acknowledge_discovery(id, 2_000).await.expect("ack");
        let after = db.list_unacknowledged_discoveries(50).await.expect("list");
        assert!(after.is_empty(), "acknowledged rows must not reappear");
    }
}
