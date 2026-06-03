//! Unified search Tauri command — delegates to the `vox-search` multi-corpus planner.

use std::collections::HashSet;
use std::sync::Arc;

use vox_db::{DbConfig, VoxDb};
use vox_repository::discover_repository_or_fallback;
use vox_search::{
    RetrievalTriggerMode, SearchPolicy, SearchRuntimeContext, UnifiedHit,
    run_search_with_verification,
};

// ─── DTOs ────────────────────────────────────────────────────────────────────

/// One search hit forwarded to the frontend.
/// Field names are part of a cross-agent DTO contract — do NOT rename.
#[derive(Debug, serde::Serialize)]
pub struct UnifiedHitDto {
    pub source: String,
    pub kind: String,
    pub path: Option<String>,
    pub title: Option<String>,
    pub snippet: String,
    pub score: f64,
    pub provenance: Vec<String>,
}

/// Top-level search response forwarded to the frontend.
/// Field names are part of a cross-agent DTO contract — do NOT rename.
#[derive(Debug, serde::Serialize)]
pub struct SearchResponseDto {
    pub hits: Vec<UnifiedHitDto>,
    pub total: usize,
    /// Names of the `SearchPlan` corpora that were consulted.
    pub corpora: Vec<String>,
}

// ─── Mapping helpers ─────────────────────────────────────────────────────────

pub(crate) fn unified_hit_to_dto(h: UnifiedHit) -> UnifiedHitDto {
    UnifiedHitDto {
        source: h.source,
        kind: h.kind,
        path: h.path,
        title: h.title,
        snippet: h.snippet,
        score: h.score,
        provenance: h.provenance,
    }
}

// ─── Tauri command ────────────────────────────────────────────────────────────

/// Perform a typed multi-corpus search and return structured hits.
///
/// * `query`  — free-text search query (required, must be non-empty).
/// * `scope`  — optional allow-list of source names (e.g. `["memory","knowledge"]`).
///              When `None` or empty all hits are returned.
/// * `limit`  — max hits to request from the planner; defaults to 30.
#[tauri::command]
pub async fn vox_search_query(
    query: String,
    scope: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<SearchResponseDto, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Err("search query must not be empty".to_string());
    }

    // ── Repo discovery (mirrors memory_cli/search.rs) ────────────────────────
    let cwd = std::env::current_dir()
        .map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = discover_repository_or_fallback(&cwd);
    let repo_root = repo_ctx.root;

    // ── Memory paths ─────────────────────────────────────────────────────────
    let mem = vox_orchestrator::MemoryConfig::default();
    let log_dir = cwd.join(&mem.log_dir);
    let memory_md_path = cwd.join(&mem.memory_md_path);

    // ── DB (optional; graceful degradation if unavailable) ───────────────────
    let db: Option<Arc<VoxDb>> = match DbConfig::resolve_canonical() {
        Ok(cfg) => VoxDb::connect(cfg).await.ok().map(Arc::new),
        Err(_) => None,
    };

    // ── Build context + policy ────────────────────────────────────────────────
    let ctx = SearchRuntimeContext::new(repo_root, db, log_dir, memory_md_path);
    let policy = SearchPolicy::from_env();
    let effective_limit = limit.unwrap_or(30);

    // ── Run search ───────────────────────────────────────────────────────────
    let (exec, _diagnostics, plan) = run_search_with_verification(
        &ctx,
        &query,
        RetrievalTriggerMode::ExplicitToolQuery,
        effective_limit,
        &policy,
        None,
        None,
    )
    .await
    .map_err(|e| format!("search failed: {e}"))?;

    // ── Map corpora names from plan ───────────────────────────────────────────
    let corpora: Vec<String> = plan
        .corpora
        .iter()
        .map(|c| format!("{c:?}").to_lowercase())
        .collect();

    // ── Map hits → DTOs, then apply optional scope filter ────────────────────
    let scope_set: Option<HashSet<String>> = scope.and_then(|v| {
        if v.is_empty() {
            None
        } else {
            Some(v.into_iter().collect())
        }
    });

    let hits: Vec<UnifiedHitDto> = exec
        .unified_hits
        .into_iter()
        .filter(|h| {
            scope_set
                .as_ref()
                .map(|s| s.contains(&h.source))
                .unwrap_or(true)
        })
        .map(unified_hit_to_dto)
        .collect();

    let total = hits.len();

    Ok(SearchResponseDto {
        hits,
        total,
        corpora,
    })
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use vox_search::UnifiedHit;

    fn make_hit(source: &str, score: f64) -> UnifiedHit {
        UnifiedHit {
            source: source.to_string(),
            kind: "doc".to_string(),
            path: Some(format!("docs/{source}.md")),
            title: Some(format!("{source} title")),
            snippet: format!("snippet for {source}"),
            score,
            provenance: vec!["bm25:1".to_string()],
        }
    }

    #[test]
    fn unified_hit_to_dto_maps_all_fields() {
        let hit = make_hit("memory", 0.95);
        let dto = unified_hit_to_dto(hit.clone());
        assert_eq!(dto.source, hit.source);
        assert_eq!(dto.kind, hit.kind);
        assert_eq!(dto.path, hit.path);
        assert_eq!(dto.title, hit.title);
        assert_eq!(dto.snippet, hit.snippet);
        assert_eq!(dto.score, hit.score);
        assert_eq!(dto.provenance, hit.provenance);
    }

    #[test]
    fn scope_filter_none_passes_all() {
        let hits = vec![make_hit("memory", 0.9), make_hit("knowledge", 0.8)];
        let scope_set: Option<HashSet<String>> = None;
        let filtered: Vec<_> = hits
            .into_iter()
            .filter(|h| {
                scope_set
                    .as_ref()
                    .map(|s| s.contains(&h.source))
                    .unwrap_or(true)
            })
            .collect();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn scope_filter_restricts_by_source() {
        let hits = vec![
            make_hit("memory", 0.9),
            make_hit("knowledge", 0.8),
            make_hit("chunk", 0.7),
        ];
        let scope_set: Option<HashSet<String>> =
            Some(["memory".to_string(), "chunk".to_string()].into());
        let filtered: Vec<_> = hits
            .into_iter()
            .filter(|h| {
                scope_set
                    .as_ref()
                    .map(|s| s.contains(&h.source))
                    .unwrap_or(true)
            })
            .collect();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|h| h.source != "knowledge"));
    }

    #[test]
    fn scope_filter_empty_vec_passes_all() {
        let hits = vec![make_hit("memory", 0.9), make_hit("web", 0.5)];
        // Simulate the None path when an empty vec is supplied
        let scope_set: Option<HashSet<String>> = {
            let v: Vec<String> = vec![];
            if v.is_empty() {
                None
            } else {
                Some(v.into_iter().collect())
            }
        };
        let filtered: Vec<_> = hits
            .into_iter()
            .filter(|h| {
                scope_set
                    .as_ref()
                    .map(|s| s.contains(&h.source))
                    .unwrap_or(true)
            })
            .collect();
        assert_eq!(filtered.len(), 2);
    }
}
