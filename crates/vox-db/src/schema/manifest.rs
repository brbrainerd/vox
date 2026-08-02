//! Single-source Arca/Codex schema: ordered fragments, baseline SQL, digest, and capability metadata.
//!
//! **SSOT:** This manifest defines the current global Arca schema collapsed into domain fragments.
//! Baseline version is [`BASELINE_VERSION`] (see monolithic DDL in `baseline_sql()`).

use super::domains;
use super::spec;
use sha3::{Digest, Keccak256};
use std::sync::OnceLock;

// 77: feat(vox-kb): add knowledge_bases, kb_entries, kb_routing_rules tables
// 78: feat(activity): add activity_log table for timeline events
// 79: (prior scientia additions)
// 80: feat(telemetry-F): add model_prompt_profiles table (Track F learned prompt profiles)
// 81: feat(capture): add agent_operations table (operation capture sub-project 1)
// 82: feat(skill-discovery): add skill_candidates table (Task 3.2, harness parity plan)
// 83: feat(skill-discovery): add lifecycle_state + source_hash to skill_candidates (Task 3.3, harness parity plan)
// 84: feat(skill-discovery): add skill_identities table (Task 3.4, harness parity plan)
// 85: refactor(schema-condensation): quarantine 42 DDL-bearing DORMANT/DEAD tables into
//     domains::quarantine, gated by the `quarantine` feature (off by default); handoff_payloads'
//     CollectionInfo entry gated the same way (Task 4, VoxDB audit condensation plan)
// 86: feat(vox-db): harness_eval_run/task_result/model_selection_event tables
pub const BASELINE_VERSION: i64 = 86;

/// One ordered SQL slice (domain-scoped DDL); empty bodies are skipped in [`baseline_sql`].
#[derive(Debug, Clone, Copy)]
pub struct SchemaFragment {
    /// Stable fragment name (e.g., "agents").
    pub name: &'static str,
    /// DDL/DML run through Turso `execute_batch` (no row-returning statements).
    pub sql: &'static str,
}

/// Baseline tables required for Codex HTTP API (ready check).
pub const CODEX_API_REQUIRED_TABLES: &[&str] = &[
    "conversations",
    "conversation_messages",
    "search_documents",
    "agent_sessions",
    "agent_events",
    "distributed_locks",
];

/// Subset of CORE chat tables for cleanup and diagnostics.
pub const CODEX_CHAT_TABLES: &[&str] = &["conversations", "conversation_messages"];

/// Tables that trigger reactivity/SSE broadcast on mutate.
pub const CODEX_REACTIVITY_TABLES: &[&str] = &[
    "conversation_messages",
    "agent_sessions",
    "behavior_events",
    "distributed_locks",
];

/// All non-empty SQL fragments to be applied as the monolithic baseline DDL.
pub const SCHEMA_FRAGMENTS: &[SchemaFragment] = &[
    SchemaFragment {
        name: "foundation",
        sql: domains::foundation::SCHEMA_FOUNDATION,
    },
    SchemaFragment {
        name: "secrets_cloudless",
        sql: domains::secrets_cloudless::SCHEMA_SECRETS_CLOUDLESS,
    },
    SchemaFragment {
        name: "cas_codex",
        sql: domains::cas_codex::SCHEMA_CAS_CODEX,
    },
    SchemaFragment {
        name: "conversations",
        sql: domains::conversations::SCHEMA_CONVERSATIONS,
    },
    SchemaFragment {
        name: "knowledge",
        sql: domains::knowledge::SCHEMA_KNOWLEDGE,
    },
    SchemaFragment {
        name: "execution",
        sql: domains::execution::SCHEMA_EXECUTION,
    },
    SchemaFragment {
        name: "external_review",
        sql: domains::external_review::SCHEMA_EXTERNAL_REVIEW,
    },
    SchemaFragment {
        name: "agents",
        sql: domains::agents::SCHEMA_AGENTS,
    },
    SchemaFragment {
        name: "packages",
        sql: domains::packages::SCHEMA_PACKAGES,
    },
    SchemaFragment {
        name: "gamification_coordination",
        sql: domains::gamification_coordination::SCHEMA_GAMIFICATION_COORDINATION,
    },
    SchemaFragment {
        name: "toestub_build",
        sql: domains::toestub_build::SCHEMA_TOESTUB_BUILD,
    },
    SchemaFragment {
        name: "ci_completion",
        sql: domains::ci_completion::SCHEMA_CI_COMPLETION,
    },
    SchemaFragment {
        name: "publish_cloud",
        sql: domains::publish_cloud::SCHEMA_PUBLISH_CLOUD,
    },
    SchemaFragment {
        name: "mens_intelligence",
        sql: domains::mens_intelligence::SCHEMA_MENS_INTELLIGENCE,
    },
    SchemaFragment {
        name: "exec_time",
        sql: domains::exec_time::SCHEMA_EXEC_TIME,
    },
    SchemaFragment {
        name: "scientia",
        sql: domains::scientia::SCHEMA_SCIENTIA,
    },
    SchemaFragment {
        name: "harness_eval",
        sql: domains::harness_eval::SCHEMA_HARNESS_EVAL,
    },
    SchemaFragment {
        name: "developer_journeys",
        sql: domains::developer_journeys::SCHEMA_DEVELOPER_JOURNEYS,
    },
    SchemaFragment {
        name: "visus",
        sql: domains::visus::SCHEMA_VISUS,
    },
    SchemaFragment {
        name: "vox_mesh",
        sql: domains::vox_mesh::SCHEMA_VOX_MESH,
    },
    SchemaFragment {
        name: "discovery",
        sql: domains::discovery::SCHEMA_DISCOVERY,
    },
    SchemaFragment {
        name: "activity_log",
        sql: domains::activity_log::SCHEMA_ACTIVITY_LOG,
    },
    SchemaFragment {
        name: "history_entries",
        sql: domains::history::SCHEMA_HISTORY,
    },
];

