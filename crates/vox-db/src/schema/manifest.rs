//! Single-source Arca/Codex schema: ordered fragments, baseline SQL, digest, and capability metadata.
//!
//! **SSOT:** This manifest defines the current global Arca schema collapsed into domain fragments.
//! Baseline version is [`BASELINE_VERSION`] (see monolithic DDL in `baseline_sql()`).

use super::domains;
use super::spec;
use sha3::{Digest, Keccak256};
use std::sync::OnceLock;

pub const BASELINE_VERSION: i64 = 70; // +1: agent_telemetry_flat.pipeline_phase column (Scientia cost-category split: extraction/critic/novelty/scholarly)

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
        full
    })
}

#[cfg(test)]
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
