/// Retrieval chunk with provenance metadata.
#[derive(Debug, Clone)]
pub struct RetrievedChunk {
    pub id: String,
    pub source: String,
    pub text: String,
    pub score: f32,
}

/// Context budget to cap prompt growth.
#[derive(Debug, Clone, Copy)]
pub struct ContextBudget {
    pub max_chunks: usize,
    pub max_chars: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_chunks: 8,
            max_chars: 8_000,
        }
    }
}

/// Compact provenance attachment for observability and reproducibility.
#[derive(Debug, Clone)]
pub struct ProvenanceRecord {
    pub chunk_id: String,
    pub source: String,
    pub score: f32,
    pub truncated: bool,
}

/// Select top-scoring chunks and enforce max char budget.
pub fn apply_context_budget(
    mut chunks: Vec<RetrievedChunk>,
    budget: ContextBudget,
) -> (Vec<RetrievedChunk>, Vec<ProvenanceRecord>) {
    chunks.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::new();
    let mut provenance = Vec::new();
    let mut used_chars = 0usize;

    for chunk in chunks.into_iter().take(budget.max_chunks) {
        if used_chars >= budget.max_chars {
            break;
        }
        let remaining = budget.max_chars - used_chars;
        if chunk.text.len() <= remaining {
            used_chars += chunk.text.len();
            provenance.push(ProvenanceRecord {
                chunk_id: chunk.id.clone(),
                source: chunk.source.clone(),
                score: chunk.score,
                truncated: false,
            });
            selected.push(chunk);
        } else {
            let truncated = RetrievedChunk {
                id: chunk.id.clone(),
                source: chunk.source.clone(),
                text: chunk.text.chars().take(remaining).collect(),
                score: chunk.score,
            };
            provenance.push(ProvenanceRecord {
                chunk_id: chunk.id,
                source: chunk.source,
                score: chunk.score,
                truncated: true,
            });
            selected.push(truncated);
            break;
        }
    }

    (selected, provenance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_budget_truncates_and_returns_provenance() {
        let chunks = vec![
            RetrievedChunk {
                id: "a".into(),
                source: "doc1".into(),
                text: "12345".into(),
                score: 0.9,
            },
            RetrievedChunk {
                id: "b".into(),
                source: "doc2".into(),
                text: "abcdef".into(),
                score: 0.8,
            },
        ];
        let (selected, provenance) = apply_context_budget(
            chunks,
            ContextBudget {
                max_chunks: 4,
                max_chars: 8,
            },
        );
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[1].text, "abc");
        assert_eq!(provenance.len(), 2);
        assert!(provenance[1].truncated);
    }
}
