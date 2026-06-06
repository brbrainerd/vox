---
title: "Jujutsu as a First-Class Vox VCS — Master Design Spec"
description: "Make jj-lib the in-process VCS engine for Vox: a vox-vcs backend trait, real conflict/lock wiring, a repo.* language primitive, an orchestrator isolation policy, a GUI surface, and the verified deletion of the hand-rolled jj-shaped code it obviates."
category: architecture
---

# Jujutsu as a First-Class Vox VCS — Master Design Spec

**Status:** approved design (2026-06-05). SSOT for the multi-phase release. Per-phase
implementation plans live under `docs/superpowers/plans/2026-06-05-jj-first-class-pNN-*.md`
and are authored just-in-time per phase to avoid drift.

**Grounding:** every claim below is verified against current code (file:line) by a 5-probe
read-only audit on 2026-06-05. Where my earlier survey over-claimed ("no dormant code"), the
hand-verified findings here supersede it. Companion research:
[`agentic-version-control-automation-research-2026.md`](../../src/architecture/agentic-version-control-automation-research-2026.md)
and the 2026-06-05 "Version Control as a Vox Language Feature" research note (landed in P0).

---

## 1. Decisions (locked)

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | **jj-lib `0.42` is the engine, as a hard (non-optional) Rust dependency.** No external `jj` executable, no system libgit2. | jj-lib is purpose-built to be embedded ("usable from a GUI or TUI, or a server"); `0.42.0` (rel. 2026-06-04) does git fetch/push via pure-Rust `gix` (`git2`/libgit2 removed). Confirmed: [docs.rs/jj-lib/0.42.0](https://docs.rs/jj-lib/0.42.0/jj_lib/). |
| D2 | **Hybrid depth.** jj-lib in-process is the default backend; the in-memory CAS `SnapshotStore`/`OpLog` survive **only** as the no-repo/degraded fallback. | User steer. Keeps Vox versioning working when no jj/git repo is present; concentrates risk behind a trait. |
| D3 | **Delete the hand-rolled re-implementations jj-lib obviates; demote (not delete) the load-bearing stores; re-home nothing risky.** Every deletion is TDD-gated by a parity test first. | "Get rid of code jj obviates in its library," constrained by the standing rule to verify retirement claims by hand (a past audit was wrong 5/10× and nearly deleted ~9,670 live test lines). |
| D4 | **All jj-lib calls confined to one new crate, `vox-vcs` (L3).** The compiler never depends on jj-lib; `repo.*` executes against an injected `dyn VcsBackend`. | jj-lib's API is intentionally unstable/`#[doc(hidden)]`-heavy. One auditable blast-radius; keeps `vox-compiler` (L3, 45k LoC budget) decoupled. |
| D5 | **Master plan + per-phase implementation plans.** | User steer; far-out phases would drift if fully detailed now. |

---

## 2. Verified substrate (file:line) — what we build on

### 2.1 The wiring gaps are REAL (P1 thesis confirmed)

| Claim | Verdict | Evidence |
|---|---|---|
| FileLockManager acquire result discarded at the live submit path | **CONFIRMED** | `let _ = self.lock_manager.try_acquire(&fa.path, agent_id, lock_kind);` — [task_submit.rs:382](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs) |
| `ConflictManager::record_conflict` is never called in prod | **CONFIRMED** | All 7 call sites are `#[test]`/`#[cfg(test)]` (`conflicts.rs:243/268/273/286`, `occ.rs:160`, `vcs_tools/conflicts.rs:254`, `tests/vcs_test.rs:137`) |
| `workspace_merge_json` destroys + counts, no conflict detection | **CONFIRMED** | [json_vcs_facade.rs:119](../../../crates/vox-orchestrator/src/json_vcs_facade.rs) → `destroy_workspace` + `{files_merged}` |
| `ScopeGuard` defaults to `Warn` | **CONFIRMED** | `#[default] Warn` — `scope.rs:22` + `config/impl_default.rs:41` |
| Live MCP write path gates on scope-glob only, not locks | **CONFIRMED** | `scope_guard::check_scope(...)` at [dispatch.rs:84](../../../crates/vox-orchestrator-mcp/src/dispatch.rs); FileLockManager not consulted |

**Precision:** locks *are* enforced at **queue admission** (`PolicyEngine::check_locks()` → `policy.rs:165`,
reachable from `check_before_queue`, can reject with `LockConflict`). They are **not** enforced at the
**live write path**. P1 closes the write-path gap; it does not invent locking from scratch.

### 2.2 jj-lib 0.42 API anchors (for P0/P2)

| Need | jj-lib module / type |
|---|---|
| Open/init workspace + working copy | `workspace::Workspace`, `local_working_copy::LocalWorkingCopy` |
| Mutate repo in a transaction | `transaction::Transaction`, `commit_builder`, `MutableRepo` |
| Operation log + undo/redo | `op_store`, `op_walk`, `operation` |
| Conflicts as data + materialize | `merge::Merge<T>`, `conflicts` |
| Git fetch/push (pure gix) | `git`, `git_backend::GitBackend` |
| Colocated git init | `git_backend` (colocated mode) |

---

## 3. Architecture

### 3.1 The `vox-vcs` crate (NEW, L3)

```
crates/vox-vcs/                         layers.toml: vox-vcs = { layer = 3, max_loc = 20_000 }
├── trait VcsBackend                    # snapshot · commit · op-log · undo/redo · conflicts · merge · fetch/push · checkout
├── struct JjBackend  (default, built in P2)  # jj-lib 0.42 in-process — ALL jj_lib:: calls live here
├── struct CasFallback                  # NEW self-contained in-memory impl, no-repo/degraded mode only
│                                        #   (NOT a reuse of vox-orchestrator's SnapshotStore — that
│                                        #    would cycle, since vox-orchestrator depends on vox-vcs)
├── enum VcsBackendKind { Jj, Cas }     # runtime selection (detect + config), NOT a cargo feature
└── detect()                            # is a jj/git repo present & initable? → Jj else Cas
```

- **Layer legality** (verified vs `layers.toml`): `vox-orchestrator` (L3) and `vox-cli` (L5) may both
  depend on `vox-vcs` (L3). `vox-compiler` (L3) must **not** — it receives a `dyn VcsBackend` by
  injection (mirrors how the interpreter holds an in-memory `DbStore` and real DB binding happens
  outside the compiler).
- **One blast radius.** A new arch-check `forbidden_pattern` bans `jj_lib::` imports and
  `Command::new("jj")` outside `crates/vox-vcs/**`. Compose with the existing `raw-git-exec`
  pattern and the `GitExec` concurrency policy at
  [vox-orchestrator-mcp/src/git_exec.rs](../../../crates/vox-orchestrator-mcp/src/git_exec.rs)
  (and fix the **stale exempt row** `crates/vox-vcs-git/src/git_exec.rs` at `layers.toml:234` —
  that crate does not exist).
- **`jj-backend` cargo feature is removed** (jj-lib non-optional). The feature-gated `JjBridge`
  calls in `workspace.rs:213` are replaced by `VcsBackend` calls, not left dangling.

### 3.2 Backend trait sketch

```rust
pub trait VcsBackend: Send + Sync {
    fn snapshot(&mut self, label: Option<&str>) -> Result<ChangeId>;     // working-copy-is-a-commit
    fn changes(&self) -> Result<Vec<Change>>;                            // op log
    fn diff(&self, a: Option<ChangeId>, b: Option<ChangeId>) -> Result<Diff>;
    fn undo(&mut self) -> Result<ChangeId>;
    fn redo(&mut self) -> Result<ChangeId>;
    fn new_change(&mut self) -> Result<ChangeId>;                        // jj `new`
    fn conflicts(&self) -> Result<Vec<Conflict>>;                       // first-class
    fn resolve(&mut self, path: &Path, strat: ResolveStrategy) -> Result<Resolution>;
    fn fetch(&mut self, remote: &str) -> Result<FetchOutcome>;          // gix
    fn push(&mut self, remote: &str, change: ChangeId) -> Result<PushOutcome>;
    fn checkout(&mut self, change: ChangeId) -> Result<()>;
}
```

All types (`ChangeId`, `Conflict`, …) are Vox-native; jj-lib types never leak across the trait.

---

## 4. Deletion / demotion ledger (TDD-gated, verified consumers)

Each row lands **only after** a parity test proves the replacement reproduces observable behavior.

| Target | Verified disposition | Evidence | Action |
|---|---|---|---|
| `ContentMerge` + methods ([jj_backend.rs:47](../../../crates/vox-orchestrator/src/jj_backend.rs)) | **truly dead** (own tests + a `lib.rs:303` re-export only) | consumer audit | **Delete.** P1 wires jj-lib `Merge<T>`/`conflicts` fresh where auto-resolution is needed. |
| `OperationDag`/`DagNodeId` ([jj_backend.rs:160](../../../crates/vox-orchestrator/src/jj_backend.rs)) | **truly dead** (doc-comment names callers that don't exist) | consumer audit | **Delete.** Use jj-lib `op_walk`/`dag_walk`. |
| `JjBridge` subprocess ([jj_backend.rs:264](../../../crates/vox-orchestrator/src/jj_backend.rs)) | **used-in-prod but feature-gated** (`workspace.rs:213`) | consumer audit | **Replace** with `VcsBackend` calls; delete the subprocess + feature gate. |
| `vox vcs` CLI `jj()` subprocess ([vcs.rs:14](../../../crates/vox-cli/src/commands/vcs.rs)) | **used-in-prod** | consumer audit | **Rewrite impl** in-process via `vox-vcs`; command surface unchanged. |
| `vox-git/sync.rs` fetch/push | **types-only stubs** | consumer audit | **Delete stubs**; real fetch/push lives in `vox-vcs` (jj-lib `git`). |
| `vox-git` `jj-backend` feature | **orphaned** (zero cfg-gated code) | consumer audit | **Remove** from `vox-git/Cargo.toml`. |
| `SnapshotStore` ([snapshot.rs:93](../../../crates/vox-orchestrator/src/snapshot.rs)) | **load-bearing hot path** (`capture_snapshot`, json facade) | consumer audit | **Demote to `CasFallback`.** Keep. |
| `OpLog` ([oplog/](../../../crates/vox-orchestrator-queue/src/oplog/)) | **load-bearing audit trail** | consumer audit | **Keep** as orchestrator audit trail; jj op-log is additive, **not** a replacement. |
| `restore_fs_snapshot` + `capture_snapshot`/`record_operation` ([vcs_ops.rs](../../../crates/vox-orchestrator/src/orchestrator/vcs_ops.rs)) | **core used** (undo/redo/list wrappers are exposed-unused) | consumer audit | **Keep** core for fallback; clean up the unused `undo/redo/list` wrappers separately. |
| `vox-git` crate (`GitBridge`, `read_cmd::read_only`) | **3 live consumers** (CodeRabbit, `vox-effort-audit`, `vox-effort-route`) | consumer audit | **Keep** as the pure-Rust git reader. **Not** folded into `vox-vcs`. |

**Net deletion** is real but surgical: the dead jj-shaped algorithms and stubs go; the working stores
are demoted behind the trait, not removed (honoring D2/D3).

---

## 5. Language primitive — `repo.*` + `Vcs` effect (P3)

Mirror the **`Browser.*`/`Scrape.*` builtin-dispatch** pattern (imperative calls), **not** the `db.*`
query-plan interception. (Verified: `db.*` is special-cased at `expr.rs:449` via the `opt_plan`
field; `repo.*` calls like `snapshot()`/`undo()` are imperative and belong in `call_builtin_method`
at [builtins.rs:108](../../../crates/vox-compiler/src/eval/builtins.rs).)

**Surface (jj-shaped):**
```
repo.snapshot(label?) -> Change      repo.changes() -> [Change]      repo.diff(a?, b?) -> Diff
repo.undo() / repo.redo()            repo.new() -> Change            repo.conflicts() -> [Conflict]
repo.resolve(path, strategy)         repo.restore(path)
```

**`Vcs` effect / capability** — add a variant at the four verified sites (the `Mcp(String)`
parameterized precedent supports `Vcs(repo)` later):
- `EffectAnnotation` + `from_keyword` + `as_str` — [ast/decl/effect.rs:8](../../../crates/vox-compiler/src/ast/decl/effect.rs)
- `HirEffectKind` — [hir/nodes/effect.rs:9](../../../crates/vox-compiler/src/hir/nodes/effect.rs)
- `HirCapability` + `effect_kind_to_cap` — [hir/nodes/decl.rs:700](../../../crates/vox-compiler/src/hir/nodes/decl.rs), `effect_check.rs:76`
- **Cheapest hook:** add `"repo" | "vcs" => Some(HirCapability::Vcs)` to `stdlib_module_capability`
  ([effect_check.rs:506](../../../crates/vox-compiler/src/typeck/effect_check.rs)) → every `repo.*`
  call is auto-governed by `uses vcs` and inferred bottom-up by `infer_expr_effects` (`:389`).

**Packaging:** follow the `Db` precedent — `hir_capability_to_packaging_id` returns `None` for `Vcs`
initially (no `runtime-capabilities.v1.yaml` row needed; governance is typeck-level). Add a real row
later only if a host permission is required.

**Interpreter `RepoStore`:** add `interp.repo: RepoStore` beside `interp.db: DbStore`
([eval/mod.rs:49](../../../crates/vox-compiler/src/eval/mod.rs)). Default = light in-memory impl
(gives every Vox app cheap time-travel); the orchestrator/CLI inject a `dyn VcsBackend` bound to
`JjBackend` for Vox's own development.

---

## 6. Orchestrator isolation model (P4)

Drive strategy from the existing per-task `FileAffinity` + `overlapping_paths()` + user policy:

1. **Shared change, file-partitioned** (default, disjoint write sets) — all agents on one jj change;
   `FileLockManager` (made authoritative in P1) grants single-writer leases per file; zero worktree tax.
2. **Per-agent change, auto-rebased** (overlapping/risky sets) — anonymous jj change per agent; merge
   back; unresolvable overlaps become **recorded conflicts** (`record_conflict`, wired in P1), never a
   hard failure.
3. **Separate branches** (long-running/human-review) — classic isolation, cheap via jj.

Policy is an in-language `@config`/app-contract field (no `vox.toml`; config projects via
[app_contract.rs:92](../../../crates/vox-compiler/src/app_contract.rs)) and is fully overridable.

**Mesh composition (open, see §9):** the per-`scope_key` distributed lease (`a2a/dispatch/lease_gate.rs`)
must compose with per-file leases — define `mesh lease ⊇ local file lease`.

---

## 7. GUI surface (P5)

No `vcs` surface entry exists yet (the `vcs` CLI group would be backfilled as `representation_tier: none`).
Follow the verified registry path:

1. Add a `curated_decorator` entry to
   [`contracts/gui/surface-registry.v1.yaml`](../../../contracts/gui/surface-registry.v1.yaml):
   `view_key: vcs · cli_group: vcs · representation_tier: curated_decorator · nav_label: VCS ·
   nav_icon: branch · nav_group: develop`.
2. `vox ci gui-surface-registry --write` regenerates
   `surfaceRegistry.generated.ts`; the CI gate ([ci/gui_surface_registry.rs](../../../crates/vox-cli/src/commands/ci/gui_surface_registry.rs))
   enforces `view_key` is wired in `App.tsx`.
3. Tauri commands in `crates/vox-gui/src/commands/vcs.rs` → call `vox-vcs` (via daemon RPC / MCP VCS tools).
4. React decorator `ui/src/components/surfaces/Vcs/VcsView.tsx`: operation log, live conflicts,
   per-agent changes, isolation-strategy selector, one-click undo. Register in `decoratorRegistry.ts`.
5. Tests: vitest unit + Playwright e2e (`ui/e2e/vcs.spec.ts`).
6. Ludus reward wiring for VCS ops (mirror Gamify integration).

---

## 8. Phase roadmap

Each phase = its own `docs/superpowers/plans/2026-06-05-jj-first-class-pNN-<slug>.md`, TDD-first.

| Phase | Goal | Key exit criteria |
|---|---|---|
| **P0 Foundations** | `vox-vcs` crate + `VcsBackend` trait + `CasFallback` (new in-memory impl) + `detect()`. Bump `=0.27`→`0.42` (jj-lib non-optional in `vox-vcs`). Land the 2026-06-05 research doc. layers.toml + WTL rows; arch-check `jj-lib-confined` pattern; fix stale exempt row. (`JjBackend` real impl + `jj-backend` feature removal move to P2.) | Builds with jj-lib linked; arch-check green; **zero behavior change** (additive only). Plan: [p0](../plans/2026-06-05-jj-first-class-p0-vox-vcs-foundation.md). |
| **P1 Wire the safety substrate** | Make `FileLockManager` authoritative at the MCP write path (`dispatch.rs`, read-only holder probe); call `record_conflict` from the merge-back path; jj-lib conflict auto-resolution deferred to P2. (~~default `ScopeGuard`→`Strict`~~ — tried and **reverted**: orchestrator scope is auto-seeded, so Strict freezes a single agent's file set after its first task; cross-agent safety already comes from locks + the always-on MCP declared-scope guard.) | Two agents on one file produce a **recorded conflict**, not a clobber; write outside another agent's lock is **rejected**; tests prove it. |
| **P2 Engine swap + dead-code removal** | Build `JjBackend` (jj-lib 0.42 methods, each TDD'd against the API); replace `JjBridge` + `vox vcs` subprocess + `vox-git/sync.rs` stubs with in-process jj-lib; **remove the `jj-backend` cargo feature**; delete `ContentMerge`/`OperationDag`; demote `SnapshotStore`/`OpLog`; git fetch/push via jj-lib. | Every deletion preceded by a green parity test; **real fetch/push integration test** passes against a throwaway local remote; no `Command::new("jj")` remains. |
| **P3 Language primitive** | `repo.*` builtins + `Vcs` effect + `RepoStore`; capability governance. | Golden `.vox` with `@test` blocks exercises `repo.snapshot/undo/conflicts`; `uses vcs` enforced by typeck. |
| **P4 Isolation policy** | Three strategies chosen from affinity/overlap/policy; in-language config. | Orchestrator selects strategy per workload; overridable; covered by tests. |
| **P5 GUI surface** | Registry → CI gate → Tauri → React decorator; reward wiring. | Surface registry gate green; vitest + Playwright pass; op-log/conflicts/undo visible. |
| **P6 Decorators + auto-snapshot** | `@versioned`/`@tracked`; auto-checkpoint at inferred `fs`/`db` effect boundaries. | Annotated fn auto-snapshots; effect-boundary checkpoint debounced via CAS dedup. |

---

## 9. Cross-cutting

- **Testing (AGENTS.md Test-First):** every new `pub fn` gets a `#[test]` in-file **before** impl;
  golden `.vox` `@test` blocks for `repo.*`. New harnesses: a **jj temp-repo fixture**; a
  **backend-parity suite** (`JjBackend` vs `CasFallback` agree on observable behavior); a **real
  gix fetch/push integration test** (de-risks push maturity — run early in P2).
- **Arch-check:** `vox-vcs` row (L3/20k); WTL row; `jj_lib::`/`jj`-exec forbidden-pattern; remove the
  stale `vox-vcs-git` exempt path.
- **Docs:** YAML frontmatter on every new `docs/src/` file.
- **Risks:** jj-lib API churn → confined to `vox-vcs` + version-pinned probe test. gix push maturity →
  early integration test. Binary assets → "last-writer + conflict record" path. Snapshot-on-write
  perf → CAS dedup + effect-granularity debounce.

## 10. Open questions / flagged missing context

1. **Repo bootstrap (assumed):** default to colocated init (`jj git init --colocated` via jj-lib
   `git_backend`) so plain `git` keeps working for non-jj users. *Confirm.*
2. **Mesh lease precedence (unresolved):** exact composition of per-`scope_key` distributed lease with
   per-file leases. Needs a P4 mini-design.
3. **`GitExec` consolidation (decision):** the existing concurrency-policy `GitExec` lives in
   `vox-orchestrator-mcp`. Should jj-lib's in-process gix ops route through (or replace) it to honor
   single-writer-to-`.git`? Recommend addressing in P2.
4. **`vox-git` long-term (resolved):** keep as git reader; not folded into `vox-vcs`.
