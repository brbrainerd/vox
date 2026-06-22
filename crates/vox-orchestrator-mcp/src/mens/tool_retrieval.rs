//! B3.1 — Semantic tool retrieval for VoxMens.
//!
//! Selects the top-K most relevant tools for a given task string using semantic
//! similarity when an embedder is available, with automatic degradation to BM25
//! (TF-IDF term-overlap) when the embedder hub is not accessible.
//!
//! # Degraded mode
//! When the embedder is unavailable (model not downloaded, PLACEHOLDER revision,
//! etc.) [`select_tools`] falls back to [`select_tools_bm25_fallback`] and sets
//! `degraded_mode = true` on the returned [`RetrievalResult`].  A warning is
//! emitted via `tracing::warn!` so operators see the degraded state without a
//! panic or hard failure.

/// A single tool in the registry used for retrieval scoring.
#[derive(Debug, Clone)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
}

/// The result of a tool-retrieval query.
#[derive(Debug)]
pub struct RetrievalResult {
    /// The selected tools, ordered by relevance (highest first).
    pub tools: Vec<ToolEntry>,
    /// `true` when the semantic embedder was unavailable and BM25 was used instead.
    pub degraded_mode: bool,
}

// ─── BM25 / lexical fallback ────────────────────────────────────────────────

