//! Companion abilities for bug battles.
//!
//! Each companion archetype has a set of abilities unlocked progressively.
//! Damage values are balanced by the active `GamifyMode` via `policy_scale`.

use serde::{Deserialize, Serialize};

use crate::battle::BugType;

// ── Archetypes ────────────────────────────────────────────

/// Companion archetype that determines the default ability set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Archetype {
    #[default]
    /// High damage, low heal — focused on aggressive offense.
    Centurion,
    /// Balanced; refactor-themed with recovery utilities.
    Architectus,
    /// Documentation-themed; provides buffs and support.
    Scriba,
    /// Coordination and handoff-themed; excels at team play.
    Legatus,
}

impl Archetype {
    /// Convert this archetype to its canonical lower-case string slug.
    pub fn as_str(&self) -> &'static str {
        match self {
            Archetype::Centurion => "centurion",
            Archetype::Architectus => "architectus",
            Archetype::Scriba => "scriba",
            Archetype::Legatus => "legatus",
        }
    }
}

/// Parse an archetype from its string slug; defaults to [`Archetype::Centurion`] on unknown input.
pub fn archetype_from_str(s: &str) -> Archetype {
    match s {
        "architectus" => Archetype::Architectus,
        "scriba" => Archetype::Scriba,
        "legatus" => Archetype::Legatus,
        _ => Archetype::Centurion,
    }
}

// ── Ability ───────────────────────────────────────────────

/// An ability usable in combat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    /// Unique identifier for this ability.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Base damage (positive) or heal (negative) amount.
    pub damage: i32,
    /// Energy cost to use.
    pub energy_cost: u32,
    /// Turns required before reuse (0 = no cooldown).
    pub cooldown: u32,
    /// Bug type this ability is effective against (if any).
    pub effective_against: Option<BugType>,
    /// Whether the companion has unlocked this ability.
    pub unlocked: bool,
    /// Crystal cost to unlock in the shop (0 = free with archetype).
    pub crystal_cost: u64,
}

impl Ability {
    /// Apply mode-based scaling to this ability's damage value.
    ///
    /// Returns a scaled copy; does not mutate self.
    pub fn scaled(&self, mode_mult: f64) -> Self {
        let mut a = self.clone();
        // Damage scaled up in Learning mode, down in Serious mode
        a.damage = (self.damage as f64 * mode_mult).round() as i32;
        a
    }
}

// ── Default ability sets ──────────────────────────────────

