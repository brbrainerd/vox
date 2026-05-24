//! Web retrieval for the research pipeline via `vox-search` policy stack.

use std::collections::HashSet;

use vox_db::Codex;
use vox_search::crag::CragRouter;
use vox_search::memory_hybrid::HybridSearchHit;
use vox_search::policy::SearchPolicy;
use vox_search::web_dispatcher::WebSearchDispatcher;
use vox_search::{
    RetrievalTriggerMode, SearchExecution, SearchRuntimeContext, run_search_with_verification,
};

use super::super::provider::ProviderRegistry;
use super::super::types::{ResearchHit, ResearchPlan, ResearchQuery, ResearchScope};

fn hybrid_from_research(hit: &ResearchHit) -> HybridSearchHit {
    HybridSearchHit {
        path: hit.url.clone(),
        title: hit.title.clone(),
        content_snippet: hit.snippet.clone(),
        score: hit.score,
        provenance: vec!["research_web_gather".to_string()],
        potential_contradiction: false,
    }
}

fn research_hit_from_hybrid(hit: HybridSearchHit) -> ResearchHit {
    ResearchHit {
        url: hit.path,
        title: hit.title,
        snippet: hit.content_snippet,
        score: hit.score,
        http_status: 0,
        trust_score: 1.0,
        raw_content: String::new(),
    }
}

fn research_hits_from_search_execution(execution: SearchExecution) -> Vec<ResearchHit> {
    let mut out = Vec::new();
    append_local_lines(&mut out, "memory", "vox://memory", execution.memory_lines);
    append_local_lines(
        &mut out,
        "knowledge",
        "vox://knowledge",
        execution.knowledge_lines,
    );
    append_local_lines(&mut out, "chunk", "vox://chunk", execution.chunk_lines);
    append_local_lines(&mut out, "repo", "repo://", execution.repo_lines);
    append_local_lines(
        &mut out,
        "tantivy",
        "repo://tantivy",
        execution.tantivy_doc_lines,
    );
    append_local_lines(&mut out, "qdrant", "vox://qdrant", execution.qdrant_lines);
    append_local_lines(&mut out, "rrf", "vox://rrf", execution.rrf_fused_lines);
    append_local_lines(
        &mut out,
        "symbol",
        "repo://symbol",
        execution.symbol_proximity_lines,
    );
    out
}

fn append_local_lines(
    out: &mut Vec<ResearchHit>,
    title_prefix: &str,
    url_prefix: &str,
    lines: Vec<String>,
) {
    for (idx, line) in lines.into_iter().enumerate() {
        let (url, title) = local_line_ref(&line, title_prefix, url_prefix, idx);
        out.push(ResearchHit {
            url,
            title,
            snippet: line,
            score: 0.65,
            http_status: 0,
            trust_score: 1.0,
            raw_content: String::new(),
        });
    }
}

