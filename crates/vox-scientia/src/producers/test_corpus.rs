//! Test-corpus signal producer.
//!
//! Scans each crate's `tests/` directory and emits a
//! `reproducibility_infra` candidate when the crate has crossed a
//! sustained test-coverage threshold. Each crate is keyed by path so
//! re-runs yield deterministic ids.

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "test_corpus";
/// Minimum number of `.rs` files under `tests/` to count as a candidate.
pub const MIN_TEST_FILES: usize = 5;
/// Minimum total bytes of test code (rough proxy for "real" tests, not just stubs).
pub const MIN_TEST_BYTES: u64 = 8_000;

pub struct TestCorpusProducer;

impl TestCorpusProducer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TestCorpusProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Producer for TestCorpusProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        scan(&ctx.repo_root.join("crates"), ctx.now_ms, &ctx.session_id)
    }
}

fn scan(
    crates_root: &std::path::Path,
    now_ms: i64,
    session_id: &str,
) -> Vec<ResearchEvent> {
    if !crates_root.is_dir() {
        return Vec::new();
    }
    let slug = date_slug(now_ms);
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(crates_root) else {
        return out;
    };
    for entry in entries.flatten() {
        let crate_dir = entry.path();
        if !crate_dir.is_dir() {
            continue;
        }
        let tests_dir = crate_dir.join("tests");
        if !tests_dir.is_dir() {
            continue;
        }
        let (file_count, total_bytes) = count_test_files(&tests_dir);
        if file_count < MIN_TEST_FILES || total_bytes < MIN_TEST_BYTES {
            continue;
        }
        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::");
        h.update(crate_dir.to_string_lossy().as_bytes());
        h.update(file_count.to_le_bytes());
        let digest = h.finalize();
        let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let finding_id = format!("repinf-{slug}-tests-{sha8}");
        let score = ((file_count as f64) / 30.0).min(1.0);
        out.push(ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids: vec![],
            worthiness_score: score,
            session_id: session_id.to_string(),
        });
    }
    out
}

fn count_test_files(tests_dir: &std::path::Path) -> (usize, u64) {
    let mut files = 0usize;
    let mut bytes = 0u64;
    fn walk(dir: &std::path::Path, files: &mut usize, bytes: &mut u64) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, files, bytes);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                *files += 1;
                if let Ok(meta) = std::fs::metadata(&path) {
                    *bytes += meta.len();
                }
            }
        }
    }
    walk(tests_dir, &mut files, &mut bytes);
    (files, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_crate(crates_root: &std::path::Path, name: &str, n_files: usize, file_bytes: usize) {
        let crate_dir = crates_root.join(name);
        let tests_dir = crate_dir.join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        for i in 0..n_files {
            std::fs::write(
                tests_dir.join(format!("test_{i}.rs")),
                "fn placeholder() {}\n".repeat(file_bytes / 20),
            )
            .unwrap();
        }
    }

    #[test]
    fn missing_crates_dir_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn crate_below_file_threshold_does_not_emit() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_crate(tmp.path(), "tiny", 2, 1000);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn crate_meeting_thresholds_emits_repinf() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_crate(tmp.path(), "well-tested", 10, 2000);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(out.len(), 1);
        match &out[0] {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                assert!(finding_id.starts_with("repinf-"));
                assert!(finding_id.contains("-tests-"));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn multiple_crates_each_emit_separately() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_crate(tmp.path(), "crate-a", 10, 2000);
        create_test_crate(tmp.path(), "crate-b", 10, 2000);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(out.len(), 2);
    }
}
