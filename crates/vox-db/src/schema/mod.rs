//! Arca SQL schema — ordered domain fragments + [`BASELINE_VERSION`] (SSOT).
//!
//! Canonical DDL is defined by `SCHEMA_FRAGMENTS` and applied via `baseline_sql()`.
//! Fresh databases store a `schema_version` row at [`BASELINE_VERSION`] on first open.
//! Idempotent `schema_cutover` runs after migrate for column-level alignment on existing files.

mod manifest;
mod pragmas;

pub mod domains;
pub mod spec;

pub use manifest::{
    BASELINE_VERSION, CODEX_API_REQUIRED_TABLES, CODEX_CHAT_TABLES, CODEX_REACTIVITY_TABLES,
    SCHEMA_FRAGMENTS, SchemaFragment, baseline_sql, schema_baseline_digest_hex,
};
pub use spec::orchestrator_schema_digest;

#[cfg(test)]
mod migration_chain_tests {
    use super::SCHEMA_FRAGMENTS;

    #[test]
    fn chat_search_and_codex_in_fragments() {
        let conversations = SCHEMA_FRAGMENTS
            .iter()
            .find(|f| f.name == "conversations")
            .map(|f| f.sql)
            .expect("conversations");
        assert!(
            conversations.contains("conversations")
                && conversations.contains("conversation_messages"),
            "conversations must define chat tables"
        );
        let knowledge = SCHEMA_FRAGMENTS
            .iter()
            .find(|f| f.name == "knowledge")
            .map(|f| f.sql)
            .expect("knowledge");
        assert!(
            knowledge.contains("search_documents"),
            "knowledge must define search tables"
        );
        let agents = SCHEMA_FRAGMENTS
            .iter()
            .find(|f| f.name == "agents")
            .map(|f| f.sql)
            .expect("agents");
        assert!(
            agents.contains("agent_sessions"),
            "agents must define agent DDL"
        );
        assert!(
            agents.contains("agent_session_events") && agents.contains("agent_events"),
            "agents must define session + telemetry tables"
        );
        let sql = super::baseline_sql();
        assert!(
            sql.contains("populi_training_run"),
            "baseline_sql must include spec-appended training DDL"
        );
        assert!(
            sql.contains("idx_memories_agent_created")
                && sql.contains("idx_a2a_ack_created")
                && sql.contains("idx_news_publish_attempts_news"),
            "baseline_sql must embed former cutover performance indexes (domain DDL SSOT)"
        );
        assert!(
            sql.contains("agent_exec_history"),
            "DEBUG_ASSERT: agent_exec_history must be in baseline_sql. SQL is:\n{}",
            sql
        );
    }
}
