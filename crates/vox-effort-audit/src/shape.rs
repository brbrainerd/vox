//! Local heuristic shape features computed without LLM.

use crate::walk::CommitRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShapeFeatures {
    pub additions: u64,
    pub deletions: u64,
    pub files_changed: u64,
    pub file_extension_histogram: HashMap<String, u32>,
    pub mechanical_sweep_score: f32,
    pub is_lockfile_only: bool,
    pub is_generated_only: bool,
    pub is_doc_only: bool,
    pub commit_kind_from_message: CommitKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitKind {
    Feat,
    Fix,
    Chore,
    Refactor,
    Docs,
    Test,
    Style,
    Ci,
    Other,
}

pub fn features(rec: &CommitRecord) -> ShapeFeatures {
    let kind = parse_commit_kind(&rec.message);

    let mut hist: HashMap<String, u32> = HashMap::new();
    for f in &rec.files {
        if let Some(ext) = std::path::Path::new(&f.path)
            .extension()
            .and_then(|s| s.to_str())
        {
            *hist.entry(ext.to_string()).or_insert(0) += 1;
        }
    }

    let lockfiles = [
        "Cargo.lock",
        "pnpm-lock.yaml",
        "package-lock.json",
        "uv.lock",
    ];
    let is_lockfile_only = !rec.files.is_empty()
        && rec
            .files
            .iter()
            .all(|f| lockfiles.iter().any(|l| f.path.ends_with(l)));
    let is_doc_only = !rec.files.is_empty()
        && rec
            .files
            .iter()
            .all(|f| f.path.starts_with("docs/") || f.path.ends_with(".md"));
    let is_generated_only =
        !rec.files.is_empty() && rec.files.iter().all(|f| f.path.contains(".generated."));

    let mechanical_sweep_score = compute_repetition(&rec.unified_diff_text);

    ShapeFeatures {
        additions: rec.additions,
        deletions: rec.deletions,
        files_changed: rec.files.len() as u64,
        file_extension_histogram: hist,
        mechanical_sweep_score,
        is_lockfile_only,
        is_generated_only,
        is_doc_only,
        commit_kind_from_message: kind,
    }
}

fn parse_commit_kind(msg: &str) -> CommitKind {
    let first = msg.split('\n').next().unwrap_or("");
    let prefix = first.split([':', '(', '!']).next().unwrap_or("");
    match prefix.trim().to_lowercase().as_str() {
        "feat" => CommitKind::Feat,
        "fix" => CommitKind::Fix,
        "chore" => CommitKind::Chore,
        "refactor" => CommitKind::Refactor,
        "docs" => CommitKind::Docs,
        "test" | "tests" => CommitKind::Test,
        "style" => CommitKind::Style,
        "ci" => CommitKind::Ci,
        _ => CommitKind::Other,
    }
}

/// Repetition score in [0.0, 1.0]. High when the diff is dominated by a
/// small number of distinct +/- lines (mechanical sweep). Low when every
/// +/- line is unique (varied real work).
fn compute_repetition(diff: &str) -> f32 {
    let mut unique = std::collections::HashSet::new();
    let mut total = 0u32;
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }
        if line.starts_with('+') || line.starts_with('-') {
            unique.insert(line);
            total += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    // 1 - unique/total: 0 when all unique, ~1 when only a few distinct lines repeated.
    1.0 - (unique.len() as f32 / total as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walk::{CommitRecord, FileChange};
    use chrono::TimeZone;

    fn rec(msg: &str, files: Vec<(&str, u64, u64)>, diff: &str) -> CommitRecord {
        CommitRecord {
            sha: "0".into(),
            parent_sha: None,
            commit_ts: chrono::Utc.timestamp_opt(0, 0).unwrap(),
            message: msg.into(),
            author_email_sha256: "x".into(),
            additions: files.iter().map(|(_, a, _)| *a).sum(),
            deletions: files.iter().map(|(_, _, d)| *d).sum(),
            files: files
                .iter()
                .map(|(p, a, d)| FileChange {
                    path: (*p).into(),
                    additions: *a,
                    deletions: *d,
                })
                .collect(),
            unified_diff_text: diff.into(),
            diff_truncated: false,
        }
    }

    #[test]
    fn lockfile_only_detection() {
        let r = rec("chore: bump deps", vec![("Cargo.lock", 4, 4)], "");
        let f = features(&r);
        assert!(f.is_lockfile_only);
        assert!(!f.is_doc_only);
    }

    #[test]
    fn doc_only_detection() {
        let r = rec(
            "docs: fix typo",
            vec![("README.md", 1, 1), ("docs/x.md", 2, 0)],
            "",
        );
        assert!(features(&r).is_doc_only);
    }

    #[test]
    fn commit_kind_from_conventional() {
        assert_eq!(
            features(&rec("fix(foo): bar", vec![], "")).commit_kind_from_message,
            CommitKind::Fix
        );
        assert_eq!(
            features(&rec("refactor!: drop", vec![], "")).commit_kind_from_message,
            CommitKind::Refactor
        );
        assert_eq!(
            features(&rec("random text", vec![], "")).commit_kind_from_message,
            CommitKind::Other
        );
    }

    #[test]
    fn mechanical_sweep_score_high_on_repetition() {
        let big =
            "-    pub const T: u64 = 30;\n+    pub const T: u64 = vox_config::T;\n".repeat(50);
        let r = rec("refactor: sweep", vec![], &big);
        let s = features(&r).mechanical_sweep_score;
        assert!(s > 0.7, "score was {s}, expected > 0.7");
    }

    #[test]
    fn mechanical_sweep_score_mid_range_on_half_identical() {
        // 10 identical "+const T..." lines + 10 distinct "+ let vN = foo();" lines.
        // Unique = 1 (repeated) + 10 (distinct) = 11; total = 20; score = 1 - 11/20 = 0.45.
        let identical = "+const T: u64 = 60;\n".repeat(10);
        let varied: String = (0..10).map(|i| format!("+ let v{i} = foo();\n")).collect();
        let diff = format!("{identical}{varied}");
        let s = features(&rec("refactor: half-and-half", vec![], &diff)).mechanical_sweep_score;
        assert!(
            s > 0.40 && s < 0.50,
            "expected ~0.45 (11 unique / 20 total), got {s}"
        );
    }

    #[test]
    fn mechanical_sweep_score_low_on_varied() {
        let varied = "+ fn alpha() {}\n+ fn beta(x: i32) {}\n+ struct Gamma;\n+ impl Gamma { fn delta(&self) {} }\n";
        let s = features(&rec("feat: misc", vec![], varied)).mechanical_sweep_score;
        assert!(s < 0.3, "score was {s}, expected < 0.3");
    }
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use crate::walk::{CommitRecord, FileChange};
    use chrono::TimeZone;

    fn make_rec(msg: &str, files: Vec<(&str, u64, u64)>, diff: &str) -> CommitRecord {
        CommitRecord {
            sha: "0".into(),
            parent_sha: None,
            commit_ts: chrono::Utc.timestamp_opt(0, 0).unwrap(),
            message: msg.into(),
            author_email_sha256: "x".into(),
            additions: files.iter().map(|(_, a, _)| *a).sum(),
            deletions: files.iter().map(|(_, _, d)| *d).sum(),
            files: files
                .iter()
                .map(|(p, a, d)| FileChange {
                    path: (*p).into(),
                    additions: *a,
                    deletions: *d,
                })
                .collect(),
            unified_diff_text: diff.into(),
            diff_truncated: false,
        }
    }

    // Catches: empty file list causing is_lockfile_only to return true (should be false).
    #[test]
    fn empty_file_list_not_lockfile_only() {
        let r = make_rec("chore: empty", vec![], "");
        let f = features(&r);
        assert!(
            !f.is_lockfile_only,
            "empty file list must NOT be lockfile_only"
        );
        assert!(!f.is_doc_only, "empty file list must NOT be doc_only");
        assert!(
            !f.is_generated_only,
            "empty file list must NOT be generated_only"
        );
    }

    // Catches: mixed lockfile + non-lockfile being incorrectly flagged as lockfile_only.
    #[test]
    fn mixed_lockfile_and_src_is_not_lockfile_only() {
        let r = make_rec(
            "fix: deps + code",
            vec![("Cargo.lock", 1, 0), ("src/main.rs", 5, 2)],
            "",
        );
        assert!(!features(&r).is_lockfile_only);
    }

    // Catches: compute_repetition returning non-zero for a diff with only header lines
    // (+++/---/@@) and no +/- content lines.
    #[test]
    fn header_only_diff_gives_zero_repetition_score() {
        let diff = "--- a/foo.rs\n+++ b/foo.rs\n@@ -1,2 +1,2 @@\n";
        let r = make_rec("fix: trivial", vec![], diff);
        assert_eq!(
            features(&r).mechanical_sweep_score,
            0.0,
            "diff with only @@ / +++ / --- lines must score 0.0"
        );
    }

    // Catches: parse_commit_kind treating "FEAT" (uppercase) as Other instead of Feat.
    #[test]
    fn commit_kind_case_insensitive() {
        let r = make_rec("FEAT: upper", vec![], "");
        assert_eq!(
            features(&r).commit_kind_from_message,
            CommitKind::Feat,
            "commit kind must be case-insensitive"
        );
    }

    // Catches: "tests" alias not mapping to CommitKind::Test.
    #[test]
    fn tests_plural_alias_maps_to_test_kind() {
        let r = make_rec("tests: add coverage", vec![], "");
        assert_eq!(features(&r).commit_kind_from_message, CommitKind::Test);
    }

    // Catches: file_extension_histogram overcounting files with no extension.
    #[test]
    fn no_extension_file_not_in_histogram() {
        let r = make_rec("chore: add", vec![("Makefile", 1, 0)], "");
        let f = features(&r);
        // "Makefile" has no extension — histogram should be empty.
        assert!(
            f.file_extension_histogram.is_empty(),
            "files with no extension must not appear in the histogram"
        );
    }

    // Catches: files_changed not matching actual file count (e.g., off-by-one or saturating).
    #[test]
    fn files_changed_equals_file_count() {
        let files: Vec<(&str, u64, u64)> = (0..5)
            .map(|i| {
                let path: &'static str = Box::leak(format!("src/f{i}.rs").into_boxed_str());
                (path, 1u64, 0u64)
            })
            .collect();
        let r = make_rec("feat: five files", files, "");
        assert_eq!(features(&r).files_changed, 5);
    }

    // Catches: is_doc_only incorrectly flagging a file that merely ends with ".md" in its
    // directory component (e.g. "docs_archive/readme.rs").
    #[test]
    fn rs_file_not_doc_only() {
        let r = make_rec("fix: src", vec![("src/lib.rs", 2, 1)], "");
        assert!(!features(&r).is_doc_only);
    }
}
