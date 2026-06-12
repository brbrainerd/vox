//! Suggestion ranking: frecency (usage) + novelty (never-seen boost) + due-ness.
//! Pure scoring so it is deterministic and unit-testable.

/// The ranking inputs for one candidate command.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub action_id: String,
    pub used_count: u32,
    pub last_used_ms: i64,
    pub seen_count: u32,
    /// FSRS due timestamp; 0 when never tracked.
    pub due_ms: i64,
    /// True when the typed prefix matches this command's path/alias.
    pub prefix_match: bool,
}

/// Score a candidate at `now_ms`. Higher = surface sooner. Prefix matches always
/// outrank non-matches; among matches, blend recent usage, novelty, and due-ness.
pub fn score(c: &Candidate, now_ms: i64) -> f64 {
    let prefix = if c.prefix_match { 1000.0 } else { 0.0 };
    // Frecency: log of usage, decayed by days since last use.
    let days_since = ((now_ms - c.last_used_ms).max(0) as f64) / 86_400_000.0;
    let frecency = (c.used_count as f64 + 1.0).ln() / (1.0 + days_since);
    // Novelty: never-seen commands get a fixed boost that fades as seen_count rises.
    let novelty = if c.seen_count == 0 {
        5.0
    } else {
        1.0 / c.seen_count as f64
    };
    // Due-ness: items past their FSRS due time are worth resurfacing.
    let due = if c.due_ms != 0 && c.due_ms <= now_ms {
        3.0
    } else {
        0.0
    };
    prefix + frecency + novelty + due
}

/// Rank candidates best-first. Stable on ties by `action_id` for determinism.
pub fn rank(mut candidates: Vec<Candidate>, now_ms: i64) -> Vec<Candidate> {
    candidates.sort_by(|a, b| {
        score(b, now_ms)
            .partial_cmp(&score(a, now_ms))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.action_id.cmp(&b.action_id))
    });
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, used: u32, seen: u32, prefix: bool) -> Candidate {
        Candidate {
            action_id: id.into(),
            used_count: used,
            last_used_ms: 0,
            seen_count: seen,
            due_ms: 0,
            prefix_match: prefix,
        }
    }

    #[test]
    fn prefix_matches_outrank_everything() {
        let out = rank(vec![cand("b", 99, 99, false), cand("a", 0, 0, true)], 1);
        assert_eq!(out[0].action_id, "a");
    }

    #[test]
    fn never_seen_beats_seen_among_equal_usage() {
        let out = rank(vec![cand("seen", 0, 10, true), cand("fresh", 0, 0, true)], 1);
        assert_eq!(out[0].action_id, "fresh");
    }

    #[test]
    fn due_items_get_resurfaced() {
        let mut due = cand("due", 0, 5, false);
        due.due_ms = 1; // due in the past relative to now=1000
        let not_due = cand("notdue", 0, 5, false);
        let out = rank(vec![not_due, due], 1000);
        assert_eq!(out[0].action_id, "due");
    }
}
