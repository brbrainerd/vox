# Crate Disentanglement: Edge Ratchet + Worst-First Decoupling

> Design for stopping cross-crate entanglement growth (human-gated edge ratchet + layer
> rule, written for LLM readers) and then unwinding the worst verified offenders for
> build-time and repo health. Brainstormed + data-verified 2026-07-03.

## 1. Problem and verified ground truth

The workspace has **125 crates and 643 in-tree dependency edges**, and every
autonomous session can add edges freely. Verified from `contracts/ci/crate-graph.v1.json`
(2026-07-03):

| Class | Crate | Number | Verdict |
|---|---|---|---|
| God (fan-out) | vox-cli | 60 out | split program already running; internal floor = command registry (see PR-4 postmortem in `2026-06-30-vox-cli-ci-pr4-guard-batches.md`) |
| God (fan-out) | vox-orchestrator-mcp | 44 out | untreated |
| God (fan-out) | vox-orchestrator | 28 out | untreated |
| Keystone (fan-in) | vox-config | 46 in | **worst structural fact**: depends on `vox-llm-egress` + `vox-git`, dragging tokio/reqwest/git into ~46 dependents' trees |
| Keystone (fan-in) | vox-db | 28 in | depends on `vox-compiler`, `vox-ast`, `vox-codegen` — editing the compiler rebuilds the DB layer and its 28 dependents |
| Keystone (fan-in) | vox-secrets | 34 in | **acquitted** — deps are only `vox-bounded-fs` + `vox-crypto`; thin keystones are good |
| Exempt | workspace-hack | 71 in | hakari feature-unification; by design |

Lesson encoded from vox-secrets: **fan-in alone is not guilt; fan-in × dependency-subtree
weight is the metric.** A thin, stable keystone is healthy architecture.

### What already exists (extend, don't duplicate)

- `vox-arch-check` — Tarjan SCC cycle detector (`checks/cycles.rs`; catches dev-dep
  cycles — Cargo forbids hard cycles), forbidden-patterns, publishability.
- `vox ci fan-in-budget` (`crates/vox-cli-ci/src/fan_in_budget.rs`) — per-crate fan-in
  **count** ratchet against `contracts/ci/fan-in-snapshot.v1.json`.
- `contracts/ci/crate-budget.v1.json`, `contracts/ci/dep-backedges.allow.json` — partial
  budget/allow infrastructure.
- `contracts/ci/crate-graph.v1.json` — the committed dependency **mirror** (regenerated
  by tooling to track reality; it is drift-detection, NOT a ratchet — a regen legitimately
  admits any new edge).

### The three gaps that let entanglement through

1. **Count ratchets miss edge swaps.** Adding `A→B` while `C→B` is removed keeps B's
   fan-in count flat and passes today. Only an exact **edge-set** ratchet catches it.
2. **No layer rule.** Nothing prevents a foundation crate from growing a dep on an app
   crate; each such edge is currently an individual fight instead of a class violation.
3. **No human gate.** An LLM can regenerate `fan-in-snapshot.v1.json` (or the graph
   mirror) to admit its own new edge. The ratchet is decorative without an
   authorization boundary the LLM is instructed not to cross.

## 2. Decisions (locked with user, 2026-07-03)

1. **Ratchet first, then decouple.** Freeze today's 643 edges as the baseline before
   spending weeks decoupling; otherwise concurrent sessions undo the work.
2. **Duplicate small, split big.** Helpers under ~50 lines get copied into the consumer
   (with provenance comment) instead of keeping a crate edge; larger shared surfaces
   split into narrow `-types`/`-core` crates. No forking of 100+ line chunks.
3. **Human-gated baseline.** Loosening the edge allow-list requires a ledger entry that
   is user-authorized-only. LLMs may propose entries in PR descriptions; they must not
   write them. Tightening is always allowed.

## 3. Phase 1 — the ratchet (one implementation plan)

### 3.1 New guard: `vox ci crate-edges`

Lives in `vox-cli-ci` (where guards now live), new `CiCmd::CrateEdges { tighten: bool }`
variant + dispatch arm, wired like every other guard.

**Check mode (default):**
- Compute the live edge set from `cargo metadata` (workspace members only; **normal +
  build dependencies**; dev-deps excluded in v1 — they don't ship in the binary closure;
  the arch-check SCC detector already covers dev-dep cycles).
