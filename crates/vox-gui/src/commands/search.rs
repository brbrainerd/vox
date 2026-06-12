//! Unified search Tauri command — delegates to the `vox-search` multi-corpus planner.

use std::collections::HashMap;
use std::sync::Arc;

use vox_db::{DbConfig, SearchCorpus, SearchPlan, VoxDb, heuristic_search_plan};
use vox_repository::discover_repository_or_fallback;
use vox_search::{
    RetrievalTriggerMode, SearchPolicy, SearchRuntimeContext, UnifiedHit, execute_search_plan,
    run_search_with_verification,
};

// ─── DTOs ────────────────────────────────────────────────────────────────────

/// Locator used to open a hit from the UI.
/// Field names are part of a cross-agent DTO contract — do NOT rename.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OpenLocatorDto {
    pub kind: String, // "file" | "web" | "memory" | "none"
    pub value: String,
}

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
    pub locator: OpenLocatorDto,
}

/// A facet bucket (source or kind) with its count.
/// Field names are part of a cross-agent DTO contract — do NOT rename.
#[derive(Debug, serde::Serialize)]
pub struct FacetCount {
    pub value: String,
    pub count: u32,
}

/// Top-level search response forwarded to the frontend.
/// Field names are part of a cross-agent DTO contract — do NOT rename.
#[derive(Debug, serde::Serialize)]
pub struct SearchResponseDto {
    pub hits: Vec<UnifiedHitDto>,
    pub facets_by_source: Vec<FacetCount>,
    pub facets_by_kind: Vec<FacetCount>,
    pub total: usize,
    pub next_cursor: Option<usize>,
    /// Names of the `SearchPlan` corpora that were consulted.
    pub corpora: Vec<String>,
}

/// Outcome of an open_locator call.
#[derive(Debug, serde::Serialize)]
pub struct OpenOutcomeDto {
    pub action: String, // "spawned" | "opened"
}

// ─── Pure helpers ─────────────────────────────────────────────────────────────

/// Derive a locator from a hit's source and path.
pub(crate) fn locator_for(source: &str, path: Option<&str>) -> OpenLocatorDto {
    let value = path.unwrap_or("").to_string();
    let kind = match source {
        "web" => "web",
        "memory" | "knowledge" => "memory",
        "chunk" | "repo" => "file",
        _ => "none",
    };
    if kind != "none" && !value.is_empty() {
        OpenLocatorDto {
            kind: kind.to_string(),
            value,
        }
    } else {
        OpenLocatorDto {
            kind: "none".to_string(),
            value: String::new(),
        }
    }
}

/// Minimal wildcard glob matcher supporting `*` (any sequence, not crossing `/`)
/// and `?` (any single character). Does NOT cross path separators for `*`.
/// A bare `*` matches any sequence including separators only when the whole
/// pattern is a single `*`.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    // Use recursive DP-style matching.
    // `*` matches any sequence of non-separator chars within a segment.
    // For cross-segment matching use `**` convention isn't required here;
    // per spec `*` should NOT match `/`, so `a/*.rs` won't match `a/b/c.rs`.
    fn matches(pat: &[u8], txt: &[u8]) -> bool {
        match (pat.first(), txt.first()) {
            (None, None) => true,
            (None, _) => false,
            (Some(b'*'), _) => {
                // `*` matches zero or more non-'/' chars
                // try matching zero chars first, then consume one non-'/' char
                if matches(&pat[1..], txt) {
                    return true;
                }
                if txt.first().map(|&c| c != b'/').unwrap_or(false) {
                    return matches(pat, &txt[1..]);
                }
                false
            }
            (Some(b'?'), Some(&c)) if c != b'/' => matches(&pat[1..], &txt[1..]),
            (Some(b'?'), _) => false,
            (Some(&p), Some(&t)) if p == t => matches(&pat[1..], &txt[1..]),
            _ => false,
        }
    }
    matches(pattern.as_bytes(), text.as_bytes())
}

/// Returns the (program, args) needed to open a URL on the given OS.
pub(crate) fn url_open_command(os: &str, url: &str) -> (String, Vec<String>) {
    match os {
        "windows" => (
            "cmd".to_string(),
            vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                url.to_string(),
            ],
        ),
        "macos" => ("open".to_string(), vec![url.to_string()]),
        _ => ("xdg-open".to_string(), vec![url.to_string()]),
    }
}

