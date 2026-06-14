//! Streak tracking with bonus XP and grace periods.

use crate::util::now_unix;
use serde::{Deserialize, Serialize};

const SECONDS_PER_DAY: i64 = 86400;

/// Tracks daily activity streaks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreakTracker {
    /// Current consecutive days active.
    pub current_streak: u64,
    /// Longest streak achieved.
    pub longest_streak: u64,
    /// Unix timestamp of the last recorded activity.
    pub last_activity_ts: i64,
    /// Grace periods (streak saves) available.
    pub grace_periods_available: u64,
    /// How many grace periods have been used.
    pub grace_periods_used: u64,
}

impl Default for StreakTracker {
    fn default() -> Self {
        Self {
            current_streak: 0,
            longest_streak: 0,
            last_activity_ts: 0,
            grace_periods_available: 1, // Start with 1 grace period
            grace_periods_used: 0,
        }
    }
}

/// The result of attempting to record daily activity.
#[derive(Debug, PartialEq, Eq)]
pub enum StreakResult {
    /// Already active today, no streak changes.
    AlreadyActive,
    /// Streak continued (or started). Returns the day count and bonus XP.
    Continued {
        /// Current consecutive-day count.
        streak: u64,
        /// Bonus XP earned for the streak.
        bonus_xp: u64,
    },
    /// Streak saved by grace period. Returns the day count and bonus XP.
    SavedByGrace {
        /// Current consecutive-day count after grace period applied.
        streak: u64,
        /// Bonus XP earned for the streak.
        bonus_xp: u64,
    },
    /// Streak broke and reset to 1. Returns the previous streak length.
    BrokenReset {
        /// The streak length before it reset to 1.
        previous: u64,
    },
}

impl StreakTracker {
    /// Create a new `StreakTracker` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the start of the day in UTC for a given timestamp.
    fn day_start(ts: i64) -> i64 {
        ts - (ts % SECONDS_PER_DAY)
    }

    /// Record activity for the current time.
    /// Analyzes the time since `last_activity_ts` and updates the streak,
    /// consuming a grace period if necessary.
    pub fn record_activity(&mut self) -> StreakResult {
        let now = now_unix();

        if self.last_activity_ts == 0 {
            // First time ever
            self.current_streak = 1;
            self.longest_streak = 1;
            self.last_activity_ts = now;
            return StreakResult::Continued {
                streak: 1,
                bonus_xp: self.calculate_bonus(1),
            };
        }

        let today = Self::day_start(now);
        let last_active_day = Self::day_start(self.last_activity_ts);
        let days_diff = (today - last_active_day) / SECONDS_PER_DAY;

        if days_diff == 0 {
            // Already active today
            return StreakResult::AlreadyActive;
        }

        self.last_activity_ts = now;

        if days_diff == 1 {
            // Active on contiguous day
            self.current_streak += 1;
            if self.current_streak > self.longest_streak {
                self.longest_streak = self.current_streak;
            }
            // Reward a grace period every 7 days
            if self.current_streak > 0 && self.current_streak.is_multiple_of(7) {
                self.grace_periods_available += 1;
            }
            StreakResult::Continued {
                streak: self.current_streak,
                bonus_xp: self.calculate_bonus(self.current_streak),
            }
        } else {
            // Missed one or more days
            let days_missed = days_diff - 1;

            if days_missed as u64 <= self.grace_periods_available {
                // We have enough grace periods to cover the absence
                self.grace_periods_available -= days_missed as u64;
                self.grace_periods_used += days_missed as u64;
                self.current_streak += 1;
                if self.current_streak > self.longest_streak {
                    self.longest_streak = self.current_streak;
                }
                StreakResult::SavedByGrace {
                    streak: self.current_streak,
                    bonus_xp: self.calculate_bonus(self.current_streak),
                }
            } else {
                // Streak broken
                let previous = self.current_streak;
                self.current_streak = 1;
                StreakResult::BrokenReset { previous }
            }
        }
    }

