//! External **DeI JSON-line RPC** integration boundary.
//!
//! The resolved process binary is **`vox-orchestrator-d`** (see `BINARY`); docs and hints may still
//! Historical docs may say `vox-dei-d`; the shipped binary is `BINARY` (`vox-orchestrator-d`).
//!
//! The staging `crates/vox-dei` library is not linked into `vox-cli` (see `vox ci no-dei-import`). All DeI RPC that the
//! slim CLI performs goes through the same JSON-line [`DispatchRequest`] / [`DispatchResponse`]
//! protocol as [`crate::dispatch::call_daemon`], with method names centralized here to avoid drift.

use serde_json::Value;

/// Resolved daemon binary name (no `.exe`; Windows resolution adds it in [`crate::dispatch`]).
pub const BINARY: &str = "vox-orchestrator-d";

/// Stable RPC method strings shared with `vox-orchestrator-d` / MCP tool wiring.
pub mod method {
    /// `ai.check` — static review / verify-style pass over a file.
    pub const AI_CHECK: &str = "ai.check";
    /// `ai.fix` — apply fixes given compiler diagnostics context.
    pub const AI_FIX: &str = "ai.fix";
    /// `ai.review` — multi-target review (diff-aware).
    pub const AI_REVIEW: &str = "ai.review";
    /// `ai.generate` — streamed codegen from a prompt.
    pub const AI_GENERATE: &str = "ai.generate";
    /// `config.get` — DEI orchestrator inference configuration snapshot.
    pub const CONFIG_GET: &str = "config.get";
    /// `ai.plan.new` — create a structured plan session.
    pub const AI_PLAN_NEW: &str = "ai.plan.new";
    /// `ai.plan.replan` — replan from session id + delta.
    pub const AI_PLAN_REPLAN: &str = "ai.plan.replan";
    /// `ai.plan.status` — read plan session status.
    pub const AI_PLAN_STATUS: &str = "ai.plan.status";
    /// `ai.plan.execute` — execute approved plan steps.
    pub const AI_PLAN_EXECUTE: &str = "ai.plan.execute";
}

/// Invoke `BINARY` (the daemon binary) with `method` / `params` and return the final `Result` JSON value.
///
/// Maps common spawn failures to an actionable hint (install / PATH / sibling binary).
pub async fn call(method: &str, params: Value, auto_open: bool) -> anyhow::Result<Value> {
    crate::dispatch::call_daemon(BINARY, method, params, auto_open)
        .await
        .map_err(enrich_dei_daemon_error)
}

fn enrich_dei_daemon_error(err: anyhow::Error) -> anyhow::Error {
    let display = format!("{err:#}");
    if display.contains(crate::dispatch::DAEMON_SPAWN_FAILED_PREFIX) {
        anyhow::anyhow!(
            "{display}\n\
             Hint: install **`vox-orchestrator-d`** on `PATH`, or place the binary next to `vox` (see `docs/src/reference/cli.md`)."
        )
    } else {
        err
    }
}

#[cfg(test)]
mod tests {
    use super::method::*;

    /// Guard against accidental renames: MCP / orchestrator daemon must agree on these ids.
    #[test]
    fn dei_method_ids_stable() {
        assert_eq!(AI_CHECK, "ai.check");
        assert_eq!(AI_FIX, "ai.fix");
        assert_eq!(AI_REVIEW, "ai.review");
        assert_eq!(AI_GENERATE, "ai.generate");
        assert_eq!(CONFIG_GET, "config.get");
        assert_eq!(AI_PLAN_NEW, "ai.plan.new");
        assert_eq!(AI_PLAN_REPLAN, "ai.plan.replan");
        assert_eq!(AI_PLAN_STATUS, "ai.plan.status");
        assert_eq!(AI_PLAN_EXECUTE, "ai.plan.execute");
    }

    #[test]
    fn binary_name_non_empty() {
        assert!(!super::BINARY.is_empty());
        assert!(!super::BINARY.contains('.'));
    }

    #[test]
    fn enrich_maps_spawn_failure() {
        let inner = anyhow::anyhow!(
            "{} 'vox-orchestrator-d': no such file",
            crate::dispatch::DAEMON_SPAWN_FAILED_PREFIX
        );
        let out = super::enrich_dei_daemon_error(inner);
        let s = format!("{out:#}");
        assert!(s.contains("Hint:"), "{s}");
    }

    #[test]
    fn enrich_passes_through_other_errors() {
        let inner = anyhow::anyhow!("Daemon error (code 3): bad params");
        let out = super::enrich_dei_daemon_error(inner);
        assert_eq!(format!("{out:#}"), "Daemon error (code 3): bad params");
    }
}