- Load `contracts/ci/crate-edges.allow.v1.json`.
- **Fail** if any live edge is absent from `edges ∪ exceptions`. The failure message is
  written for LLM readers (the `[diag id=.. heal=..]` idiom): name the edge, state the
  rule, and give the two legal moves — (a) don't add the dep (check for a narrower
  `-types`/`-core` crate, or duplicate a <50-line helper per the defactor policy), or
  (b) ask the user to authorize a ledger entry; never write one yourself.
- **Warn** (not fail) if allow-list entries are stale (edge no longer exists) —
  prompts a tighten.
- `workspace-hack` edges are exempt both directions.

**Tighten mode (`--tighten`):** regenerate `edges` = current live set and drop stale
exceptions. The guard's own check verifies tightening is removal-only: a tighten that
would ADD an edge fails with the same heal text. Safe for any session to run.

### 3.2 Contract: `contracts/ci/crate-edges.allow.v1.json`

```json
{
  "schema_version": 1,
  "edges": [["vox-actor-runtime", "vox-bounded-fs"], "… sorted, full 643-edge baseline …"],
  "exceptions": [
    {
      "from": "crate-a", "to": "crate-b",
      "reason": "why this coupling is accepted",
      "date": "2026-07-03",
      "authorized_by": "brbrainerd"
    }
  ]
}
```

`edges` is the frozen baseline (machine-tightened). `exceptions` is the human-gated
ledger — append-only by policy, each entry requiring explicit user authorization.
Initial state: `edges` = the current 643, `exceptions` = [].

### 3.3 Contract: `contracts/ci/crate-layers.v1.json` + layer rule

```json
{ "schema_version": 1, "layers": { "vox-foundation": 0, "vox-config-core": 1, "vox-cli": 4 } }
```

Five layers: **L0** leaf foundation (no vox-deps: bounded-fs, crypto, foundation,
db-types, plugin-types…) · **L1** infrastructure (config-core, secrets, http-client,
telemetry, repository…) · **L2** domain (db, compiler, codegen, corpus, publisher…) ·
**L3** services (orchestrator, actor-runtime, search, plugin-host…) · **L4** apps/shells
(vox-cli, vox-gui, vox-ml-cli, orchestrator-mcp, integration-tests).

Rule, enforced inside the same `crate-edges` guard: an edge `A→B` requires
`layer(A) ≥ layer(B)`. Same-layer edges are allowed (the edge ratchet still constrains
them individually). Upward edges fail with heal text naming the layer definitions and
pointing at `where-things-live.md`. Initial assignments are generated from graph
topology (longest path from leaves) and hand-adjusted once during implementation;
crates missing from the file fail the guard (forces classification of new crates at
creation time). Known pre-existing upward edges (e.g. `vox-db(L2)→vox-compiler(L2)` is
same-layer and legal; anything genuinely upward found during baselining) get grandfathered
as ledger exceptions so the gate lands green, and become Phase 2 targets.

### 3.4 LLM-prevention rules (AGENTS.md — normative, cross-tool)

New "Dependency discipline" section, exact obligations:

1. Adding a workspace-crate dependency edge is **CI-gated** (`vox ci crate-edges`).
   Before adding one: check for a narrower `-types`/`-core` crate; consider the
   defactor policy (rule 3). If the edge is genuinely needed, **propose** a ledger
   entry in the PR description and stop — `exceptions` entries in
   `crate-edges.allow.v1.json` are **user-authorized-only**; never write one, and never
   regenerate baselines/snapshots (`crate-edges.allow`, `fan-in-snapshot`) to admit
   your own new edge.
2. New crates must be assigned a layer in `crate-layers.v1.json` at creation, per
   `where-things-live.md`. Dependencies must point same-layer or downward.
3. **Defactor policy:** a helper under ~50 lines may be duplicated into the consumer
   with a `// vox:defactored-from <crate> <date>` provenance comment instead of taking
   a crate edge. Larger shared surfaces are split into `-types`/`-core` crates. Never
   fork 100+ line chunks.
4. Keystone protection: prefer `vox-config-core`-style narrow crates over the fat
   keystone when only types/loading are needed.

