//! Independent-source corroboration counting: a domain-agnostic trust
//! fallback for hits that have no DOI/academic venue signal. Counts the
//! number of *distinct domains* whose retrieved evidence supports a claim,
//! so three pages on the same site don't count as three corroborations.

use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct CorroboratingHit {
    pub url: String,
    pub supports_claim: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorroborationCount {
    pub claim_id: String,
    pub supporting_domains: Vec<String>,
}

impl CorroborationCount {
    pub fn count(&self) -> usize {
        self.supporting_domains.len()
    }
}

/// Extracts the registrable domain from a URL for dedup purposes (e.g.
/// `https://www.reuters.com/world/x` and `https://reuters.com/y` both key
/// to `reuters.com`). Strips a leading `www.` only; does not attempt full
/// public-suffix-list registrable-domain parsing (good enough to dedup
/// same-site hits, not to resolve `co.uk`-style TLDs). Reuses
/// `web_dispatcher`'s scheme/query/fragment stripping (`canonical_url_key`)
/// rather than re-implementing it, then takes just the host portion.
fn domain_of(url: &str) -> Option<String> {
    let stripped = crate::web_dispatcher::canonical_url_key(url);
    let host = stripped.split('/').next()?;
    if host.is_empty() {
        return None;
    }
    Some(host.strip_prefix("www.").unwrap_or(host).to_string())
}

pub fn count_corroboration(claim_id: &str, hits: &[CorroboratingHit]) -> CorroborationCount {
    let supporting_domains: Vec<String> = hits
        .iter()
        .filter(|h| h.supports_claim)
        .filter_map(|h| domain_of(&h.url))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    CorroborationCount {
        claim_id: claim_id.to_string(),
        supporting_domains,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_distinct_domains_only() {
        let hits = vec![
            CorroboratingHit {
                url: "https://www.reuters.com/a".into(),
                supports_claim: true,
            },
            CorroboratingHit {
                url: "https://reuters.com/b".into(),
                supports_claim: true,
            },
            CorroboratingHit {
                url: "https://apnews.com/c".into(),
                supports_claim: true,
            },
            CorroboratingHit {
                url: "https://blog.example/d".into(),
                supports_claim: false,
            },
        ];

        let count = count_corroboration("claim-1", &hits);

        assert_eq!(
            count.count(),
            2,
            "reuters.com counted once despite www./bare variants, apnews.com counted once, blog excluded as non-supporting"
        );
    }

    #[test]
    fn zero_supporting_hits_yields_zero_count() {
        let hits = vec![CorroboratingHit {
            url: "https://example.com/a".into(),
            supports_claim: false,
        }];
        assert_eq!(count_corroboration("claim-2", &hits).count(), 0);
    }
}
