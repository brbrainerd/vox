//! Hybrid BM25 + optional vector search over markdown memory docs persisted via `vox-db`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use vox_db::{RetrievalEvidenceSource, RetrievalResult, VoxDb, fuse_hybrid_results};

use crate::embeddings::EmbeddingService;

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Matches from the search engine.
#[derive(Debug, Clone)]
pub struct HybridSearchHit {
    /// Repository-relative or virtual path of the hit.
    pub path: String,
    /// Document title parsed from front matter or first heading.
    pub title: String,
    /// Short excerpt around the best matching span.
    pub content_snippet: String,
    /// Combined relevance score (higher is better; BM25 + optional vector weight).
    pub score: f64,
    /// How this hit was produced (`bm25`, `vector`, fusion notes).
    pub provenance: Vec<String>,
    /// Heuristic: multiple strong sources disagree on the same entity (best-effort).
    pub potential_contradiction: bool,
}

/// Tokenize text into alphanumeric lowercase words.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Latest commit unix time (seconds) per repo-relative path (`/` separators), from one `git log` walk.
pub type GitMtimeMap = HashMap<String, u64>;

fn parse_git_log_ct_name_only(stdout: &str) -> GitMtimeMap {
    let mut map = GitMtimeMap::new();
    let mut cur_ts: Option<u64> = None;
    for line in stdout.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.chars().all(|c| c.is_ascii_digit()) {
            cur_ts = t.parse().ok();
        } else if let Some(ts) = cur_ts {
            map.entry(t.replace('\\', "/")).or_insert(ts);
        }
    }
    map
}

