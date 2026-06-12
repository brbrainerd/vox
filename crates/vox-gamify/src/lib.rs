//! # vox-gamify
//!
//! Gamification layer for the Vox programming language.
//!
//! Provides code companions, daily quests, bug battles, ASCII sprites,
//! and a free multi-provider AI client (Pollinations / Ollama / Gemini).
//!
//! All features work fully offline with deterministic fallbacks.

#![allow(clippy::type_complexity)]
#![allow(clippy::collapsible_if)]

pub mod ability; // toestub-ignore(unwired/module)
pub mod achievement; // toestub-ignore(unwired/module)
pub mod ai; // toestub-ignore(unwired/module)
pub mod battle; // toestub-ignore(unwired/module)
pub mod challenge; // toestub-ignore(unwired/module)
pub mod combat; // toestub-ignore(unwired/module)
pub mod combo; // toestub-ignore(unwired/module)
pub mod companion; // toestub-ignore(unwired/module)
pub mod config_gate; // toestub-ignore(unwired/module)
pub mod cost; // toestub-ignore(unwired/module)
pub mod db; // toestub-ignore(unwired/module)
pub mod db_ext; // toestub-ignore(unwired/module)
pub mod discovery; // toestub-ignore(unwired/module)
pub mod event_router; // toestub-ignore(unwired/module)
pub mod feedback; // toestub-ignore(unwired/module)
pub mod ingest; // toestub-ignore(unwired/module)
pub mod kpi; // toestub-ignore(unwired/module)
pub mod leaderboard; // toestub-ignore(unwired/module)
pub mod lex_pack; // toestub-ignore(unwired/module)
pub mod lsp_telemetry; // toestub-ignore(unwired/module)
pub mod mcp_privacy; // toestub-ignore(unwired/module)
pub mod notifications; // toestub-ignore(unwired/module)
pub mod output_policy; // toestub-ignore(unwired/module)
pub mod periodic_reward; // toestub-ignore(unwired/module)
pub mod profile; // toestub-ignore(unwired/module)
pub mod quest; // toestub-ignore(unwired/module)
pub mod quest_engine; // toestub-ignore(unwired/module)
pub mod reward_policy; // toestub-ignore(unwired/module)
pub mod run; // toestub-ignore(unwired/module)
pub mod schema; // toestub-ignore(unwired/module)
pub mod shop; // toestub-ignore(unwired/module)
pub mod simulation; // toestub-ignore(unwired/module)
pub mod sprite; // toestub-ignore(unwired/module)
pub mod sprite_svg; // toestub-ignore(unwired/module)
pub mod streak; // toestub-ignore(unwired/module)
pub mod teaching; // toestub-ignore(unwired/module)
pub mod util; // toestub-ignore(unwired/module)

// Re-export key types for ergonomic access.
pub use achievement::{Achievement, AchievementTracker};
pub use ai::{AiError, FreeAiClient, FreeAiProvider, LudusStreamBackend, StreamRoute};
pub use battle::{Battle, BugType};
pub use challenge::{Challenge, ChallengeManager, ChallengeType};
pub use companion::{Companion, Interaction, Mood};
pub use cost::{CostAggregator, CostSummary};
pub use db::{ArenaEvent, CostRecord, PlayerRankEntry, leaderboard, lumens_leaderboard};
pub use feedback::{AiFeedback, DAILY_FEEDBACK_CAP, xp_for_feedback};
pub use leaderboard::{Leaderboard, LeaderboardMetric};
pub use lex_pack::{LexGlyph, LexPack, LumensWeight, load_lex_pack, save_lex_pack};
pub use notifications::{Notification, NotificationManager, NotificationType};
pub use periodic_reward::{
    PeriodicCondition, PeriodicReward, current_weekly_reward, generate_weekly_reward,
};
pub use profile::{
    LudusProfile, full_title, level_from_xp, prestige_title, title_for_level, xp_for_level,
    xp_threshold_for_level,
};

/// Renamed to [`LudusProfile`]. This alias will be removed in a future release.
#[deprecated(note = "Renamed to LudusProfile")]
pub type GamifyProfile = LudusProfile;

pub use db_ext::{
    get_daily_counter, increment_daily_counter, load_event_config_overrides,
    set_event_config_override,
};
pub use ingest::ingest_orchestrator_event;
pub use kpi::LudusKpiSummary;
pub use quest::{
    DAILY_QUEST_COUNT, QUEST_TEMPLATES, Quest, QuestModifier, QuestTemplate, QuestType,
    generate_daily_quests, slot_fill, todays_quests,
};
pub use run::{
    BattleFinding, BattleStartOutcome, BattleSubmitOutcome, BattleSubmitResult, run_battle_start,
    run_battle_submit,
};
pub use schema::{
    ALL_MIGRATIONS, SCHEMA_V5, SCHEMA_V6, SCHEMA_V7, SCHEMA_V8, SCHEMA_V9, SCHEMA_V10, SCHEMA_V11,
    SCHEMA_V14, SCHEMA_V14B, SCHEMA_V15, SCHEMA_V16, SCHEMA_V17, SCHEMA_V18, SCHEMA_V19,
    SCHEMA_V20, SCHEMA_V21, SCHEMA_V22, SCHEMA_V23, SCHEMA_V24, SCHEMA_V25,
};
pub use streak::{StreakResult, StreakTracker};
