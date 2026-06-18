//! Hybrid BM25 skill search backed by the same tokenization strategy as `vox-search`.

use std::collections::HashMap;

use vox_skills::SkillManifest;

/// One ranked skill search hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSearchHit {
    pub id: String,
    pub score: u64,
}

/// In-memory BM25 index over skill manifests.
#[derive(Debug, Clone, Default)]
pub struct SkillSearchIndex {
    docs: Vec<SkillDoc>,
    df: HashMap<String, usize>,
    avg_len: f64,
}

#[derive(Debug, Clone)]
struct SkillDoc {
    id: String,
    #[allow(dead_code)]
    text: String,
    term_freq: HashMap<String, usize>,
    length: usize,
}

impl SkillSearchIndex {
    pub fn from_manifests(manifests: &[SkillManifest]) -> Self {
        let mut idx = Self::default();
        for m in manifests {
            let body_preview = m
                .tags
                .iter()
                .chain(std::iter::once(&m.description))
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            let text = format!(
                "{} {} {} {} {}",
                m.id,
                m.name,
                m.description,
                m.tags.join(" "),
                body_preview
            );
            idx.add_doc(m.id.clone(), text);
        }
        idx.recompute_stats();
        idx
    }

    pub fn rebuild(&mut self, manifests: &[SkillManifest]) {
        *self = Self::from_manifests(manifests);
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SkillSearchHit> {
        if self.docs.is_empty() {
            return Vec::new();
        }
        let terms = tokenize(query);
        if terms.is_empty() {
            return Vec::new();
        }
        let k1 = 1.2_f64;
        let b = 0.75_f64;
        let n = self.docs.len() as f64;
        let mut scored: Vec<(usize, f64)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, doc)| {
                let mut score = 0.0;
                for term in &terms {
                    let tf = *doc.term_freq.get(term).unwrap_or(&0) as f64;
                    if tf == 0.0 {
                        continue;
                    }
                    let df = *self.df.get(term).unwrap_or(&0) as f64;
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    let norm = 1.0 - b + b * (doc.length as f64 / self.avg_len.max(1.0));
                    score += idf * (tf * (k1 + 1.0)) / (tf + k1 * norm);
                }
                (i, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(limit)
            .map(|(i, s)| SkillSearchHit {
                id: self.docs[i].id.clone(),
                score: (s * 1000.0) as u64,
            })
            .collect()
    }

    fn add_doc(&mut self, id: String, text: String) {
        let tokens = tokenize(&text);
        let mut term_freq = HashMap::new();
        for t in &tokens {
            *term_freq.entry(t.clone()).or_insert(0) += 1;
        }
        self.docs.push(SkillDoc {
            id,
            length: tokens.len(),
            term_freq,
            text,
        });
    }

    fn recompute_stats(&mut self) {
        self.df.clear();
        for doc in &self.docs {
            let unique: std::collections::HashSet<_> = doc.term_freq.keys().cloned().collect();
            for term in unique {
                *self.df.entry(term).or_insert(0) += 1;
            }
        }
        let total_len: usize = self.docs.iter().map(|d| d.length).sum();
        self.avg_len = if self.docs.is_empty() {
            0.0
        } else {
            total_len as f64 / self.docs.len() as f64
        };
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_skills::SkillManifest;

    fn manifest(id: &str, description: &str) -> SkillManifest {
        SkillManifest {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: description.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn skill_search_ranks_description_semantically() {
        let idx = SkillSearchIndex::from_manifests(&[
            manifest(
                "systematic-debugging",
                "Debug failing tests: find root cause before fixing",
            ),
            manifest("writing-plans", "Implementation plan authoring"),
        ]);
        let hits = idx.search("debug failing test", 3);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].id, "systematic-debugging");
    }
}
