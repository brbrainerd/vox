//! ADR (Architectural Decision Record) emergence producer.
//!
//! Walks the docs tree and emits a `policy_governance` candidate for each
//! file matching common ADR-naming conventions:
//!
//! - `**/adr/**.md`
//! - `**/decisions/**.md`
//! - `**/0001-*.md` through `**/9999-*.md` (numbered-decision pattern)
//!
//! ADRs capture institutional decisions whose publication is genuinely
//! `policy_governance` work (governance frameworks, deprecation rationales,
//! API-stability commitments).

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "adr_emergence";

pub struct AdrEmergenceProducer;

impl AdrEmergenceProducer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdrEmergenceProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Producer for AdrEmergenceProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        scan(&ctx.repo_root, ctx.now_ms, &ctx.session_id)
    }
}

/// True iff `path` matches any of the canonical ADR-naming patterns.
///
/// Public so callers can validate path classifications without invoking
/// the full producer.
pub fn is_adr_path(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    if !path_str.ends_with(".md") {
        return false;
    }
    // Path-segment match: any ancestor named `adr` or `decisions`.
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let lower = name.to_string_lossy().to_lowercase();
            if lower == "adr" || lower == "decisions" {
                return true;
            }
        }
    }
    // Filename-prefix match: `NNNN-...md` for 1-4 digit N.
    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
        let mut chars = filename.chars();
        let mut digit_count = 0;
        while let Some(c) = chars.next() {
            if c.is_ascii_digit() {
                digit_count += 1;
            } else if c == '-' && (1..=4).contains(&digit_count) {
                return true;
            } else {
                break;
            }
        }
    }
    false
}

fn scan(
    repo_root: &std::path::Path,
    now_ms: i64,
    session_id: &str,
) -> Vec<ResearchEvent> {
    let docs_root = repo_root.join("docs");
    if !docs_root.is_dir() {
        return Vec::new();
    }
    let slug = date_slug(now_ms);
    let mut out = Vec::new();
    walk_adrs(&docs_root, &mut |path, contents| {
        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::");
        h.update(path.to_string_lossy().as_bytes());
        h.update(b"::");
        // Hash the first 4KB only — caption/title is the load-bearing
        // identifier; full-content hashing makes id churn on every edit.
        let prefix = &contents.as_bytes()[..contents.len().min(4096)];
        h.update(prefix);
        let digest = h.finalize();
        let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let finding_id = format!("polgov-{slug}-adr-{sha8}");
        out.push(ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids: vec![],
            worthiness_score: 0.55,
            session_id: session_id.to_string(),
        });
    });
    out
}

fn walk_adrs(dir: &std::path::Path, on_adr: &mut dyn FnMut(&std::path::Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_adrs(&path, on_adr);
        } else if is_adr_path(&path) {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                on_adr(&path, &contents);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestor_named_adr_matches() {
        assert!(is_adr_path(std::path::Path::new("docs/adr/001-foo.md")));
        assert!(is_adr_path(std::path::Path::new("a/b/adr/c.md")));
    }

    #[test]
    fn ancestor_named_decisions_matches() {
        assert!(is_adr_path(std::path::Path::new("docs/decisions/2026-04-pick.md")));
    }

    #[test]
    fn numbered_filename_prefix_matches() {
        assert!(is_adr_path(std::path::Path::new("0001-foo.md")));
        assert!(is_adr_path(std::path::Path::new("docs/0042-bar.md")));
        assert!(is_adr_path(std::path::Path::new("123-something.md")));
    }

    #[test]
    fn non_md_does_not_match() {
        assert!(!is_adr_path(std::path::Path::new("docs/adr/notes.txt")));
    }

    #[test]
    fn non_adr_md_does_not_match() {
        assert!(!is_adr_path(std::path::Path::new("docs/intro.md")));
        assert!(!is_adr_path(std::path::Path::new("README.md")));
    }

    #[test]
    fn numbered_pattern_requires_dash_separator() {
        assert!(!is_adr_path(std::path::Path::new("0001foo.md")));
        assert!(!is_adr_path(std::path::Path::new("0001_foo.md")));
    }

    #[test]
    fn missing_docs_dir_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn adr_file_emits_polgov_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        let adr_dir = tmp.path().join("docs").join("adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        std::fs::write(
            adr_dir.join("001-deprecation-policy.md"),
            "# 001 — Deprecation policy\n\nWe deprecate via SemVer.",
        )
        .unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(out.len(), 1);
        match &out[0] {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                assert!(finding_id.starts_with("polgov-"));
                assert!(finding_id.contains("-adr-"));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn non_adr_md_in_docs_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs").join("src");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("guide.md"), "# Guide").unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }
}
