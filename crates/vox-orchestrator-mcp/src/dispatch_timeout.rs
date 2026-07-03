//! T4.3: per-tool-call execution timeouts.
//!
//! `dispatch.rs`'s `handle_tool_call_with_mode` wraps the actual tool
//! dispatch (`TimedExecution::run` around `handle_tool_call_inner`) in a
//! `tokio::time::timeout`, so a hung tool implementation can no longer block
//! the MCP handler indefinitely. `vox_db::TimedExecution` only *measures*
//! duration for telemetry — it never bounded execution — so this is a
//! genuinely new, independent guard, not a replacement for it.
//!
//! This is deliberately NOT the same mechanism as the dangerous-tool HITL
//! approval-wait (`APPROVAL_TIMEOUT`, 300s, in `dispatch.rs`): that timeout
//! bounds how long the server waits for a *human decision* before a gated
//! tool even starts executing. This module's timeout bounds the *execution*
//! of the tool body itself, for every tool (gated or not), starting only
//! after any approval has already been granted.
//!
//! ## Scope-down note
//! True per-tool granularity would require adding a `timeout_ms` field to
//! `contracts/mcp/tool-registry.canonical.yaml` and its generated
//! `vox_mcp_registry::McpToolRegistryEntry` — a machine-generated pipeline
//! (`vox-mcp-registry/build.rs`) touching ~389 tool rows. Per T4.3's
//! documented fallback, this module instead classifies tools by name/prefix
//! in Rust (mirroring the existing `permission_modes::RISK_CLASSES`
//! hand-written-table pattern), with a single coarse exception carve-out for
//! the agy delegation tools. This keeps the change additive and reviewable
//! without regenerating the canonical registry.

use std::time::Duration;

/// Global default execution timeout for any tool not explicitly classified
/// below. Chosen well above typical in-process tool latency (DB reads,
/// compiler/lint invocations, small subprocess calls) but well below
/// something that should be considered "hung". `vox_run_tests` /
/// `vox_check_workspace` / `vox_test_all` / `vox_build_crate` /
/// `vox_lint_crate` / `vox_coverage_report` invoke `cargo` subprocesses that
/// can legitimately run a couple of minutes on a cold cache; 120s covers the
/// large majority of those while still bounding a genuinely hung call.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// The agy delegation tools are the canonical long-running exception: each
/// call shells out to the `agy` CLI (Gemini-backed code generation) via
/// `AgyExec::run`, which already self-bounds the *subprocess* with its own
/// `tokio::time::timeout` + `kill_on_drop(true)` (see `agy_exec.rs`) sized by
/// the caller-supplied `timeout_secs` arg (default 900s — see
/// `agy_tools::delegate_validate` / `batch_validate`). The outer dispatch
/// timeout here must exceed that inner bound (defense-in-depth, not a
/// competing bound), plus headroom for worktree setup/teardown, the 3-attempt
/// quota/timeout retry loop, and (for the batch variant) up to
/// `MAX_CONCURRENCY` workers each individually retried before the batch
/// future resolves. 20 minutes covers a full 900s inner attempt plus one
/// retry with backoff and slack.
pub const AGY_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// T4.3 follow-up: explicit exception timeout for the "cargo-shelling"
/// compiler tools (`vox_test_all`, `vox_check_workspace`, `vox_build_crate`,
/// `vox_lint_crate`, `vox_coverage_report` — see `compiler_tools.rs`). These
/// shell out to `cargo test|check|build|clippy --workspace`/per-crate
/// equivalents against this repo's ~150-crate workspace, with no internal
/// timeout of their own. Ad hoc measurement in this session showed even a
/// *subset* of the workspace taking 98-112s on a warm cache; a cold-cache
/// full-workspace `cargo test`/`cargo build` would plausibly exceed the 120s
/// [`DEFAULT_TIMEOUT`] and get killed by the outer dispatch timeout, turning
/// a legitimately slow-but-succeeding build/test run into a false timeout
/// error. 15 minutes gives roughly 8x headroom over the observed
/// warm-cache-subset baseline to comfortably absorb a cold cache plus the
/// full workspace, while still bounding a genuinely hung `cargo` invocation
/// well below the 20-minute agy exception.
pub const COMPILER_WORKSPACE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Tool names given an explicit non-default execution timeout. Everything
/// else falls back to [`DEFAULT_TIMEOUT`] via [`timeout_for`].
const EXPLICIT_TIMEOUTS: &[(&str, Duration)] = &[
    ("vox_agy_delegate", AGY_TIMEOUT),
    ("vox_agy_delegate_batch", AGY_TIMEOUT),
    ("vox_test_all", COMPILER_WORKSPACE_TIMEOUT),
    ("vox_check_workspace", COMPILER_WORKSPACE_TIMEOUT),
    ("vox_build_crate", COMPILER_WORKSPACE_TIMEOUT),
    ("vox_lint_crate", COMPILER_WORKSPACE_TIMEOUT),
    ("vox_coverage_report", COMPILER_WORKSPACE_TIMEOUT),
    // T4.3 RED-test doubles (see dispatch.rs's `#[cfg(test)]` match arms).
    // `vox_test_hang_forever` gets a short timeout so the RED test proving
    // the outer guard actually fires doesn't need to wait out the real
    // global default. `vox_test_agy_like_long_running` gets a timeout
    // between the (short) global default and the hang-forever tool's bound,
    // proving the "longer than default, shorter than its own long budget"
    // exception is real rather than merely documented.
    #[cfg(test)]
    ("vox_test_hang_forever", Duration::from_millis(200)),
    #[cfg(test)]
    (
        "vox_test_agy_like_long_running",
        Duration::from_millis(2000),
    ),
];

