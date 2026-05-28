//! Core search execution (shared by MCP, orchestrator, CLI).

use std::path::Path;

use walkdir::WalkDir;

use vox_db::{
    RetrievalEvidenceSource, RetrievalMode, RetrievalResult, SearchBackend, SearchCorpus,
    SearchDiagnostics, SearchPlan, SearchRefinementAction,
};

use crate::context::SearchRuntimeContext;
use crate::embedding_env::embedding_config_from_env;
use crate::embeddings::EmbeddingService;
use crate::memory_cache::cached_memory_engine;
use crate::memory_hybrid::HybridSearchHit;
use crate::policy::SearchPolicy;

/// Substring scan fallback for markdown memory when BM25 returns empty.
pub trait LexicalMemoryFallback: Send + Sync {
    /// Human-readable lines (same shape MCP used from `MemoryManager::search`).
    fn substring_search_lines(&self, query: &str) -> Result<Vec<String>, String>;
}

/// A securely persisted retrieval artifact replacing inline context.
#[derive(Debug, Clone)]
pub struct DurableArtifact {
    pub uri: String,
    pub token: Option<String>,
    pub expires_at_unix_ms: Option<u64>,
    pub chunk_count: usize,
}

/// One retrieval pass output before MCP/orchestrator envelope wrapping.
#[derive(Debug, Clone)]
pub struct SearchExecution {
    pub memory_lines: Vec<String>,
    pub knowledge_lines: Vec<String>,
    pub chunk_lines: Vec<String>,
    pub repo_lines: Vec<String>,
    /// Optional Tantivy-backed doc mirror hits (empty when feature or index disabled).
    pub tantivy_doc_lines: Vec<String>,
    /// Optional Qdrant ANN sidecar hits (`VOX_SEARCH_QDRANT_URL` + `qdrant-vector` feature).
    pub qdrant_lines: Vec<String>,
    /// Cross-corpus RRF ordering when [`SearchPolicy::prefer_rrf_merge`] is set (≥2 non-empty lists).
    pub rrf_fused_lines: Vec<String>,
    /// Optional web-retrieval hits (SearXNG, DuckDuckGo, or Tavily).
    pub web_lines: Vec<String>,
    /// Candidates from semantic proximity scanning (split-brain detection).
    pub symbol_proximity_lines: Vec<String>,
    /// Secure references to large evidence bodies, replacing inline text for high-volume results.
    pub durable_artifacts: Vec<DurableArtifact>,
    /// Non-fatal issues (e.g. Qdrant HTTP errors) copied into [`SearchDiagnostics::notes`].
    pub warnings: Vec<String>,
    pub used_vector: bool,
    pub used_bm25: bool,
    pub lexical_fallback_used: bool,
    pub contradiction_count: usize,
    pub top_score: Option<f64>,
    pub backend_mix: Vec<SearchBackend>,
    pub source_diversity: usize,
    pub evidence_quality: f64,
    pub citation_coverage: f64,
    pub recommended_next_action: Option<SearchRefinementAction>,
}

/// Distinct corpus buckets that can contribute evidence (aligned with `execute_search_plan` corpora).
const SEARCH_CORPUS_BUCKET_COUNT: f64 = 8.0;

fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn query_tokens(query: &str) -> Vec<String> {
    normalize_query(query)
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '/' && c != '.')
        .filter(|t| !t.is_empty())
        .map(std::string::ToString::to_string)
        .collect()
}

fn query_looks_like_code_navigation(query: &str) -> bool {
    let q = query.to_ascii_lowercase();
    q.contains("file")
        || q.contains("path")
        || q.contains("module")
        || q.contains("symbol")
        || q.contains("crate")
        || q.contains("function")
        || q.contains("struct")
        || q.contains("enum")
        || q.contains("trait")
        || q.contains("impl")
        || q.contains(".rs")
        || q.contains("src/")
        || q.contains("crates/")
        || q.contains("::")
}

