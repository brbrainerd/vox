//! Player profile with XP, leveling, energy, and crystals.

use crate::streak::{StreakResult, StreakTracker};
use crate::util::now_unix;
use serde::{Deserialize, Serialize};

// ─── Constants ───────────────────────────────────────────

/// XP required per level: `level = xp / XP_PER_LEVEL + 1`.
const XP_PER_LEVEL: u64 = 100;

/// Energy regenerated per tick.
const ENERGY_PER_REGEN: u64 = 1;

/// Seconds between energy regen ticks.
const REGEN_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Starting crystals for a new profile.
const STARTING_CRYSTALS: u64 = 100;

/// Starting/base energy.
const BASE_ENERGY: u64 = 100;

/// Energy bonus per level.
const ENERGY_PER_LEVEL: u64 = 10;

// ─── Profile ─────────────────────────────────────────────

/// A player's gamification profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamifyProfile {
    pub user_id: String,
    pub level: u64,
    pub xp: u64,
    pub crystals: u64,
    pub energy: u64,
    pub max_energy: u64,
    pub last_energy_regen: i64, // unix timestamp
    pub last_active: i64,       // unix timestamp
    #[serde(default)]
    pub streak: StreakTracker,
}

impl GamifyProfile {
    /// Create a new default profile for a user.

    pub fn new_default(user_id: impl Into<String>) -> Self {
        let now = now_unix();
        Self {
            user_id: user_id.into(),
            level: 1,
            xp: 0,
            crystals: STARTING_CRYSTALS,
            energy: BASE_ENERGY,
            max_energy: BASE_ENERGY,
            last_energy_regen: now,
            last_active: now,
            streak: StreakTracker::default(),
        }
    }

    /// Record daily activity and return any streak result. Award bonus XP if continued or saved.
    pub fn record_daily_activity(&mut self) -> StreakResult {
        let result = self.streak.record_activity();
        match result {
            StreakResult::Continued { bonus_xp, .. }
            | StreakResult::SavedByGrace { bonus_xp, .. } => {
                self.add_xp(bonus_xp);
            }
            _ => {}
        }
        self.touch();
        result
    }

    /// Add XP and check for level-up. Returns true if leveled up.
    pub fn add_xp(&mut self, amount: u64) -> bool {
        self.xp += amount;
        let new_level = self.xp / XP_PER_LEVEL + 1;
        if new_level > self.level {
            self.level = new_level;
            self.max_energy = BASE_ENERGY + (self.level - 1) * ENERGY_PER_LEVEL;
            self.energy = self.max_energy; // Full energy on level-up
            true
        } else {
            false
        }
    }

    /// Add crystals.
    pub fn add_crystals(&mut self, amount: u64) {
        self.crystals += amount;
    }

    /// Spend crystals. Returns false if insufficient.
    pub fn spend_crystals(&mut self, amount: u64) -> bool {
        if self.crystals >= amount {
            self.crystals -= amount;
            true
        } else {
            false
        }
    }

    /// Spend energy. Returns false if insufficient.
    pub fn spend_energy(&mut self, amount: u64) -> bool {
        if self.energy >= amount {
            self.energy -= amount;
            true
        } else {
            false
        }
    }

    /// Regenerate energy based on elapsed time since last regen.
    pub fn regen_energy(&mut self) {
        let now = now_unix();
        let elapsed = (now - self.last_energy_regen).max(0) as u64;
        let ticks = elapsed / REGEN_INTERVAL_SECS;
        if ticks > 0 {
            let gained = ticks * ENERGY_PER_REGEN;
            self.energy = (self.energy + gained).min(self.max_energy);
            self.last_energy_regen = now;
        }
    }

    /// XP needed to reach the next level.
    pub fn xp_to_next_level(&self) -> u64 {
        let next_threshold = self.level * XP_PER_LEVEL;
        next_threshold.saturating_sub(self.xp)
    }

    /// XP progress as a percentage within the current level (0.0 - 1.0).
    pub fn xp_progress(&self) -> f64 {
        let within_level = self.xp % XP_PER_LEVEL;
        within_level as f64 / XP_PER_LEVEL as f64
    }

    /// Touch the last_active timestamp.
    pub fn touch(&mut self) {
        self.last_active = now_unix();
    }
}

// ─── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_profile_defaults() {
        let p = GamifyProfile::new_default("user-1");
        assert_eq!(p.level, 1);
        assert_eq!(p.xp, 0);
        assert_eq!(p.crystals, 100);
        assert_eq!(p.energy, 100);
        assert_eq!(p.max_energy, 100);
    }

    #[test]
    fn level_up() {
        let mut p = GamifyProfile::new_default("user-1");
        assert!(!p.add_xp(50)); // 50 XP, still level 1
        assert_eq!(p.level, 1);

        assert!(p.add_xp(50)); // 100 XP → level 2
        assert_eq!(p.level, 2);
        assert_eq!(p.max_energy, 110); // 100 + (2-1)*10
        assert_eq!(p.energy, 110); // Full energy on level-up
    }

    #[test]
    fn multi_level_up() {
        let mut p = GamifyProfile::new_default("user-1");
        assert!(p.add_xp(250)); // Should jump to level 3
        assert_eq!(p.level, 3);
        assert_eq!(p.max_energy, 120); // 100 + 2*10
    }

    #[test]
    fn crystal_spending() {
        let mut p = GamifyProfile::new_default("user-1");
        assert!(p.spend_crystals(50));
        assert_eq!(p.crystals, 50);
        assert!(!p.spend_crystals(100)); // Not enough
        assert_eq!(p.crystals, 50); // Unchanged
    }

    #[test]
    fn energy_spending() {
        let mut p = GamifyProfile::new_default("user-1");
        assert!(p.spend_energy(20));
        assert_eq!(p.energy, 80);
        assert!(!p.spend_energy(100)); // Not enough
    }

    #[test]
    fn xp_progress() {
        let mut p = GamifyProfile::new_default("user-1");
        p.xp = 75;
        assert!((p.xp_progress() - 0.75).abs() < 0.01);
        assert_eq!(p.xp_to_next_level(), 25);
    }

    #[test]
    fn energy_regen_calculation() {
        let mut p = GamifyProfile::new_default("user-1");
        p.energy = 50;
        // Simulate 15 minutes elapsed (3 ticks × 5 min)
        p.last_energy_regen = now_unix() - 900;
        p.regen_energy();
        assert_eq!(p.energy, 53); // 50 + 3
    }

    #[test]
    fn energy_regen_caps_at_max() {
        let mut p = GamifyProfile::new_default("user-1");
        p.energy = 99;
        p.last_energy_regen = now_unix() - 1800; // 30 min → 6 ticks
        p.regen_energy();
        assert_eq!(p.energy, 100); // Capped at max
    }
}
