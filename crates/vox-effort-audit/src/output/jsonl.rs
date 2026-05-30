//! Append-only JSONL writer for `findings.jsonl`.

use super::FindingRow;
use std::io::Write;
use std::path::Path;

/// Append-only JSONL writer. Each `append` flushes so partial progress is visible.
pub struct JsonlWriter {
    file: std::fs::File,
}

impl JsonlWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn append(&mut self, row: &FindingRow) -> std::io::Result<()> {
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
    use crate::hybrid::MeasuredCost;
    use crate::shape::{CommitKind, ShapeFeatures};
    use std::collections::HashMap;

    fn row() -> FindingRow {
        FindingRow {
            schema_version: "1.0".into(),
            commit_sha: "abc".into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            author_email_sha256: "z".into(),
            branch_hint: "main".into(),
            message_first_line: "m".into(),
            shape: ShapeFeatures {
                additions: 0,
                deletions: 0,
                files_changed: 0,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0,
                is_lockfile_only: false,
                is_generated_only: false,
                is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Unavailable,
            judge: super::super::JudgeMeta {
                model_id: "mock".into(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Judged".into(),
            },
            finding: None,
        }
    }

    #[test]
    fn append_writes_one_line_per_row() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut w = JsonlWriter::create(tmp.path()).unwrap();
        w.append(&row()).unwrap();
        w.append(&row()).unwrap();
        let body = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(body.lines().count(), 2);
        assert!(
            body.lines()
                .all(|l| l.contains("\"schema_version\":\"1.0\""))
        );
    }
}