/// Map a scope string to a `SearchCorpus` variant.
fn scope_to_corpus(s: &str) -> Option<SearchCorpus> {
    match s {
        "memory" => Some(SearchCorpus::Memory),
        "knowledge" => Some(SearchCorpus::KnowledgeGraph),
        "chunk" => Some(SearchCorpus::DocumentChunks),
        "repo" => Some(SearchCorpus::RepoInventory),
        "web" => Some(SearchCorpus::WebResearch),
        "symbol" => Some(SearchCorpus::SymbolProximity),
        _ => None,
    }
}

/// Aggregate facet counts sorted by count desc.
fn build_facets(hits: &[UnifiedHitDto], key: impl Fn(&UnifiedHitDto) -> &str) -> Vec<FacetCount> {
    let mut map: HashMap<String, u32> = HashMap::new();
    for h in hits {
        *map.entry(key(h).to_string()).or_insert(0) += 1;
    }
    let mut v: Vec<FacetCount> = map
        .into_iter()
        .map(|(value, count)| FacetCount { value, count })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
    v
}

// ─── Mapping helper ───────────────────────────────────────────────────────────

pub(crate) fn unified_hit_to_dto(h: UnifiedHit) -> UnifiedHitDto {
    let locator = locator_for(&h.source, h.path.as_deref());
    UnifiedHitDto {
        source: h.source,
        kind: h.kind,
        path: h.path,
        title: h.title,
        snippet: h.snippet,
        score: h.score,
        provenance: h.provenance,
        locator,
    }
}

// ─── Tauri commands ───────────────────────────────────────────────────────────