/// Default abilities for each archetype, in unlock order.
/// The first ability is always unlocked.
pub fn default_abilities(archetype: Archetype) -> Vec<Ability> {
    match archetype {
        Archetype::Centurion => vec![
            Ability {
                id: "gladius".to_string(),
                name: "Gladius Strike".to_string(),
                damage: 20,
                energy_cost: 10,
                cooldown: 0,
                effective_against: Some(BugType::Syntax),
                unlocked: true,
                crystal_cost: 0,
            },
            Ability {
                id: "pilum".to_string(),
                name: "Pilum Throw".to_string(),
                damage: 35,
                energy_cost: 20,
                cooldown: 2,
                effective_against: Some(BugType::Logic),
                unlocked: false,
                crystal_cost: 50,
            },
            Ability {
                id: "testudo".to_string(),
                name: "Testudo Heal".to_string(),
                damage: -25,
                energy_cost: 15,
                cooldown: 3,
                effective_against: None,
                unlocked: false,
                crystal_cost: 75,
            },
            Ability {
                id: "decimatio".to_string(),
                name: "Decimatio".to_string(),
                damage: 60,
                energy_cost: 40,
                cooldown: 5,
                effective_against: Some(BugType::Security),
                unlocked: false,
                crystal_cost: 150,
            },
        ],
        Archetype::Architectus => vec![
            Ability {
                id: "blueprint".to_string(),
                name: "Blueprint Analysis".to_string(),
                damage: 15,
                energy_cost: 8,
                cooldown: 0,
                effective_against: Some(BugType::Performance),
                unlocked: true,
                crystal_cost: 0,
            },
            Ability {
                id: "refactor".to_string(),
                name: "Refactor Strike".to_string(),
                damage: 30,
                energy_cost: 18,
                cooldown: 2,
                effective_against: Some(BugType::Logic),
                unlocked: false,
                crystal_cost: 50,
            },
            Ability {
                id: "consolidate".to_string(),
                name: "Consolidate".to_string(),
                damage: -30,
                energy_cost: 20,
                cooldown: 3,
                effective_against: None,
                unlocked: false,
                crystal_cost: 80,
            },
        ],
        Archetype::Scriba => vec![
            Ability {
                id: "annotate".to_string(),
                name: "Annotate".to_string(),
                damage: 12,
                energy_cost: 6,
                cooldown: 0,
                effective_against: Some(BugType::Syntax),
                unlocked: true,
                crystal_cost: 0,
            },
            Ability {
                id: "codex".to_string(),
                name: "Codex Smash".to_string(),
                damage: 28,
                energy_cost: 16,
                cooldown: 2,
                effective_against: None,
                unlocked: false,
                crystal_cost: 45,
            },
            Ability {
                id: "restore_scroll".to_string(),
                name: "Restore Scroll".to_string(),
                damage: -20,
                energy_cost: 12,
                cooldown: 2,
                effective_against: None,
                unlocked: false,
                crystal_cost: 60,
            },
        ],
        Archetype::Legatus => vec![
            Ability {
                id: "command".to_string(),
                name: "Command Strike".to_string(),
                damage: 18,
                energy_cost: 10,
                cooldown: 0,
                effective_against: None,
                unlocked: true,
                crystal_cost: 0,
            },
            Ability {
                id: "rally".to_string(),
                name: "Rally".to_string(),
                damage: -35,
                energy_cost: 22,
                cooldown: 3,
                effective_against: None,
                unlocked: false,
                crystal_cost: 70,
            },
            Ability {
                id: "vanguard".to_string(),
                name: "Vanguard Assault".to_string(),
                damage: 45,
                energy_cost: 30,
                cooldown: 4,
                effective_against: Some(BugType::Security),
                unlocked: false,
                crystal_cost: 120,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_ability_always_unlocked() {
        for arch in [
            Archetype::Centurion,
            Archetype::Architectus,
            Archetype::Scriba,
            Archetype::Legatus,
        ] {
            let abilities = default_abilities(arch);
            assert!(
                abilities[0].unlocked,
                "{:?} first ability should be unlocked",
                arch
            );
        }
    }

    #[test]
    fn scaling_affects_damage() {
        let ability = Ability {
            id: "test".to_string(),
            name: "Test".to_string(),
            damage: 20,
            energy_cost: 10,
            cooldown: 0,
            effective_against: None,
            unlocked: true,
            crystal_cost: 0,
        };
        let scaled = ability.scaled(1.5);
        assert_eq!(scaled.damage, 30);
    }
}

#[cfg(test)]
mod semcov_wave1d_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn archetype_from_str_maps_known_and_defaults_unknown() {
        assert_eq!(archetype_from_str("architectus"), Archetype::Architectus);
        assert_eq!(archetype_from_str("scriba"), Archetype::Scriba);
        assert_eq!(archetype_from_str("legatus"), Archetype::Legatus);
        assert_eq!(archetype_from_str("centurion"), Archetype::Centurion);
        // Unknown input (including empty and wrong case) falls back to the default.
        assert_eq!(archetype_from_str("Centurion"), Archetype::Centurion);
        assert_eq!(archetype_from_str("paladin"), Archetype::Centurion);
        assert_eq!(archetype_from_str(""), Archetype::Centurion);
    }

    #[test]
    fn centurion_default_set_is_exact() {
        let abilities = default_abilities(Archetype::Centurion);
        // Exactly four abilities in canonical unlock order.
        assert_eq!(abilities.len(), 4);
        let ids: Vec<&str> = abilities.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["gladius", "pilum", "testudo", "decimatio"]);
        // Damage/heal values per the balance table.
        let damage: Vec<i32> = abilities.iter().map(|a| a.damage).collect();
        assert_eq!(damage, vec![20, 35, -25, 60]);
        // Exactly the first ability is unlocked; the rest require crystals.
        assert!(abilities[0].unlocked);
        assert!(abilities[1..].iter().all(|a| !a.unlocked));
        // Free starter, paid unlocks afterward.
        assert_eq!(abilities[0].crystal_cost, 0);
        assert!(abilities[1..].iter().all(|a| a.crystal_cost > 0));
    }
}
