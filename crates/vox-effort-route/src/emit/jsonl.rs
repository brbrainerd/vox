//! Append-only JSONL writer for `recommendations.jsonl`.
//!
//! Mirrors S1's `vox-effort-audit::output::jsonl`: per-row flush so partial
//! progress is visible. This is the stable machine-readable contract S4 reads.

use super::RecommendationRow;
use std::io::Write;
use std::path::Path;

/// Append-only JSONL writer. Each `append` flushes so partial progress is visible.
pub struct JsonlWriter {
    file: std::fs::File,
}

impl JsonlWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn append(&mut self, row: &RecommendationRow) -> std::io::Result<()> {
        let line = serde_json::to_string(row)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(self.file, "{line}")?;
        self.file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{ArtifactForm, RemediationDecision};

    fn row(cluster_id: &str) -> RecommendationRow {
        RecommendationRow::new(RemediationDecision {
            cluster_id: cluster_id.into(),
            member_commit_shas: vec!["deadbeef".into()],
            member_count: 1,
            total_member_tokens: 150,
            artifact_form: ArtifactForm::CiGate,
            confidence: 0.8,
            synthesized_fix_summary: "summary".into(),
            drafted_artifact: None,
            verified: true,
            refutation_note: "note".into(),
            judge_tokens_used: 0,
        })
    }

    #[test]
    fn append_writes_one_line_per_row() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut w = JsonlWriter::create(tmp.path()).unwrap();
        w.append(&row("c1")).unwrap();
        w.append(&row("c2")).unwrap();
        let body = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(body.lines().count(), 2);
        assert!(
            body.lines()
                .all(|l| l.contains("\"schema_version\":\"1.0\""))
        );
    }

    #[test]
    fn round_trip_parses_back() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut w = JsonlWriter::create(tmp.path()).unwrap();
        w.append(&row("c1")).unwrap();
        w.append(&row("c2")).unwrap();
        let body = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: Vec<RecommendationRow> = body
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].schema_version, "1.0");
        assert_eq!(parsed[0].decision.cluster_id, "c1");
        assert_eq!(parsed[1].decision.cluster_id, "c2");
        assert_eq!(parsed[0].decision.total_member_tokens, 150);
        assert_eq!(parsed[0].decision.artifact_form, ArtifactForm::CiGate);
    }
}
