//! Achievement system for gamifying agent activities.
//!
//! Tracks milestones like first task completion, first handoff,
//! error-free streaks, and cost efficiency. Achievements are
//! persisted and shown on the dashboard.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique achievement identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AchievementId(pub String);

impl std::fmt::Display for AchievementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An achievement that can be unlocked by agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    /// Unique identifier.
    pub id: AchievementId,
    /// Display name.
    pub name: String,
    /// Description of how to unlock.
    pub description: String,
    /// Emoji icon.
    pub icon: String,
    /// Category of achievement.
    pub category: AchievementCategory,
    /// XP reward for unlocking.
    pub xp_reward: u32,
    /// Crystal reward for unlocking.
    pub crystal_reward: u32,
}

/// Achievement categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementCategory {
    /// Task-related milestones.
    Tasks,
    /// Collaboration milestones.
    Collaboration,
    /// Efficiency milestones.
    Efficiency,
    /// Exploration milestones.
    Discovery,
    /// Streak milestones.
    Streaks,
}

/// Record of an unlocked achievement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockedAchievement {
    pub achievement_id: AchievementId,
    pub agent_id: String,
    pub unlocked_at: u64,
}

/// Tracks achievements per agent.
#[derive(Debug, Default)]
pub struct AchievementTracker {
    /// All available achievements.
    definitions: Vec<Achievement>,
    /// Per-agent unlocked achievements.
    unlocked: HashMap<String, Vec<UnlockedAchievement>>,
    /// Per-agent counters for tracking progress.
    counters: HashMap<String, HashMap<String, u32>>,
}

impl AchievementTracker {
    /// Create a new tracker with the default achievement set.
    pub fn new() -> Self {
        let mut tracker = Self::default();
        tracker.register_defaults();
        tracker
    }

    /// Register the default set of achievements.
    fn register_defaults(&mut self) {
        let defaults = vec![
            Achievement {
                id: AchievementId("first_task".into()),
                name: "Hello World".into(),
                description: "Complete your first task".into(),
                icon: "🎯".into(),
                category: AchievementCategory::Tasks,
                xp_reward: 50,
                crystal_reward: 10,
            },
            Achievement {
                id: AchievementId("five_tasks".into()),
                name: "Getting Started".into(),
                description: "Complete 5 tasks".into(),
                icon: "⭐".into(),
                category: AchievementCategory::Tasks,
                xp_reward: 100,
                crystal_reward: 25,
            },
            Achievement {
                id: AchievementId("twenty_five_tasks".into()),
                name: "Workhorse".into(),
                description: "Complete 25 tasks".into(),
                icon: "🏆".into(),
                category: AchievementCategory::Tasks,
                xp_reward: 500,
                crystal_reward: 100,
            },
            Achievement {
                id: AchievementId("first_handoff".into()),
                name: "Team Player".into(),
                description: "Successfully hand off a plan to another agent".into(),
                icon: "🤝".into(),
                category: AchievementCategory::Collaboration,
                xp_reward: 75,
                crystal_reward: 15,
            },
            Achievement {
                id: AchievementId("error_free_five".into()),
                name: "Flawless Five".into(),
                description: "Complete 5 tasks in a row without errors".into(),
                icon: "💎".into(),
                category: AchievementCategory::Streaks,
                xp_reward: 200,
                crystal_reward: 50,
            },
            Achievement {
                id: AchievementId("budget_saver".into()),
                name: "Budget Conscious".into(),
                description: "Complete 10 tasks under $0.01 total cost".into(),
                icon: "💰".into(),
                category: AchievementCategory::Efficiency,
                xp_reward: 150,
                crystal_reward: 30,
            },
            Achievement {
                id: AchievementId("first_continuation".into()),
                name: "Self Starter".into(),
                description: "Receive an auto-continuation prompt".into(),
                icon: "▶️".into(),
                category: AchievementCategory::Discovery,
                xp_reward: 25,
                crystal_reward: 5,
            },
            Achievement {
                id: AchievementId("speed_demon".into()),
                name: "Speed Demon".into(),
                description: "Complete a task in under 30 seconds".into(),
                icon: "⚡".into(),
                category: AchievementCategory::Efficiency,
                xp_reward: 100,
                crystal_reward: 20,
            },
            Achievement {
                id: AchievementId("streak_7".into()),
                name: "Week Warrior".into(),
                description: "Maintain a 7-day activity streak".into(),
                icon: "🔥".into(),
                category: AchievementCategory::Streaks,
                xp_reward: 300,
                crystal_reward: 75,
            },
            Achievement {
                id: AchievementId("streak_30".into()),
                name: "Month Master".into(),
                description: "Maintain a 30-day activity streak".into(),
                icon: "🌕".into(),
                category: AchievementCategory::Streaks,
                xp_reward: 1000,
                crystal_reward: 250,
            },
            Achievement {
                id: AchievementId("challenge_solved".into()),
                name: "Challenger".into(),
                description: "Successfully solve a daily coding challenge".into(),
                icon: "🧩".into(),
                category: AchievementCategory::Discovery,
                xp_reward: 200,
                crystal_reward: 50,
            },
            Achievement {
                id: AchievementId("first_memory".into()),
                name: "Elephant Memory".into(),
                description: "Store your first long-term memory entry".into(),
                icon: "🧠".into(),
                category: AchievementCategory::Discovery,
                xp_reward: 25,
                crystal_reward: 5,
            },
            Achievement {
                id: AchievementId("polyglot".into()),
                name: "Polyglot Programmer".into(),
                description: "Work on files in 5 different programming languages".into(),
                icon: "🌍".into(),
                category: AchievementCategory::Discovery,
                xp_reward: 150,
                crystal_reward: 30,
            },
        ];

        self.definitions = defaults;
    }

