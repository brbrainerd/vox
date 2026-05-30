//! Markdown report renderer for `report.md`.
//!
//! NEVER emits author info — that lives only in `findings.jsonl` (hashed).

use super::FindingRow;

pub fn render(rows: &[FindingRow], top_n: usize) -> String {
    let mut s = String::new();
    s.push_str("# Effort Audit Report\n\n");

    // 1. Run summary
    let total = rows.len();
    let judged = rows.iter().filter(|r| r.judge.outcome == "Judged").count();
    s.push_str(&format!("- Commits judged: {judged} / {total}\n\n"));

    // 2. Top-N
    s.push_str("## Top commits by waste_score\n\n");
    let mut ranked: Vec<&FindingRow> = rows.iter().filter(|r| r.finding.is_some()).collect();
    ranked
        .sort_by_key(|r| std::cmp::Reverse(r.finding.as_ref().map(|f| f.waste_score).unwrap_or(0)));
    for r in ranked.iter().take(top_n) {
        let f = r.finding.as_ref().unwrap();
        s.push_str(&format!(
            "- **[{}]** `{}` - {} ({:?})\n  - {}\n",
            f.waste_score,
            &r.commit_sha[..r.commit_sha.len().min(8)],
            r.message_first_line,
            f.suggested_remediation_kind,
            f.rationale_one_line,
        ));
    }

    // 3. Waste-category breakdown
    s.push_str("\n## Waste categories\n\n| Category | Count |\n|---|---:|\n");
    let mut cat_counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for r in rows.iter().filter_map(|r| r.finding.as_ref()) {
        *cat_counts
            .entry(format!("{:?}", r.waste_category))
            .or_insert(0) += 1;
    }
    for (k, v) in &cat_counts {
        s.push_str(&format!("| {k} | {v} |\n"));
    }

    // 4. Remediation kinds
    s.push_str("\n## Remediation kinds (preview for S2)\n\n| Kind | Count |\n|---|---:|\n");
    let mut rem_counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for r in rows.iter().filter_map(|r| r.finding.as_ref()) {
        *rem_counts
            .entry(format!("{:?}", r.suggested_remediation_kind))
            .or_insert(0) += 1;
    }
    for (k, v) in &rem_counts {
        s.push_str(&format!("| {k} | {v} |\n"));
    }

    // 5. Methodology
    s.push_str("\n## Methodology\n\nJudge model resolved via vox-orchestrator::models registry for the CodeEffortJudge task class. Hybrid signal: measured tokens from Claude Code transcripts when correlatable; LLM estimate otherwise.\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hybrid::MeasuredCost;
    use crate::judge::schema::{JudgeFinding, RemediationKind, WasteCategory};
    use crate::output::JudgeMeta;
    use crate::shape::{CommitKind, ShapeFeatures};
    use std::collections::HashMap;

    fn synth(
        sha: &str,
        msg: &str,
        score: u8,
        cat: WasteCategory,
        rem: RemediationKind,
    ) -> FindingRow {
        FindingRow {
            schema_version: "1.0".into(),
            commit_sha: sha.into(),
            parent_sha: None,
            // Fixed timestamp so snapshot is deterministic.
            commit_ts: chrono::DateTime::parse_from_rfc3339("2026-05-28T12:00:00Z")
                .unwrap()
                .to_utc(),
            author_email_sha256: "0".repeat(64),
            branch_hint: "main".into(),
            message_first_line: msg.into(),
            shape: ShapeFeatures {
                additions: 10,
                deletions: 5,
                files_changed: 1,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0,
                is_lockfile_only: false,
                is_generated_only: false,
                is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Unavailable,
            judge: JudgeMeta {
                model_id: "mock".into(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Judged".into(),
            },
            finding: Some(JudgeFinding {
                waste_score: score,
                waste_category: cat,
                suggested_remediation_kind: rem,
                rationale_one_line: format!("rationale for {sha}"),
                evidence_pointers: vec![],
            }),
        }
    }

    #[test]
    fn report_snapshot() {
        let rows = vec![
            synth(
                "aaaaaaaa",
                "refactor: mass sweep",
                9,
                WasteCategory::MechanicalSweep,
                RemediationKind::ScriptAutomation,
            ),
            synth(
                "bbbbbbbb",
                "fix: real bug",
                3,
                WasteCategory::LegitBugfix,
                RemediationKind::NoneNeeded,
            ),
            synth(
                "cccccccc",
                "docs: typo",
                1,
                WasteCategory::LegitDocs,
                RemediationKind::NoneNeeded,
            ),
        ];
        insta::assert_snapshot!(render(&rows, 20));
    }

    #[test]
    fn does_not_emit_author_email() {
        let rows = vec![synth(
            "aaa",
            "x",
            1,
            WasteCategory::Other,
            RemediationKind::Unknown,
        )];
        let out = render(&rows, 20);
        assert!(!out.contains("@"));
        assert!(!out.contains("author"));
        assert!(!out.contains("0000000000")); // hash should not leak
    }
}