    /// Calculate bonus XP for logging in today based on current streak.
    fn calculate_bonus(&self, streak: u64) -> u64 {
        let base_bonus = 10;
        let cap = 100;
        // e.g., Day 1: 10, Day 2: 20... capped at 100
        (base_bonus * streak).min(cap)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod semcov_wave13_tests {
    use super::*;

    // ── Bonus calculation boundary tests ──────────────────────────────────

    #[test]
    fn bonus_at_streak_10_is_capped_at_100() {
        // Catches: cap logic using > instead of .min(); streak 10 would give 100
        let t = StreakTracker::new();
        assert_eq!(t.calculate_bonus(10), 100);
    }

    #[test]
    fn bonus_at_streak_11_does_not_exceed_cap() {
        // Catches: off-by-one where cap is applied at streak >= 11 instead of >= 10
        let t = StreakTracker::new();
        assert_eq!(t.calculate_bonus(11), 100);
        assert_eq!(t.calculate_bonus(11), t.calculate_bonus(10));
    }

    #[test]
    fn bonus_at_streak_9_is_strictly_below_cap() {
        // Catches: premature capping that would make streak-9 == streak-10
        let t = StreakTracker::new();
        assert_eq!(t.calculate_bonus(9), 90);
        assert!(t.calculate_bonus(9) < t.calculate_bonus(10));
    }

    #[test]
    fn bonus_zero_streak_returns_zero() {
        // Catches: base_bonus * 0 producing non-zero via faulty formula
        let t = StreakTracker::new();
        assert_eq!(t.calculate_bonus(0), 0);
    }

    #[test]
    fn bonus_multiplier_doubles_with_double_streak() {
        // Catches: non-linear formula bug before the cap
        let t = StreakTracker::new();
        let b2 = t.calculate_bonus(2);
        let b4 = t.calculate_bonus(4);
        assert_eq!(b4, b2 * 2, "bonus should be linear (10*streak) before cap");
    }

    // ── Grace period boundary tests ────────────────────────────────────────

    #[test]
    fn grace_saves_exactly_one_missed_day_with_one_grace() {
        // Catches: off-by-one in days_missed calculation (diff-1 instead of diff)
        let mut t = StreakTracker::new();
        t.grace_periods_available = 1;
        t.current_streak = 5;
        t.last_activity_ts = now_unix() - SECONDS_PER_DAY * 2; // missed exactly 1 day
        let res = t.record_activity();
        assert!(
            matches!(res, StreakResult::SavedByGrace { streak: 6, .. }),
            "expected SavedByGrace got {res:?}"
        );
        assert_eq!(t.grace_periods_available, 0);
    }

    #[test]
    fn no_grace_for_two_missed_days_with_one_grace() {
        // Catches: grace consumed even when days_missed > grace_available
        let mut t = StreakTracker::new();
        t.grace_periods_available = 1;
        t.current_streak = 5;
        t.last_activity_ts = now_unix() - SECONDS_PER_DAY * 3; // missed 2 days
        let res = t.record_activity();
        assert!(
            matches!(res, StreakResult::BrokenReset { previous: 5 }),
            "expected BrokenReset got {res:?}"
        );
    }

    #[test]
    fn two_graces_save_two_missed_days() {
        // Catches: grace check uses strict < instead of <=
        let mut t = StreakTracker::new();
        t.grace_periods_available = 2;
        t.current_streak = 3;
        t.last_activity_ts = now_unix() - SECONDS_PER_DAY * 3; // missed 2 days
        let res = t.record_activity();
        assert!(
            matches!(res, StreakResult::SavedByGrace { streak: 4, .. }),
            "expected SavedByGrace got {res:?}"
        );
        assert_eq!(t.grace_periods_available, 0);
    }

    // ── Longest streak monotonicity ────────────────────────────────────────

    #[test]
    fn longest_streak_never_decreases_after_break() {
        // Catches: longest_streak reset to 1 when streak breaks
        let mut t = StreakTracker::new();
        t.grace_periods_available = 0;
        t.current_streak = 10;
        t.longest_streak = 10;
        t.last_activity_ts = now_unix() - SECONDS_PER_DAY * 5; // break streak
        t.record_activity();
        assert_eq!(
            t.longest_streak, 10,
            "longest_streak should not decrease after break"
        );
        assert_eq!(t.current_streak, 1, "current_streak should reset to 1");
    }

    // ── Grace awarded at every 7-day multiple ─────────────────────────────

    #[test]
    fn grace_awarded_at_day_7() {
        // Catches: is_multiple_of(7) not firing at streak==7
        let mut t = StreakTracker::new();
        t.grace_periods_available = 0;
        t.current_streak = 6;
        t.longest_streak = 6;
        t.last_activity_ts = now_unix() - SECONDS_PER_DAY; // exactly yesterday
        t.record_activity(); // now at streak 7
        assert_eq!(
            t.grace_periods_available, 1,
            "should gain 1 grace period at streak 7"
        );
    }

    #[test]
    fn grace_not_awarded_at_day_8() {
        // Catches: off-by-one in multiple-of-7 check awarding grace too early
        let mut t = StreakTracker::new();
        t.grace_periods_available = 0;
        t.current_streak = 7;
        t.longest_streak = 7;
        t.last_activity_ts = now_unix() - SECONDS_PER_DAY;
        t.record_activity(); // now at streak 8
        assert_eq!(
            t.grace_periods_available, 0,
            "should NOT gain grace period at streak 8"
        );
    }

    // ── day_start helper ──────────────────────────────────────────────────

    #[test]
    fn day_start_strips_intraday_offset() {
        // Catches: day_start computing wrong value (e.g. off-by-one in modulo)
        let midnight = 1_700_000_000 - (1_700_000_000 % SECONDS_PER_DAY);
        assert_eq!(StreakTracker::day_start(midnight), midnight);
        assert_eq!(
            StreakTracker::day_start(midnight + 3_600),
            midnight,
            "1 hour into day should still map to day start"
        );
        assert_eq!(
            StreakTracker::day_start(midnight + SECONDS_PER_DAY - 1),
            midnight,
            "last second of day should map to same day start"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_streak() {
        let mut t = StreakTracker::new();
        let res = t.record_activity();
        assert_eq!(
            res,
            StreakResult::Continued {
                streak: 1,
                bonus_xp: 10
            }
        );
        assert_eq!(t.current_streak, 1);
        assert_eq!(t.longest_streak, 1);
    }

    #[test]
    fn already_active() {
        let mut t = StreakTracker::new();
        t.record_activity();
        assert_eq!(t.record_activity(), StreakResult::AlreadyActive);
    }

    #[test]
    fn miss_days_with_grace() {
        let mut t = StreakTracker::new();
        t.grace_periods_available = 1;
        t.last_activity_ts = now_unix() - (SECONDS_PER_DAY * 2); // Missed 1 day
        t.current_streak = 3;

        let res = t.record_activity();
        assert_eq!(
            res,
            StreakResult::SavedByGrace {
                streak: 4,
                bonus_xp: 40
            }
        );
        assert_eq!(t.grace_periods_available, 0);
        assert_eq!(t.current_streak, 4);
    }

    #[test]
    fn break_streak() {
        let mut t = StreakTracker::new();
        t.grace_periods_available = 0;
        t.last_activity_ts = now_unix() - (SECONDS_PER_DAY * 2); // Missed 1 day
        t.current_streak = 5;

        let res = t.record_activity();
        assert_eq!(res, StreakResult::BrokenReset { previous: 5 });
        assert_eq!(t.current_streak, 1);
    }
}
