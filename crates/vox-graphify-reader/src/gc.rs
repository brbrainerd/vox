//! Deterministic keep-vs-discard policy for corpora. Pure functions; the future learning loop
//! feeds real signals (usage from the search-log, churn from manifest diffs, cost from builds).

/// Higher = more worth maintaining. `usage` = recent query/search hits; `recency_days` = days
/// since last use; `churn` = node/community delta magnitude since last rebuild; `cost_secs` =
/// last build wall-time. Bounded, monotone in usage and recency.
pub fn value_score(usage: u64, recency_days: f64, churn: u64, cost_secs: f64) -> f64 {
    let usage_term = (usage as f64 + 1.0).ln();
    let recency_term = 1.0 / (1.0 + recency_days.max(0.0));
    let churn_term = (churn as f64 + 1.0).ln();
    let cost_penalty = 1.0 / (1.0 + cost_secs.max(0.0) / 60.0);
    usage_term * recency_term + 0.5 * churn_term * cost_penalty
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Retention {
    Maintain,
    Expire,
    Discard,
}

/// `score >= maintain_above` → keep fresh; `score < discard_below` → GC; else let TTL expire.
pub fn retention_decision(score: f64, maintain_above: f64, discard_below: f64) -> Retention {
    if score >= maintain_above {
        Retention::Maintain
    } else if score < discard_below {
        Retention::Discard
    } else {
        Retention::Expire
    }
}

/// Data-size escape hatch: above `threshold` nodes, prefer the coarse `modules` lens (Plan B).
pub fn pick_lens(node_count: usize, threshold: usize) -> &'static str {
    if node_count > threshold {
        "modules"
    } else {
        "structural"
    }
}
