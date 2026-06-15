//! Retrieval planning types and rank fusion for hybrid (vector + keyword) search.
//!
//! These are shared contracts used by MCP, the orchestrator, and VoxDb-backed retrieval surfaces.
//! The helpers here do not query Turso by themselves, but they define the typed search plan,
//! diagnostics, and provenance shape that concrete search backends should preserve.

use serde::{Deserialize, Serialize};

/// Which retrieval branch produced evidence for a hit (or both after hybrid fusion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalEvidenceSource {
    /// Caller did not classify the branch (treated as neutral during merge).
    #[default]
    Unknown,
    /// Vector / embedding similarity path.
    Vector,
    /// Keyword or full-text path.
    FullText,
    /// Both vector and keyword paths agreed on this `chunk_id`.
    Hybrid,
}

impl RetrievalEvidenceSource {
    /// Combines evidence from two hits of the same chunk (e.g. vector + FTS overlap).
    pub fn merge(self, other: Self) -> Self {
        use RetrievalEvidenceSource::*;
        match (self, other) {
            (Unknown, b) => b,
            (a, Unknown) => a,
            (Vector, Vector) => Vector,
            (FullText, FullText) => FullText,
            (Hybrid, _) | (_, Hybrid) => Hybrid,
            (Vector, FullText) | (FullText, Vector) => Hybrid,
        }
    }
}

/// How a [`RetrievalQuery`] should be interpreted upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    /// Embedding-only retrieval.
    Vector,
    /// Keyword / FTS-only retrieval.
    FullText,
    /// Run both branches and merge (see [`fuse_hybrid_results`]).
    Hybrid,
}

/// Inputs for a retrieval request (caller maps to concrete SQL or RPC).
#[derive(Debug, Clone)]
pub struct RetrievalQuery {
    /// User or agent query string.
    pub query_text: String,
    /// Retrieval branch selection for the caller’s pipeline.
    pub mode: RetrievalMode,
    /// Maximum hits to return after fusion.
    pub top_k: usize,
    /// Drop hits below this score (caller-defined scale).
    pub min_score: f32,
}

impl Default for RetrievalQuery {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            mode: RetrievalMode::Hybrid,
            top_k: 8,
            min_score: 0.0,
        }
    }
}

/// What kind of information need is being served by the current search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchIntent {
    /// Need a concrete grounded answer or citation.
    FactualLookup,
    /// Need file/module/symbol level navigation inside the repository.
    CodeNavigation,
    /// Need broad architecture or repository-shape context.
    RepoStructure,
    /// Need broader comparative or exploratory research.
    #[default]
    BroadResearch,
    /// Need corroboration or contradiction resolution before answering.
    Verification,
}

/// Logical corpora that the search planner can target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCorpus {
    Memory,
    KnowledgeGraph,
    DocumentChunks,
    RepoInventory,
    WebResearch,
    SymbolProximity,
}

/// Concrete retrieval backends or ranking legs used during execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchBackend {
    MemoryBm25,
    MemoryVector,
    KnowledgeFts,
    ChunkFts,
    ChunkVector,
    /// Sidecar Qdrant ANN (`vox-search` + `VOX_SEARCH_QDRANT_URL`).
    QdrantVector,
    RepoPath,
    Web,
    LexicalFallback,
    SymbolProximity,
}

/// Recommended next move after evaluating search quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRefinementAction {
    AskUser,
    RetryLexical,
    RetryHybrid,
    FocusRepo,
    FocusCodex,
    BroadenScope,
    NarrowScope,
    Abstain,
}

/// Typed search plan selected before retrieval begins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPlan {
    /// Raw query supplied by the caller after lightweight normalization.
    pub normalized_query: String,
    /// Optional follow-up query used during a verification or refinement pass.
    pub rewritten_query: Option<String>,
    /// High-level information need classification.
    pub intent: SearchIntent,
    /// Preferred retrieval blend for the first pass.
    pub retrieval_mode: RetrievalMode,
    /// Corpora that should be consulted.
    pub corpora: Vec<SearchCorpus>,
    /// Preferred backend order for telemetry and routing.
    pub preferred_backends: Vec<SearchBackend>,
    /// Whether a second verification pass is allowed when evidence is weak.
    pub allow_verification_pass: bool,
    /// Human-readable planner notes for debugging and telemetry.
    pub notes: Vec<String>,
}

