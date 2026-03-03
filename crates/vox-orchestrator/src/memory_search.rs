use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use vox_db::VoxDb;

/// Matches from the search engine.
#[derive(Debug, Clone)]
pub struct HybridSearchHit {
    pub path: String,
    pub title: String,
    pub content_snippet: String,
    pub score: f64,
}

/// Tokenize text into alphanumeric lowercase words.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Simple BM25 scoring configuration.
const K1: f64 = 1.2;
const B: f64 = 0.75;

struct IndexedDocument {
    path: String,
    title: String,
    content: String,
    term_freq: HashMap<String, usize>,
    length: usize,
}

/// Search engine combining local file BM25 and DB vector search.
pub struct MemorySearchEngine {
    docs: Vec<IndexedDocument>,
    avg_doc_len: f64,
    df: HashMap<String, usize>, // Document frequency
    total_docs: usize,
    /// DB for vector searches (schema V7 `embeddings` or similar table).
    db: Option<Arc<VoxDb>>,
}

impl Default for MemorySearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySearchEngine {
    pub fn new() -> Self {
        Self {
            docs: Vec::new(),
            avg_doc_len: 0.0,
            df: HashMap::new(),
            total_docs: 0,
            db: None,
        }
    }

    pub fn with_db(mut self, db: Arc<VoxDb>) -> Self {
        self.db = Some(db);
        self
    }

    /// Recursively index all markdown files in a directory.
    pub fn index_dir(&mut self, dir: &Path) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.index_dir(&path);
            } else if path.extension().unwrap_or_default() == "md" {
                self.index_file(&path);
            }
        }
        self.recompute_stats();
    }

    /// Index a single file.
    pub fn index_file(&mut self, path: &Path) {
        let content = match fs::read_to_string(path) {
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

        self.docs.push(IndexedDocument {
            path: path.to_string_lossy().to_string(),
            title: file_name,
            content,
            term_freq,
            length,
        });

        self.total_docs += 1;
    }

    fn recompute_stats(&mut self) {
        if self.total_docs == 0 {
            self.avg_doc_len = 0.0;
            return;
        }
        let total_len: usize = self.docs.iter().map(|d| d.length).sum();
        self.avg_doc_len = total_len as f64 / self.total_docs as f64;
    }

    fn idf(&self, term: &str) -> f64 {
        let n = self.total_docs as f64;
        let df = *self.df.get(term).unwrap_or(&0) as f64;
        // Standard BM25 IDF
        f64::ln(1.0 + (n - df + 0.5) / (df + 0.5))
    }

    /// Execute BM25 search over indexed files.
    pub fn search(&self, query: &str, limit: usize) -> Vec<HybridSearchHit> {
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
                    let len_norm = 1.0 - B + B * (doc.length as f64 / self.avg_doc_len);
                    score += idf * (f * (K1 + 1.0)) / (f + K1 * len_norm);
                }
            }
            if score > 0.0 {
                scores.push((i, score));
            }
        }

        // Sort descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .take(limit)
            .map(|(i, score)| {
                let doc = &self.docs[i];
                HybridSearchHit {
                    path: doc.path.clone(),
                    title: doc.title.clone(),
                    content_snippet: Self::extract_snippet(&doc.content, &query_tokens),
                    score,
                }
            })
            .collect()
    }

    /// Hybrid search combining BM25 and VoxDB vector search.
    pub async fn hybrid_search(&self, query: &str, limit: usize, embedding_service: Option<&crate::services::embeddings::EmbeddingService>) -> Vec<HybridSearchHit> {
        // Run BM25
        let mut bm25_hits = self.search(query, limit);

        // Run Vector search
        if let (Some(db), Some(service)) = (&self.db, embedding_service) {
            if let Ok(query_vector) = service.embed_query(query).await {
                if let Ok(db_hits) = db.search_embeddings(&query_vector, None, limit as i64).await {
                    let mut vector_hits = Vec::new();
                    for (entry, dist) in db_hits {
                        // Rescale score: cosine distance is 0..2 (higher = farther).
                        // Convert to 0..1 similarity: 1.0 - (dist / 2.0)
                        let similarity = 1.0 - (dist / 2.0);
                        vector_hits.push(HybridSearchHit {
                            path: format!("vox-db:{}", entry.id),
                            title: format!("{}.{}", entry.source_type, entry.source_id),
                            content_snippet: entry.metadata.clone().unwrap_or_else(|| "No snippet available".to_string()),
                            score: similarity as f64 * 2.0, // Weight vector search higher
                        });
                    }

                    // Simple merge: add vector hits to bm25 hits
                    for v_hit in vector_hits {
                        if let Some(existing) = bm25_hits.iter_mut().find(|h| h.path == v_hit.path) {
                            existing.score += v_hit.score;
                        } else {
                            bm25_hits.push(v_hit);
                        }
                    }
                }
            }
        }

        // Final sort
        bm25_hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        bm25_hits.truncate(limit);
        bm25_hits
    }

    /// Extract a contextual snippet around the most frequent query terms.
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

        // Return the line with context
        let start = best_line_idx.saturating_sub(1);
        let end = (best_line_idx + 2).min(lines.len());
        lines[start..end].join("\n")
    }
}