/// Perform a typed multi-corpus search and return structured hits with facets + pagination.
///
/// * `query`     — free-text search query (required, must be non-empty).
/// * `scope`     — optional corpus allow-list: `"memory"`, `"knowledge"`, `"chunk"`, `"repo"`, `"web"`.
///                 When `None` or empty, all corpora from the heuristic plan are used.
/// * `kinds`     — optional hit-kind filter (e.g. `["doc","code"]`).
/// * `path_glob` — optional glob pattern applied to `hit.path`.
/// * `limit`     — page size; defaults to 30.
/// * `offset`    — page start; defaults to 0.
#[tauri::command]
pub async fn vox_search_query(
    query: String,
    scope: Option<Vec<String>>,
    kinds: Option<Vec<String>>,
    path_glob: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<SearchResponseDto, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Err("search query must not be empty".to_string());
    }

    let off = offset.unwrap_or(0);
    let lim = limit.unwrap_or(30);
    let engine_limit = off + lim;

    // ── Repo discovery ────────────────────────────────────────────────────────
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = discover_repository_or_fallback(&cwd);
    let repo_root = repo_ctx.root;

    // ── Memory paths ──────────────────────────────────────────────────────────
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

    // ── Scope -> corpora ──────────────────────────────────────────────────────
    let scope_tags: Vec<String> = scope.clone().unwrap_or_default();
    let scope_corpora: Option<Vec<SearchCorpus>> = scope.and_then(|v| {
        if v.is_empty() {
            None
        } else {
            let mapped: Vec<SearchCorpus> = v.iter().filter_map(|s| scope_to_corpus(s)).collect();
            if mapped.is_empty() {
                None
            } else {
                Some(mapped)
            }
        }
    });

    // ── Execute search ────────────────────────────────────────────────────────
    let (exec, plan) = if let Some(corpora) = scope_corpora {
        // Build heuristic plan then override corpora.
        let mut plan: SearchPlan = heuristic_search_plan(&query, false, None);
        plan.corpora = corpora;
        let execution = execute_search_plan(&ctx, &query, &plan, engine_limit, &policy, None)
            .await
            .map_err(|e| format!("search failed: {e}"))?;
        (execution, plan)
    } else {
        let (execution, _diag, plan) = run_search_with_verification(
            &ctx,
            &query,
            RetrievalTriggerMode::ExplicitToolQuery,
            engine_limit,
            &policy,
            None,
            None,
        )
        .await
        .map_err(|e| format!("search failed: {e}"))?;
        (execution, plan)
    };

    // ── Map corpora names ─────────────────────────────────────────────────────
    let corpora: Vec<String> = plan
        .corpora
        .iter()
        .map(|c| format!("{c:?}").to_lowercase())
        .collect();

    // ── Map hits → DTOs ───────────────────────────────────────────────────────
    let kinds_set: Option<std::collections::HashSet<String>> = kinds.and_then(|v| {
        if v.is_empty() {
            None
        } else {
            Some(v.into_iter().collect())
        }
    });

    let mut all_hits: Vec<UnifiedHitDto> = exec
        .unified_hits
        .into_iter()
        .map(unified_hit_to_dto)
        .collect();

    // Chats corpus: LIKE search over GUI conversation messages when scoped.
    let wants_chats = scope_tags.is_empty() || scope_tags.iter().any(|x| x == "chats");
    if wants_chats
        && let Some(db_ref) = ctx.db.as_ref()
        && let Ok(chat_rows) = db_ref.chat_search_gui_messages(&query, lim).await
    {
        for (msg_id, _conv_id, session_id, role, snippet) in chat_rows {
            all_hits.push(UnifiedHitDto {
                source: "chats".to_string(),
                kind: "chat".to_string(),
                path: Some(session_id.clone()),
                title: Some(format!("{role} message")),
                snippet,
                score: 0.75,
                provenance: vec!["chats:like".to_string()],
                locator: OpenLocatorDto {
                    kind: "chat".to_string(),
                    value: serde_json::json!({
                        "sessionId": session_id,
                        "messageId": msg_id,
                    })
                    .to_string(),
                },
            });
        }
    }

    all_hits = all_hits
        .into_iter()
        .filter(|h| {
            // kind filter
            kinds_set
                .as_ref()
                .map(|s| s.contains(&h.kind))
                .unwrap_or(true)
        })
        .filter(|h| {
            // path_glob filter
            if let Some(pat) = &path_glob {
                h.path
                    .as_deref()
                    .map(|p| glob_match(pat, p))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    // ── Facets (over full filtered set before paging) ─────────────────────────
    let facets_by_source = build_facets(&all_hits, |h| &h.source);
    let facets_by_kind = build_facets(&all_hits, |h| &h.kind);
    let total = all_hits.len();

    // ── Pagination ────────────────────────────────────────────────────────────
    let page: Vec<UnifiedHitDto> = if off < all_hits.len() {
        all_hits.drain(off..).take(lim).collect()
    } else {
        Vec::new()
    };
    let next_cursor = if off + page.len() < total {
        Some(off + lim)
    } else {
        None
    };

    Ok(SearchResponseDto {
        hits: page,
        facets_by_source,
        facets_by_kind,
        total,
        next_cursor,
        corpora,
    })
}

/// Open a locator in the appropriate application.
#[tauri::command]
pub async fn open_locator(locator: OpenLocatorDto) -> Result<OpenOutcomeDto, String> {
    let OpenLocatorDto { kind, value } = locator;
    match kind.as_str() {
        "file" => {
            let editor = std::env::var("VOX_EDITOR").unwrap_or_else(|_| "code".into());
            std::process::Command::new(&editor)
                .arg(&value)
                .spawn()
                .map_err(|e| format!("failed to spawn editor: {e}"))?;
            Ok(OpenOutcomeDto {
                action: "spawned".to_string(),
            })
        }
        "web" => {
            let (prog, args) = url_open_command(std::env::consts::OS, &value);
            std::process::Command::new(&prog)
                .args(&args)
                .spawn()
                .map_err(|e| format!("failed to open URL: {e}"))?;
            Ok(OpenOutcomeDto {
                action: "opened".to_string(),
            })
        }
        "chat" => Ok(OpenOutcomeDto {
            action: "focus_chat".to_string(),
        }),
        "command" => Ok(OpenOutcomeDto {
            action: "focus_command".to_string(),
        }),
        _ => Err("not openable".to_string()),
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use vox_search::UnifiedHit;

    fn make_hit(source: &str, kind: &str, score: f64, path: Option<&str>) -> UnifiedHit {
        UnifiedHit {
            source: source.to_string(),
            kind: kind.to_string(),
            path: path.map(str::to_string),
            title: Some(format!("{source} title")),
            snippet: format!("snippet for {source}"),
            score,
            provenance: vec!["bm25:1".to_string()],
        }
    }

    // ── locator_for ──────────────────────────────────────────────────────────

    #[test]
    fn locator_web_source() {
        let l = locator_for("web", Some("https://example.com"));
        assert_eq!(l.kind, "web");
        assert_eq!(l.value, "https://example.com");
    }

    #[test]
    fn locator_memory_source() {
        let l = locator_for("memory", Some("notes/foo.md"));
        assert_eq!(l.kind, "memory");
    }

    #[test]
    fn locator_knowledge_source() {
        let l = locator_for("knowledge", Some("kg/node.json"));
        assert_eq!(l.kind, "memory");
    }

    #[test]
    fn locator_chunk_source() {
        let l = locator_for("chunk", Some("src/main.rs"));
        assert_eq!(l.kind, "file");
    }

    #[test]
    fn locator_repo_source() {
        let l = locator_for("repo", Some("crates/foo/lib.rs"));
        assert_eq!(l.kind, "file");
    }

    #[test]
    fn locator_unknown_source_is_none() {
        let l = locator_for("other", Some("some/path"));
        assert_eq!(l.kind, "none");
        assert_eq!(l.value, "");
    }

    #[test]
    fn locator_no_path_is_none() {
        let l = locator_for("chunk", None);
        assert_eq!(l.kind, "none");
    }

    // ── glob_match ───────────────────────────────────────────────────────────

    #[test]
    fn glob_star_matches_single_segment() {
        assert!(glob_match("a/*.rs", "a/b.rs"));
        assert!(glob_match("a/*.rs", "a/main.rs"));
    }

    #[test]
    fn glob_star_does_not_cross_separator() {
        assert!(!glob_match("a/*.rs", "a/b/c.rs"));
    }

    #[test]
    fn glob_bare_star_matches_flat() {
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("foo/bar.rs", "foo/bar.rs"));
        assert!(!glob_match("foo/bar.rs", "foo/baz.rs"));
    }

    #[test]
    fn glob_question_mark() {
        assert!(glob_match("a?.rs", "ab.rs"));
        assert!(!glob_match("a?.rs", "a/.rs"));
    }

    // ── url_open_command ─────────────────────────────────────────────────────

    #[test]
    fn url_open_windows() {
        let (prog, args) = url_open_command("windows", "https://example.com");
        assert_eq!(prog, "cmd");
        assert!(args.contains(&"/C".to_string()));
        assert!(args.contains(&"start".to_string()));
        assert!(args.contains(&"https://example.com".to_string()));
    }

    #[test]
    fn url_open_macos() {
        let (prog, args) = url_open_command("macos", "https://example.com");
        assert_eq!(prog, "open");
        assert_eq!(args, vec!["https://example.com"]);
    }

    #[test]
    fn url_open_linux() {
        let (prog, args) = url_open_command("linux", "https://example.com");
        assert_eq!(prog, "xdg-open");
        assert_eq!(args, vec!["https://example.com"]);
    }

    // ── facets ───────────────────────────────────────────────────────────────

    #[test]
    fn facets_aggregate_and_sort_by_count_desc() {
        let hits = vec![
            unified_hit_to_dto(make_hit("memory", "doc", 0.9, Some("a.md"))),
            unified_hit_to_dto(make_hit("chunk", "code", 0.8, Some("b.rs"))),
            unified_hit_to_dto(make_hit("memory", "doc", 0.7, Some("c.md"))),
        ];
        let facets = build_facets(&hits, |h| &h.source);
        assert_eq!(facets[0].value, "memory");
        assert_eq!(facets[0].count, 2);
        assert_eq!(facets[1].value, "chunk");
        assert_eq!(facets[1].count, 1);
    }

    // ── pagination ────────────────────────────────────────────────────────────

    #[test]
    fn pagination_first_page() {
        let mut hits: Vec<UnifiedHitDto> = (0..5)
            .map(|i| unified_hit_to_dto(make_hit("chunk", "doc", i as f64, Some("x.rs"))))
            .collect();
        let total = hits.len();
        let off = 0usize;
        let lim = 2usize;
        let page: Vec<_> = hits.drain(off..).take(lim).collect();
        let next_cursor = if off + page.len() < total {
            Some(off + lim)
        } else {
            None
        };
        assert_eq!(page.len(), 2);
        assert_eq!(next_cursor, Some(2));
    }

    #[test]
    fn pagination_last_page_has_no_cursor() {
        let mut hits: Vec<UnifiedHitDto> = (0..3)
            .map(|i| unified_hit_to_dto(make_hit("chunk", "doc", i as f64, Some("x.rs"))))
            .collect();
        let total = hits.len();
        let off = 2usize;
        let lim = 5usize;
        let page: Vec<_> = if off < hits.len() {
            hits.drain(off..).take(lim).collect()
        } else {
            vec![]
        };
        let next_cursor = if off + page.len() < total {
            Some(off + lim)
        } else {
            None
        };
        assert_eq!(page.len(), 1);
        assert_eq!(next_cursor, None);
    }

    // ── legacy mapping test (kept for regression) ─────────────────────────────

    #[test]
    fn unified_hit_to_dto_maps_all_fields() {
        let hit = make_hit("memory", "doc", 0.95, Some("docs/memory.md"));
        let dto = unified_hit_to_dto(hit.clone());
        assert_eq!(dto.source, hit.source);
        assert_eq!(dto.kind, hit.kind);
        assert_eq!(dto.path, hit.path);
        assert_eq!(dto.title, hit.title);
        assert_eq!(dto.snippet, hit.snippet);
        assert_eq!(dto.score, hit.score);
        assert_eq!(dto.provenance, hit.provenance);
        assert_eq!(dto.locator.kind, "memory");
    }
}