impl Default for SearchPlan {
    fn default() -> Self {
        Self {
            normalized_query: String::new(),
            rewritten_query: None,
            intent: SearchIntent::BroadResearch,
            retrieval_mode: RetrievalMode::Hybrid,
            corpora: vec![
                SearchCorpus::Memory,
                SearchCorpus::KnowledgeGraph,
                SearchCorpus::DocumentChunks,
            ],
            preferred_backends: vec![
                SearchBackend::MemoryBm25,
                SearchBackend::MemoryVector,
                SearchBackend::KnowledgeFts,
                SearchBackend::ChunkFts,
            ],
            allow_verification_pass: true,
            notes: Vec::new(),
        }
    }
}

fn normalized_query_tokens(query_text: &str) -> Vec<String> {
    query_text
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '/' && c != '.')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

fn looks_like_code_navigation(query_text: &str) -> bool {
    let q = query_text.to_ascii_lowercase();
    q.contains("file")
        || q.contains("path")
        || q.contains("module")
        || q.contains("symbol")
        || q.contains("crate")
        || q.contains("function")
        || q.contains("struct")
        || q.contains("enum")
        || q.contains("trait")
        || q.contains("defined")
        || q.contains(".rs")
        || q.contains("src/")
        || q.contains("crates/")
        || q.contains("::")
}

fn looks_like_repo_structure(query_text: &str) -> bool {
    let q = query_text.to_ascii_lowercase();
    q.contains("architecture")
        || q.contains("repository")
        || q.contains("repo")
        || q.contains("overview")
        || q.contains("structure")
        || q.contains("where")
        || q.contains("how")
}

/// Shared lightweight search planner used by MCP/orchestrator call sites.
#[must_use]
pub fn heuristic_search_plan(
    query_text: &str,
    verification_pass: bool,
    mode_hint: Option<RetrievalMode>,
) -> SearchPlan {
    let normalized_query = query_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut plan = SearchPlan {
        normalized_query: normalized_query.clone(),
        retrieval_mode: mode_hint.unwrap_or(RetrievalMode::Hybrid),
        ..SearchPlan::default()
    };
    if verification_pass {
        plan.intent = SearchIntent::Verification;
        plan.retrieval_mode = RetrievalMode::Hybrid;
        plan.allow_verification_pass = false;
        plan.notes
            .push("verification pass broadens search before answering".to_string());
        return plan;
    }
    if looks_like_code_navigation(query_text) {
        plan.intent = SearchIntent::CodeNavigation;
        plan.retrieval_mode = mode_hint.unwrap_or(RetrievalMode::FullText);
        plan.corpora = vec![
            SearchCorpus::SymbolProximity,
            SearchCorpus::RepoInventory,
            SearchCorpus::DocumentChunks,
            SearchCorpus::KnowledgeGraph,
            SearchCorpus::Memory,
        ];
        plan.preferred_backends = vec![
            SearchBackend::SymbolProximity,
            SearchBackend::RepoPath,
            SearchBackend::ChunkFts,
            SearchBackend::KnowledgeFts,
            SearchBackend::MemoryBm25,
        ];
        plan.notes.push(
            "code-navigation query prioritizes proximity, repo path and lexical search".to_string(),
        );
        return plan;
    }
    if looks_like_repo_structure(query_text) {
        plan.intent = SearchIntent::RepoStructure;
        plan.corpora = vec![
            SearchCorpus::RepoInventory,
            SearchCorpus::Memory,
            SearchCorpus::KnowledgeGraph,
            SearchCorpus::DocumentChunks,
        ];
        plan.preferred_backends = vec![
            SearchBackend::RepoPath,
            SearchBackend::MemoryBm25,
            SearchBackend::MemoryVector,
            SearchBackend::KnowledgeFts,
            SearchBackend::ChunkFts,
            SearchBackend::ChunkVector,
        ];
        plan.notes.push(
            "repo-structure query uses repo inventory plus hybrid supporting evidence".to_string(),
        );
        return plan;
    }
    let token_count = normalized_query_tokens(query_text).len();
    plan.intent = if token_count > 8 {
        SearchIntent::BroadResearch
    } else {
        SearchIntent::FactualLookup
    };
    plan.corpora = vec![
        SearchCorpus::Memory,
        SearchCorpus::KnowledgeGraph,
        SearchCorpus::DocumentChunks,
        SearchCorpus::RepoInventory,
        SearchCorpus::WebResearch,
    ];
    plan.preferred_backends = vec![
        SearchBackend::MemoryBm25,
        SearchBackend::MemoryVector,
        SearchBackend::KnowledgeFts,
        SearchBackend::ChunkFts,
        SearchBackend::ChunkVector,
        SearchBackend::RepoPath,
        SearchBackend::Web,
    ];
    plan.notes
        .push("default plan prefers hybrid retrieval across memory and Codex corpora".to_string());
    plan
}

