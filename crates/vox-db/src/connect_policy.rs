//! Canonical VoxDB connection policy helpers (SSOT for degraded-mode messaging).
//!
//! Surfaces choose one of:
//! - **Strict** — propagate [`crate::StoreError`]; no silent drops.
//! - **Degraded optional** — Codex features off; log structured remediation (MCP, optional CLI paths).
//! - **Legacy schema** — [`crate::VoxDb::connect_default`] returns [`crate::StoreError::LegacySchemaChain`]; migrate per cutover runbook (no automatic sidecar DB).
//!
//! Human-oriented inventory: `docs/src/architecture/voxdb-connect-policy.md`.

use crate::{DbConfig, StoreError, VoxDb};

/// Product-facing remediation when canonical SQLite/Turso is unavailable.
pub const REMEDIATION_CANONICAL_DB: &str = "Set VOX_DB_PATH to a writable SQLite file, or VOX_DB_URL + token for Turso. Run `vox secrets doctor` / `vox codex verify` after fixing env. For legacy multi-step schema_version chains, export with `vox codex export-legacy`, init a fresh DB, then `vox codex import-legacy`.";

/// Short label for logs/metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbConnectSurface {
    /// `vox-mcp` stdio server.
    Mcp,
    /// Populi cloud resolver (throughput profiles; optional DB).
    PopuliCloudResolver,
    /// `vox-actor-runtime` and similar always-on services.
    Runtime,
    /// CLI paths that require Codex.
    CliStrict,
    /// Mens training DB thread (canonical connect; fails on legacy primary until migration).
    MensTraining,
    /// Repo-scoped CLI commands (`vox agent`, `vox snippet`, …) using workspace journey resolution.
    CliWorkspace,
}

impl DbConnectSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::PopuliCloudResolver => "populi_cloud_resolver",
            Self::Runtime => "runtime",
            Self::CliStrict => "cli_strict",
            Self::MensTraining => "mens_training",
            Self::CliWorkspace => "cli_workspace",
        }
    }
}

/// Format a single structured warning line for degraded optional connections.
pub fn format_degraded_optional_connect(surface: DbConnectSurface, err: &StoreError) -> String {
    format!(
        "VoxDB degraded (surface={}): {}. Persistence disabled until resolved. {}",
        surface.as_str(),
        err,
        REMEDIATION_CANONICAL_DB
    )
}

/// Resolve config and connect (strict). Used by runtime and CLI commands that must fail loud.
pub async fn connect_canonical_strict() -> Result<VoxDb, StoreError> {
    let cfg = DbConfig::resolve_canonical()
        .map_err(|e| StoreError::NotFound(format!("resolve_canonical: {e}")))?;
    VoxDb::connect(cfg).await
}

/// Canonical connect for **optional** DB features: returns `Ok(None)` on resolution/connection failure.
///
/// Logs at `warn` with [`format_degraded_optional_connect`] unless `skip_log` is true (tests).
pub async fn connect_canonical_optional(
    surface: DbConnectSurface,
    skip_log: bool,
) -> Option<VoxDb> {
    let cfg = match DbConfig::resolve_canonical() {
        Ok(c) => c,
        Err(e) => {
            if !skip_log {
                tracing::warn!(
                    target: "vox_db::connect_policy",
                    surface = surface.as_str(),
                    phase = "resolve_canonical",
                    "failed to resolve canonical DbConfig: {e}. {}",
                    REMEDIATION_CANONICAL_DB
                );
            }
            return None;
        }
    };
    match VoxDb::connect(cfg).await {
        Ok(db) => Some(db),
        Err(e) => {
            if !skip_log {
                tracing::warn!(
                    target: "vox_db::connect_policy",
                    surface = surface.as_str(),
                    phase = "connect",
                    "{}",
                    format_degraded_optional_connect(surface, &e)
                );
            }
            None
        }
    }
}

#[cfg(test)]
mod semcov_wave1b_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn format_degraded_optional_connect_assembles_full_remediation_line() {
        let err = StoreError::NotFound("boom".to_string());
        let line = format_degraded_optional_connect(DbConnectSurface::Mcp, &err);
        let expected = format!(
            "VoxDB degraded (surface=mcp): Not found: boom. Persistence disabled until resolved. {}",
            REMEDIATION_CANONICAL_DB
        );
        assert_eq!(line, expected);
        assert!(line.starts_with("VoxDB degraded (surface=mcp): Not found: boom."));
        assert!(line.ends_with(REMEDIATION_CANONICAL_DB));
    }
}
