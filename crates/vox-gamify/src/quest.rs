//! Daily quest system with templated generation and progress tracking.

use crate::util::now_unix;
use serde::{Deserialize, Serialize};

// ─── Constants ───────────────────────────────────────────

/// Number of quests generated per day.
const DAILY_QUEST_COUNT: usize = 3;

/// Quest expiration: 24 hours in seconds.
const QUEST_DURATION_SECS: i64 = 86_400;

// ─── Quest Type ──────────────────────────────────────────

/// Categories of daily quests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestType {
    /// Create N new components/companions.
    Create,
    /// Complete N peer reviews.
    Review,
    /// Win N bug battles.
    Battle,
    /// Improve code quality of N components.
    Improve,
    /// Complete N tasks without errors.
    AgentComplete,
    /// Hand off plan to another agent successfully.
    Collaborate,
}

impl QuestType {
    /// All quest types, for iteration.
    pub const ALL: &'static [QuestType] = &[
        QuestType::Create,
        QuestType::Review,
        QuestType::Battle,
        QuestType::Improve,
        QuestType::AgentComplete,
        QuestType::Collaborate,
    ];

    /// Slug for DB storage and display.
    pub const fn as_str(&self) -> &str {
        match self {
            QuestType::Create => "create",
            QuestType::Review => "review",
            QuestType::Battle => "battle",
            QuestType::Improve => "improve",
            QuestType::AgentComplete => "agent_complete",
            QuestType::Collaborate => "collaborate",
        }
    }

    /// Emoji icon.
    pub fn emoji(&self) -> &str {
        match self {
            QuestType::Create => "🔨",
            QuestType::Review => "📝",
            QuestType::Battle => "⚔️",
            QuestType::Improve => "📈",
            QuestType::AgentComplete => "✅",
            QuestType::Collaborate => "🤝",
        }
    }
}

impl std::fmt::Display for QuestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Quest Template ──────────────────────────────────────

/// A template for generating quests, with difficulty tiers.
#[derive(Debug, Clone)]
pub struct QuestTemplate {
    pub quest_type: QuestType,
    pub description: &'static str,
    pub target: u32,
    pub crystal_reward: u64,
    pub xp_reward: u64,
}

/// All available quest templates (4 types × 3 difficulty levels).
pub const QUEST_TEMPLATES: &[QuestTemplate] = &[
    // Create quests
    QuestTemplate {
        quest_type: QuestType::Create,
        description: "Create a new companion",
        target: 1,
        crystal_reward: 10,
        xp_reward: 15,
    },
    QuestTemplate {
        quest_type: QuestType::Create,
        description: "Create 2 new companions",
        target: 2,
        crystal_reward: 25,
        xp_reward: 30,
    },
    QuestTemplate {
        quest_type: QuestType::Create,
        description: "Create 3 new companions",
        target: 3,
        crystal_reward: 40,
        xp_reward: 50,
    },
    // Review quests
    QuestTemplate {
        quest_type: QuestType::Review,
        description: "Complete a peer review",
        target: 1,
        crystal_reward: 15,
        xp_reward: 20,
    },
    QuestTemplate {
        quest_type: QuestType::Review,
        description: "Complete 2 peer reviews",
        target: 2,
        crystal_reward: 30,
        xp_reward: 40,
    },
    QuestTemplate {
        quest_type: QuestType::Review,
        description: "Complete 3 peer reviews",
        target: 3,
        crystal_reward: 50,
        xp_reward: 60,
    },
    // Battle quests
    QuestTemplate {
        quest_type: QuestType::Battle,
        description: "Win a bug battle",
        target: 1,
        crystal_reward: 10,
        xp_reward: 15,
    },
    QuestTemplate {
        quest_type: QuestType::Battle,
        description: "Win 2 bug battles",
        target: 2,
        crystal_reward: 25,
        xp_reward: 35,
    },
    QuestTemplate {
        quest_type: QuestType::Battle,
        description: "Win 3 bug battles",
        target: 3,
        crystal_reward: 45,
        xp_reward: 55,
    },
    // Improve quests
    QuestTemplate {
        quest_type: QuestType::Improve,
        description: "Improve a companion's code quality",
        target: 1,
        crystal_reward: 20,
        xp_reward: 25,
    },
    QuestTemplate {
        quest_type: QuestType::Improve,
        description: "Improve 2 companions' code quality",
        target: 2,
        crystal_reward: 35,
        xp_reward: 45,
    },
    QuestTemplate {
        quest_type: QuestType::Improve,
        description: "Improve 3 companions' code quality",
        target: 3,
        crystal_reward: 55,
        xp_reward: 70,
    },
    // AgentComplete
    QuestTemplate {
        quest_type: QuestType::AgentComplete,
        description: "Complete a task without errors.",
        target: 1,
        crystal_reward: 30,
        xp_reward: 40,
    },
    QuestTemplate {
        quest_type: QuestType::AgentComplete,
        description: "Complete 3 tasks without errors.",
        target: 3,
        crystal_reward: 80,
        xp_reward: 100,
    },
    QuestTemplate {
        quest_type: QuestType::AgentComplete,
        description: "Complete 5 tasks without errors.",
        target: 5,
        crystal_reward: 150,
        xp_reward: 200,
    },
    // Collaborate
    QuestTemplate {
        quest_type: QuestType::Collaborate,
        description: "Hand off plan to another agent successfully.",
        target: 1,
        crystal_reward: 20,
        xp_reward: 35,
    },
    QuestTemplate {
        quest_type: QuestType::Collaborate,
        description: "Hand off plans to another agent 3 times successfully.",
        target: 3,
        crystal_reward: 70,
        xp_reward: 90,
    },
    QuestTemplate {
        quest_type: QuestType::Collaborate,
        description: "Hand off plans to another agent 5 times successfully.",
        target: 5,
        crystal_reward: 120,
        xp_reward: 160,
    },
];

