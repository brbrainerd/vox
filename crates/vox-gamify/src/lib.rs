//! # vox-gamify
//!
//! Gamification layer for the Vox programming language.
//!
//! Provides code companions, daily quests, bug battles, ASCII sprites,
//! and a free multi-provider AI client (Pollinations / Ollama / Gemini).
//!
//! All features work fully offline with deterministic fallbacks.

pub mod achievement;
pub mod ai;
pub mod battle;
pub mod challenge;
pub mod companion;
pub mod cost;
pub mod db;
pub mod leaderboard;
pub mod notifications;
pub mod profile;
pub mod quest;
pub mod schema;
pub mod sprite;
pub mod streak;
pub mod util;

// Re-export key types for ergonomic access.
pub use achievement::{Achievement, AchievementTracker};
pub use ai::{AiError, FreeAiClient, FreeAiProvider};
pub use battle::{Battle, BugType};
pub use challenge::{Challenge, ChallengeManager, ChallengeType};
pub use companion::{Companion, Interaction, Mood};

pub use cost::{CostAggregator, CostRecord, CostSummary};
pub use leaderboard::{Leaderboard, LeaderboardMetric};
pub use notifications::{Notification, NotificationManager, NotificationType};
pub use profile::GamifyProfile;

pub use quest::{Quest, QuestType};
pub use schema::{SCHEMA_V5, SCHEMA_V6};
pub use streak::{StreakResult, StreakTracker};