/// Hex encoded Keccak-256 digest of [`baseline_sql`].
pub fn schema_baseline_digest_hex() -> String {
    let mut hasher = Keccak256::new();
    hasher.update(baseline_sql());
    format!("0x{}", hex::encode(hasher.finalize()))
}

/// Monolithic SQL string containing all active fragments joined by double-newlines.
pub fn baseline_sql() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut full = String::new();
        for fragment in SCHEMA_FRAGMENTS {
            let sql = fragment.sql.trim();
            if !sql.is_empty() {
                full.push_str(sql);
                full.push_str("\n\n");
            }
        }
        let extra = [
            spec::POPULI_TRAINING_RUN_DDL.trim(),
            spec::CODEX_CAPABILITY_MAP_DDL.trim(),
            spec::CORPUS_SNAPSHOTS_DDL.trim(),
        ];
        for sql in extra {
            if !sql.is_empty() {
                full.push_str(sql);
                full.push_str("\n\n");
            }
        }
        #[cfg(feature = "quarantine")]
        {
            let sql = domains::quarantine::SCHEMA_QUARANTINE.trim();
            if !sql.is_empty() {
                full.push_str(sql);
                full.push_str("\n\n");
            }
        }
        full
    })
}

#[cfg(test)]
mod quarantine_gating {
    use super::baseline_sql;

    /// A table that lives only in `domains::quarantine` (Task 4.1/4.2, VoxDB
    /// audit condensation plan). With the `quarantine` feature OFF (the
    /// default), its DDL must be absent from the compiled baseline.
    #[test]
    #[cfg(not(feature = "quarantine"))]
    fn quarantined_table_absent_when_feature_off() {
        let sql = baseline_sql();
        assert!(
            !sql.contains("CREATE TABLE IF NOT EXISTS toestub_file_cache"),
            "quarantine feature is OFF by default; toestub_file_cache DDL must not appear in baseline_sql()"
        );
    }

    /// Same table must be present when the `quarantine` feature is enabled.
    #[test]
    #[cfg(feature = "quarantine")]
    fn quarantined_table_present_when_feature_on() {
        let sql = baseline_sql();
        assert!(
            sql.contains("CREATE TABLE IF NOT EXISTS toestub_file_cache"),
            "quarantine feature is ON; toestub_file_cache DDL must appear in baseline_sql()"
        );
    }
}

// Skipped under the `quarantine` feature: that feature deliberately appends
// additional DDL to `baseline_sql()` (VoxDB audit condensation Task 4),
// producing a different digest by design. The policy tracks the canonical
// default-feature schema only, so the whole module (including its otherwise-
// unused imports under that feature) is gated off rather than just the test.
#[cfg(all(test, not(feature = "quarantine")))]
mod baseline_digest_policy {
    use super::{BASELINE_VERSION, schema_baseline_digest_hex};

    /// Guards that the recorded baseline policy stays in lockstep with the
    /// compiled schema: the digest + integer in
    /// `contracts/db/baseline-version-policy.yaml` must match
    /// `schema_baseline_digest_hex()` / [`BASELINE_VERSION`]. When
    /// `SCHEMA_FRAGMENTS` change, update that YAML — the expected digest is in
    /// the assert message. (Mirrors the `vox ci check-codex-ssot` gate at
    /// unit-test speed.)
    #[test]
    fn baseline_policy_matches_compiled_schema() {
        let policy_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/db/baseline-version-policy.yaml");
        let policy = std::fs::read_to_string(&policy_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", policy_path.display()));

        let digest = schema_baseline_digest_hex();
        assert!(
            policy.contains(&digest),
            "baseline-version-policy.yaml digest is stale; set repository_baseline_digest_hex to {digest}"
        );
        assert!(
            policy.contains(&format!("repository_baseline_integer: {BASELINE_VERSION}")),
            "baseline-version-policy.yaml repository_baseline_integer must equal BASELINE_VERSION ({BASELINE_VERSION})"
        );
    }
}
