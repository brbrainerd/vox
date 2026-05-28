//! ChronoFact: timestamp-aware evidence filtering.
//!
//! Restricts `NormalizedHit`s to those published strictly before the year of
//! the claim timestamp, preventing forward-knowledge contamination.

use vox_research_events::schema_types::NormalizedHit;

/// Filters evidence hits to those predating the claim timestamp.
pub struct ChronoFilter {
    /// Unix timestamp (seconds) of the claim.  Evidence must predate this.
    pub claim_timestamp: i64,
}

impl ChronoFilter {
    pub fn new(claim_timestamp: i64) -> Self {
        Self { claim_timestamp }
    }

    /// Approximate calendar year of `claim_timestamp`.
    ///
    /// Uses the average Gregorian year length (365.2425 days).
    /// Accurate to ±1 year for 1970-2100; sufficient for year-granularity filtering.
    pub fn claim_year(&self) -> i32 {
        // Average Gregorian year = 365.2425 days; accurate to ±1 year for 1970-2100
        ((self.claim_timestamp as f64 / 86_400.0 / 365.2425) as i64 + 1970) as i32
    }

    /// Return only hits whose `year` is strictly less than `claim_year()`.
    ///
    /// Hits with `year = None` are excluded (cannot verify they predate the claim).
    pub fn filter_hits<'a>(&self, hits: &'a [NormalizedHit]) -> Vec<&'a NormalizedHit> {
        let claim_year = self.claim_year();
        hits.iter()
            .filter(|h| h.year.is_some_and(|y| y < claim_year))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::schema_types::{NormalizedHit, NoveltySource};

    /// Unix timestamp for 2024-01-01 00:00:00 UTC ≈ 1_704_067_200.
    const CLAIM_2024: i64 = 1_704_067_200;

    fn hit_with_year(year: Option<i32>) -> NormalizedHit {
        NormalizedHit {
            source: NoveltySource::Manual,
            work_uri: "doi:10.test".to_string(),
            title: "Test".to_string(),
            year,
            lexical_score: None,
            semantic_score: None,
            overlap_note: None,
            cited_by_count: None,
        }
    }

    #[test]
    fn filter_removes_future_hits() {
        let filter = ChronoFilter::new(CLAIM_2024);
        let hits = vec![hit_with_year(Some(2025))];
        assert!(filter.filter_hits(&hits).is_empty());
    }

    #[test]
    fn filter_keeps_past_hits() {
        let filter = ChronoFilter::new(CLAIM_2024);
        let hits = vec![hit_with_year(Some(2022))];
        assert_eq!(filter.filter_hits(&hits).len(), 1);
    }

    #[test]
    fn filter_removes_same_year_hits() {
        // Strict less-than: same year as claim is not prior art.
        let filter = ChronoFilter::new(CLAIM_2024);
        let hits = vec![hit_with_year(Some(2024))];
        assert!(filter.filter_hits(&hits).is_empty());
    }
}
