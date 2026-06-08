# Semantic Behavior Map — `vox-bounded-fs`

Workspace SSOT for scaling-policy-aware capped UTF-8 file reads (used by CI, MCP, publisher, Populi). From 3 extracted Behavior claims, only one crate symbol carries genuine behavioral proof; the remaining fallback/async wrappers and one of the two documented failure modes are unproven, and one claim is a workspace guardrail invariant rather than a crate-API behavior.

## `read_utf8_path_capped()`
Source: `crates/vox-bounded-fs/src/lib.rs:21`

Distinct proven behaviors:
- **Happy** (`reads_small_utf8`): reads a small valid-UTF-8 file and returns its content as a `String`.
- **Error** (`rejects_oversized_file`): returns an error whose message contains `exceeds scaling policy max_file_bytes_hint` when `meta.len() > max_file_bytes_hint()`.

Coverage flags:
- Error-path proof: **yes** (oversize only).
- Edge/invariant proof: **no**.

Contract has three failure modes (`stat` failure, size cap, invalid UTF-8); only the size-cap branch is exercised. The `String::from_utf8` rejection branch and the `fs::metadata`/`fs::read` context errors are unproven.

## `scripts/extract_mcp_tool_registry.py` (workspace guardrail invariant)
Source: `crates/vox-bounded-fs/tests/workspace_guardrails.rs`

Distinct proven behaviors:
- **Invariant** (`legacy_mcp_extract_script_stays_explicitly_gated`): if the legacy script exists, it contains both the `VOX_ALLOW_LEGACY_MCP_EXTRACT` env gate and the `--allow-legacy` CLI flag gate.

Coverage flags: this is a conditional ("if it exists") repo-hygiene assertion, not a behavior of the `vox-bounded-fs` public API. It proves nothing about the capped-read surface and trivially passes when the script is absent.

## Unproven symbols (zero claims)
- `max_file_bytes_hint()` — `lib.rs:16` (exercised indirectly only).
- `read_utf8_path_capped_or_empty()` — `lib.rs:39`.
- `read_utf8_path_capped_opt()` — `lib.rs:45`.
- `read_utf8_path_capped_async()` — `lib.rs:51` (feature `async`).

## Semantic gaps

The crate's entire reason for existing is *bounded, failure-tolerant* reads, yet most failure and fallback behavior is unproven. Most actionable:

1. **Invalid-UTF-8 rejection is untested.** `read_utf8_path_capped()` documents two error modes (size cap + non-UTF-8) but only the cap is proven. The `String::from_utf8 -> "invalid UTF-8"` branch is a real, reachable failure path with no rejection test. This is the classic "validator with no rejection test for one of its branches."

2. **Fallback wrappers never proven to swallow failure.** `read_utf8_path_capped_or_empty()` (empty string on failure) and `read_utf8_path_capped_opt()` (None on failure) are pure error-handling contracts with zero coverage — exactly the symbols whose value *is* the failure path. A regression turning a swallowed error into a panic/propagation would pass CI.

3. **Async wrapper + join-error path unproven.** `read_utf8_path_capped_async()` adds a `spawn_blocking` join-error branch on top of the sync contract, with no test of either the happy path or the `read join error` mapping.

4. **stat/read I/O-error context unproven.** The missing-file path (`stat {path}` context) — the most common real-world failure — has no test.

The guardrail invariant claim, while valid, should not be counted toward API behavioral coverage; it is a repo-hygiene check that passes vacuously when the legacy script is absent.