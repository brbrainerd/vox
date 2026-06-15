# Band A Phase 5 — LLM/AI Accessor Snapshot-Cache (Perf)

**Status:** Plan-only (design map). For implementation by Claude Sonnet 4.6 with TDD.
**Date:** 2026-06-15
**Predecessor:** LLM/AI settings SSOT Band A (landed on main); `vox-config::snapshot` watch/bump/rev already exists.

## Goal

Eliminate redundant `std::env::var` / `vox_secrets` reads on the LLM hot path by giving the
20+ vox-config LLM/AI accessors a snapshot-backed lazy cache: **one read per accessor per
snapshot revision**, invalidated atomically on `snapshot::bump()`.

## Current State

vox-config LLM/AI accessors re-read env/secrets *per call*:
- `inference.rs`: `inference_profile_from_env`, `openrouter_base_url`, `openai_compatible_base_url`, `local_ollama_populi_base_url`, `tuning_*`, etc.
- `resolve_egress.rs`: `resolve_api_key`, `resolve_max_concurrent`, `openrouter_route_hint_from_env`.

Hot-path callers (`vox-actor-runtime` `llm/cascade.rs:84-85`, `mens.rs`, `model_resolution.rs`)
invoke these once per cascade step, multiplied across research stages (Planner, ClaimExtraction,
Verification, Synthesis, Judge, SelfVerification). The `snapshot.rs` watch channel already
notifies on config changes; **no cache invalidation mechanism is wired yet**. The cost path
(`estimate_cost`, `resolve_egress` max_concurrent/timeout) inherits this overhead.
`env_scratch.rs` test helper enables safe env mutation but does not invalidate snapshots.

## SSOT Principle

One read per accessor per snapshot revision (tracked by `snapshot.rs::current_rev()`).
`bump()` invalidates all caches atomically. Accessors stay pure (no `&mut self`), return owned
values; caching is **transparent** — no public signature changes. The cost path inherits the
savings. `vox_secrets::resolve_secret` itself stays uncached (own resolution layer); we cache at
the accessor level *after* resolution.

## Scope

**In:** snapshot-backed lazy caches for 20+ LLM/AI env accessors behind a `SnapshotCache<T>`
generic watching `current_rev()`; 4 secret-resolution paths (OpenRouter, OpenAI, Anthropic, HF),
8 base-URL paths.
**Out:** no async/await, no thread-local mutable state, no hidden allocations, no public signature
changes; direct `vox_secrets::resolve_secret` stays uncached.

## Phases

- **5.1** Design `SnapshotCache<T>` generic + integration points (inference.rs, resolve_egress.rs, snapshot.rs API extensions). *Sequential — blocks others.*
- **5.2** Migrate 8 base-URL accessors to cached.
- **5.3** Migrate 4 secret+config resolution paths to cached.
- **5.4** Counting-shim TDD harness proving N hot-path accessor calls read env ≤ once per snapshot rev.
- **5.5** `env_scratch` + snapshot invalidation helper (`EnvScratch::drop` calls `snapshot::bump` on affected keys).
- **5.6** Verify cost path (`resolve_egress`) inherits savings; benchmark env/secrets ops/call before vs after.

## Parallelism

5.1 sequential (design blocks). 5.2 ∥ 5.3 once the generic is ready. 5.4 ∥ 5.5. 5.6 sequential (verification on completed cache).

## TDD Notes

- **5.1:** `SnapshotCache<T>` contract — invalidate-on-bump, reuse-across-calls, thread-safe once-per-rev.
- **5.2–5.3:** one test per accessor — cache-hit verifies no env re-read; cache-miss on bump verifies re-read. Mock env that counts `std::env::var` calls.
- **5.4:** simulate `cascade_for_research_stage` calling 3+ accessors 6× (one/stage); assert `env::var` called ≤ N (N = unique env keys).
- **5.5:** set env var → bump → cache invalidates → re-read succeeds; drop `EnvScratch` → env restored → no second bump.
- **5.6:** time `resolve_egress` across 100 calls; expect 60–80% wall-clock overhead reduction (env+secrets ~0.3–0.8ms cold, ~0.01ms cache-hit).

## Risks

1. Thread-safety of `OnceLock<T>` with snapshot channel — store `(Rev, T)`, inline `current_rev()` (SeqCst) load before return.
2. Invalidation completeness — clippy/codegen lint flags any direct `std::env::var`/`vox_secrets` in inference.rs/resolve_egress.rs (already in place from Phase 1).
3. `EnvScratch` drop + `bump` re-entrancy — use `bump_for_test` synthetic rev, not the live listener.
4. Memory footprint — ~5–6 KB/process (20 caches × rev+string). Acceptable.
5. Cost-path latency regression from contention — unlikely (OnceLock lock-free on read); measure in 5.6.

## Key Files

- `crates/vox-config/src/snapshot.rs` — add `SnapshotCacheRef<T>`, `get_or_init_cached<T,F>`, tests
- `crates/vox-config/src/inference.rs` — migrate ~15 accessors
- `crates/vox-config/src/resolve_egress.rs` — migrate 4–5 accessors
- `crates/vox-test-harness/src/env_scratch.rs` — snapshot invalidation on drop
- `crates/vox-config/tests/snapshot_cache_counting_shim.rs` (new)
- `crates/vox-config/tests/env_scratch_snapshot_integration.rs` (new)
