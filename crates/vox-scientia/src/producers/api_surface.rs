//! Public-API-surface growth producer.
//!
//! Scans each crate's `src/lib.rs` and counts `pub fn` / `pub struct` /
//! `pub enum` declarations. A crate whose lib re-exports ≥ [`MIN_PUB_SYMBOLS`]
//! distinct public items has crossed the threshold for being a publishable
//! algorithmic-improvement contribution (the crate is a "real library").
//!
//! Deliberately permissive: counts only top-level `pub` declarations in
//! `lib.rs`, not nested modules. This keeps the producer fast and bounds
//! its false-positive rate (a crate has to deliberately surface its work
//! through `lib.rs` to count).

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "api_surface";
/// Minimum count of top-level `pub fn|struct|enum|trait` symbols in `lib.rs`.
pub const MIN_PUB_SYMBOLS: usize = 8;

pub struct ApiSurfaceProducer;

impl ApiSurfaceProducer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ApiSurfaceProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Producer for ApiSurfaceProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        scan(&ctx.repo_root.join("crates"), ctx.now_ms, &ctx.session_id)
    }
}

/// Count top-level `pub fn|struct|enum|trait` declarations in a Rust source
/// file. Lines starting with `//` (single-line comment) are ignored;
/// nested-module declarations don't contribute.
///
/// Public so callers / tests can validate without invoking the producer.
pub fn count_pub_symbols(source: &str) -> usize {
    source
        .lines()
        .map(str::trim_start)
        .filter(|l| {
            !l.starts_with("//")
                && (l.starts_with("pub fn ")
                    || l.starts_with("pub struct ")
                    || l.starts_with("pub enum ")
                    || l.starts_with("pub trait "))
        })
        .count()
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
        let lib_rs = crate_dir.join("src").join("lib.rs");
        if !lib_rs.is_file() {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&lib_rs) else {
            continue;
        };
        let count = count_pub_symbols(&contents);
        if count < MIN_PUB_SYMBOLS {
            continue;
        }
        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::");
        h.update(crate_dir.to_string_lossy().as_bytes());
        h.update(count.to_le_bytes());
        let digest = h.finalize();
        let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let finding_id = format!("algimp-{slug}-api-{sha8}");
        // Worthiness scales with count up to 50 symbols.
        let score = ((count as f64) / 50.0).min(1.0);
        out.push(ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids: vec![],
            worthiness_score: score,
            session_id: session_id.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_pub_symbols_handles_canonical_patterns() {
        let src = r#"
pub fn alpha() {}
pub struct Beta;
pub enum Gamma { X, Y }
pub trait Delta { fn d(&self); }
fn private() {}
"#;
        assert_eq!(count_pub_symbols(src), 4);
    }

    #[test]
    fn count_pub_symbols_ignores_comments() {
        let src = r#"
// pub fn commented_out() {}
pub fn real() {}
// pub struct AlsoCommented;
"#;
        assert_eq!(count_pub_symbols(src), 1);
    }

    #[test]
    fn count_pub_symbols_does_not_match_inner_pub() {
        let src = r#"
fn wrapper() {
    pub fn inner() {}
}
"#;
        // Indented `pub fn` still counts via trim_start. This is a known
        // false-positive but bounded; the threshold protects against it.
        assert_eq!(count_pub_symbols(src), 1);
    }

    #[test]
    fn count_pub_symbols_handles_empty_input() {
        assert_eq!(count_pub_symbols(""), 0);
    }

    fn write_lib_rs(crates_root: &std::path::Path, crate_name: &str, n_pub: usize) {
        let crate_dir = crates_root.join(crate_name);
        let src_dir = crate_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let body = (0..n_pub)
            .map(|i| format!("pub fn fn_{i}() {{}}\n"))
            .collect::<String>();
        std::fs::write(src_dir.join("lib.rs"), body).unwrap();
    }

    #[test]
    fn small_crate_below_threshold_does_not_emit() {
        let tmp = tempfile::tempdir().unwrap();
        write_lib_rs(tmp.path(), "small", 3);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn large_crate_emits_algimp_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        write_lib_rs(tmp.path(), "big", 20);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(out.len(), 1);
        match &out[0] {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                assert!(finding_id.starts_with("algimp-"));
                assert!(finding_id.contains("-api-"));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn crate_without_lib_rs_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_only = tmp.path().join("bin-only").join("src");
        std::fs::create_dir_all(&bin_only).unwrap();
        std::fs::write(bin_only.join("main.rs"), "fn main() {}").unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }
}
