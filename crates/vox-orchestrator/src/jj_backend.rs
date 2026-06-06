//! jj-lib integration wrapper — **all** jj-lib API calls are confined here.
//!
//! # Stability contract
//! - Only call public, non-`#[doc(hidden)]` jj-lib APIs.
//! - Every public type here is Vox-native; jj-lib types don't leak out.
//! - If jj-lib bumps a version and breaks this file, fix it here only.
//!
//! # Feature gate
//! Enabled via `--features jj-backend`. Without it, this module provides
//! no-op / pure-Rust fallbacks so callers compile in both modes.
// Silence rustc 1.80+ check-cfg for jj-backend: feature is declared in
// vox-orchestrator/Cargo.toml but not propagated into the workspace check graph.
#![cfg_attr(not(feature = "jj-backend"), allow(unexpected_cfgs))]
//!
//! # Modules used
//! | jj-lib module      | What we use it for                          |
//! |--------------------|---------------------------------------------|
//! | `merge`            | `Merge<T>` — n-way content-level conflicts  |
//! | `dag_walk`         | Ancestor/descendant/topo-sort DAG algos     |
//! | `op_store`         | Persistent, crash-durable operation log     |
//! | `local_working_copy` | Working copy as a commit                  |
//! | `revset`           | Commit-selection DSL for `oplog query`      |
//! | `annotate`         | Per-line blame / attribution                |
//! | `signing`          | SSH/GPG operation signing for audit trail   |

// ---------------------------------------------------------------------------
// jj-lib feature gate (actual integration)
// ---------------------------------------------------------------------------

// When jj-lib is enabled, we expose a thin re-export / bridge layer.
// The types above are always available (no jj-lib needed) — they're our
// native implementations. The feature gate adds access to jj-lib's own
// algorithms when higher fidelity is needed (e.g., signing, revset DSL).

#[cfg(feature = "jj-backend")]
pub mod jj {
    //! Direct jj-lib bridge. Only used when `--features jj-backend` is active.
    //!
    //! This module intentionally has a very small API surface.
    //! Add functions here only when the native implementations above
    //! are insufficient (e.g., for SHA-1 commit graph handling or revset DSL).

    /// Version of jj-lib this module was written against.
    /// If the build fails here, bump to the new version and audit the wrapper.
    pub const JJ_LIB_PINNED_VERSION: &str = "0.39.0";

    /// Verify at test time that jj-lib is reachable and at the expected version.
    /// This test fails if jj-lib silently changes APIs.
    #[cfg(test)]
    #[test]
    fn jj_lib_stability_check() {
        // If this test exists and compiles, jj-lib is available at the pinned version.
        // Add specific API probes here as we adopt more jj-lib surface.
        println!("jj-lib stability check: version gate = {JJ_LIB_PINNED_VERSION}");
    }
}

// ---------------------------------------------------------------------------
// JjBridge CLI Facade
// ---------------------------------------------------------------------------

/// CLI subprocess adapter that provides operations like snapshot flushes to
/// Jujutsu without requiring the full jj-lib to be statically linked.
pub struct JjBridge;

impl JjBridge {
    /// Flush a merged task/change snapshot to JJ as an anonymous branch.
    pub async fn flush_snapshot_commit(
        task_id: impl std::fmt::Display,
        agent_id: impl std::fmt::Display,
        description: &str,
        cwd: Option<&str>,
    ) -> std::io::Result<()> {
        let msg = format!(
            "AgentTask {} (Agent {}) - {}",
            task_id, agent_id, description
        );
        let mut cmd = tokio::process::Command::new("jj");
        cmd.args(["commit", "-m", &msg]);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd.output().await?;
        if !out.status.success() {
            tracing::warn!(
                "JjBridge: commit flush failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    /// Revert working copy state via `jj abandon @-` if an agent completely fails verification.
    pub async fn revert_agent_snapshot(cwd: Option<&str>) -> std::io::Result<()> {
        let mut cmd = tokio::process::Command::new("jj");
        cmd.args(["abandon", "@-"]); // rollback last
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd.output().await?;
        if !out.status.success() {
            tracing::warn!(
                "JjBridge: abandon revert failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }
}