/// Structured diagnostics emitted after one retrieval cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchDiagnostics {
    /// Effective versioned search-policy schema for this pass (orchestrator / `vox-search` tunables).
    #[serde(default)]
    pub policy_version: u32,
    /// Short human/machine-readable rationale lines (intent, mode, corpora).
    #[serde(default)]
    pub selection_rationale: Vec<String>,
    /// Planner-selected mode for the executed pass.
    pub selected_mode: Option<RetrievalMode>,
    /// Backends that actually contributed evidence.
    pub backends_used: Vec<SearchBackend>,
    /// Whether an automatic verification pass was run.
    pub verification_performed: bool,
    /// Why verification was triggered, if it ran.
    pub verification_reason: Option<String>,
    /// The query used for verification/refinement, when different from the initial one.
    pub verification_query: Option<String>,
    /// Suggested next action if evidence remains weak.
    pub recommended_action: Option<SearchRefinementAction>,
    /// Coarse evidence quality proxy in `[0, 1]`.
    pub evidence_quality: f64,
    /// Citation coverage proxy in `[0, 1]`.
    pub citation_coverage: f64,
    /// Number of distinct corpora that returned at least one hit.
    pub source_diversity: usize,
    /// Highest score seen before verification.
    pub initial_top_score: Option<f64>,
    /// Highest score seen after verification.
    pub verified_top_score: Option<f64>,
    /// Difference between verified and initial top scores.
    pub verification_top_score_delta: Option<f64>,
    /// Additional notes suitable for telemetry and debugging.
    pub notes: Vec<String>,
}

/// One ranked chunk for provenance or UI (not a full DB row).
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    /// Stable id for deduplication across modalities (e.g. chunk primary key).
    pub chunk_id: String,
    /// Document or URI label for attribution.
    pub source: String,
    /// Fused relevance score (higher is better).
    pub score: f32,
    /// Short excerpt for display or LLM context.
    pub snippet: String,
    /// Which branch(es) supported this hit after optional caller hints + fusion.
    pub evidence_source: RetrievalEvidenceSource,
    /// Wall-clock ms when this hit was retrieved (best-effort; fusion keeps the max).
    pub retrieved_at_ms: Option<u64>,
    /// Opaque id tying this hit to a single retrieval request (fusion clears on conflict).
    pub query_id: Option<String>,
    /// Optional claim / span ids for downstream fact-checking or citation graphs.
    pub supporting_claim_ids: Vec<String>,
    /// Human-readable hints when two sources disagree (caller- or postprocessor-filled).
    pub contradiction_hints: Vec<String>,
}

impl RetrievalResult {
    /// Normalizes branch metadata before inserting into the fusion map from the vector list.
    pub fn normalized_for_vector_branch(mut self) -> Self {
        self.evidence_source = self.evidence_source.merge(RetrievalEvidenceSource::Vector);
        self
    }

    /// Normalizes branch metadata before inserting into the fusion map from the FTS list.
    pub fn normalized_for_text_branch(mut self) -> Self {
        self.evidence_source = self
            .evidence_source
            .merge(RetrievalEvidenceSource::FullText);
        self
    }
}

