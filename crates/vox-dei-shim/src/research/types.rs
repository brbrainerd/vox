//! Shared types for the research pipeline. Phase 0a stub — types are real;
//! values populated by stub modules are typically empty/default.

use serde::{Deserialize, Serialize};

// Re-export verifier types used directly from `super::super::types` in stages.rs.
pub use super::verifier::{ClaimVerdict, EvidenceSpan, SpanType, Verdict};

/// Scope of a research query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchScope {
    /// Web sources only.
    Web,
    /// Local Codex only.
    Local,
    /// Web + local.
    Both,
}

/// A single research query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQuery {
    pub query: String,
    pub scope: ResearchScope,
    pub max_sources: usize,
    pub persist_to_docs: bool,
    pub verify_claims: bool,
    /// Optional site-scoped crawl restriction (domain only, no scheme).
    /// When set, `web_gather` will also call `ProviderRegistry::map_site`.
    pub site_scope: Option<String>,
}

/// A decomposed research plan: original query + N subqueries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPlan {
    pub original_query: String,
    pub subqueries: Vec<String>,
    pub scope: ResearchScope,
    pub max_sources_per_subquery: usize,
}

/// One retrieved source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchHit {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
    /// HTTP status returned when crawling this source (0 = not crawled).
    pub http_status: i32,
    /// Trust score from the provider (>=1.0 = high-trust domain).
    pub trust_score: f64,
    /// Full page content after extraction (empty = not extracted).
    pub raw_content: String,
}

/// Retrieval-stage diagnostics surfaced to the gate and to telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalDiagnostics {
    pub coverage_pct: f64,
    pub subquery_coverage_pct: f64,
    pub avg_provider_score: f64,
    pub fusion_weights: (f64, f64, f64),
    pub dropped_source_count: usize,
    pub hit_rate: f64,
}

impl Default for RetrievalDiagnostics {
    fn default() -> Self {
        Self {
            coverage_pct: 0.0,
            subquery_coverage_pct: 0.0,
            avg_provider_score: 0.0,
            fusion_weights: (0.0, 0.0, 0.0),
            dropped_source_count: 0,
            hit_rate: 0.0,
        }
    }
}

/// One citation in the final answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub source_id: i64,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub confidence: f64,
}

/// Claim-to-evidence support relation used by citation auditing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimSupport {
    pub claim_id: u64,
    pub citation_source_id: i64,
    pub quote: String,
    pub support_type: SpanType,
}

/// Post-synthesis check that citations are backed by verifier evidence spans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationAuditResult {
    pub checked_citations: usize,
    pub supported_citations: usize,
    pub unsupported_citation_indices: Vec<usize>,
    pub precision: f64,
    pub supports: Vec<ClaimSupport>,
}

/// Routing tier the gate selects per query.
///
/// **Stability guarantee:** the `Debug` representation of each variant is
/// used as a telemetry value (`format!("{:?}", routing_tier)`). Changing
/// a variant name is a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingTier {
    Direct,
    Light,
    DeepResearch,
}

/// Aggregated competence signal derived from the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetenceSignal {
    pub confidence: f32,
    pub quality: f32,
    pub verified_claim_count: usize,
    pub had_verification: bool,
}

impl CompetenceSignal {
    /// Build a competence signal from the gate's confidence score, the
    /// judge's quality score, and the verifier's per-claim verdicts.
    #[must_use]
    pub fn from_verdicts(
        confidence: f32,
        quality: i32,
        verdicts: &[ClaimVerdict],
        had_verification: bool,
    ) -> Self {
        Self {
            confidence,
            // Safe: judge_quality returns 0..=100 i32; lossless to f32.
            quality: quality as f32,
            verified_claim_count: verdicts.len(),
            had_verification,
        }
    }
}

/// Result of the CoVE-style self-verification step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfVerificationResult {
    pub checked: bool,
    pub questions_generated: usize,
    pub inconsistency_count: usize,
    pub critical_inconsistency: bool,
}

/// Cross-stage telemetry bundle attached to every result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchMetadata {
    pub session_id: i64,
    pub duration_ms: u64,
    pub provider: String,
    pub routing_tier: RoutingTier,
    pub confidence: f64,
    pub subquery_count: usize,
    pub source_count: usize,
    pub claim_verdicts: Vec<ClaimVerdict>,
    pub retrieval_diagnostics: RetrievalDiagnostics,
    /// Quality score from LLM-as-judge (0-100; i32 to match `judge_quality` return type).
    pub quality_score: i32,
    pub competence: Option<CompetenceSignal>,
    pub self_verification: Option<SelfVerificationResult>,
    pub citation_audit: Option<CitationAuditResult>,
}

/// Final research result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    pub answer: String,
    pub sources: Vec<ResearchHit>,
    pub citations: Vec<Citation>,
    pub research_metadata: ResearchMetadata,
}

/// Durable artifact persisted for CLI/MCP result retrieval and report export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRunArtifact {
    pub schema_version: u32,
    pub query: ResearchQuery,
    pub plan: ResearchPlan,
    pub result: ResearchResult,
    pub report_markdown: String,
}
