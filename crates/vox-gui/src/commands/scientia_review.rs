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
}