/// One subprocess: recent commit timestamps for paths under `under` (relative to `repo_root`).
#[must_use]
pub fn git_latest_mtime_map(repo_root: &Path, under: &Path) -> Option<GitMtimeMap> {
    let rel = under
        .strip_prefix(repo_root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let output = std::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(repo_root)
        .args([
            "log",
            "--pretty=format:%ct",
            "--name-only",
            "--no-renames",
            "--",
            rel.trim_end_matches('/'),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_git_log_ct_name_only(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

#[derive(Clone)]
struct IndexedDocument {
    path: String,
    title: String,
    content: String,
    term_freq: HashMap<String, usize>,
    length: usize,
    status: String,
    last_updated_unix: u64,
}

fn parse_frontmatter_meta(content: &str) -> (String, Option<String>) {
    let mut status = "current".to_string();
    let mut last_updated = None;
    if let Some(after) = content.strip_prefix("---\n")
        && let Some(end) = after.find("\n---")
    {
        let yaml = &after[..end];
        for line in yaml.lines() {
            let line = line.trim();
            if let Some(st) = line.strip_prefix("status:") {
                status = st
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
            } else if let Some(lu) = line.strip_prefix("last_updated:") {
                let raw = lu
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
                if !raw.is_empty() {
                    last_updated = Some(raw);
                }
            }
        }
    }
    (status, last_updated)
}

fn get_git_last_updated_unix(path: &Path, cwd: Option<&Path>) -> u64 {
    let mut cmd = std::process::// vox-arch-check: allow git-exec
        Command::new("git");
    if let Some(root) = cwd {
        cmd.current_dir(root);
    }
    let output = cmd
        .args(["log", "-1", "--format=%ct", "--"])
        .arg(path)
        .output();

    if let Ok(out) = output
        && out.status.success()
    {
        let date_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(unix) = date_str.parse::<u64>() {
            return unix;
        }
    }
    0
}

fn resolve_last_updated_unix(path: &Path, repo_root: &Path, git_map: Option<&GitMtimeMap>) -> u64 {
    if let Some(map) = git_map
        && let Ok(rel) = path.strip_prefix(repo_root)
    {
        let key = rel.to_string_lossy().replace('\\', "/");
        if let Some(&ts) = map.get(&key) {
            return ts;
        }
    }
    get_git_last_updated_unix(path, Some(repo_root))
}

/// Search engine combining local file BM25 and DB vector search.
#[derive(Clone)]
pub struct MemorySearchEngine {
    docs: Vec<IndexedDocument>,
    /// Path (as indexed) → position in `docs` for O(1) lookup during hybrid merge.
    doc_index_by_path: HashMap<String, usize>,
    avg_doc_len: f64,
    df: HashMap<String, usize>, // Document frequency
    total_docs: usize,
    /// DB for vector searches (schema V7 `embeddings` or similar table).
    db: Option<Arc<VoxDb>>,
    bm25_k1: f64,
    bm25_b: f64,
}

impl Default for MemorySearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySearchEngine {
    /// Empty index with no database-backed vector leg.
    pub fn new() -> Self {
        Self {
            docs: Vec::new(),
            doc_index_by_path: HashMap::new(),
            avg_doc_len: 0.0,
            df: HashMap::new(),
            total_docs: 0,
            db: None,
            bm25_k1: 1.2,
            bm25_b: 0.75,
        }
    }

    /// Override BM25 tuning (typically from [`crate::policy::SearchPolicy`]).
    #[must_use]
    pub fn with_bm25_params(mut self, k1: f64, b: f64) -> Self {
        self.bm25_k1 = k1;
        self.bm25_b = b;
        self
    }

    /// Enables embedding-backed recall against the `embeddings` table (schema V7+).
    pub fn with_db(mut self, db: Arc<VoxDb>) -> Self {
        self.db = Some(db);
        self
    }

    /// Recursively index all markdown files in a directory (best-effort git times per file).
    pub fn index_dir(&mut self, dir: &Path) {
        self.index_dir_with_repo(dir, dir, None);
    }

    /// Like [`Self::index_dir`], but resolves git mtimes relative to `repo_root` and optional bulk map.
    pub fn index_dir_with_repo(
        &mut self,
        dir: &Path,
        repo_root: &Path,
        git_map: Option<&GitMtimeMap>,
    ) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.index_dir_with_repo(&path, repo_root, git_map);
            } else if path.extension().unwrap_or_default() == "md" {
                self.index_file_with_repo(&path, repo_root, git_map);
            }
        }
        self.recompute_stats();
    }

    /// Index a single file (legacy: treats parent dir as repo root for git probes).
    pub fn index_file(&mut self, path: &Path) {
        let repo_root = path.parent().unwrap_or_else(|| Path::new("."));
        self.index_file_with_repo(path, repo_root, None);
    }

    /// Index a single markdown file under `repo_root` with optional bulk git timestamps.
    pub fn index_file_with_repo(
        &mut self,
        path: &Path,
        repo_root: &Path,
        git_map: Option<&GitMtimeMap>,
    ) {
        let content = match vox_bounded_fs::read_utf8_path_capped(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let file_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let tokens = tokenize(&content);
        let length = tokens.len();

        let mut term_freq = HashMap::new();
        let mut unique_terms = HashSet::new();

        for t in &tokens {
            *term_freq.entry(t.clone()).or_insert(0) += 1;
            unique_terms.insert(t.clone());
        }

        for t in unique_terms {
            *self.df.entry(t).or_insert(0) += 1;
        }

        let (status, _) = parse_frontmatter_meta(&content);
        let last_updated_unix = resolve_last_updated_unix(path, repo_root, git_map);

        self.docs.push(IndexedDocument {
            path: path.to_string_lossy().to_string(),
            title: file_name,
            content,
            term_freq,
            length,
            status,
            last_updated_unix,
        });

        self.total_docs += 1;
        self.recompute_stats();
    }

    fn recompute_stats(&mut self) {
        if self.total_docs == 0 {
            self.avg_doc_len = 0.0;
            self.doc_index_by_path.clear();
            return;
        }
        let total_len: usize = self.docs.iter().map(|d| d.length).sum();
        self.avg_doc_len = total_len as f64 / self.total_docs as f64;
        self.doc_index_by_path = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, d)| (d.path.clone(), i))
            .collect();
    }

    fn idf(&self, term: &str) -> f64 {
        let n = self.total_docs as f64;
        let df = *self.df.get(term).unwrap_or(&0) as f64;
        f64::ln(1.0 + (n - df + 0.5) / (df + 0.5))
    }

    /// BM25-ranked document indices (descending score), at most `take` rows.
    fn bm25_ranked_indices(&self, query: &str, take: usize) -> Vec<(usize, f64)> {
        let query_tokens = tokenize(query);
        let mut scores: Vec<(usize, f64)> = Vec::new();

        if self.avg_doc_len == 0.0 {
            return Vec::new();
        }

        for (i, doc) in self.docs.iter().enumerate() {
            let mut score = 0.0;
            for q in &query_tokens {
                let f = *doc.term_freq.get(q).unwrap_or(&0) as f64;
                if f > 0.0 {
                    let idf = self.idf(q);
                    let b = self.bm25_b;
                    let k1 = self.bm25_k1;
                    let len_norm = 1.0 - b + b * (doc.length as f64 / self.avg_doc_len);
                    score += idf * (f * (k1 + 1.0)) / (f + k1 * len_norm);
                }
            }
            if score > 0.0 {
                // Apply status boost
                let status_multiplier = match doc.status.as_str() {
                    "current" => 1.2,
                    "experimental" => 0.8,
                    "research" | "roadmap" => 0.6,
                    "legacy" | "deprecated" => 0.2,
                    _ => 1.0,
                };

                // Apply temporal decay (half-life of ~180 days)
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let age_seconds = now.saturating_sub(doc.last_updated_unix);
                let age_days = (age_seconds as f64) / 86400.0;
                let decay = f64::exp(-age_days * std::f64::consts::LN_2 / 180.0).clamp(0.1, 1.0);

                score = score * status_multiplier * decay;
                scores.push((i, score));
            }
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(take);
        scores
    }

    fn hits_from_bm25_ranked(
        &self,
        query_tokens: &[String],
        ranked: &[(usize, f64)],
    ) -> Vec<HybridSearchHit> {
        let mut out: Vec<HybridSearchHit> = ranked
            .iter()
            .map(|(i, score)| {
                let doc = &self.docs[*i];
                HybridSearchHit {
                    path: doc.path.clone(),
                    title: doc.title.clone(),
                    content_snippet: Self::extract_snippet(&doc.content, query_tokens),
                    score: *score,
                    provenance: vec![format!("bm25:{}", doc.path)],
                    potential_contradiction: false,
                }
            })
            .collect();
        Self::annotate_contradictions(&mut out);
        out
    }

    /// Execute BM25 search over indexed files.
    pub fn search(&self, query: &str, limit: usize) -> Vec<HybridSearchHit> {
        let ranked = self.bm25_ranked_indices(query, limit);
        let query_tokens = tokenize(query);
        self.hits_from_bm25_ranked(&query_tokens, &ranked)
    }

    fn hit_from_retrieval_result(
        &self,
        r: &RetrievalResult,
        query_tokens: &[String],
    ) -> HybridSearchHit {
        let path = r.chunk_id.clone();
        let (title, snippet) = if let Some(&idx) = self.doc_index_by_path.get(&r.chunk_id) {
            let doc = &self.docs[idx];
            let sn = if r.snippet.is_empty() {
                Self::extract_snippet(&doc.content, query_tokens)
            } else {
                r.snippet.clone()
            };
            (doc.title.clone(), sn)
        } else {
            let stem = Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            (stem, r.snippet.clone())
        };
        let mut provenance = vec![format!("evidence:{:?}", r.evidence_source)];
        if !r.contradiction_hints.is_empty() {
            provenance.extend(r.contradiction_hints.iter().cloned());
        }
        let pot = !r.contradiction_hints.is_empty();
        HybridSearchHit {
            path,
            title,
            content_snippet: snippet,
            score: f64::from(r.score),
            provenance,
            potential_contradiction: pot,
        }
    }

    /// Hybrid search combining BM25 and VoxDB vector search via [`fuse_hybrid_results`].
    ///
    /// `vector_fusion_weight` is taken from [`crate::policy::SearchPolicy`] (not a crate-local constant).
    pub async fn hybrid_search(
        &self,
        query: &str,
        limit: usize,
        embedding_service: Option<&EmbeddingService>,
        vector_fusion_weight: f32,
    ) -> Vec<HybridSearchHit> {
        let query_tokens = tokenize(query);
        let bm25_take = limit.saturating_mul(4).max(limit);
        let ranked = self.bm25_ranked_indices(query, bm25_take);
        let bm25_candidates = ranked.len();
        let ts = unix_ms();

        let text_hits: Vec<RetrievalResult> = ranked
            .iter()
            .map(|(i, sc)| {
                let doc = &self.docs[*i];
                RetrievalResult {
                    chunk_id: doc.path.clone(),
                    source: doc.path.clone(),
                    score: *sc as f32,
                    snippet: Self::extract_snippet(&doc.content, &query_tokens),
                    evidence_source: RetrievalEvidenceSource::FullText,
                    retrieved_at_ms: Some(ts),
                    query_id: None,
                    supporting_claim_ids: Vec::new(),
                    contradiction_hints: Vec::new(),
                }
            })
            .collect();

        let mut vector_hits: Vec<RetrievalResult> = Vec::new();
        if let (Some(db_arc), Some(service)) = (&self.db, embedding_service)
            && let Ok(query_vector) = service.embed_query(query).await
            && let Ok(db_hits) = db_arc
                .search_embeddings(&query_vector, None, limit as i64)
                .await
        {
            for (entry, dist) in db_hits {
                let dist_f = f64::from(dist);
                let similarity = (1.0_f64 - (dist_f / 2.0)).clamp(0.0, 1.0) as f32;
                let chunk_id = if entry.source_id.is_empty() {
                    format!("embedding:{}", entry.id)
                } else {
                    entry.source_id.clone()
                };
                let snippet = entry
                    .metadata
                    .clone()
                    .unwrap_or_else(|| "No snippet available".to_string());
                vector_hits.push(RetrievalResult {
                    chunk_id,
                    source: entry.source_id.clone(),
                    score: similarity * 2.0_f32,
                    snippet,
                    evidence_source: RetrievalEvidenceSource::Vector,
                    retrieved_at_ms: Some(ts),
                    query_id: None,
                    supporting_claim_ids: Vec::new(),
                    contradiction_hints: Vec::new(),
                });
            }
        }

        let vector_n = vector_hits.len();
        let w = vector_fusion_weight.clamp(0.0, 1.0);
        let fused = fuse_hybrid_results(&vector_hits, &text_hits, w);
        let mut out: Vec<HybridSearchHit> = fused
            .into_iter()
            .take(limit)
            .map(|r| self.hit_from_retrieval_result(&r, &query_tokens))
            .collect();
        Self::annotate_contradictions(&mut out);

        if let Some(db_arc) = &self.db {
            let contra = out.iter().filter(|h| h.potential_contradiction).count();
            let top = out.first().map(|h| h.score);
            if let Err(e) = db_arc
                .record_memory_hybrid_retrieval(
                    query,
                    bm25_candidates,
                    vector_n,
                    out.len(),
                    contra,
                    top,
                )
                .await
            {
                tracing::warn!(error = %e, "memory hybrid retrieval telemetry failed");
            }
        }

        out
    }

    fn annotate_contradictions(hits: &mut [HybridSearchHit]) {
        if hits.len() < 2 {
            return;
        }
        let a = &hits[0];
        let b = &hits[1];
        let title_disagree = a.score > 0.01
            && b.score > 0.01
            && !a.title.eq_ignore_ascii_case(&b.title)
            && tokenize(&a.title)
                .iter()
                .any(|t| t.len() > 3 && b.title.to_lowercase().contains(t.as_str()));
        if title_disagree {
            hits[0].potential_contradiction |= true;
            hits[1].potential_contradiction |= true;
        }
    }

    fn extract_snippet(content: &str, query_tokens: &[String]) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut best_line_idx = 0;
        let mut max_matches = 0;

        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let matches = query_tokens
                .iter()
                .filter(|q| line_lower.contains(*q))
                .count();
            if matches > max_matches {
                max_matches = matches;
                best_line_idx = i;
            }
        }

        let start = best_line_idx.saturating_sub(1);
        let end = (best_line_idx + 2).min(lines.len());
        lines[start..end].join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn search_adds_bm25_provenance() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("alpha_notes.md");
        fs::write(&p, "alpha beta gamma").expect("write");

        let mut engine = MemorySearchEngine::new();
        engine.index_file(&p);
        let hits = engine.search("alpha", 5);
        assert!(!hits.is_empty());
        assert!(hits[0].provenance.iter().any(|s| s.starts_with("bm25:")));
    }

    #[test]
    fn contradiction_flag_marks_top_hits_with_overlapping_topic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p1 = dir.path().join("alpha_claim.md");
        let p2 = dir.path().join("alpha_counterclaim.md");
        fs::write(&p1, "alpha evidence says yes").expect("write p1");
        fs::write(&p2, "alpha evidence says no").expect("write p2");

        let mut engine = MemorySearchEngine::new();
        engine.index_file(&p1);
        engine.index_file(&p2);
        let hits = engine.search("alpha evidence", 2);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].potential_contradiction);
        assert!(hits[1].potential_contradiction);
    }
}
