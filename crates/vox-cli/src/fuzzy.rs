//! Fuzzy matching utilities for CLI.
//!
//! Uses `nucleo` for fast scoring when the `fuzzy-search` feature is enabled.
//! Falls back to identity ordering (score = 0) when the feature is disabled so
//! call-sites compile unconditionally.

#[cfg(feature = "fuzzy-search")]
pub struct FuzzyMatcher {
    matcher: nucleo::Matcher,
}

#[cfg(feature = "fuzzy-search")]
impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "fuzzy-search")]
impl FuzzyMatcher {
    /// Create a new fuzzy matcher (allocates scratch memory; reuse across calls).
    pub fn new() -> Self {
        Self {
            matcher: nucleo::Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    /// Ranks candidates by fuzzy match score for the given pattern.
    /// Returns a list of `(index, score)` pairs sorted by score descending.
    pub fn rank<T: AsRef<str>>(&mut self, pattern: &str, candidates: &[T]) -> Vec<(usize, u32)> {
        use nucleo::pattern::{CaseMatching, Normalization, Pattern};

        let pat = Pattern::parse(pattern, CaseMatching::Smart, Normalization::Smart);
        let mut results = Vec::with_capacity(candidates.len());

        for (idx, candidate) in candidates.iter().enumerate() {
            let mut haystack_buf = Vec::new();
            let haystack = nucleo::Utf32Str::new(candidate.as_ref(), &mut haystack_buf);
            if let Some(score) = pat.score(haystack, &mut self.matcher) {
                if score > 0 {
                    results.push((idx, score));
                }
            }
        }

        results.sort_by_key(|r| std::cmp::Reverse(r.1));
        results
    }
}

/// Fallback implementation when `fuzzy-search` feature is disabled.
#[cfg(not(feature = "fuzzy-search"))]
pub struct FuzzyMatcher;

#[cfg(not(feature = "fuzzy-search"))]
impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "fuzzy-search"))]
impl FuzzyMatcher {
    /// No-op constructor.
    pub fn new() -> Self {
        Self
    }
    /// Returns original indices with zero score (no re-ranking).
    pub fn rank<T: AsRef<str>>(&mut self, _pattern: &str, candidates: &[T]) -> Vec<(usize, u32)> {
        candidates.iter().enumerate().map(|(i, _)| (i, 0)).collect()
    }
}