fn memory_hit_flags(hits: &[HybridSearchHit]) -> (bool, bool, usize, Option<f64>) {
    let mut used_vector = false;
    let mut used_bm25 = false;
    let mut contradictions = 0usize;
    let mut top_score = None;
    for (idx, hit) in hits.iter().enumerate() {
        if idx == 0 {
            top_score = Some(hit.score);
        }
        if hit.potential_contradiction {
            contradictions += 1;
        }
        for prov in &hit.provenance {
            let p = prov.to_ascii_lowercase();
            if p.contains("evidence:vector") || p.contains("evidence:hybrid") {
                used_vector = true;
            }
            if p.contains("evidence:fulltext")
                || p.contains("evidence:hybrid")
                || p.contains("bm25:")
            {
                used_bm25 = true;
            }
        }
    }
    (used_vector, used_bm25, contradictions, top_score)
}

/// Repository path inventory ranked by token overlap (until persistent indexes land).
pub fn repo_path_search(
    repo_root: &Path,
    query: &str,
    limit: usize,
    policy: &SearchPolicy,
) -> Vec<RetrievalResult> {
    let tokens = query_tokens(query);
    if tokens.is_empty() {
        return Vec::new();
    }
    let skip: std::collections::HashSet<&str> = policy
        .repo_inventory_skip_dirs
        .iter()
        .map(|s| s.as_str())
        .collect();
    let mut out = Vec::new();
    for entry in WalkDir::new(repo_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !skip.contains(name.as_ref())
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .take(policy.repo_inventory_max_files)
    {
        let rel = entry
            .path()
            .strip_prefix(repo_root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let rel_lower = rel.to_ascii_lowercase();
        let score = tokens.iter().fold(0.0_f32, |acc, token| {
            if rel_lower.contains(token) {
                let bonus =
                    if rel_lower.ends_with(token) || rel_lower.contains(&format!("/{token}")) {
                        1.3_f32
                    } else {
                        1.0_f32
                    };
                acc + bonus
            } else {
                acc
            }
        });
        if score > 0.0 {
            out.push(RetrievalResult {
                chunk_id: rel.clone(),
                source: rel.clone(),
                score,
                snippet: rel.clone(),
                evidence_source: RetrievalEvidenceSource::FullText,
                retrieved_at_ms: None,
                query_id: Some(format!("repo-path:{query}")),
                supporting_claim_ids: Vec::new(),
                contradiction_hints: Vec::new(),
            });
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(limit);
    out
}

#[cfg(feature = "tantivy-lexical")]
fn tantivy_supplemental_lines(
    _ctx: &SearchRuntimeContext,
    policy: &SearchPolicy,
    query: &str,
    limit: usize,
) -> Vec<String> {
    let Some(root) = policy.tantivy_index_root.as_ref() else {
        return Vec::new();
    };
    let index_dir = root.join("docs");
    match crate::lexical_tantivy::TantivyDocsIndex::open(&index_dir) {
        Ok(idx) => idx
            .search(query, limit)
            .unwrap_or_default()
            .into_iter()
            .map(|hit| {
                format!(
                    "[tantivy:{} score:{:.3}] {}",
                    hit.path,
                    hit.score,
                    hit.snippet.replace('\n', " ")
                )
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(not(feature = "tantivy-lexical"))]
fn tantivy_supplemental_lines(
    ctx: &SearchRuntimeContext,
    policy: &SearchPolicy,
    query: &str,
    limit: usize,
) -> Vec<String> {
    let _ = (
        std::hint::black_box(ctx as *const _ as usize),
        std::hint::black_box(policy as *const _ as usize),
        std::hint::black_box(query.len()),
        std::hint::black_box(limit),
    );
    Vec::new()
}

/// Execute a concrete [`SearchPlan`] against `VoxDb` (when attached) + local memory paths.
pub async fn execute_search_plan(
    ctx: &SearchRuntimeContext,
    query: &str,
    plan: &SearchPlan,
    limit: usize,
    policy: &SearchPolicy,
    lexical_fallback: Option<&dyn LexicalMemoryFallback>,
) -> Result<SearchExecution, String> {
    if let Some(t) = ctx
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        tracing::info!(
            target: "vox_search::trace",
            trace_id = %t,
            "execute_search_plan"
        );
    }
    let engine = cached_memory_engine(ctx, policy).await;
    let mut warnings: Vec<String> = Vec::new();

    let mut lexical_fallback_used = false;
    let mut used_vector = false;
    let mut used_bm25 = false;
    let mut contradiction_count = 0usize;
    let mut top_score = None;
    let mut backend_mix: Vec<SearchBackend> = Vec::new();

    let db_opt = ctx.db.clone();
    let embedder = db_opt.as_ref().and_then(|db| {
        embedding_config_from_env().map(|llm_cfg| EmbeddingService::new(db.clone(), llm_cfg))
    });
    let query_vector = if matches!(
        plan.retrieval_mode,
        RetrievalMode::Vector | RetrievalMode::Hybrid
    ) {
        if let Some(service) = embedder.as_ref() {
            service.embed_query(query).await.ok()
        } else {
            None
        }
    } else {
        None
    };

    let fusion_w = policy.clamped_memory_vector_weight();

    let memory_lines = if plan.corpora.contains(&SearchCorpus::Memory) {
        let hybrid_hits = if let Some(db) = db_opt.as_ref() {
            let engine = engine.with_db(db.clone());
            let embedder = if matches!(plan.retrieval_mode, RetrievalMode::FullText) {
                None
            } else {
                embedder.as_ref()
            };
            engine.hybrid_search(query, limit, embedder, fusion_w).await
        } else {
            let embedder = if matches!(plan.retrieval_mode, RetrievalMode::FullText) {
                None
            } else {
                embedder.as_ref()
            };
            engine.hybrid_search(query, limit, embedder, fusion_w).await
        };
        if hybrid_hits.is_empty() {
            lexical_fallback_used = true;
            if let Some(fb) = lexical_fallback {
                let lines = fb.substring_search_lines(query)?;
                if !lines.is_empty() {
                    backend_mix.push(SearchBackend::LexicalFallback);
                }
                lines
            } else {
                Vec::new()
            }
        } else {
            let (memory_used_vector, memory_used_bm25, memory_contradictions, memory_top_score) =
                memory_hit_flags(&hybrid_hits);
            used_vector |= memory_used_vector;
            used_bm25 |= memory_used_bm25;
            contradiction_count += memory_contradictions;
            top_score = top_score.or(memory_top_score);
            if memory_used_bm25 {
                backend_mix.push(SearchBackend::MemoryBm25);
            }
            if memory_used_vector {
                backend_mix.push(SearchBackend::MemoryVector);
            }
            hybrid_hits
                .into_iter()
                .map(|h| {
                    format!(
                        "[{}] {} (score {:.3}; provenance: {}; contradiction: {})",
                        h.path,
                        h.content_snippet.replace('\n', " "),
                        h.score,
                        h.provenance.join(", "),
                        h.potential_contradiction
                    )
                })
                .collect()
        }
    } else {
        Vec::new()
    };

    let knowledge_lines = if let Some(db) = db_opt.as_ref() {
        if plan.corpora.contains(&SearchCorpus::KnowledgeGraph) {
            let rows = db
                .query_knowledge_nodes(query, limit as i64)
                .await
                .map_err(|e| e.to_string())?;
            if !rows.is_empty() {
                backend_mix.push(SearchBackend::KnowledgeFts);
                used_bm25 = true;
            }
            rows.into_iter()
                .map(|(id, label, snippet)| {
                    let snip = snippet.replace('\n', " ");
                    format!("[node:{id}] {label} — {snip}")
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let (chunk_lines, chunk_diagnostics) = if let Some(db) = db_opt.as_ref() {
        if plan.corpora.contains(&SearchCorpus::DocumentChunks) {
            let (rows, diagnostics) = db
                .query_search_document_chunks_hybrid(
                    query,
                    query_vector.as_deref(),
                    limit as i64,
                    policy.clamped_chunk_vector_weight(),
                )
                .await
                .map_err(|e| e.to_string())?;
            if diagnostics.backends_used.contains(&SearchBackend::ChunkFts) {
                used_bm25 = true;
            }
            if diagnostics
                .backends_used
                .contains(&SearchBackend::ChunkVector)
            {
                used_vector = true;
            }
            let lines = rows
                .into_iter()
                .map(|hit| {
                    format!(
                        "[chunk:{} title:{}] {} (score {:.3}; provenance: {:?})",
                        hit.chunk_id,
                        hit.source,
                        hit.snippet.replace('\n', " "),
                        hit.score,
                        hit.evidence_source
                    )
                })
                .collect::<Vec<_>>();
            (lines, diagnostics)
        } else {
            (Vec::new(), SearchDiagnostics::default())
        }
    } else {
        (Vec::new(), SearchDiagnostics::default())
    };
    for backend in &chunk_diagnostics.backends_used {
        if !backend_mix.contains(backend) {
            backend_mix.push(*backend);
        }
    }
    if top_score.is_none() {
        top_score = chunk_diagnostics
            .verified_top_score
            .or(chunk_diagnostics.initial_top_score);
    }

    let repo_hits = if plan.corpora.contains(&SearchCorpus::RepoInventory) {
        repo_path_search(&ctx.repo_root, query, limit, policy)
    } else {
        Vec::new()
    };
    if !repo_hits.is_empty() {
        backend_mix.push(SearchBackend::RepoPath);
        used_bm25 = true;
        if top_score.is_none() {
            top_score = repo_hits.first().map(|h| f64::from(h.score));
        }
    }
    let repo_lines = repo_hits
        .into_iter()
        .map(|hit| format!("[repo:{}] {}", hit.chunk_id, hit.snippet))
        .collect::<Vec<_>>();

    let tantivy_doc_lines = tantivy_supplemental_lines(ctx, policy, query, limit);

    let qdrant_lines: Vec<String> = {
        #[cfg(feature = "qdrant-vector")]
        {
            let mut lines = Vec::new();
            if let Some(url) = policy.qdrant_url.as_deref().filter(|u| !u.is_empty())
                && plan.corpora.contains(&SearchCorpus::DocumentChunks)
                && let Some(ref qv) = query_vector
                && !qv.is_empty()
            {
                let client = crate::vector_qdrant::QdrantSemanticClient::new(
                    url,
                    policy.qdrant_collection.as_str(),
                );
                let trace = ctx
                    .trace_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                match client
                    .search_vectors(
                        qv.as_slice(),
                        limit,
                        policy.qdrant_vector_name.as_deref(),
                        trace,
                    )
                    .await
                {
                    Ok(hits) if !hits.is_empty() => {
                        if !backend_mix.contains(&SearchBackend::QdrantVector) {
                            backend_mix.push(SearchBackend::QdrantVector);
                        }
                        used_vector = true;
                        if top_score.is_none() {
                            top_score = hits.first().map(|(_, s, _)| f64::from(*s));
                        }
                        for (id, sc, snip) in hits {
                            let tail = snip.as_deref().unwrap_or("sidecar_ann").replace('\n', " ");
                            lines.push(format!("[qdrant:{id} score:{sc:.4}] {tail}"));
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warnings.push(format!("qdrant_sidecar_failed:{e}"));
                        tracing::debug!(
                            target: "vox_search::qdrant",
                            error = %e,
                            "Qdrant sidecar search skipped"
                        );
                    }
                }
            }
            lines
        }
        #[cfg(not(feature = "qdrant-vector"))]
        {
            Vec::new()
        }
    };

    let web_lines = if plan.corpora.contains(&SearchCorpus::WebResearch) {
        match crate::web_dispatcher::WebSearchDispatcher::search(query, policy).await {
            Ok(hits) => {
                if policy.persist_web_hits
                    && let Some(db) = db_opt.as_ref()
                {
                    for h in &hits {
                        let db_arc = db.clone();
                        let engine_name = h
                            .provenance
                            .iter()
                            .find_map(|p| p.strip_prefix("engine:"))
                            .unwrap_or("unknown")
                            .to_string();
                        let source_uri = format!("web-ingest:{engine_name}:{}", h.path);
                        let title = h.title.clone();
                        let body = h.content_snippet.clone();
                        tokio::spawn(async move {
                            let _ = crate::ingest::persist_text_document_chunk(
                                db_arc.as_ref(),
                                &source_uri,
                                &title,
                                &body,
                                "text/plain",
                            )
                            .await;
                        });
                    }
                }
                if !hits.is_empty() {
                    backend_mix.push(SearchBackend::Web);
                    if top_score.is_none() {
                        top_score = hits.first().map(|h| h.score);
                    }
                }
                hits.into_iter()
                    .map(|h| {
                        let engine = h
                            .provenance
                            .iter()
                            .find_map(|p| p.strip_prefix("engine:"))
                            .unwrap_or("unknown");
                        format!(
                            "[web:{}] {} (score {:.3}; engine: {})",
                            h.path,
                            h.content_snippet.replace('\n', " "),
                            h.score,
                            engine
                        )
                    })
                    .collect()
            }
            Err(e) => {
                warnings.push(format!("web_search_failed:{}", e));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let symbol_proximity_lines = if plan.corpora.contains(&SearchCorpus::SymbolProximity) {
        let hits = crate::symbol_proximity::scan_symbol_proximity(
            ctx,
            query,
            policy,
            query_vector.as_deref(),
        )
        .await;
        if !hits.is_empty() {
            backend_mix.push(SearchBackend::SymbolProximity);
        }
        hits
    } else {
        Vec::new()
    };

    let rrf_fused_lines: Vec<String> = if policy.prefer_rrf_merge {
        let lists = vec![
            memory_lines.clone(),
            knowledge_lines.clone(),
            chunk_lines.clone(),
            repo_lines.clone(),
            tantivy_doc_lines.clone(),
            qdrant_lines.clone(),
            web_lines.clone(),
            symbol_proximity_lines.clone(),
        ];
        let non_empty: Vec<_> = lists.into_iter().filter(|v| !v.is_empty()).collect();
        if non_empty.len() >= 2 {
            crate::rrf::rrf_merge_line_lists(
                &non_empty,
                limit.saturating_mul(2).max(8),
                policy.clamped_rrf_k(),
            )
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let effective_diversity = usize::from(!memory_lines.is_empty())
        + usize::from(!knowledge_lines.is_empty())
        + usize::from(!chunk_lines.is_empty())
        + usize::from(!repo_lines.is_empty())
        + usize::from(!tantivy_doc_lines.is_empty())
        + usize::from(!qdrant_lines.is_empty())
        + usize::from(!web_lines.is_empty())
        + usize::from(!symbol_proximity_lines.is_empty());

    let evidence_total = memory_lines.len()
        + knowledge_lines.len()
        + chunk_lines.len()
        + repo_lines.len()
        + tantivy_doc_lines.len()
        + qdrant_lines.len()
        + web_lines.len()
        + symbol_proximity_lines.len();

    let citation_coverage = if evidence_total == 0 {
        0.0
    } else {
        (effective_diversity as f64 / SEARCH_CORPUS_BUCKET_COUNT).clamp(0.0, 1.0)
    };
    let evidence_quality = if evidence_total == 0 {
        0.0
    } else {
        let top = top_score.unwrap_or(0.0).clamp(0.0, 1.0);
        ((top * policy.evidence_quality_top_weight)
            + (citation_coverage * policy.evidence_quality_coverage_weight))
            .clamp(0.0, 1.0)
    };
    let recommended_next_action = if contradiction_count > 0 {
        Some(SearchRefinementAction::AskUser)
    } else if evidence_total == 0 {
        Some(SearchRefinementAction::BroadenScope)
    } else if effective_diversity <= 1 && query_looks_like_code_navigation(query) {
        Some(SearchRefinementAction::FocusRepo)
    } else if effective_diversity <= 1 {
        Some(SearchRefinementAction::FocusCodex)
    } else if lexical_fallback_used {
        Some(SearchRefinementAction::RetryHybrid)
    } else {
        None
    };

    Ok(SearchExecution {
        memory_lines,
        knowledge_lines,
        chunk_lines,
        repo_lines,
        tantivy_doc_lines,
        qdrant_lines,
        rrf_fused_lines,
        web_lines,
        symbol_proximity_lines,
        durable_artifacts: Vec::new(),
        warnings,
        used_vector,
        used_bm25,
        lexical_fallback_used,
        contradiction_count,
        top_score,
        backend_mix,
        source_diversity: effective_diversity,
        evidence_quality,
        citation_coverage,
        recommended_next_action,
    })
}

pub(crate) fn best_effort_verification_query(query: &str) -> Option<String> {
    let tokens = query_tokens(query);
    let filtered: Vec<String> = tokens
        .into_iter()
        .filter(|t| t.len() > 2)
        .filter(|t| {
            !matches!(
                t.as_str(),
                "the"
                    | "and"
                    | "for"
                    | "with"
                    | "from"
                    | "into"
                    | "what"
                    | "where"
                    | "when"
                    | "does"
                    | "how"
                    | "why"
            )
        })
        .collect();
    if filtered.is_empty() {
        None
    } else {
        Some(filtered.join(" "))
    }
}
