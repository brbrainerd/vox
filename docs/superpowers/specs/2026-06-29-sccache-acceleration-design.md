---
title: sccache acceleration — make compiler caching actually deliver
category: Operations
status: design (investigation-led)
date: 2026-06-29
companion: 2026-06-29-build-infra-health-doctor-design.md (detection/guard — separate track)
---

# sccache acceleration

## Context

sccache was configured as the global `rustc-wrapper` to "deduplicate compilation across
worktrees" and give warm same-worktree rebuilds (the config claims a measured 71 s → 1 s,
100% hit). In practice it delivered **0.4 % lifetime hit rate, and 0 hits even on a clean
rebuild of identical content same-worktree**, while crashing rustc on a corrupt 41 GB cache.
It was disabled as a stopgap (corrupt cache cleared, `rustc-wrapper` commented out). The
goal now is the *acceleration*, not the off-switch: stop rebuilding `target/` from scratch
constantly, without reintroducing silent breakage.

This is **investigation-led**: the root cause of 0% hits is a hypothesis set, not a known
fact, so the design is "instrument → confirm → fix the confirmed cause," guarded by the
build-health check (companion spec) so a regression auto-surfaces.

## Goal & success criteria

- **Primary:** a clean rebuild of unchanged code is near-instant (cache-served), measured:
  `cargo clean -p <crate>` then build → sccache hit-rate **> 80%** for that crate, wall-time
  collapse comparable to the config's 71 s → 1 s claim.
- **Guarded:** re-enabled only once hits are proven by the build-health check; auto-disabled
  if the crash- or 0%-hit-signature returns (companion spec owns the guard).
- **No silent cost:** if no configuration yields real hits on this machine's layout, the
  honest outcome is "stay disabled + document why," not a cache that pretends to help.

## Hypotheses for 0% hits (to confirm/refute, in order)

1. **`CARGO_INCREMENTAL` not actually 0 at compile time.** sccache cannot cache incremental
   artifacts; the config sets `CARGO_INCREMENTAL=0` in `[env]`, but `cargo check` and some
   invocation paths may not inherit it. *Test:* force `CARGO_INCREMENTAL=0` explicitly and
   re-measure a clean rebuild.
2. **Absolute-path / cwd in cache keys.** sccache keys include the compiler's absolute
   invocation and (per the config's own note) crate metadata beyond source paths; per-worktree
   absolute `target/` dirs and CWD differences may make every key unique. *Test:* same
   worktree, same path, two clean builds — do keys match? (`SCCACHE_LOG=debug`.)
3. **`cargo check` vs `cargo build` artifact differences** — check produces `.rmeta` only;
   confirm the cacheable unit. *Test:* measure `build` not just `check`.
4. **Wrapper interaction** — the (now-neutralized) build-broker shim sat between cargo and
   rustc; with the shim removed and `~/.cargo/bin` restored, re-measure cleanly.

## Approach

1. **Instrument:** `SCCACHE_LOG=debug` + `sccache --show-stats --stats-format=json` around a
   controlled clean rebuild; capture the per-compilation cache-miss *reason* (sccache logs
   why each is a miss).
2. **Confirm** which hypothesis holds (likely #1 and/or #2).
3. **Fix the confirmed cause**, e.g.:
   - ensure `CARGO_INCREMENTAL=0` is unconditional for cached builds;
   - if path-keying is the cause, evaluate `--remap-path-prefix` to a stable virtual root, or
     a single shared `target/` for same-machine builds (trade vs the per-worktree-target
     strategy that exists to avoid cargo lock contention);
   - bound `SCCACHE_CACHE_SIZE` sanely and keep the cache outside any repo.
4. **Prove** the >80% success criterion, then re-enable `rustc-wrapper` and let the
   build-health check guard it.

## Alternatives if sccache can't deliver here

Captured so the investigation has an exit, not an infinite tuning loop:
- **cargo's own incremental** (`CARGO_INCREMENTAL=1`, per-worktree) for fast *same-worktree*
  rebuilds — mutually exclusive with sccache, but may beat a 0%-hit sccache outright.
- **Shared `target/` + a build lock** instead of per-worktree targets — lets cargo's native
  fingerprinting reuse artifacts across worktrees (the thing sccache was meant to provide).
- **Faster linker** (`lld`/`mold`-equivalent on Windows) — orthogonal win on link-heavy
  rebuilds regardless of caching.

## Non-goals

- GHA/CI-side sccache (the config notes cross-worktree sharing needs an identical ephemeral
  path, which only CI has) — this spec is local-dev acceleration.
- The detection/guard/heal machinery (companion spec).

## Testing

- A reproducible bench: `cargo clean -p vox-secrets && time cargo build -p vox-secrets`,
  hit-rate captured before/after — the >80% gate is the pass condition.
- Regression: the build-health check's sccache-stats probe asserts hit-rate stays above the
  floor once enabled.
