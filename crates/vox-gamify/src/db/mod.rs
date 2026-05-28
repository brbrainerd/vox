//! Database access for Ludus gamification tables.

mod agent_telemetry;
mod arena;
mod collegium;
mod companion;
mod counters;
mod dedupe;
mod feedback;
mod helpers;
mod hint_telemetry;
mod kpi;
mod leaderboards;
mod notifications;
mod periodic;
mod process_rewards;
mod profile;
mod quest_battle;
mod teaching;

pub use agent_telemetry::{
    AgentEventRecord, AgentSessionRecord, CostRecord, acknowledge_message, end_agent_session,
    get_agent_cost_usd, get_agent_metrics, get_agent_session, get_events,
    get_tenant_monthly_token_usage, insert_agent_session, insert_cost_record, insert_event,
    list_active_sessions, list_cost_records, update_agent_session, upsert_agent_metric,
};
pub use arena::{
    ArenaEvent, arena_event_leaderboard, get_active_arena_event, get_arena_contribution,
    join_arena_event,
};
pub use collegium::{
    create_collegium, get_collegium, get_user_collegium, join_collegium, list_collegiums,
    update_collegium_lumens,
};
pub use companion::{
    delete_companion, get_companion, list_companions, migrate_default_orchestrator_companion_id,
    upsert_companion,
};
pub use counters::{get_counter, increment_counter, set_counter};
pub use dedupe::try_claim_processed_event;
pub use feedback::insert_feedback;
pub use helpers::{apply_ludus_migrations, canonical_user_id};
pub use hint_telemetry::log_hint_event;
pub use kpi::{
    PolicySnapshotRow, list_policy_snapshots_since_days, list_recent_policy_snapshots,
    load_kpi_summary,
};
pub use leaderboards::{PlayerRankEntry, get_profile_stats, leaderboard, lumens_leaderboard};
pub use notifications::{
    cleanup_expired_notifications, insert_notification, list_unread_notifications,
    mark_all_notifications_read, mark_notification_read, mark_notification_read_for_user,
};
pub use periodic::{get_reward_claim, upsert_periodic_reward};
pub use process_rewards::process_event_rewards;
pub use profile::{
    get_profile, list_unlocked_achievements, merge_default_profile_into_user, record_level_up,
    unlock_achievement, upsert_profile,
};
pub use quest_battle::{
    count_battles, count_quests, delete_quest, get_battle, get_quest, insert_battle, list_battles,
    list_quests, update_battle, update_quest_status, upsert_quest,
};
pub use teaching::{get_teaching_profile, insert_policy_snapshot, upsert_teaching_profile};
mod disputes;
pub use disputes::*;
