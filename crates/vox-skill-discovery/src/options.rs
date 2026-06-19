//! Tunables for the discovery engine. Defaults mirror the design spec.

#[derive(Debug, Clone)]
pub struct DiscoverOptions {
    /// Minimum token count for a code block to be considered.
    pub min_tokens: usize,
    /// Minimum occurrences for a code cluster to be reported.
    pub min_occurrences: usize,
    /// Shingle window size (tokens).
    pub shingle_k: usize,
    /// LSH bands.
    pub bands: usize,
    /// LSH rows per band. `bands * rows` is the minhash length.
    pub rows: usize,
    /// Confirmed-jaccard threshold for clustering / overlap.
    pub min_jaccard: f32,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            min_tokens: 40,
            min_occurrences: 3,
            shingle_k: 5,
            bands: 32,
            rows: 4,
            min_jaccard: 0.7,
        }
    }
}

impl DiscoverOptions {
    /// Minhash length implied by the band configuration.
    pub fn num_hashes(&self) -> usize {
        self.bands * self.rows
    }
}
