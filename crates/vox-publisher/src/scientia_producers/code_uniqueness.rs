//! Code-uniqueness signal (Task 15).
//!
//! Embedding-distance novelty is WEAK-MODERATE evidence: it can corroborate a
//! finding but must never decide one. This module exposes:
//!
//! - [`uniqueness_score`] — pure: `1.0 - max(similarity)`, empty corpus → 1.0.
//! - [`extract_snippets`] — pure: doc-comment + signature per top-level Rust
//!   symbol, line-scanned (no `syn`), capped at 40 lines/snippet.
//! - [`assess_code_uniqueness`] — async: embeds each unique snippet via the
//!   cache-backed [`Embedder`] seam and looks up max similarity in a
//!   [`CodeKnnIndex`]. If no index is configured the assessment is `None`
//!   (skipped) — a missing index NEVER fabricates a score.

use crate::scientia_evidence::{
    DiscoverySignal, DiscoverySignalFamily, DiscoverySignalProvenance, DiscoverySignalStrength,
};
use crate::scientia_semantic::Embedder;
use std::collections::HashMap;

/// PURE: `1.0 - max(similarities)`; an empty corpus → `1.0` (everything is
/// unique). Similarities are clamped into `[0, 1]` before the max so a noisy
/// negative cosine cannot push uniqueness above 1.0.
#[must_use]
pub fn uniqueness_score(similarities: &[f64]) -> f64 {
    let max_sim = similarities
        .iter()
        .copied()
        .map(|s| s.clamp(0.0, 1.0))
        .fold(0.0_f64, f64::max);
    (1.0 - max_sim).clamp(0.0, 1.0)
}

/// A doc-comment + signature slice of one top-level Rust symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSnippet {
    pub path: String,
    pub text: String,
}

/// Maximum lines retained per extracted snippet.
const SNIPPET_LINE_CAP: usize = 40;

/// Is `trimmed` the start of a top-level item we want to snapshot?
fn is_symbol_start(trimmed: &str) -> bool {
    // Strip common visibility / qualifier prefixes so `pub async fn` etc match.
    let mut rest = trimmed;
    for kw in [
        "pub(crate)",
        "pub",
        "default",
        "async",
        "unsafe",
        "const",
        "extern",
    ] {
        if let Some(stripped) = rest.strip_prefix(kw)
            && (stripped.starts_with(|c: char| c.is_whitespace()) || stripped.starts_with('"'))
        {
            rest = stripped.trim_start();
        }
    }
    rest.starts_with("fn ")
        || rest.starts_with("struct ")
        || rest.starts_with("impl ")
        || rest.starts_with("impl<")
        || rest.starts_with("trait ")
        || rest.starts_with("enum ")
}

