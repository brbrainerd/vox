# Semantic Behavior Map — `vox-foundation`

One-paragraph summary: Six extracted Behavior claims cover four symbols in two primitive modules: a tool-to-mutation-kind classifier (`agentos_mutation.rs`) and three pure backoff helpers (`backoff.rs`). After grouping and dedup, every symbol is proven only on happy-path or simple edge inputs — there are no error-path, conflict, or fallback-branch proofs anywhere in the set. The standout hole is the policy/security classifier `mutation_kind_for_tool()`, whose entire fallback heuristic and `read_only` default (the branch that governs unknown/unlisted tools) is untested.

## `mutation_kind_for_tool()` — `src/primitives/agentos_mutation.rs`
Policy SSOT mapping MCP tool names to coarse mutation-kind strings (`read_only` / `local_mutation` / `external_side_effect`) consumed by orchestrator policy, ACI envelopes, and checkpoint hints.

Proven behaviors (all happy-path, exact-match arms only):
- `vox_git_status` -> `read_only`
- `vox_run_shell` -> `external_side_effect`
- `vox_write_file` -> `local_mutation`

Error/edge/invariant proof: NONE. The fallback block (lines 58-69) — substring heuristics for `upsert`/`append`/`insert`, the `vox_openclaw_*` prefix rule, and the `read_only` catch-all default — is entirely unproven.

## `next_exponential_backoff_duration()` — `src/primitives/backoff.rs`
Multiplies a `Duration` by a multiplier, capped at `max`.

Proven behaviors:
- Edge: result is clamped to `max` when `current * multiplier` exceeds `max`.

Error/edge/invariant proof: PARTIAL (cap edge only). Below-cap pass-through and `multiplier < 1` / fractional behavior unproven.

## `next_backoff_ms_double_clamped()` — `src/primitives/backoff.rs`
Doubles `current_ms` and clamps to `[min_ms, max_ms]`.

Proven behaviors:
- Doubling 500 -> 1000 within bounds.
- Clamp to `max_ms` (20000 -> 30000).

Error/edge/invariant proof: NONE for the lower bound. Clamp-to-`min_ms` and `saturating_mul` overflow are coded invariants with no proof.

## `backoff_ms_geometric_attempt()` — `src/primitives/backoff.rs`
Computes `base_ms * 2^(failed_attempt-1)`, exponent capped at `max_exponent`, result clamped to `max_ms`.

Proven behaviors:
- Base case `failed_attempt=1` -> `base_ms`.
- Geometric growth (attempt 2 -> 1600).
- Exponent cap honored (attempt 7 with cap 6 -> 51200).

Error/edge/invariant proof: PARTIAL. The `max_ms` clamp is never asserted on a value that actually exceeds it (51200 < 60000), and the `failed_attempt=0` saturating_sub edge is unproven.

## Semantic gaps

All four symbols are proven only on happy/edge inputs; none has an error-path or true-failure-mode proof. Most actionable:

1. **`mutation_kind_for_tool()` — unproven classifier fallback (highest priority).** This is a security/policy surface: the unmatched default returns `read_only`, meaning any unknown or newly-added tool is silently treated as non-mutating. The heuristic branch on line 59 mixes `&&` and `||` (`name.starts_with("vox_db_") && name.contains("upsert") || name.contains("append") || ...`) — a precedence-fragile expression with zero coverage. Needs tests for: an unknown tool -> `read_only`; a `*append*`/`*insert*`/`*upsert*` tool -> `local_mutation`; a `vox_openclaw_*`-prefixed tool not in the explicit list -> `external_side_effect`.

2. **`next_backoff_ms_double_clamped()` — no lower-bound or overflow proof.** Clamp-to-`min_ms` (when `current_ms` starts below `min_ms`) and `saturating_mul` overflow near `u64::MAX` are stated invariants with no rejection/edge test.

3. **`backoff_ms_geometric_attempt()` — `max_ms` clamp never actually triggered**, and `failed_attempt=0` saturating edge unproven.

4. **`next_exponential_backoff_duration()` — only the cap edge is proven**; below-cap and `multiplier < 1` paths have no coverage.