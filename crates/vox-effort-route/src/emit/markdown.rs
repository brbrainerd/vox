//! Markdown report renderer for `recommendations.md`.
//!
//! NEVER emits author info. Author identity lives only (hashed) in S1's
//! `findings.jsonl`; it never enters a `RemediationDecision`, so it cannot leak
//! here. The `does_not_emit_author_identity` test guards that invariant.

use super::RecommendationRow;

/// Default number of clusters shown in the Top-N table.
pub const DEFAULT_TOP_N: usize = 20;

/// Render the human-readable recommendations report.
///
/// Layout: run summary (counts + verified vs not), a Top-N table ranked by
/// `total_member_tokens` desc then confidence desc (verified decisions first), a
/// per-`artifact_form` breakdown, and a methodology note.
pub fn render(rows: &[RecommendationRow]) -> String {
    let mut s = String::new();
    s.push_str("# Effort Route Recommendations\n\n");

    // 1. Run summary.
    let total = rows.len();
    let verified = rows.iter().filter(|r| r.decision.verified).count();
    let drafted = rows
        .iter()
        .filter(|r| r.decision.drafted_artifact.is_some())
        .count();
    s.push_str(&format!("- Clusters routed: {total}\n"));
    s.push_str(&format!(
        "- Verified: {verified} / {total} (unverified: {})\n",
        total - verified
    ));
    s.push_str(&format!("- Draft artifacts staged: {drafted}\n\n"));

    // 2. Top-N table. Verified first, then tokens desc, then confidence desc.
    s.push_str("## Top clusters by reclaimable tokens\n\n");
    s.push_str("| Cluster | Form | Commits | Tokens | Confidence | Verified |\n");
    s.push_str("|---|---|---:|---:|---:|:---:|\n");
    let mut ranked: Vec<&RecommendationRow> = rows.iter().collect();
    ranked.sort_by(|a, b| {
        b.decision
            .verified
            .cmp(&a.decision.verified)
            .then(
                b.decision
                    .total_member_tokens
                    .cmp(&a.decision.total_member_tokens),
            )
            .then(
                b.decision
                    .confidence
                    .partial_cmp(&a.decision.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.decision.cluster_id.cmp(&b.decision.cluster_id))
    });
    for r in ranked.iter().take(DEFAULT_TOP_N) {
        let d = &r.decision;
        s.push_str(&format!(
            "| `{}` | {:?} | {} | {} | {:.2} | {} |\n",
            d.cluster_id,
            d.artifact_form,
            d.member_count,
            d.total_member_tokens,
            d.confidence,
            if d.verified { "yes" } else { "no" },
        ));
    }

    // 3. Per-artifact_form breakdown.
    s.push_str("\n## Artifact forms\n\n| Form | Count |\n|---|---:|\n");
    let mut form_counts: std::collections::BTreeMap<String, u32> =
        std::collections::BTreeMap::new();
    for r in rows {
        *form_counts
            .entry(format!("{:?}", r.decision.artifact_form))
            .or_insert(0) += 1;
    }
    for (k, v) in &form_counts {
        s.push_str(&format!("| {k} | {v} |\n"));
    }

    // 4. Methodology.
    s.push_str(
        "\n## Methodology\n\nClusters of related waste findings are re-judged by the \
         routing model into a single enforcement artifact, then adversarially \
         verified (a refutation pass that asks whether the artifact would actually \
         have prevented every member commit). Only verified decisions stage a \
         `.proposed` draft under the staging dir; nothing is written into the build \
         tree. Token totals are the summed measured/estimated cost of the cluster's \
         member commits (see S1 `findings.jsonl`). Author identity is never carried \
         into routing and never appears in this report.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{ArtifactForm, DraftedArtifact, RemediationDecision};

    /// Deterministic fixture row: fixed cluster ids, no timestamps.
    fn row(
        cluster_id: &str,
        form: ArtifactForm,
        member_count: usize,
        tokens: u64,
        confidence: f32,
        verified: bool,
    ) -> RecommendationRow {
        let drafted = if matches!(form, ArtifactForm::None) {
            None
        } else {
            Some(DraftedArtifact {
                form,
                staging_path: format!("{cluster_id}.{}", form.staging_extension()),
                body: "proposed body".into(),
                form_rationale: "rationale".into(),
                authoring_model_vox_capable: false,
            })
        };
        RecommendationRow::new(RemediationDecision {
            cluster_id: cluster_id.into(),
            member_commit_shas: (0..member_count).map(|i| format!("sha{i}")).collect(),
            member_count,
            total_member_tokens: tokens,
            artifact_form: form,
            confidence,
            synthesized_fix_summary: format!("fix for {cluster_id}"),
            drafted_artifact: drafted,
            verified,
            refutation_note: "note".into(),
            judge_tokens_used: 0,
        })
    }

    fn fixture_rows() -> Vec<RecommendationRow> {
        vec![
            row("cluster-0001", ArtifactForm::CiGate, 5, 9000, 0.91, true),
            row(
                "cluster-0002",
                ArtifactForm::CodeAuditDetector,
                3,
                4200,
                0.77,
                true,
            ),
            row(
                "cluster-0003",
                ArtifactForm::AgentsMdRule,
                2,
                1500,
                0.55,
                false,
            ),
            row("cluster-0004", ArtifactForm::None, 1, 300, 0.20, false),
        ]
    }

    #[test]
    fn does_not_emit_author_identity() {
        let rows = fixture_rows();
        let out = render(&rows);
        assert!(!out.contains('@'));
        assert!(!out
            .as_bytes()
            .windows(64)
            .any(|w| w.iter().all(|b| b.is_ascii_hexdigit())));
    }

    #[test]
    fn ranks_verified_first_then_tokens() {
        // cluster-0003 has more tokens (1500) than nothing, but is unverified, so
        // both verified rows must precede it; among verified, higher tokens first.
        let rows = fixture_rows();
        let out = render(&rows);
        let i1 = out.find("cluster-0001").unwrap();
        let i2 = out.find("cluster-0002").unwrap();
        let i3 = out.find("cluster-0003").unwrap();
        assert!(i1 < i2, "higher-token verified cluster ranks first");
        assert!(i2 < i3, "verified clusters precede unverified");
    }

    #[test]
    fn report_snapshot() {
        insta::assert_snapshot!(render(&fixture_rows()));
    }
}
