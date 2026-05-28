use crate::claim_extractor::types::{AtomicClaim, SciClaimTuple, SpanBound, VerifiabilityClass};

#[derive(Debug, Clone)]
pub struct AtomicConfig {
    pub max_claims_per_sentence: usize,
}

impl Default for AtomicConfig {
    fn default() -> Self {
        Self {
            max_claims_per_sentence: 5,
        }
    }
}

#[derive(Default)]
pub struct AtomicDecomposer {
    pub config: AtomicConfig,
}

impl AtomicDecomposer {
    pub fn new(config: AtomicConfig) -> Self {
        Self { config }
    }

    pub fn decompose(&self, sentence: &str) -> Vec<AtomicClaim> {
        let split_patterns = [" and ", ", and ", " while ", "; ", ", but "];
        let mut segments: Vec<(usize, &str)> = vec![(0, sentence)];

        for pattern in &split_patterns {
            let mut new_segments = Vec::new();
            for (base_offset, seg) in &segments {
                let mut start = 0;
                for m in seg.match_indices(pattern) {
                    if m.0 > start {
                        new_segments.push((*base_offset + start, &seg[start..m.0]));
                    }
                    start = m.0 + pattern.len();
                }
                if start < seg.len() {
                    new_segments.push((*base_offset + start, &seg[start..]));
                }
            }
            if !new_segments.is_empty() {
                segments = new_segments;
            }
        }

        segments
            .iter()
            .take(self.config.max_claims_per_sentence)
            .filter(|(_, s)| s.trim().len() > 5)
            .map(|(offset, seg)| {
                let text = seg.trim().to_string();
                let id = fnv1a_hash(&text);
                let tuple = extract_tuple(&text);
                AtomicClaim {
                    id,
                    span: SpanBound {
                        start: *offset,
                        end: *offset + seg.len(),
                    },
                    verifiability: if tuple.is_some() {
                        VerifiabilityClass::Numeric
                    } else {
                        VerifiabilityClass::Semantic
                    },
                    verifiability_score: if tuple.is_some() { 0.85 } else { 0.6 },
                    tuple,
                    text,
                }
            })
            .collect()
    }
}

fn extract_tuple(text: &str) -> Option<SciClaimTuple> {
    let lower = text.to_ascii_lowercase();
    let change_verbs = [
        ("increased by", "increased_by"),
        ("decreased by", "decreased_by"),
        ("rose by", "increased_by"),
        ("fell by", "decreased_by"),
        ("rose to", "rose_to"),
        ("fell to", "fell_to"),
        ("improved by", "improved_by"),
        ("reduced by", "reduced_by"),
    ];
    for (phrase, relation) in &change_verbs {
        if let Some(idx) = lower.find(phrase) {
            let before = text[..idx].trim();
            let after = text[idx + phrase.len()..].trim();
            if !before.is_empty() && !after.is_empty() {
                return Some(SciClaimTuple {
                    variable_a: before.to_string(),
                    relation: relation.to_string(),
                    variable_b: after
                        .split_whitespace()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" "),
                    qualifier: None,
                });
            }
        }
    }
    None
}

pub fn fnv1a_hash(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    s.bytes()
        .fold(FNV_OFFSET, |h, b| (h ^ b as u64).wrapping_mul(FNV_PRIME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_simple_increase_claim() {
        let decomposer = AtomicDecomposer::default();
        let claims = decomposer
            .decompose("Provider X p95 latency increased by 12ms and refusal rate rose to 3.2%.");
        assert!(!claims.is_empty());
        for claim in &claims {
            assert!(!claim.text.is_empty());
            assert!(claim.span.end > claim.span.start);
        }
    }

    #[test]
    fn assigns_stable_ids() {
        let decomposer = AtomicDecomposer::default();
        let c1 = decomposer.decompose("Latency rose by 10ms.");
        let c2 = decomposer.decompose("Latency rose by 10ms.");
        assert_eq!(c1[0].id, c2[0].id);
    }
}