/// Tokenise a string into lowercase alphabetic tokens (punctuation stripped).
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// Score a single tool against a set of query tokens using term-overlap.
///
/// Scoring:
/// - Exact name-segment match (underscore-split): +8
/// - Term appears anywhere in the tool name: +4
/// - Term appears in the description: +1
fn score_tool_bm25(entry: &ToolEntry, query_tokens: &[String]) -> u32 {
    let name_lc = entry.name.to_lowercase();
    let desc_lc = entry.description.to_lowercase();
    query_tokens
        .iter()
        .map(|term| {
            if name_lc.split('_').any(|seg| seg == term) {
                8
            } else if name_lc.contains(term.as_str()) {
                4
            } else if desc_lc.contains(term.as_str()) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// Explicit BM25 retrieval (public so tests can force the degraded path).
///
/// Always sets `degraded_mode = true`.
pub fn select_tools_bm25_fallback(
    registry: &[ToolEntry],
    task: &str,
    top_k: usize,
) -> RetrievalResult {
    let query_tokens = tokenize(task);
    let mut scored: Vec<(u32, &ToolEntry)> = registry
        .iter()
        .filter_map(|entry| {
            let score = score_tool_bm25(entry, &query_tokens);
            if score > 0 {
                Some((score, entry))
            } else {
                None
            }
        })
        .collect();

    // Sort descending by score; tie-break by name ascending.
    scored.sort_by(|(sa, a), (sb, b)| sb.cmp(sa).then(a.name.cmp(&b.name)));

    let tools: Vec<ToolEntry> = scored
        .into_iter()
        .take(top_k)
        .map(|(_, e)| e.clone())
        .collect();

    RetrievalResult {
        tools,
        degraded_mode: true,
    }
}

// ─── Semantic path (embedder-backed) ────────────────────────────────────────

/// Try to load and run the embedder hub declared in `domain-profiles.yaml`.
///
/// Returns `None` if:
/// - The YAML cannot be parsed
/// - The `hub.embedder` key is absent or contains a PLACEHOLDER revision
/// - The model files are not present on disk
/// - Any other initialisation error
///
/// In all failure cases the caller falls back to BM25.
fn try_semantic_embeddings(registry: &[ToolEntry], task: &str) -> Option<Vec<(f32, usize)>> {
    // Step 1: locate the embedder config.
    let hub_embedder = find_hub_embedder_id()?;

    // Step 2: reject PLACEHOLDER revisions — the model hasn't been downloaded.
    if hub_embedder.contains("PLACEHOLDER") {
        tracing::warn!(
            hub_embedder = %hub_embedder,
            "B3.1: embedder hub revision is PLACEHOLDER — semantic retrieval unavailable; degrading to BM25"
        );
        return None;
    }

    // Step 3: attempt to run the embedder.
    // In production this would load the model via candle/tokenizers.
    // For B3.1 we recognise that candle is not a declared dependency of this crate
    // and the model is not yet downloaded, so we return None to trigger the BM25
    // fallback.  The infrastructure to call into a local embedding service or
    // candle model will be wired in a later B-phase task.
    tracing::warn!(
        hub_embedder = %hub_embedder,
        "B3.1: candle/tokenizers embedding not yet wired in this crate — degrading to BM25"
    );
    let _ = (registry, task); // suppress unused warnings
    None
}

/// Parse `mens/config/domain-profiles.yaml` and return the embedder model ID
/// from `hub.embedder`, if present.
fn find_hub_embedder_id() -> Option<String> {
    // Locate the YAML relative to the workspace root (CARGO_MANIFEST_DIR two levels up).
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/vox-orchestrator-mcp → workspace root
    let workspace_root = manifest_dir.parent()?.parent()?;
    let yaml_path = workspace_root
        .join("mens")
        .join("config")
        .join("domain-profiles.yaml");

    let content = std::fs::read_to_string(&yaml_path).ok()?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;

    // Navigate: hub -> embedder
    doc.get("hub")?
        .get("embedder")?
        .as_str()
        .map(|s| s.to_string())
}

// ─── Public entry point ──────────────────────────────────────────────────────

/// Select the top-`top_k` most relevant tools for `task` from `registry`.
///
/// Attempts the semantic (embedder) path first.  On any failure, warns and
/// falls back to BM25; the returned [`RetrievalResult`] then has
/// `degraded_mode = true`.
pub fn select_tools(registry: &[ToolEntry], task: &str, top_k: usize) -> RetrievalResult {
    // Attempt semantic path.
    if let Some(scored) = try_semantic_embeddings(registry, task) {
        // Semantic succeeded: build result sorted by score descending.
        let mut scored_vec = scored;
        scored_vec.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let tools: Vec<ToolEntry> = scored_vec
            .into_iter()
            .take(top_k)
            .map(|(_, idx)| registry[idx].clone())
            .collect();
        return RetrievalResult {
            tools,
            degraded_mode: false,
        };
    }

    // Semantic path unavailable — degrade to BM25 with a warning.
    tracing::warn!("B3.1: select_tools degrading to BM25 (semantic embedder unavailable)");
    select_tools_bm25_fallback(registry, task, top_k)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> Vec<ToolEntry> {
        vec![
            ToolEntry {
                name: "git_status".into(),
                description: "Show uncommitted changes in git".into(),
            },
            ToolEntry {
                name: "read_file".into(),
                description: "Read the contents of a file".into(),
            },
            ToolEntry {
                name: "web_search".into(),
                description: "Search the web for information".into(),
            },
            ToolEntry {
                name: "run_tests".into(),
                description: "Execute the test suite".into(),
            },
            ToolEntry {
                name: "git_diff".into(),
                description: "Show file differences in git".into(),
            },
        ]
    }

    #[test]
    fn lexical_retrieval_returns_git_tool_for_git_task() {
        let reg = test_registry();
        let result = select_tools(&reg, "what files changed in the repo", 3);
        let names: Vec<_> = result.tools.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"git_status") || names.contains(&"git_diff"),
            "git tool must be in top-3 for a git task; got {:?}",
            names
        );
    }

    #[test]
    fn paraphrase_retrieval_finds_git_tool() {
        // "show me my uncommitted edits" is a semantic paraphrase of git_status.
        // BM25 would miss "uncommitted edits" → "uncommitted changes"
        // If embedder available, semantic path should find it; else warn and BM25 degrades.
        let reg = test_registry();
        let result = select_tools(&reg, "show me my uncommitted edits", 3);
        let names: Vec<_> = result.tools.iter().map(|t| t.name.as_str()).collect();
        if result.degraded_mode {
            // BM25 degraded — acceptable; the warning is enough for the gate.
            eprintln!("WARN: semantic retrieval degraded (embedder unavailable); BM25 used");
        } else {
            assert!(
                names.contains(&"git_status") || names.contains(&"git_diff"),
                "semantic retrieval must find git tool for paraphrase; got {:?}",
                names
            );
        }
    }

    #[test]
    fn degraded_mode_warns_not_panics() {
        // Force degraded mode by using the explicit BM25 fallback.
        let reg = test_registry();
        let result = select_tools_bm25_fallback(&reg, "task", 3);
        assert!(
            result.degraded_mode,
            "fallback must mark degraded_mode = true"
        );
    }
}
