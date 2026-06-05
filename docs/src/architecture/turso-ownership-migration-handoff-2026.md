---
title: "Turso Ownership Migration — Handoff (2026)"
description: "Handoff for the Turso-ownership migration: moving direct turso:: database usage out of satellite crates into the sanctioned vox-db/vox-secrets data layer. Covers the policy SSOT (data-storage-policy.v1.yaml), the two CI guards, the YAML-vs-txt allowlist rule, the per-crate backlog with definitions of done, and the nomenclature (vox-populi→vox-ml) overlap."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-06-05"
training_eligible: true
training_rationale: "Captures the current state and remaining backlog of the Turso-ownership migration so the crate-leaf-design track can complete it without re-deriving the policy/guard mechanics."
---

# Turso Ownership Migration — Handoff (2026)

> **Why this exists.** While landing an unrelated PR, the pre-push CI guards
> (`turso-import-guard`, `policy-allowlist-parity`) surfaced this half-finished
> migration. This doc captures its current state, the governing SSOT, the two
> guards, and the exact remaining backlog so the owning track can finish it
> cleanly. It also records a foot-gun that cost time (see §6).

## 1. What the migration is

Vox's relational store is **libSQL/Turso**. Policy: **only the sanctioned data
crates may call `turso::` directly** (so the schema, migrations, and query
surface stay in one place). Everything else must go through **typed `vox-db`
store ops**. Several satellite crates still use Turso directly and are
tracked as *transitional exceptions* until their usage is folded into `vox-db`.

## 2. The policy SSOT (authoritative)

`contracts/db/data-storage-policy.v1.yaml`, tier `a_relational`:

```yaml
tiers:
  a_relational:
    store: libsql
    owners:            [vox-db, vox-secrets]      # canonical data layer
    allow_direct_access: [vox-db, vox-secrets]
    temporary_exceptions: [vox-package, vox-gamify, vox-cli]   # transitional, in-YAML
```

**This YAML is the single source of truth** for "who may use Turso directly."
New owners or exceptions are declared **here**, not anywhere else.

## 3. The two guards

Both run inside `vox ci ssot-drift` (and thus the pre-push gate). Source:
`crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs` and
`policy_allowlist_parity.rs`.

- **`turso-import-guard`** — regex-scans `.rs` files for `\bturso::`. A hit
  outside the allowlist fails the build. The allowlist is assembled from three
  sources, in this order:
  1. **Built-in prefixes:** `crates/vox-db/`, `crates/vox-package/`, `crates/vox-compiler/`.
  2. **YAML-derived:** every crate in `owners ∪ allow_direct_access ∪ temporary_exceptions`.
  3. **Transitional txt:** each line of `docs/agents/turso-import-allowlist.txt`.
- **`policy-allowlist-parity`** — keeps the txt and YAML consistent. It fails if
  a txt entry **redundantly** lists a crate that the YAML already covers
  ("…redundantly — already a policy owner. Remove the txt entry; the YAML is the
  source of truth"), if a txt entry points at a non-existent dir, etc.

### The golden rule
> **YAML wins. The txt file is *only* for crates not yet in the YAML.**
> If a crate is a YAML owner/exception, **do not** add it to the txt — that is a
> guard failure, not a fix. To grant Turso access, add to the **YAML**; to remove
> it, **migrate the code** into `vox-db` and delete the txt line.

## 4. Current transitional txt allowlist

`docs/agents/turso-import-allowlist.txt` currently lists: `vox-corpus/`,
`vox-workflow-runtime/tests/`, `vox-populi/src/transport/store/`,
`vox-codegen/src/codegen_rust/`, `vox-plugin-populi-mesh/src/transport/store/`,
`vox-gui/src/commands/`, `vox-scientia/src/producers/`.

## 5. The backlog (definition of done per entry)

**A. Remove false positives (no real `turso::` linkage) — quick wins:**

| Entry | Why it's a false positive | Done when |
|---|---|---|
| `vox-workflow-runtime/tests/` | no `turso::` usage detected | txt line removed |
| `vox-codegen/src/codegen_rust/` | only *emits* `turso::params![]` as generated source text; doesn't link turso | txt line removed (and/or the guard taught to skip codegen string contexts) |
| `vox-plugin-populi-mesh/src/transport/store/` | JSON-file fallback store; no `turso::` | txt line removed |

**B. Real migrations (move code into typed `vox-db` ops):**

| Crate · site | Turso usage | Done when |
|---|---|---|
| `vox-corpus` · `arca_replay.rs` | `turso::params!` A2A/event extraction | `vox-db/src/store/ops_corpus.rs` exposes a typed `query_corpus_pairs()`; vox-corpus calls it; txt line removed |
| `vox-populi` · `transport/store/voxdb.rs` | full `VoxDbMeshStore` impl (params, row unpack, error map) | `vox-db/src/store/ops_mesh.rs` holds the typed mesh-store ops; vox-populi calls through them; txt line removed. **Coordinate with the `vox-populi→vox-ml` rename (§7).** |
| `vox-gui` · `commands/memory.rs` | `turso::{params,Rows,Row}` for the REPL memory surface | `vox-db/src/store/ops_gui.rs` exposes typed `fetch_memory_*()`; gui calls them; txt line removed |
| `vox-scientia` · `producers/bench_history.rs` | `turso::params!` tool-latency extraction | new `vox-db/src/store/ops_scientia.rs` exposes typed `fetch_tool_durations()`; producer calls it; txt line removed |

**Migration complete when:** all of B is folded into `vox-db`, all false
positives in A are removed, and `docs/agents/turso-import-allowlist.txt` contains
only genuinely-transitional entries (ideally empty). The two guards then enforce
the invariant permanently.

## 6. Foot-gun encountered (read before touching this)

A **stale installed `vox.exe` (0.5.0)** — run for *direct* `vox ci` diagnostics
while the workspace was `0.6.0` — reported turso-guard failures for `vox-gamify`
and `vox-secrets/backend`. Those are **already covered** by the current source
(vox-gamify is a YAML `temporary_exception`; vox-secrets is an `owner`). Acting
on the stale output, a contributor *added* them to the txt allowlist — which the
**current** `policy-allowlist-parity` guard correctly rejects as redundant. The
fix was to revert that edit. **Lesson:** diagnose with `cargo run -p vox-cli --
ci …` (source), not a possibly-stale installed binary — see the companion
[Vox binary freshness & SSOT](../contributors/vox-binary-freshness-and-ssot-2026.md).

## 7. Nomenclature overlap

The Latin→English crate-rename migration (`nomenclature_guard.rs` denylist:
`populi→ml`, `gamify→gamification`, `oratio→speech`, `schola→tutorial`, …)
intersects this work at exactly one point: **`vox-populi`** carries both a
pending Turso migration (mesh transport store) and a pending rename to
**`vox-ml`**. Land `vox-db/src/store/ops_mesh.rs` **before or atomically with**
the rename so the renamed crate immediately calls the typed API rather than
re-introducing direct `turso::` under a new name. The other Turso-bearing crates
(`vox-corpus`, `vox-gui`, `vox-scientia`) are **not** on the rename list, so they
can migrate independently.

## 8. Ownership & cross-refs

- Likely owner: the **crate-leaf-design remediation** track
  (`docs/superpowers/plans/2026-06-05-crate-leaf-design-remediation.md`, authored
  in a sibling worktree) — confirm it adopts this backlog.
- Policy doc: `docs/agents/codex-turso-allowlist.md`.
- Guards: `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`,
  `crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs`.
- Nomenclature map: `docs/src/archive/research-2026-q1/nomenclature-migration-map.md`.