// ─── Quest ───────────────────────────────────────────────

/// An active quest instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub user_id: String,
    pub quest_type: QuestType,
    pub description: String,
    pub target: u32,
    pub progress: u32,
    pub crystal_reward: u64,
    pub xp_reward: u64,
    pub completed: bool,
    pub expires_at: i64, // unix timestamp
}

impl Quest {
    /// Create a quest from a template.
    pub fn from_template(
        id: impl Into<String>,
        user_id: impl Into<String>,
        template: &QuestTemplate,
    ) -> Self {
        let now = now_unix();
        Self {
            id: id.into(),
            user_id: user_id.into(),
            quest_type: template.quest_type,
            description: template.description.to_string(),
            target: template.target,
            progress: 0,
            crystal_reward: template.crystal_reward,
            xp_reward: template.xp_reward,
            completed: false,
            expires_at: now + QUEST_DURATION_SECS,
        }
    }

    /// Increment progress. Returns true if the quest just completed.
    pub fn increment(&mut self, amount: u32) -> bool {
        if self.completed {
            return false;
        }
        self.progress = (self.progress + amount).min(self.target);
        if self.progress >= self.target {
            self.completed = true;
            true
        } else {
            false
        }
    }

    /// Check if this quest has expired.
    pub fn is_expired(&self) -> bool {
        now_unix() > self.expires_at
    }

    /// Progress as a percentage (0.0 - 1.0).
    pub fn progress_pct(&self) -> f64 {
        if self.target == 0 {
            return 1.0;
        }
        self.progress as f64 / self.target as f64
    }

    /// Hint text for how to complete this quest.
    pub fn hint(&self) -> &str {
        match self.quest_type {
            QuestType::Create => "Use `vox gamify companion create` to make a new companion",
            QuestType::Review => "Review a peer's code submission to earn credit",
            QuestType::Battle => "Enter a bug battle with `vox gamify battle start`",
            QuestType::Improve => "Fix TOESTUB findings to raise your companion's code quality",
            QuestType::AgentComplete => "Successfully submit and complete an orchestrator task",
            QuestType::Collaborate => "Use the `vox_agent_handoff` tool to coordinate with peers",
        }
    }
}

/// Generate daily quests by picking random templates.
///
/// Uses a simple deterministic shuffle based on the day to ensure
/// the same quests appear for the same user on the same day.
pub fn generate_daily_quests(user_id: &str, seed: u64) -> Vec<Quest> {
    let templates = QUEST_TEMPLATES;
    let count = DAILY_QUEST_COUNT.min(templates.len());

    // Simple deterministic selection: spread across quest types
    let mut selected = Vec::with_capacity(count);
    let mut type_idx = (seed as usize) % QuestType::ALL.len();

    for i in 0..count {
        let target_type = QuestType::ALL[type_idx % QuestType::ALL.len()];
        // Find a template of this type, cycling through difficulty
        let type_templates: Vec<&QuestTemplate> = templates
            .iter()
            .filter(|t| t.quest_type == target_type)
            .collect();

        if let Some(template) = type_templates.get(((seed as usize) + i) % type_templates.len()) {
            let id = format!("quest-{}-{}-{}", user_id, seed, i);
            selected.push(Quest::from_template(id, user_id, template));
        }

        type_idx += 1;
    }

    selected
}

// ─── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_from_template() {
        let q = Quest::from_template("q-1", "u-1", &QUEST_TEMPLATES[0]);
        assert_eq!(q.quest_type, QuestType::Create);
        assert_eq!(q.target, 1);
        assert_eq!(q.progress, 0);
        assert!(!q.completed);
    }

    #[test]
    fn quest_increment_and_complete() {
        let mut q = Quest::from_template("q-1", "u-1", &QUEST_TEMPLATES[0]);
        assert!(!q.increment(0));
        assert!(q.increment(1)); // target=1, now complete
        assert!(q.completed);
        assert!(!q.increment(1)); // Already complete, no-op
    }

    #[test]
    fn quest_progress_capped() {
        let mut q = Quest::from_template("q-1", "u-1", &QUEST_TEMPLATES[0]);
        q.increment(10); // Over target
        assert_eq!(q.progress, 1); // Capped at target
    }

    #[test]
    fn quest_progress_pct() {
        let mut q = Quest::from_template("q-1", "u-1", &QUEST_TEMPLATES[1]); // target=2
        q.increment(1);
        assert!((q.progress_pct() - 0.5).abs() < 0.01);
    }

    #[test]
    fn generate_daily_quests_count() {
        let quests = generate_daily_quests("u-1", 42);
        assert_eq!(quests.len(), DAILY_QUEST_COUNT);
    }

    #[test]
    fn generate_daily_quests_deterministic() {
        let q1 = generate_daily_quests("u-1", 42);
        let q2 = generate_daily_quests("u-1", 42);
        assert_eq!(q1.len(), q2.len());
        for (a, b) in q1.iter().zip(q2.iter()) {
            assert_eq!(a.quest_type, b.quest_type);
            assert_eq!(a.target, b.target);
        }
    }

    #[test]
    fn quest_type_all() {
        assert_eq!(QuestType::ALL.len(), 6);
    }

    #[test]
    fn quest_templates_cover_all_types() {
        for qt in QuestType::ALL {
            let count = QUEST_TEMPLATES
                .iter()
                .filter(|t| t.quest_type == *qt)
                .count();
            assert!(count >= 3, "{:?} has only {} templates", qt, count);
        }
    }
}
