---
title: "Naming & Anti-Entanglement Guards (2026-05-08)"
description: "Design for LLM-navigation naming conventions and CI drift guards after the 2026-05-08 workspace reorg."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Naming & Anti-Entanglement Guards (2026-05-08)

> **Naming note (2026-05-08):** The CI guard binary referenced as `vox-layer-check` in this narration was renamed to `vox-arch-check` later in the same series; references below are historical.

Companion to [2026-05-08-workspace-reorg-outcome.md](./2026-05-08-workspace-reorg-outcome.md).
The reorg's first half (Phases 0–9) bought build-time wins. This second half
buys **navigation latency wins for LLM tool calls** and **drift prevention**
so future LLM-generated code doesn't re-tangle the architecture we just
straightened out.

## Problem

LLMs (this assistant included) navigate code by crate name first, grep second.
Confusing names burn tool calls: searching `vox-toestub` for "code quality"
fails; the assistant must read the lib.rs to discover what the crate actually
does. Multiply by every future code-generation session.

The previous reorg also revealed a tendency: this branch's commits added
~3.2 dep-edges for every one removed. The current ratio is explained by
extract-and-shim work, but the workspace lacks any automated mechanism to
flag that the trend is wrong in steady state.

## Goals

1. **Self-documenting names** — every crate's name signals its purpose strongly
   enough that an LLM with no prior context picks the right crate first try.
2. **Drift detection** — CI flags fan-in spikes, LoC bloat, accidental
   orphans, and dep-edge growth before it lands.
3. **A flat lookup table** for "I need to add X, where does it go?" — so the
   answer doesn't require reading 79 lib.rs files.

The plan deliberately does **not** promise build-time wins. Phases 4–5 of the
prior reorg already extracted the big LoC chunks (mcp 33K, queue 3K). The
wins here compound differently: every future code-gen session is faster,
every future regression is caught at PR time.

## Track A — Renames (8 crates)

Each rename uses an atomic-commit pattern: `git mv` the directory, update
the crate's `Cargo.toml` `name`, update `[workspace.dependencies]` in the
root `Cargo.toml`, find/replace `vox_old` → `vox_new` in source files and
`vox-old` → `vox-new` in `Cargo.toml` deps, `cargo check`, commit. **No
re-export shims** — workspace-internal scope means atomic rename is safe.

| # | From | To | Consumers | Rationale |
|---|---|---|---|---|
| 1 | `vox-toestub` | `vox-code-audit` | 3 | Acronym is self-defeating; "code audit" is what the crate does |
| 2 | `vox-ars-runtime` | `vox-openclaw-runtime` | 3 | "ARS" undefined; OpenClaw is the actual coupling |
| 3 | `vox-pm` | `vox-package` | ~6 | Single-letter abbrev; collides with "project manager" |
| 4 | `vox-mens` | `vox-ml-cli` | ~5 | Latin → industry term |
| 5 | `vox-ludus` | `vox-gamify` | ~6 | Latin → industry term |
| 6 | `vox-clavis` | `vox-secrets` | 15 | Latin → industry term |
| 7 | `vox-compiler-emit` | `vox-codegen` | ~6 | "emit" is opaque; "codegen" is universal |
| 8 | `vox-runtime` | `vox-actor-runtime` | ~25 | Disambiguates from `vox-skill-runtime`, `vox-workflow-runtime`, `vox-openclaw-runtime` — `*-runtime` suffix becomes a coherent family |

Phasing: P-A1 = renames 1–4 (cheap), P-A2 = 5–7 (moderate), P-A3 = 8 alone
(biggest blast radius).

## Track B — Anti-entanglement guards

Rename `crates/vox-layer-check` → `crates/vox-arch-check` and extend it with
four new rules. All read from a single source of truth: `layers.toml` with
an extended schema.

```toml
[crates.vox-orchestrator]
layer = 3
kind = "library"            # library | plugin | binary | test-only
max_dependents = 30
max_loc = 60_000
```

The rules:

1. **Fan-in tracker** — workspace dependents per crate vs. `max_dependents`. Warn on exceed.
2. **LoC budget** — `wc -l` over `src/**/*.rs` vs. `max_loc`. Warn on exceed.
3. **Orphan detector** — flag crates with 0 in-tree consumers AND `kind != "plugin" | "binary" | "test-only"`.
4. **Edge-flux log** — append `(commit_sha, edges_added, edges_removed)` to `dep-edge-log.csv` each CI run. No alerting; trends visible at a glance.

Plus one cheap regex check: warn if any `lib.rs` opens without a `//!` docstring (Track C requires it).

All rules default to **warn-only**; can flip individual rules to strict via
`[guards] fan_in = "error"`.

## Track C — Where-things-live map

Three deliverables, all under `docs/src/architecture/`:

1. **`where-things-live.md`** — flat lookup table covering the 80% of
   "where does this go?" questions. ~30 rows.
2. **One-sentence purpose docstring** at the top of every crate's `lib.rs`.
   The first `//!` line is the LLM signal. Enforced by the regex check in
   Track B.
3. **Pointer block** in `CLAUDE.md` and `AGENTS.md` that directs future
   sessions to consult `where-things-live.md` before adding code.

## Phasing

```
P-A1 → P-A2 → P-A3 → P-B → P-C
```

- P-A1, P-A2, P-A3 are sequential because P-B's `layers.toml` schema
  extension references the post-rename names.
- Each rename is one commit. P-B is one commit. P-C is one commit.

Estimated total: 11 commits, ~250 file edits (most are mechanical
`use vox_X::` → `use vox_Y::` rewrites).

## Acceptance criteria

For each rename phase:
1. `cargo check --workspace` green.
2. `cargo run -p vox-arch-check` (strict) clean.
3. No file still contains the old crate name in a way that would compile-fail.

For P-B:
4. The four new rules each produce sensible output on the current workspace
   (e.g. fan-in tracker reports `vox-db: 17, vox-compiler: 16, …`).
5. Old binary `vox-layer-check` removed; CI calls the new `vox-arch-check`.

For P-C:
6. Every `lib.rs` in `crates/*/src/` starts with `//! `.
7. `where-things-live.md` answers each row's question with a concrete crate
   path.
8. CLAUDE.md and AGENTS.md link to the new doc.

## Risk register

| Risk | Mitigation |
|---|---|
| Rename of `vox-runtime` breaks an external consumer (cargo binary, vox app, downstream user) | Internal scope only; if external consumers found, defer P-A3 with a noted exception |
| Find/replace mis-rewrites a Latin string in user-visible docs (e.g. `vox-clavis` mentioned in user-facing text) | Search for the string first; explicit allowlist of doc files to skip |
| `layers.toml` extended schema breaks the existing layer-check until updated | Same commit as P-B updates schema atomically; layer-check binary keeps the old schema readable |
| LoC budget guard triggers immediately on `vox-orchestrator` (52K, budget 60K close to ceiling) | Set budgets generously above current state; tighten over time |

## Out of scope

- Combining near-orphan crates (audit recommends keeping for now)
- New runtime extractions (Phase 6 stays deferred)
- Plugin family changes (already structurally clean)
- vox-db split (still deferred per Phase 3 audit)