fn merge_retrieval_provenance(into: &mut RetrievalResult, from: &RetrievalResult) {
    into.evidence_source = into.evidence_source.merge(from.evidence_source);
    into.retrieved_at_ms = match (into.retrieved_at_ms, from.retrieved_at_ms) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    into.query_id = match (&into.query_id, &from.query_id) {
        (Some(a), Some(b)) if a == b => Some(a.clone()),
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (None, None) => None,
        _ => None,
    };
    for id in &from.supporting_claim_ids {
        if !into.supporting_claim_ids.contains(id) {
            into.supporting_claim_ids.push(id.clone());
        }
    }
    for h in &from.contradiction_hints {
        if !into.contradiction_hints.contains(h) {
            into.contradiction_hints.push(h.clone());
        }
    }
}

/// Merge vector and full-text hit lists by `chunk_id`, re-scoring overlaps with `vector_weight`.
///
/// - Chunks only in `vector_hits` keep their vector score (with vector-branch provenance).
/// - Chunks only in `text_hits` keep their text score (with FTS-branch provenance).
/// - Chunks in both get `existing * vector_weight + hit * (1 - vector_weight)` and merged
///   provenance (`merge_retrieval_provenance` in this module).
pub fn fuse_hybrid_results(
    vector_hits: &[RetrievalResult],
    text_hits: &[RetrievalResult],
    vector_weight: f32,
) -> Vec<RetrievalResult> {
    let mut merged: std::collections::HashMap<String, RetrievalResult> =
        std::collections::HashMap::new();
    for hit in vector_hits {
        let h = hit.clone().normalized_for_vector_branch();
        merged.insert(h.chunk_id.clone(), h);
    }
    for hit in text_hits {
        let h = hit.clone().normalized_for_text_branch();
        merged
            .entry(h.chunk_id.clone())
            .and_modify(|existing| {
                existing.score =
                    (existing.score * vector_weight) + (h.score * (1.0 - vector_weight));
                if existing.snippet.is_empty() {
                    existing.snippet = h.snippet.clone();
                }
                merge_retrieval_provenance(existing, &h);
            })
            .or_insert(h);
    }
    let mut out: Vec<_> = merged.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_fusion_sorts_by_score() {
        let v = vec![RetrievalResult {
            chunk_id: "a".into(),
            source: "doc".into(),
            score: 0.9,
            snippet: "alpha".into(),
            evidence_source: RetrievalEvidenceSource::Vector,
            retrieved_at_ms: Some(10),
            query_id: Some("q1".into()),
            supporting_claim_ids: vec!["c1".into()],
            contradiction_hints: vec![],
        }];
        let t = vec![RetrievalResult {
            chunk_id: "b".into(),
            source: "doc".into(),
            score: 0.8,
            snippet: "beta".into(),
            evidence_source: RetrievalEvidenceSource::FullText,
            retrieved_at_ms: Some(20),
            query_id: Some("q1".into()),
            supporting_claim_ids: vec![],
            contradiction_hints: vec![],
        }];
        let fused = fuse_hybrid_results(&v, &t, 0.7);
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].chunk_id, "a");
        assert_eq!(fused[0].evidence_source, RetrievalEvidenceSource::Vector);
        assert_eq!(fused[1].evidence_source, RetrievalEvidenceSource::FullText);
    }

    #[test]
    fn hybrid_fusion_merges_provenance_on_overlap() {
        let v = vec![RetrievalResult {
            chunk_id: "x".into(),
            source: "s".into(),
            score: 0.9,
            snippet: "v".into(),
            evidence_source: RetrievalEvidenceSource::Vector,
            retrieved_at_ms: Some(100),
            query_id: Some("same".into()),
            supporting_claim_ids: vec!["a".into()],
            contradiction_hints: vec!["h1".into()],
        }];
        let t = vec![RetrievalResult {
            chunk_id: "x".into(),
            source: "s".into(),
            score: 0.5,
            snippet: String::new(),
            evidence_source: RetrievalEvidenceSource::FullText,
            retrieved_at_ms: Some(200),
            query_id: Some("same".into()),
            supporting_claim_ids: vec!["b".into()],
            contradiction_hints: vec!["h2".into()],
        }];
        let fused = fuse_hybrid_results(&v, &t, 0.6);
        assert_eq!(fused.len(), 1);
        let h = &fused[0];
        assert!((h.score - (0.9_f32 * 0.6 + 0.5 * 0.4)).abs() < 1e-5);
        assert_eq!(h.evidence_source, RetrievalEvidenceSource::Hybrid);
        assert_eq!(h.retrieved_at_ms, Some(200));
        assert_eq!(h.query_id.as_deref(), Some("same"));
        assert!(h.supporting_claim_ids.contains(&"a".to_string()));
        assert!(h.supporting_claim_ids.contains(&"b".to_string()));
        assert!(h.contradiction_hints.contains(&"h1".to_string()));
        assert!(h.contradiction_hints.contains(&"h2".to_string()));
        assert_eq!(h.snippet, "v");
    }

    #[test]
    fn query_id_cleared_on_conflict() {
        let v = vec![RetrievalResult {
            chunk_id: "x".into(),
            source: "s".into(),
            score: 0.8,
            snippet: "".into(),
            evidence_source: RetrievalEvidenceSource::Vector,
            retrieved_at_ms: None,
            query_id: Some("q-a".into()),
            supporting_claim_ids: vec![],
            contradiction_hints: vec![],
        }];
        let t = vec![RetrievalResult {
            chunk_id: "x".into(),
            source: "s".into(),
            score: 0.7,
            snippet: "".into(),
            evidence_source: RetrievalEvidenceSource::FullText,
            retrieved_at_ms: None,
            query_id: Some("q-b".into()),
            supporting_claim_ids: vec![],
            contradiction_hints: vec![],
        }];
        let fused = fuse_hybrid_results(&v, &t, 0.5);
        assert_eq!(fused[0].query_id, None);
    }

    #[test]
    fn evidence_source_merge_matrix() {
        assert_eq!(
            RetrievalEvidenceSource::Vector.merge(RetrievalEvidenceSource::FullText),
            RetrievalEvidenceSource::Hybrid
        );
        assert_eq!(
            RetrievalEvidenceSource::Unknown.merge(RetrievalEvidenceSource::Vector),
            RetrievalEvidenceSource::Vector
        );
    }

    #[test]
    fn heuristic_search_plan_prefers_repo_for_code_navigation() {
        let plan = heuristic_search_plan("where is MemorySearchEngine defined", false, None);
        assert_eq!(plan.intent, SearchIntent::CodeNavigation);
        assert_eq!(plan.retrieval_mode, RetrievalMode::FullText);
        assert!(plan.corpora.contains(&SearchCorpus::RepoInventory));
        assert!(plan.preferred_backends.contains(&SearchBackend::RepoPath));
    }
}

