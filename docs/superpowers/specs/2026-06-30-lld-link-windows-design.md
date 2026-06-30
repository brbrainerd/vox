---
title: lld-link on Windows — root-cause the test-binary lock, adopt the fast linker
category: Operations
status: design (investigation-led)
date: 2026-06-30
---

# lld-link on Windows

## Context

Linking dominates incremental Rust rebuilds, and Vox's Windows binaries are large
(`vox-cli.exe`/`vox-gui.exe` ~115 MB). The tracked `.cargo/config.toml` already uses
`lld` on Linux but **keeps the slow MSVC `link.exe` on Windows**, with this note:

> `[target.x86_64-pc-windows-msvc]` — lld-link is available
> (`C:\Program Files\LLVM\bin\lld-link.exe`) but causes **permission-denied errors when
> overwriting locked test binaries**. Keeping MSVC linker. Revisit when fixed.

The note records the symptom but **not the holder** — *which process* locks the `.exe` —
so it can't be acted on. This investigation finds the holder, fixes that cause, and
adopts lld-link. It complements the just-landed acceleration work (sccache killed, cargo
native incremental restored) and the build-health doctor (which gains a linker check).

## Goals

- **Diagnose the lock holder** at the moment of failure (a PID, not a guess).
- **Fix the confirmed root cause** and adopt lld-link on `x86_64-pc-windows-msvc`.
- **Prove it:** a measurable link-time cut on a large crate AND `cargo test -p vox-cli`
  passing 5× consecutively with no permission-denied.
- **Guard it:** a `vox doctor` linker check so a regression (or missing AV exclusion)
  surfaces loudly + LLM-readably.
- **Or** conclude cleanly: if unfixable, stay on MSVC and record the *holder + why* so it's
  never re-litigated blind.

## Non-goals

- Changing the per-worktree `target/` strategy (it exists to avoid cross-agent lock
  contention — see `2026-06-05-target-artifact-gc-design.md`).
- `rust-lld` (Rust's future bundled MSVC linker) — a separate path if approach A fails;
  it isn't in the toolchain yet.
- A permanent retry-wrapper as the *primary* fix (only adopted if the root cause genuinely
  is lld-link's non-retry on a transient sharing violation).

## Design

### Phase 0 — Reproduce & identify the holder (the keystone)

Point the Windows target at lld-link (`-C linker=lld-link`, or
`-C link-arg=-fuse-ld=lld-link`), then loop `cargo test -p vox-cli` until the
permission-denied reproduces. **At the instant of failure, capture which PID holds the
handle** on the failing `.exe` — Sysinternals `handle.exe -p <exe>` (or
`Get-Process`/`openfiles`/`Restart-Manager` query). That holder is the diagnosis. Record
the exact error text + holder process name/PID.

### Phase 1 — Classify the holder

| Holder | Cause | Phase-2 fix |
|---|---|---|
| `MsMpEng.exe` | Windows Defender real-time scan holds the fresh `.exe` | add per-worktree `target/` to Defender exclusions |
| a `*-<hash>.exe` test binary | a **zombie test process** still alive (the orphan class we hit) holding its own image | kill stale test exes before relink (process hygiene) |
| no foreign holder | **lld-link doesn't retry** a transient sharing violation that `link.exe` silently retries | minimal retry-on-`ERROR_SHARING_VIOLATION` (this *is* the root-cause fix here, not a band-aid) |

### Phase 2 — Fix the confirmed cause (only the one Phase 1 proves)

- **Defender:** `Add-MpPreference -ExclusionPath` for the worktree `target/` roots; document
  as machine setup; a doctor check verifies it.
- **Zombie procs:** a pre-relink reaper for stale `target/**/deps/*-<hash>.exe` test
  processes (reuses the orphan-hygiene approach; gated to the build/test path, not always-on).
- **lld no-retry:** a thin linker shim that re-invokes lld-link on `ERROR_SHARING_VIOLATION`
  with short backoff (≤3 tries) — the precise behavior link.exe already has.

### Phase 3 — Adopt & prove

- Switch `.cargo/config.toml` `[target.x86_64-pc-windows-msvc]` to lld-link (keeping the
  existing `/DEBUG:NONE` + `/STACK:8388608` link-args, translated to lld-link syntax).
- **Bench:** link-time for a large crate (`touch crates/vox-cli/src/main.rs` → timed
  rebuild) under lld-link vs MSVC link.exe; report the delta.
- **Stability gate:** `cargo test -p vox-cli` passes **5×** consecutively, zero
  permission-denied.

### Guard — doctor linker check

Extend `checks_standard/build_health.rs`: a check that the configured Windows linker is
present + (if Defender is active) the `target/` exclusion exists, emitting the structured
`[diag id=linker.* …]` tag. Registers a new `linker.msvc_fallback` / `linker.av_no_exclusion`
diagnosis id.

## Success criteria

lld-link active on Windows, a clear link-time reduction on a large crate (target: **≥25%**
faster link), and `cargo test -p vox-cli` green 5× running. Config + a docs record updated;
doctor guards it.

## Exit criterion

If the holder is Defender AND exclusion is disallowed (policy), or the lock is a race no
bounded retry survives, **stay on MSVC** and replace the config comment with the *measured
holder + why* — turning a vague "revisit when fixed" into an actionable record.

## Testing

- Phase 0/3 are operational measurements (not unit-testable): the holder capture + the
  5×-cargo-test stability gate + the link-time bench are the verifications.
- The retry shim (if built) gets a unit test: `ERROR_SHARING_VIOLATION` → retried;
  other errors → propagated immediately.
- The doctor linker check: unit-test the pure classifier (configured-linker string →
  present/absent verdict) per the build_health pattern.

## Risks

- **Defender exclusion needs admin / may be policy-blocked** on managed machines → the exit
  criterion covers it.
- **lld-link arg translation**: MSVC `/DEBUG:NONE` `/STACK:…` are lld-link-compatible
  (lld-link accepts MSVC-style flags), but verify the produced binary runs + stacks size.
- **Per-worktree exclusions**: new worktrees need the exclusion too → the doctor check is
  what keeps that honest.
