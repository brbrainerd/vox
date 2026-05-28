use crate::claim_extractor::types::SpanBound;

pub struct SpanChecker {
    pub min_overlap_fraction: f64,
}

impl Default for SpanChecker {
    fn default() -> Self {
        Self {
            min_overlap_fraction: 0.6,
        }
    }
}

impl SpanChecker {
    pub fn check(&self, claim_text: &str, span: &SpanBound, source: &str) -> bool {
        if span.end > source.len() || span.start >= span.end {
            return false;
        }
        // Word-overlap against the span slice (not the entire source)
        let span_slice = &source[span.start..span.end];
        let claim_words: std::collections::HashSet<&str> = claim_text.split_whitespace().collect();
        let span_words: std::collections::HashSet<&str> = span_slice.split_whitespace().collect();
        if claim_words.is_empty() {
            return false;
        }
        let overlap = claim_words.intersection(&span_words).count();
        (overlap as f64 / claim_words.len() as f64) >= self.min_overlap_fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_within_source_passes() {
        let checker = SpanChecker::default();
        let source = "p95 latency rose by 10ms in April.";
        assert!(checker.check(
            "p95 latency rose by 10ms",
            &SpanBound { start: 0, end: 24 },
            source
        ));
    }

    #[test]
    fn span_outside_source_fails() {
        let checker = SpanChecker::default();
        let source = "short text";
        assert!(!checker.check("other claim", &SpanBound { start: 0, end: 50 }, source));
    }
}
