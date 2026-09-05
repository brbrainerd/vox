//! Database helpers shared across `crate::db` submodules.

use anyhow::Result;
use vox_db::migration::Migration;

/// Apply all Ludus-specific migrations to the given database.
pub async fn apply_ludus_migrations(db: &vox_db::Codex) -> Result<()> {
    let mut migrations = Vec::new();
    for (i, (version_label, sql)) in crate::schema::ALL_MIGRATIONS.iter().enumerate() {
        // We use versions 1000+ to avoid conflict with Arca builtin migrations (baseline is 1).
        migrations.push(Migration::new(
            1000 + i as i64,
            version_label.to_string(),
            sql.to_string(),
        ));
    }
    db.apply_migrations(&migrations).await?;
    Ok(())
}

/// Canonical user-identity normalisation.
///
/// Priority: non-empty `vox_config::local_user_id()` > `DEFAULT_USER_ID`.
/// All reward/event write paths MUST call this instead of constructing IDs inline.
pub fn canonical_user_id() -> String {
    let from_db = vox_config::local_user_id();
    if !from_db.is_empty() && from_db != "user" {
        from_db
    } else {
        crate::util::DEFAULT_USER_ID.to_string()
    }
}

/// Parse a quest-type string from DB without losing `agent_complete` / `collaborate`.
pub(super) fn parse_quest_type(s: &str) -> crate::quest::QuestType {
    use crate::quest::QuestType;
    match s {
        "create" => QuestType::Create,
        "review" => QuestType::Review,
        "battle" => QuestType::Battle,
        "improve" => QuestType::Improve,
        "agent_complete" => QuestType::AgentComplete,
        "collaborate" => QuestType::Collaborate,
        "ai_feedback" => QuestType::AiFeedback,
        "populi_contribute" => QuestType::PopuliContribute,
        "build_streak" => QuestType::BuildStreak,
        "doc_sprint" => QuestType::DocSprint,
        "toestub_fix" => QuestType::ToestubFix,
        "testing" => QuestType::Testing,
        "research" => QuestType::Research,
        "first_time" => QuestType::FirstTime,
        other => {
            tracing::warn!("unknown quest_type '{}' in DB, defaulting to Create", other);
            QuestType::Create
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_user_id_never_placeholder() {
        let id = canonical_user_id();
        assert!(!id.is_empty(), "canonical_user_id() must never be empty");
        assert_ne!(
            id, "user",
            "the generic 'user' id must fall back to DEFAULT_USER_ID"
        );
    }

    #[test]
    fn parse_quest_type_falls_back_to_create() {
        use crate::quest::QuestType;
        assert!(matches!(parse_quest_type("battle"), QuestType::Battle));
        assert!(matches!(parse_quest_type("nonsense"), QuestType::Create));
    }
}
