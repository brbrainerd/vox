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
    Continued { streak: u64, bonus_xp: u64 },
    /// Streak saved by grace period. Returns the day count and bonus XP.
    SavedByGrace { streak: u64, bonus_xp: u64 },
    /// Streak broke and reset to 1. Returns the previous streak length.
    BrokenReset { previous: u64 },
}

impl StreakTracker {
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