    /// Increment a counter for an agent and check for unlocks.
    pub fn increment_counter(&mut self, agent_id: &str, counter: &str) -> Vec<Achievement> {
        let count = {
            let c = self
                .counters
                .entry(agent_id.to_string())
                .or_default()
                .entry(counter.to_string())
                .or_insert(0);
            *c += 1;
            *c
        };

        self.check_unlocks(agent_id, counter, count)
    }

    /// Check if any achievements should unlock based on a counter value.
    fn check_unlocks(&mut self, agent_id: &str, counter: &str, value: u32) -> Vec<Achievement> {
        let mut unlocked = Vec::new();

        let thresholds: Vec<(&str, u32)> = match counter {
            "tasks_completed" => vec![
                ("first_task", 1),
                ("five_tasks", 5),
                ("twenty_five_tasks", 25),
            ],
            "handoffs_completed" => vec![("first_handoff", 1)],
            "error_free_streak" => vec![("error_free_five", 5)],
            "continuations_received" => vec![("first_continuation", 1)],
            "activity_streak" => vec![("streak_7", 7), ("streak_30", 30)],
            "challenges_solved" => vec![("challenge_solved", 1)],
            "memory_entries" => vec![("first_memory", 1)],
            "langs_used" => vec![("polyglot", 5)],
            _ => vec![],
        };

        for (achievement_id, threshold) in thresholds {
            if value >= threshold && !self.has_achievement(agent_id, achievement_id) {
                if let Some(achievement) =
                    self.definitions.iter().find(|a| a.id.0 == achievement_id)
                {
                    let record = UnlockedAchievement {
                        achievement_id: AchievementId(achievement_id.to_string()),
                        agent_id: agent_id.to_string(),
                        unlocked_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    };
                    self.unlocked
                        .entry(agent_id.to_string())
                        .or_default()
                        .push(record);
                    unlocked.push(achievement.clone());
                }
            }
        }

        unlocked
    }

    /// Check if an agent has a specific achievement.
    pub fn has_achievement(&self, agent_id: &str, achievement_id: &str) -> bool {
        self.unlocked
            .get(agent_id)
            .map(|list| list.iter().any(|a| a.achievement_id.0 == achievement_id))
            .unwrap_or(false)
    }

    /// Get all unlocked achievements for an agent.
    pub fn agent_achievements(&self, agent_id: &str) -> Vec<&Achievement> {
        let unlocked_ids: Vec<&str> = self
            .unlocked
            .get(agent_id)
            .map(|list| list.iter().map(|a| a.achievement_id.0.as_str()).collect())
            .unwrap_or_default();

        self.definitions
            .iter()
            .filter(|a| unlocked_ids.contains(&a.id.0.as_str()))
            .collect()
    }

    /// Get all available achievements.
    pub fn all_achievements(&self) -> &[Achievement] {
        &self.definitions
    }

    /// Get the counter value for an agent.
    pub fn counter_value(&self, agent_id: &str, counter: &str) -> u32 {
        self.counters
            .get(agent_id)
            .and_then(|c| c.get(counter))
            .copied()
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_task_achievement() {
        let mut tracker = AchievementTracker::new();
        let unlocked = tracker.increment_counter("agent-1", "tasks_completed");

        assert_eq!(unlocked.len(), 1);
        assert_eq!(unlocked[0].id.0, "first_task");
        assert!(tracker.has_achievement("agent-1", "first_task"));
    }

    #[test]
    fn no_duplicate_unlock() {
        let mut tracker = AchievementTracker::new();
        tracker.increment_counter("agent-1", "tasks_completed");
        let unlocked = tracker.increment_counter("agent-1", "tasks_completed");
        // Second increment should not re-unlock
        assert!(unlocked.is_empty());
    }

    #[test]
    fn multiple_achievements_at_threshold() {
        let mut tracker = AchievementTracker::new();
        for _ in 0..4 {
            tracker.increment_counter("agent-1", "tasks_completed");
        }
        let unlocked = tracker.increment_counter("agent-1", "tasks_completed");
        // At 5 tasks: "five_tasks" unlocks
        assert_eq!(unlocked.len(), 1);
        assert_eq!(unlocked[0].id.0, "five_tasks");

        assert_eq!(tracker.agent_achievements("agent-1").len(), 2);
    }

    #[test]
    fn counter_tracking() {
        let mut tracker = AchievementTracker::new();
        tracker.increment_counter("agent-1", "tasks_completed");
        tracker.increment_counter("agent-1", "tasks_completed");
        assert_eq!(tracker.counter_value("agent-1", "tasks_completed"), 2);
    }
}