#[cfg(test)]
mod semcov_wave1b_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn normalized_query_tokens_splits_lowercases_and_keeps_path_chars() {
        // ':' and whitespace and '!' split; '.', '/', '_', '-' are kept in-token; uppercase -> lowercase
        let tokens = normalized_query_tokens("Foo.Bar::Baz src/Qux_Quux-1 Hi!");
        assert_eq!(
            tokens,
            vec![
                "foo.bar".to_string(),
                "baz".to_string(),
                "src/qux_quux-1".to_string(),
                "hi".to_string(),
            ]
        );
        // empty tokens (from consecutive delimiters / leading-trailing) are dropped
        assert!(normalized_query_tokens("   ::  ").is_empty());
    }

    #[test]
    fn normalized_query_tokens_splits_lowercases_and_preserves_special_chars() {
        // ':' and ' ' are delimiters; '/', '.', '_', '-' are kept inside tokens.
        let tokens = normalized_query_tokens("Foo::bar baz/qux file.rs my_mod-A");
        assert_eq!(
            tokens,
            vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz/qux".to_string(),
                "file.rs".to_string(),
                "my_mod-a".to_string(),
            ]
        );

        // Leading/trailing/consecutive delimiters produce no empty tokens.
        assert_eq!(
            normalized_query_tokens("  a,,b  "),
            vec!["a".to_string(), "b".to_string()]
        );

        // All-delimiter input yields an empty vec.
        assert!(normalized_query_tokens("!!! ???").is_empty());
    }
}