/// PURE: extract a doc-comment + signature snippet per top-level `fn` / `struct`
/// / `impl` / `trait` / `enum` in `source`. Line-scan (no `syn`). Each snippet
/// includes the contiguous block of `///` / `//!` doc lines that immediately
/// precede the symbol plus the signature lines up to (and including) the line
/// that opens the body (`{`) or ends the item (`;`), capped at
/// [`SNIPPET_LINE_CAP`] lines.
#[must_use]
pub fn extract_snippets(path: &str, source: &str) -> Vec<CodeSnippet> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        // Only consider items at column 0 or one indent (top-level / impl-body).
        let indent = lines[i].len() - trimmed.len();
        if indent <= 4 && is_symbol_start(trimmed) {
            // Walk backwards to gather a contiguous doc-comment block.
            let mut start = i;
            while start > 0 {
                let prev = lines[start - 1].trim_start();
                if prev.starts_with("///") || prev.starts_with("//!") || prev.starts_with("#[") {
                    start -= 1;
                } else {
                    break;
                }
            }
            // Walk forward to the end of the signature.
            let mut end = i;
            while end < lines.len() {
                let l = lines[end].trim_end();
                if l.ends_with('{') || l.ends_with(';') {
                    break;
                }
                end += 1;
                if end - i > SNIPPET_LINE_CAP {
                    break;
                }
            }
            let last = end.min(lines.len().saturating_sub(1));
            let mut block: Vec<&str> = lines[start..=last].to_vec();
            if block.len() > SNIPPET_LINE_CAP {
                block.truncate(SNIPPET_LINE_CAP);
            }
            let text = block.join("\n").trim().to_string();
            if !text.is_empty() {
                out.push(CodeSnippet {
                    path: path.to_string(),
                    text,
                });
            }
            i = last + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// kNN seam over a code-embedding vector index.
///
/// Implemented in the CLI by wrapping `vox_search::vector_qdrant::QdrantSemanticClient`.
/// `max_similarity` returns the single highest cosine-style score for `vector`,
/// or `None` when the index is unreachable/unconfigured (the caller then SKIPS
/// the snippet rather than treating it as maximally novel).
#[async_trait::async_trait]
pub trait CodeKnnIndex: Send + Sync {
    async fn max_similarity(&self, vector: &[f32]) -> Option<f64>;
}

/// Outcome of [`assess_code_uniqueness`].
#[derive(Debug, Clone, PartialEq)]
pub struct CodeUniquenessAssessment {
    /// `1.0 - max(similarity)` over assessed snippets.
    pub score: f64,
    /// How many unique snippet texts were embedded AND scored against the index.
    pub snippets_assessed: usize,
    /// Optional emitted signal (present only when the score clears the bar over
    /// enough snippets). Embedding distance is weak-moderate evidence, so this
    /// is always `Supporting`.
    pub signal: Option<DiscoverySignal>,
}

/// Minimum assessed snippets before an emitted novelty signal is meaningful.
const MIN_ASSESSED_SNIPPETS: usize = 2;
/// Uniqueness threshold above which a (still Supporting) novelty signal fires.
const NOVELTY_THRESHOLD: f64 = 0.6;

/// Async: assess code uniqueness for `snippets` using a cache-backed
/// [`Embedder`] and an optional [`CodeKnnIndex`].
///
/// Returns `None` (skip — never fabricate) when there is no index, when no
/// snippet could be both embedded and scored, or when `snippets` is empty.
/// Each unique snippet text is embedded exactly once (dedup by text), so the
/// efficiency invariant — one embed call per distinct snippet — holds at this
/// layer in addition to the embedder's own cache.
pub async fn assess_code_uniqueness(
    snippets: &[CodeSnippet],
    embedder: &dyn Embedder,
    index: Option<&dyn CodeKnnIndex>,
    source_ref: Option<&str>,
) -> Option<CodeUniquenessAssessment> {
    let index = index?;
    if snippets.is_empty() {
        return None;
    }

    // Dedup by text so a repeated snippet costs only one embed call.
    let mut embedded: HashMap<&str, Option<Vec<f32>>> = HashMap::new();
    let mut sims: Vec<f64> = Vec::new();
    for snip in snippets {
        let text = snip.text.as_str();
        let vec = match embedded.get(text) {
            Some(v) => v.clone(),
            None => {
                let v = embedder.embed(text).await;
                embedded.insert(text, v.clone());
                v
            }
        };
        let Some(vec) = vec else { continue };
        if let Some(sim) = index.max_similarity(&vec).await {
            sims.push(sim);
        }
    }

    if sims.is_empty() {
        // Index configured but nothing scored (all embeds failed / empty corpus
        // returned no hits) — skip rather than fabricate.
        return None;
    }

    let score = uniqueness_score(&sims);
    let snippets_assessed = sims.len();
    let signal = if score >= NOVELTY_THRESHOLD && snippets_assessed >= MIN_ASSESSED_SNIPPETS {
        Some(DiscoverySignal {
            code: "code_novelty_embedding".to_string(),
            summary: format!(
                "Changed code embeds far from the indexed corpus (uniqueness {score:.2} over {snippets_assessed} snippets). Weak-moderate evidence — corroborate, do not decide."
            ),
            strength: DiscoverySignalStrength::Supporting,
            source_ref: source_ref.map(str::to_string),
            family: DiscoverySignalFamily::LinkedCorpus,
            provenance: DiscoverySignalProvenance {
                origin: Some("code_uniqueness".to_string()),
                metric_type: Some("embedding_distance".to_string()),
                ..Default::default()
            },
        })
    } else {
        None
    };

    Some(CodeUniquenessAssessment {
        score,
        snippets_assessed,
        signal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn uniqueness_is_one_minus_max_similarity() {
        let s = uniqueness_score(&[0.2, 0.9, 0.4]);
        assert!((s - 0.1).abs() < 1e-9, "got {s}");
    }

    #[test]
    fn empty_corpus_is_fully_unique() {
        assert_eq!(uniqueness_score(&[]), 1.0);
    }

    #[test]
    fn negative_similarity_does_not_exceed_one() {
        assert_eq!(uniqueness_score(&[-0.3]), 1.0);
    }

    #[test]
    fn doc_comment_snippets_are_extracted_per_changed_symbol() {
        let src = r#"
use std::fmt;

/// Adds two numbers.
/// Returns the sum.
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// A point in 2D space.
pub struct Point {
    pub x: f64,
    pub y: f64,
}

fn private_helper() {
    // no doc comment, still a symbol
}
"#;
        let snips = extract_snippets("src/math.rs", src);
        assert_eq!(
            snips.len(),
            3,
            "fn + struct + helper => 3 symbols, got {snips:?}"
        );
        assert!(snips[0].text.contains("Adds two numbers"));
        assert!(snips[0].text.contains("pub fn add"));
        assert!(snips[1].text.contains("A point in 2D space"));
        assert!(snips[1].text.contains("pub struct Point"));
        assert_eq!(snips[0].path, "src/math.rs");
    }

    #[test]
    fn snippet_respects_line_cap() {
        let mut src = String::from("/// doc\npub fn big(\n");
        for n in 0..100 {
            src.push_str(&format!("    arg{n}: i64,\n"));
        }
        src.push_str(") -> i64 {\n    0\n}\n");
        let snips = extract_snippets("src/x.rs", &src);
        assert_eq!(snips.len(), 1);
        let line_count = snips[0].text.lines().count();
        assert!(
            line_count <= SNIPPET_LINE_CAP,
            "capped at {SNIPPET_LINE_CAP}, got {line_count}"
        );
    }

    /// Counting fake embedder: records how many times `embed` was called and
    /// with what text. Returns a deterministic vector derived from the text.
    struct CountingEmbedder {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl Embedder for CountingEmbedder {
        async fn embed(&self, text: &str) -> Option<Vec<f32>> {
            self.calls.lock().unwrap().push(text.to_string());
            // length-derived stub vector (value is irrelevant; the fake index
            // ignores it).
            Some(vec![text.len() as f32, 1.0, 0.0])
        }
    }

    /// Fake index returning a fixed similarity for every query.
    struct FixedIndex(f64);

    #[async_trait::async_trait]
    impl CodeKnnIndex for FixedIndex {
        async fn max_similarity(&self, _vector: &[f32]) -> Option<f64> {
            Some(self.0)
        }
    }

    #[tokio::test]
    async fn repeated_snippet_text_embeds_once() {
        let emb = CountingEmbedder {
            calls: Mutex::new(Vec::new()),
        };
        let idx = FixedIndex(0.1);
        let snippets = vec![
            CodeSnippet {
                path: "a.rs".into(),
                text: "fn foo() {}".into(),
            },
            CodeSnippet {
                path: "b.rs".into(),
                text: "fn foo() {}".into(),
            }, // identical text
            CodeSnippet {
                path: "c.rs".into(),
                text: "fn bar() {}".into(),
            },
        ];
        let out = assess_code_uniqueness(&snippets, &emb, Some(&idx), Some("git:abc"))
            .await
            .expect("assessment present");
        // Two unique texts => exactly two embed calls.
        assert_eq!(
            emb.calls.lock().unwrap().len(),
            2,
            "must embed each unique text once"
        );
        // Three snippets all scored (sim 0.1 each) => uniqueness 0.9.
        assert_eq!(out.snippets_assessed, 3);
        assert!((out.score - 0.9).abs() < 1e-9);
        assert!(out.signal.is_some(), "0.9 >= 0.6 over 3 snippets => signal");
        assert_eq!(
            out.signal.as_ref().unwrap().strength,
            DiscoverySignalStrength::Supporting
        );
    }

    #[tokio::test]
    async fn no_index_skips_assessment() {
        let emb = CountingEmbedder {
            calls: Mutex::new(Vec::new()),
        };
        let snippets = vec![CodeSnippet {
            path: "a.rs".into(),
            text: "fn foo() {}".into(),
        }];
        let out = assess_code_uniqueness(&snippets, &emb, None, None).await;
        assert!(out.is_none(), "no index => skip, never fabricate");
        assert!(
            emb.calls.lock().unwrap().is_empty(),
            "must not embed when skipping"
        );
    }

    #[tokio::test]
    async fn high_similarity_yields_no_signal_but_records_score() {
        let emb = CountingEmbedder {
            calls: Mutex::new(Vec::new()),
        };
        let idx = FixedIndex(0.95);
        let snippets = vec![
            CodeSnippet {
                path: "a.rs".into(),
                text: "fn foo() {}".into(),
            },
            CodeSnippet {
                path: "b.rs".into(),
                text: "fn bar() {}".into(),
            },
        ];
        let out = assess_code_uniqueness(&snippets, &emb, Some(&idx), None)
            .await
            .expect("assessment present");
        assert!((out.score - 0.05).abs() < 1e-9);
        assert!(out.signal.is_none(), "0.05 < 0.6 => no novelty signal");
    }
}