The same rules go in the guard's failure output — enforcement text and instruction
text must never drift apart, so the AGENTS.md section is quoted from one source
(the guard's heal constant) or kept deliberately short and stable.

### 3.5 Existing-guard disposition

`fan-in-budget` stays as-is (harmless, subsumed); retire it in a later cleanup once
`crate-edges` has been green for a while. `dep-backedges.allow.json` semantics fold
into the layer rule during implementation (migrate entries, delete file).
arch-check cycles stays (dev-dep cycles are out of `crate-edges` v1 scope).

## 4. Phase 2 — worst-first decoupling (one plan per target, separate cycle)

Each target gets a dossier (mapping workflow fan-out, same method as the PR-4 execmap)
then its own spec→plan. Priority order by fan-in × subtree-weight:

1. **vox-config split** — biggest single build-time win in the repo. Extract
   `vox-config-core`: pure config types + TOML/file loading; **no `vox-llm-egress`, no
   `vox-git`, no network**. The ~40 dependents that only read config re-point to it
   (mechanical import rewrite). LLM/VCS-flavored surfaces stay in `vox-config` (which
   depends on `-core`) behind the existing `vox-llm-config` boundary. Success metric:
   `cargo tree -i tokio` no longer reaches the ~40 re-pointed crates via config.
2. **vox-db → {vox-compiler, vox-ast, vox-codegen}** — determine whether those edges
   are only exercised by `legacy-import`/codegen features; if so, make them
   feature-optional (default off); if not, extract the used slice into a narrow crate.
   Success metric: editing vox-compiler no longer rebuilds vox-db's 28 dependents by
   default.
3. **vox-orchestrator-mcp (44-out) / vox-orchestrator (28-out)** — dossier first;
   likely tool-family splits following the vox-cli-ci precedent.
4. **vox-cli (60-out)** — program continues (share/research/review/ci already
   extracted); the command-registry hub is its documented floor. Candidate follow-up:
   extract the command registry itself, which also unlocks the parked dispatcher move.
5. **Explicitly NOT targets:** vox-secrets (thin), workspace-hack (by design),
   vox-bounded-fs / vox-foundation (thin L0 keystones).

## 5. Phase 3 — deferred, out of scope here

Module-granular coupling budgets via graphify (the vox-cli command-registry lesson:
crate-level edges don't see intra-crate hubs). Revisit after Phase 2's first two
targets land.

## 6. Testing

- Unit tests in the guard (synthetic graphs, no cargo invocation): new edge → fail
  with heal text; removed edge → warn-stale, tighten passes; tighten-that-adds → fail;
  ledger entry admits its edge; upward layer edge → fail; missing-layer crate → fail;
  workspace-hack exempt.
- Contract-file goldens: schema round-trip for both v1 JSON files.
- One integration test: guard runs against the real workspace and passes at the
  committed baseline (this is also the CI wiring proof).
- CI: added to the same lane as the sibling guards (check-targets entry), blocking.

## 7. Risks

- **Baseline churn from concurrent sessions:** between writing the 643-edge baseline
  and merging, another session may add an edge. Mitigation: `--tighten` regenerates at
  land time; the gate flips blocking only after one green run on main.
- **crate-graph.v1.json mirror interplay:** the mirror keeps regenerating freely — the
  guard reads `cargo metadata` directly, not the mirror, so mirror regen cannot admit
  edges. (fan-in-budget reads the mirror; acceptable, it's subsumed.)
- **Layer misassignment friction:** a wrong initial layer produces false failures —
  mitigated by the one-time hand-adjustment pass and same-layer tolerance.
- **Ledger social engineering:** an LLM writing the exception itself remains possible
  mechanically; the countermeasure is layered (AGENTS.md prohibition + PR-diff
  visibility of a dedicated ledger file + user review). Accepted residual risk per the
  human-gated-baseline decision.

## 8. Effort

| Piece | Estimate |
|---|---|
| Phase 1: guard + 2 contract files + AGENTS.md + tests + CI wiring | 1–1.5 days |
| Phase 2.1 vox-config split | 2–3 days |
| Phase 2.2 vox-db feature-gating | 1–2 days |
| Phase 2.3/2.4 dossiers (workflow) | 0.5 day each |

Phase 1 is the implementation plan that follows this spec. Each Phase 2 target gets
its own plan once its dossier confirms the approach.