/// Resolve the outer dispatch timeout for `tool_name`. Falls back to
/// [`DEFAULT_TIMEOUT`] for any tool absent from [`EXPLICIT_TIMEOUTS`] —
/// including every tool not yet classified, which is the safe default
/// (bounded, not unbounded).
#[must_use]
pub fn timeout_for(tool_name: &str) -> Duration {
    EXPLICIT_TIMEOUTS
        .iter()
        .find(|(name, _)| *name == tool_name)
        .map(|(_, d)| *d)
        .unwrap_or(DEFAULT_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agy_tools_get_the_long_exception_timeout() {
        assert_eq!(timeout_for("vox_agy_delegate"), AGY_TIMEOUT);
        assert_eq!(timeout_for("vox_agy_delegate_batch"), AGY_TIMEOUT);
    }

    #[test]
    fn unclassified_tool_gets_the_global_default() {
        assert_eq!(timeout_for("vox_write_file"), DEFAULT_TIMEOUT);
        assert_eq!(timeout_for("vox_totally_unknown_tool"), DEFAULT_TIMEOUT);
    }

    #[test]
    fn agy_timeout_exceeds_the_inner_agy_exec_subprocess_bound() {
        // agy_tools::delegate_validate / batch_validate default `timeout_secs`
        // to 900s when the caller omits it (see agy_tools.rs); AgyExec::run
        // bounds the subprocess itself to exactly that. The outer dispatch
        // timeout must stay strictly larger than that inner bound so the
        // outer timeout never fires first and races the inner one.
        const AGY_INNER_DEFAULT_TIMEOUT_SECS: u64 = 900;
        assert!(AGY_TIMEOUT > Duration::from_secs(AGY_INNER_DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn default_timeout_is_smaller_than_agy_exception() {
        assert!(DEFAULT_TIMEOUT < AGY_TIMEOUT);
    }

    #[test]
    fn cargo_shelling_compiler_tools_get_the_longer_workspace_timeout() {
        // Proves these 5 tools genuinely resolve to COMPILER_WORKSPACE_TIMEOUT
        // rather than silently falling through to DEFAULT_TIMEOUT (e.g. via a
        // typo'd tool name in EXPLICIT_TIMEOUTS).
        for tool in [
            "vox_test_all",
            "vox_check_workspace",
            "vox_build_crate",
            "vox_lint_crate",
            "vox_coverage_report",
        ] {
            assert_eq!(
                timeout_for(tool),
                COMPILER_WORKSPACE_TIMEOUT,
                "{tool} should get COMPILER_WORKSPACE_TIMEOUT, not the default"
            );
            assert_ne!(
                timeout_for(tool),
                DEFAULT_TIMEOUT,
                "{tool} must not fall through to DEFAULT_TIMEOUT"
            );
        }
    }

    #[test]
    fn compiler_workspace_timeout_is_between_default_and_agy_exception() {
        assert!(DEFAULT_TIMEOUT < COMPILER_WORKSPACE_TIMEOUT);
        assert!(COMPILER_WORKSPACE_TIMEOUT < AGY_TIMEOUT);
    }
}
