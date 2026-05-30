//! Load + validate + filter S1's findings.jsonl.

use vox_effort_audit::output::FindingRow;
use vox_effort_audit::judge::schema::SCHEMA_VERSION;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse failed at line {line}: {source}")]
    Parse { line: usize, source: serde_json::Error },
    #[error("schema_version mismatch: found {found:?}, expected {expected:?}")]
    SchemaMismatch { found: String, expected: String },
}

/// A finding that survived filtering (guaranteed `finding.is_some()` and score >= threshold).
#[derive(Debug, Clone)]
pub struct LoadedFinding {
    pub row: FindingRow,
}

/// Parse, schema-validate, and filter findings.jsonl.
pub fn read(path: &Path, min_waste_score: u8) -> Result<Vec<LoadedFinding>, LoadError> {
    let body = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() { continue; }
        let row: FindingRow = serde_json::from_str(line)
            .map_err(|source| LoadError::Parse { line: i + 1, source })?;
        if row.schema_version != SCHEMA_VERSION {
            return Err(LoadError::SchemaMismatch {
                found: row.schema_version.clone(),
                expected: SCHEMA_VERSION.to_string(),
            });
        }
        match &row.finding {
            Some(f) if f.waste_score >= min_waste_score => out.push(LoadedFinding { row }),
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/findings.jsonl")
    }

    #[test]
    fn filters_null_and_low_score() {
        let v = read(&fixture(), 4).unwrap();
        // 4 fixture rows; 1 null-finding + 1 low-score dropped → 2 remain.
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|f| f.row.finding.as_ref().unwrap().waste_score >= 4));
    }

    #[test]
    fn schema_mismatch_aborts() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"schema_version":"9.9","commit_sha":"x","parent_sha":null,"commit_ts":"2026-05-28T00:00:00Z","author_email_sha256":"0","branch_hint":"main","message_first_line":"m","shape":{"additions":0,"deletions":0,"files_changed":0,"file_extension_histogram":{},"mechanical_sweep_score":0.0,"is_lockfile_only":false,"is_generated_only":false,"is_doc_only":false,"commit_kind_from_message":"other"},"cost":{"kind":"Unavailable"},"judge":{"model_id":"m","latency_ms":0,"judge_input_tokens":0,"judge_output_tokens":0,"outcome":"Judged"},"finding":null}"#).unwrap();
        assert!(matches!(read(tmp.path(), 4), Err(LoadError::SchemaMismatch { .. })));
    }
}
