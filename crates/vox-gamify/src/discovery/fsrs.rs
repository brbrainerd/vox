//! Minimal FSRS-style spaced-repetition state update. Deterministic, no LLM.
//! `stability` is roughly "days until ~90% recall"; `difficulty` in [1,10].

/// A discovery item's memory state. `due_ms` is an absolute epoch-ms timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryState {
    pub stability: f64,
    pub difficulty: f64,
    pub due_ms: i64,
}

/// Outcome of an exposure: did the user actually *use* the surfaced command?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recall {
    /// Saw it in the rail/tips but did not invoke it.
    Seen,
    /// Invoked the command (strong signal).
    Used,
}

const DAY_MS: i64 = 86_400_000;

/// Update memory state given a review outcome at `now_ms`.
///
/// `prev == None` is the first-ever exposure. Deterministic: same inputs → same
/// output, so it is trivially testable and replay-safe.
pub fn update(prev: Option<MemoryState>, recall: Recall, now_ms: i64) -> MemoryState {
    match prev {
        None => {
            // Initial state. "Used" earns more initial stability than "Seen".
            let (stability, difficulty) = match recall {
                Recall::Used => (3.0, 5.0),
                Recall::Seen => (1.0, 6.0),
            };
            MemoryState {
                stability,
                difficulty,
                due_ms: now_ms + (stability * DAY_MS as f64) as i64,
            }
        }
        Some(p) => {
            // Difficulty drifts down on use, up on a passive "seen".
            let difficulty = match recall {
                Recall::Used => (p.difficulty - 0.5).clamp(1.0, 10.0),
                Recall::Seen => (p.difficulty + 0.3).clamp(1.0, 10.0),
            };
            // Stability grows on use (easier items grow faster); a passive "seen"
            // grows it only slightly so the item resurfaces again soon.
            let growth = match recall {
                Recall::Used => 1.0 + (11.0 - difficulty) / 10.0,
                Recall::Seen => 1.05,
            };
            let stability = (p.stability * growth).max(p.stability + 0.1);
            MemoryState {
                stability,
                difficulty,
                due_ms: now_ms + (stability * DAY_MS as f64) as i64,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_use_sets_positive_stability_and_future_due() {
        let next = update(None, Recall::Used, 1_000);
        assert!(next.stability >= 1.0, "stability {}", next.stability);
        assert!(next.due_ms > 1_000);
    }

    #[test]
    fn seen_without_use_keeps_due_soon() {
        let used = update(None, Recall::Used, 0);
        let seen_again = update(Some(used), Recall::Seen, used.due_ms);
        // A "seen but not used" review must not push the item far out; it should
        // remain due sooner than a successful "used" review would have.
        let used_again = update(Some(used), Recall::Used, used.due_ms);
        assert!(seen_again.due_ms < used_again.due_ms);
    }

    #[test]
    fn repeated_use_grows_stability_monotonically() {
        let a = update(None, Recall::Used, 0);
        let b = update(Some(a), Recall::Used, a.due_ms);
        assert!(b.stability > a.stability);
    }
}