fn local_line_ref(
    line: &str,
    title_prefix: &str,
    url_prefix: &str,
    idx: usize,
) -> (String, String) {
    let id = line
        .strip_prefix('[')
        .and_then(|rest| rest.split(']').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(title_prefix);
    let suffix = id
        .strip_prefix("repo:")
        .or_else(|| id.strip_prefix("node:"))
        .or_else(|| id.strip_prefix("chunk:"))
        .unwrap_or(id)
        .replace('\\', "/");
    let url = if url_prefix == "repo://" {
        format!("repo://{suffix}")
    } else {
        format!("{url_prefix}/{suffix}")
    };
    (url, format!("{title_prefix}:{idx}"))
}

/// Returns true when `url`'s host matches `site_scope` (domain only, no scheme).
fn host_matches_site_scope(url: &str, site_scope: &str) -> bool {
    let scope = site_scope
        .trim()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if scope.is_empty() {
        return true;
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower.split("://").nth(1).unwrap_or(&lower);
    let host = rest
        .split('/')
        .next()
        .unwrap_or(rest)
        .split(':')
        .next()
        .unwrap_or(rest)
        .trim_start_matches("www.");
    host == scope.as_str() || host.ends_with(&format!(".{}", scope))
}

async fn search_one_subquery(
    subquery: &str,
    policy: &SearchPolicy,
    site_scope: Option<&str>,
    seen_urls: &mut HashSet<String>,
    all_hits: &mut Vec<ResearchHit>,
) -> (usize, usize) {
    match WebSearchDispatcher::search(subquery, policy).await {
        Ok(hybrids) => {
            let attempted = hybrids.len().max(1);
            let mut accepted = 0usize;
            for h in hybrids {
                let rh = research_hit_from_hybrid(h);
                if let Some(scope) = site_scope {
                    if !host_matches_site_scope(&rh.url, scope) {
                        continue;
                    }
                }
                if seen_urls.insert(rh.url.clone()) {
                    all_hits.push(rh);
                    accepted += 1;
                }
            }
            (attempted, accepted)
        }
        Err(_) => (1usize, 0usize),
    }
}

fn avg_score(hits: &[ResearchHit]) -> f64 {
    if hits.is_empty() {
        return 0.0;
    }
    hits.iter().map(|h| h.score).sum::<f64>() / hits.len() as f64
}

pub(super) async fn gather_web_hits_for_plan(
    _db: Option<&Codex>,
    _session_id: i64,
    query: &ResearchQuery,
    plan: &ResearchPlan,
    registry: &ProviderRegistry,
    policy: &SearchPolicy,
) -> (Vec<ResearchHit>, usize, usize, usize) {
    let _ = registry;

    if matches!(query.scope, ResearchScope::Local) {
        return (Vec::new(), 0, 0, 0);
    }

    let site_scope = query
        .site_scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut all_hits: Vec<ResearchHit> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut subqueries_with_hits = 0usize;
    let mut total_sources_attempted = 0usize;

    for sq in &plan.subqueries {
        let (att, got) =
            search_one_subquery(sq, policy, site_scope, &mut seen_urls, &mut all_hits).await;
        total_sources_attempted += att;
        if got > 0 {
            subqueries_with_hits += 1;
        }
    }

    // CRAG refinement rounds (bounded by `web_search_max_hops`).
    let max_hops = policy.web_search_max_hops.max(1);
    let mut rounds_done: u8 = 0;
    while rounds_done < max_hops.saturating_sub(1) {
        let hop_remaining = max_hops.saturating_sub(rounds_done + 1);
        let quality = avg_score(&all_hits);
        if !CragRouter::should_continue(quality, 0.75, hop_remaining) {
            break;
        }

        let hybrids: Vec<HybridSearchHit> = all_hits.iter().map(hybrid_from_research).collect();
        let refined = CragRouter::expand_queries_from_partial_evidence(&query.query, &hybrids);
        if refined.is_empty() {
            break;
        }

        let mut any_new = false;
        for rq in refined {
            let (att, got) =
                search_one_subquery(&rq, policy, site_scope, &mut seen_urls, &mut all_hits).await;
            total_sources_attempted += att;
            if got > 0 {
                subqueries_with_hits += 1;
                any_new = true;
            }
        }

        if !any_new {
            break;
        }
        rounds_done += 1;
    }

    let dropped_source_count = total_sources_attempted.saturating_sub(all_hits.len());
    (
        all_hits,
        subqueries_with_hits,
        dropped_source_count,
        total_sources_attempted,
    )
}

pub(super) async fn gather_local_hits_for_plan(
    ctx: &SearchRuntimeContext,
    query: &ResearchQuery,
    plan: &ResearchPlan,
    policy: &SearchPolicy,
) -> (Vec<ResearchHit>, usize, usize, usize) {
    if matches!(query.scope, ResearchScope::Web) {
        return (Vec::new(), 0, 0, 0);
    }

    let mut all_hits: Vec<ResearchHit> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut subqueries_with_hits = 0usize;
    let mut total_sources_attempted = 0usize;

    for sq in &plan.subqueries {
        total_sources_attempted += 1;
        match run_search_with_verification(
            ctx,
            sq,
            RetrievalTriggerMode::ExplicitToolQuery,
            query.max_sources,
            policy,
            None,
            None,
        )
        .await
        {
            Ok((execution, _diagnostics, _search_plan)) => {
                let mut got = 0usize;
                for hit in research_hits_from_search_execution(execution) {
                    if seen_urls.insert(hit.url.clone()) {
                        all_hits.push(hit);
                        got += 1;
                    }
                }
                if got > 0 {
                    subqueries_with_hits += 1;
                }
            }
            Err(e) => tracing::warn!(query = %sq, error = %e, "local research retrieval failed"),
        }
    }

    let dropped_source_count = total_sources_attempted.saturating_sub(all_hits.len());
    (
        all_hits,
        subqueries_with_hits,
        dropped_source_count,
        total_sources_attempted,
    )
}

#[cfg(test)]
mod tests {
    use super::{gather_local_hits_for_plan, host_matches_site_scope};
    use crate::dei_shim::research::types::{ResearchPlan, ResearchQuery, ResearchScope};
    use vox_search::{SearchPolicy, SearchRuntimeContext};

    #[test]
    fn site_scope_filters_subdomains() {
        assert!(host_matches_site_scope(
            "https://docs.example.com/page",
            "example.com"
        ));
        assert!(!host_matches_site_scope(
            "https://evil.com/page",
            "example.com"
        ));
    }

    #[tokio::test]
    async fn local_scope_uses_search_runtime_context() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let doc = tmp.path().join("docs").join("deep-research.md");
        std::fs::create_dir_all(doc.parent().unwrap()).expect("mkdir");
        std::fs::write(&doc, "# Deep Research\nlocal retrieval evidence").expect("write doc");
        let memory_dir = tmp.path().join("memory-log");
        std::fs::create_dir_all(&memory_dir).expect("mkdir memory");
        let memory_md = tmp.path().join("MEMORY.md");
        std::fs::write(&memory_md, "deep research memory").expect("write memory");
        let ctx = SearchRuntimeContext::new(tmp.path().to_path_buf(), None, memory_dir, memory_md);
        let query = ResearchQuery {
            query: "deep research docs file".to_string(),
            scope: ResearchScope::Local,
            max_sources: 5,
            persist_to_docs: false,
            verify_claims: false,
            site_scope: None,
        };
        let plan = ResearchPlan {
            original_query: query.query.clone(),
            subqueries: vec![query.query.clone()],
            scope: ResearchScope::Local,
            max_sources_per_subquery: 5,
        };

        let policy = SearchPolicy::from_env();
        let (hits, subqueries_with_hits, _, _) =
            gather_local_hits_for_plan(&ctx, &query, &plan, &policy).await;

        assert!(subqueries_with_hits > 0);
        assert!(
            hits.iter()
                .any(|hit| hit.url.starts_with("repo://") || hit.url.starts_with("vox://"))
        );
    }
}
