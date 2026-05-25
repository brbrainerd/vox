---
title: "Post-Sprint Forward Plan (2026-05-25)"
description: "Forward plan covering all crates / tracks NOT fixed in the F-* sprint that closed out P0–P4 of the crate-audit-and-plan. Defines scope, gate conditions, and acceptance criteria for every remaining item so a future session can execute without re-planning. Companion to free-by-default-and-residual-work-plan-2026.md (which is now fully executed except for F-A push) and crate-audit-and-plan-2026.md (the original 50-task audit)."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-25"
training_eligible: true
training_rationale: "Forward-looking plan with explicit gates; designed for LLM continuation in future sessions."
sort_order: 36
---

# Post-Sprint Forward Plan (2026-05-25)

**Sprint just completed:** F-B (Tier-D polish), F-C (stale-ref sweep), F-D (ADR-042), F-E (vox-populi-types extraction), F-F-1..7 (Fast/Free tiers + free-by-default + audit), F-G (selection/ re-activation), B-2 (voxup workspace fix), plus follow-ups: exploration-bonus parity for Free/Fast, `QualityLevel::Balanced → Economy`, where-things-live.md updates.

**Companion docs:**
- [`free-by-default-and-residual-work-plan-2026.md`](./free-by-default-and-residual-work-plan-2026.md) — predecessor; all tracks except F-A executed
- [`crate-audit-and-plan-2026.md`](./crate-audit-and-plan-2026.md) — original 50-task audit (P0–P4 complete)
- [`free-by-default-audit-2026-05-24.md`](./free-by-default-audit-2026-05-24.md) — call-site audit with all three follow-ups now closed
- [`adr-042-vox-populi-types.md`](./adr-042-vox-populi-types.md) — layer-2 topology decision used in F-E

---

## 0. Ground truth (verified 2026-05-25)

| Signal | Value |
|---|---|
| `vox-arch-check` | `build.1237`: **clean ✓** |
| `cargo check --workspace` | **clean ✓**, ~34 s incremental |
| `vox-orchestrator` LoC | ~60,681 / 70,000 (13.3 % headroom) |
| `vox-dei-shim` LoC | ~5,016 / 8,000 (after A-12 + F-G) |
| Working tree | **clean** (last commit `e83a29e9a8`) |
| Local `main` vs `origin/main` | **+49 commits** (was +41 before this sprint; +8 from sprint) |
| Audit tasks complete | **50 of 50 actionable** (A-9, D-7 step 3+, A-19/F-H, A-20/F-I retained as documented deferrals; A-22, B-13, D-17 retired as resolved/FP this session) |
| Free-by-default | **live** — `default_cost_preference() = Economy`, `QualityLevel::Flash | Balanced → Economy`, `ModelTier::Free`/`Fast` first-class |
| `selection/` in `vox-dei-shim` | **compiling + tested** (10 tests passing) |
| `vox-populi-types` L2 crate | **landed** with `NodeRecord`, `PopuliRegistryFile`, helpers; `vox-populi` re-exports |

---

## 1. Track index

Each track below is self-contained: pick one, read §2 for its prescription, execute, update §3 acceptance. Ordering is **by gate**, not priority.

| Track | Code | Size | Gate | Status |
|---|---|---|---|---|
| **A. Push 49 commits** | **R-A** | S | user approval ✓ (this session) | **✅ DONE** (`5f1e9f1cd9` + R-B push `e5bb6afccb`) |
| **B. Retire resolved/FP audit IDs** | **R-B** | XS | none | **✅ DONE** (commit `e5bb6afccb`) |
| **C. Reconcile master forward plan** | **R-C** | S | none | **✅ DONE** (sprint §9 log, commit `7f2edd8e7e` etc.) |
| **D. C-16: `_frozen.md` decision** | **R-D** | M | owner sign-off | **✅ DONE** (Option 2 implemented in session 2: CR-A3 updated, `frozen_crates.rs` redirects to `data-storage-policy.v1.yaml`) |
| **E. D-7-rescope Step 3+ (MeshDriver routing)** | **R-E** | L | design decision needed | deferred |
| **F. D-9-rescope (vox-container impls → plugin)** | **R-F** | M+ | none, but no current pressure | deferred |
| **G. A-9: vox-secrets split** | **R-G** | L | won't reduce fan-in (documented in layers.toml) | **retired** (informational) |
| **H. F-H / A-19: vox-orchestrator-core** | **R-H** | XL | Rule 13 (>15 % LoC growth from v0.5.0) | gated |
| **I. F-I / A-20: vox-cli-ci** | **R-I** | L | no LoC pressure | deferred |
| **J. Stub remediation backlog** | **R-J** | varies | per-stub release wave | **scoped per-stub** |
| **K. C-2 follow-up: vox-plugin-mens-candle-metal** | **R-K** | M | requires Apple Silicon hardware | deferred (hardware) |

---

## 2. Detailed prescriptions

### R-A. Push 49 commits to `origin/main` (S, immediate)

**Goal:** Land the entire F-* sprint (and predecessor 41 audit commits) on origin so they survive machine loss and are visible to other agents.

**Steps:**

1. `git status` → confirm working tree clean.
2. `git rev-list --count origin/main..HEAD` → expect 49.
3. `git push origin main` → no `--force`, no hook skip.
4. Watch the workspace hooks (line-endings check) succeed.
5. If GitHub Actions are wired, watch the CI dashboard turn green.

**Acceptance:** `git rev-list --count origin/main..HEAD == 0` after push.

**Safety:** No force-push. No `--no-verify`. If the push is rejected (e.g., remote moved), `git pull --rebase --autostash` and re-push.

---

### R-B. Retire resolved + false-positive audit IDs (XS)

**Goal:** Stop carrying retired items in the audit doc; future audit agents waste cycles re-investigating.

**Edits to `docs/src/architecture/crate-audit-and-plan-2026.md`:**

| ID | Current status | New status | Reason |
|---|---|---|---|
| A-22 | Pending | **Retired (NOT NEEDED)** | `vox-distributed-training` + `vox-inference` already absent from `vox-populi/Cargo.toml`; `orphan_exempt = true` in layers.toml is the canonical pattern. |
| B-13 | NEW (tempfile dup) | **Retired (FALSE POSITIVE)** | `crates/vox-orchestrator/Cargo.toml:116` has single `tempfile = { workspace = true }` entry; no duplicate. |
| D-17 | "session-7 done, not verified" | **Verified DONE** | Commit `33f912a4258` cleanly unified `SkillManifest` across `vox-plugin-types` / `vox-plugin-api` / `vox-plugin-host`. |
| D-3 | Pending | **Verified DONE** | `vox-webhook` library does not exist; only `vox-plugin-webhook` is present. Audit was stale. |
| D-8 | Pending | **Verified DONE** | `vox-plugin-oratio-mic` does not exist; only `vox-plugin-oratio` is present. Audit was stale. |
| B-1 | Pending | **Verified DONE** | Root `Cargo.toml:220` already shows `dirs = "6"` and tantivy with `default-features = false`. |
| B-6 | Pending | **Verified DONE** | No `mockito` anywhere in the workspace Cargo.tomls; `wiremock = { workspace = true }` is in `vox-populi/Cargo.toml` dev-deps. |
| B-2 | Pending | **DONE (sprint, commit `e83a29e9a8`)** | `voxup` wired into workspace metadata + `dirs` removed. |
| D-7-rescope Step 2 | Deferred | **DONE (sprint, commit `a0a236ee44`)** | `vox-populi-types` L2 crate (ADR-042) extracted; `NodeRecord` lives at L2, not L0. |
| A-9 | Pending | **Retired (documented deferral)** | `layers.toml:89` already notes: "deferred: all consumers use resolution fns, so split would not reduce fan-in." Add cross-reference to this plan §2/R-G. |

**Steps:**

1. Open `crate-audit-and-plan-2026.md`, jump to the §7 execution log.
2. Add a Session-10 (2026-05-25) row summarising the retirements above.
3. For each table in the doc that lists those IDs as Pending/Todo, flip the cell to ✅ with a one-line reason.
4. Do **not** delete the rows — leave them with `Retired` so historical context remains.
5. Commit with message: `docs(audit): retire 10 resolved/FP items + record session-10 completions`.

**Acceptance:** Re-running an audit agent against the doc returns ≤ 6 truly-open items (R-D, R-E, R-F, R-H, R-I, R-J).

---

### R-C. Reconcile master forward plan with sprint completions (S)

**Goal:** Update `free-by-default-and-residual-work-plan-2026.md` so its §6 acceptance checklist reflects reality.

**Edits to §0 "Verified ground truth" table:**
- `cargo check --workspace`: 34 s incremental (was 54.5 s clean).
- `vox-orchestrator` LoC: 60,681 / 70,000 (unchanged).
- Local-vs-origin: **49 commits unpushed** (was 41).
- Free-tier infrastructure: now **active**; `default_cost_preference() = Economy`, `ModelTier::Free`/`Fast` first-class.
- ModelTier variants: now `Unknown`, `Local`, `Free`, `Fast`, `Light`, `Pro`, `Elite`.

**Edits to §6 acceptance:**
- Items 5, 6, 7, 8, 9, 10 → flip ⬜ to ✅ with commit-sha breadcrumbs.

**Edits to §2 work-track index:**
- F-B, F-C, F-D, F-E, F-F, F-G → mark **complete**.
- F-A → mark **READY** (was unstarted; user approval now in hand).
- F-H, F-I → unchanged (gated/deferred).

**Steps:**

1. Read §0/§2/§6 verbatim from `free-by-default-and-residual-work-plan-2026.md`.
2. Replace each row with its post-sprint value as above.
3. Add a §9 "Session 10 completion log" at the end with the seven commit SHAs and the F-A push confirmation.
4. Commit: `docs(plan): mark F-B/C/D/E/F/G complete; F-A push landed`.

**Acceptance:** §6 has at most one unchecked box (F-A), and that box flips when R-A pushes.

---

### R-D. C-16: resolve `crates/_frozen.md` (M, blocks v1.0)

**Goal:** Either restore `crates/_frozen.md` as a curated frozen-crate list or formally retire CR-A3 (the v1-release-criteria entry that references it). Currently this is the **only P0-priority audit item left open**.

**Background:** A prior session deleted `crates/_frozen.md` and made `frozen_crates.rs` redirect to `contracts/db/data-storage-policy.v1.yaml`. CR-A3 in `v1-release-criteria.md` still says "frozen crate list must exist at `crates/_frozen.md`" and there's a CI guard that checks for the file.

**Choice (requires owner sign-off):**

**Option 1 — Restore.** Re-create `crates/_frozen.md` with the curated list (sourced from the storage-policy YAML). Update `frozen_crates.rs` to use it as the canonical source. CI guard remains.

**Option 2 — Retire.** Update CR-A3 to point to `contracts/db/data-storage-policy.v1.yaml` as the canonical frozen list. Delete the CI guard. Leave `frozen_crates.rs` as-is.

**Recommendation:** Option 2. The YAML is already the canonical source; the markdown file was duplicate data prone to drift.

**Steps (Option 2):**

1. Read `docs/src/architecture/v1-release-criteria.md` and find CR-A3.
2. Replace `crates/_frozen.md` references with `contracts/db/data-storage-policy.v1.yaml`.
3. Search `.github/workflows/` for a step that checks for `crates/_frozen.md`; delete or repoint.
4. Search `tools/` and `scripts/` similarly.
5. Add a short note in `frozen_crates.rs`'s module doc pointing to the YAML.
6. Commit: `chore(C-16): retire crates/_frozen.md path; YAML is canonical (CR-A3)`.

**Acceptance:** Grep for `_frozen.md` across the repo returns only historical/changelog mentions. CR-A3 reads naturally with the YAML path.

**Estimated effort:** 1 hour. Most of the work is finding stale references.

---

### R-E. D-7-rescope Step 3+: MeshDriver routing for non-plugin callers (L, deferred)

**Goal:** Route non-plugin callers off `vox-populi::*` runtime fns through a `MeshDriver` trait so `vox-populi` can be split into "policy library" and "runtime daemon" cleanly.

**Status:** Step 1 (port transport files to plugin) and Step 2 (extract `vox-populi-types`) are done. Step 3 is the L-effort caller migration.

**Scope:**

1. Define `trait MeshDriver` in `vox-mesh-types` (L0) with methods that today live as free fns in `vox-populi`:
   - `populi_env() -> PopuliEnv`
   - `populi_env_resolved(path) -> PopuliEnv`
   - `populi_advertise_gpu_effective(path) -> bool`
   - `node_record_for_current_process(id, addr) -> NodeRecord`
   - `normalize_http_control_base(s) -> Option<String>`
2. Provide a default implementation in `vox-populi` that delegates to the existing fns (no behaviour change).
3. Update `vox-cli`, `vox-orchestrator`, `vox-ml-cli` to take an `Arc<dyn MeshDriver>` instead of calling `vox_populi::*` directly.
4. Move the implementation to `vox-plugin-populi-mesh` so the daemon can ship without `vox-populi` as a library dep.
5. Delete the free fns from `vox-populi` once the migration is complete.

**Why deferred:** No active pressure. The current arrangement compiles cleanly and is documented in ADR-042 as the intentional L2 home. Premature trait-ification would just be ceremony.

**Acceptance:** `cargo tree` shows no non-plugin caller depending on `vox-populi` as a library.

**Trigger to start:** Plugin-isolation work (Hopper P0-T2 in the mesh SSOT) hits this surface.

---

### R-F. D-9-rescope: vox-container Docker/Podman impls → plugin (M+, deferred)

**Goal:** Move container runtime impls (`Docker`, `Podman`) out of `vox-container` and into `vox-plugin-runtime-container`, leaving `vox-container` as a thin trait + exec-grammar types crate.

**Status:** `vox-container-types` was extracted (layers.toml:144). Step 1 done. The impl move is what remains.

**Steps:**

1. Identify the 4 known consumers: `vox-cli`, `vox-skills`, `vox-plugin-runtime-container`, `vox-deploy-codegen`.
2. For each consumer, replace `vox_container::DockerRuntime` with `dyn ContainerRuntime` taken from the plugin host.
3. Move `vox-container/src/docker.rs` and `podman.rs` into `vox-plugin-runtime-container/src/`.
4. Delete the impls from `vox-container`.
5. Update tests + the catalog.toml plugin entry.

**Why deferred:** No active pressure; the current arrangement is layer-clean.

**Trigger to start:** Plugin-host-as-only-runtime work begins (currently no roadmap entry).

---

### R-G. A-9: vox-secrets split → types + store (L, RETIRED)

**Goal (original):** Split `vox-secrets` into `vox-secrets-types` (L0) + `vox-secrets-store` (L3) to reduce fan-in.

**Retirement reason:** `layers.toml:89` records: "deferred: all consumers use resolution fns, so split would not reduce this fan-in." Splitting would create churn (deprecation shims for 26 consumers) without architectural payoff. **Retire this task.**

**If this changes (trigger to reconsider):** Some future feature requires that a no-async / WASM / sandboxed crate import secret *types* (`SecretId` enum) without importing the resolver. Today no such consumer exists.

---

### R-H. F-H / A-19: extract `vox-orchestrator-core` (XL, Rule 13 gated)

**Goal:** Sister to A-12 (dei_shim) — extract policy / config / model-registry from `vox-orchestrator` into a lean `vox-orchestrator-core` L3 crate.

**Status:** Plan exists at [`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md).

**Gate (Rule 13):** Only triggered when `vox-orchestrator` grows >15 % from the v0.5.0 tag.

**Current LoC headroom:** 13.3 % below the budget cap; not at the trigger.

**No action required this calendar quarter.** Re-check after any major feature lands in `vox-orchestrator`.

---

### R-I. F-I / A-20: extract `vox-cli-ci` (L, deferred)

**Goal:** Move CI-only commands out of `vox-cli` into `vox-cli-ci` so the default `vox` binary doesn't carry CI deps.

**Status:** No plan doc exists yet. No LoC pressure on `vox-cli`. Deferred indefinitely.

**Trigger to start:** `vox-cli` LoC or compile-time grows uncomfortable, or someone wants to ship a CI-only image.

---

### R-J. Stub remediation backlog (varies, per-wave)

These are stubs/scaffolds that exist with explicit "not yet implemented" returns. Each is tracked against a release wave; no in-flight pressure. The "never ship new stubs" rule applies — these were inherited.

| ID | Location | Returns | Wave / gate |
|---|---|---|---|
| STUB-1 | `vox-skill-runtime/src/microvm.rs:46,53` | `Err("not yet implemented — firecracker/kata bindings deferred to v1.x")` | v1.x |
| STUB-2 | `vox-plugin-cloud/src/sync.rs` (3 methods) | "not yet implemented; SP7 scaffold" | SP7 plan |
| STUB-3 | `vox-plugin-oratio/src/audio.rs` (3 methods) | "not yet implemented; SP7 scaffold" | SP7 plan |
| STUB-4 | `vox-plugin-webhook/src/webhook/channel.rs:148` | logs "WebSocket send not yet implemented, message dropped" | implement or document HTTP-only |
| STUB-5 | `vox-actor-runtime/src/transport.rs:15` | comment "not yet implemented in codegen" | wire into codegen or remove variant |
| STUB-6 | `vox-grammar-export/src/lib.rs:105` | "Wave 3 backlog" | Wave 3 |

**Action for now:** None — each is gated on its wave. Plan revisit when the corresponding wave starts.

**Tracking:** Add a one-line note in each module's doc comment referencing this section so a future implementer knows the context.

---

### R-K. C-2 follow-up: `vox-plugin-mens-candle-metal` enablement (M, hardware-gated)

**Goal:** Verify the Apple Silicon path of the metal plugin compiles and runs an inference on real hardware.

**Status:** Plugin added to `catalog.toml` with `requires-tag = "apple-silicon"`. Bundle `vox-ml-metal` created. Not yet smoke-tested on hardware (no Apple Silicon in CI).

**Gate:** Owner with Apple Silicon hardware available.

**Steps when triggered:** standard plugin-build + plugin-smoke procedure documented in `vox-plugin-host/README.md`.

---

## 3. Acceptance criteria (all-tracks)

This forward plan is "fully executed" when:

1. ✅ R-A: `git rev-list --count origin/main..HEAD == 0` (52 commits pushed, head `e5bb6afccb`).
2. ✅ R-B: audit doc §7f added; 10 items retired/verified; ≤ 6 open items confirmed (R-E/F/H/I/J/K all gated).
3. ✅ R-C: master forward plan §9 session-10 log complete; all 10 acceptance boxes ✅.
4. ✅ R-D: C-16 Option 2 implemented in session 2 (`frozen_crates.rs` redirects; CR-A3 updated); verified 2026-05-25.
5. ✅ R-G: A-9 retired in audit doc §7f (documented deferral; fan-in split won't reduce dependents).
6. ⏸ R-E, R-F, R-H, R-I, R-J, R-K: gated/deferred; tracked here for visibility.

**This plan does not require any of the gated/deferred items to ship.** They are documented so future sessions don't waste cycles re-discovering them.

---

## 4. What this plan deliberately does **not** address

- **Mens distributed training (Mn-T1..T15)** — separate SSOT (`mesh-and-language-distribution-ssot-2026.md §3.5`).
- **Telemetry unification rollout** — separate SSOT (`telemetry-unification-design-2026.md`).
- **Vox language v1 release criteria (CR-L*)** — separate SSOT (`vox-as-llm-target-audit-and-plan-2026.md`).
- **Vox interop Phase 5 (React bridge)** — separate SSOT (`external-frontend-interop-plan-2026.md`).
- **Plugin restructures D-9-rescope, R-F** — deferred until consumer pressure.

If a future session feels the urge to expand scope into one of those, **stop and read the relevant SSOT first**. Each has its own gate criteria.

---

## 5. How to use this plan in a future session

1. Read **§0** ground truth to anchor.
2. Read **§1** index, pick the next unstarted track.
3. Read **§2** for that track's full prescription — every step is concrete.
4. Execute — each track is self-contained.
5. Update **§3 acceptance** when you complete a track.
6. Update **§0 ground truth table** at the bottom of the session.

If you complete R-A, R-B, and R-C, the audit & residual work is **fully closed**. Anything else is genuine new feature work that lives in a different SSOT.

---

## 6. Session-10 (2026-05-25) breadcrumb

Commits added by the sprint that produced this plan:

| SHA | Subject |
|---|---|
| `94ce7d5b5f` | feat(arch): extract dei_shim as vox-dei-shim wedge crate (A-12) |
| `900bfd58c5` | chore(claude): expand permissions allowlist (session 9) |
| `298c4d81e8` | docs(audit): record session-9 completion — all P0-P4 tasks done |
| `4f9c40f9fa` | fix(refs): update stale dei_shim path references after A-12 extraction |
| `f93efdbb03` | docs(arch): update Tier-D plan to reflect post-A-12 orchestrator LoC |
| `bbf9bed00b` | docs(arch): master forward plan + F-B/F-C stale-ref cleanup |
| `175a03d6c8` | feat(routing): land free-by-default + Fast/Free model tiers (F-F + F-G) |
| `21b98edc74` | docs(routing): free-by-default audit + catalog tier reclassification |
| `a0a236ee44` | feat(F-E/ADR-042): extract vox-populi-types L2 crate |
| `f28e58daf0` | docs: add vox-populi-types row to where-things-live.md |
| `7f2edd8e7e` | feat(free-by-default): close remaining F-F-7 audit gaps |
| `e83a29e9a8` | fix(B-2): wire voxup into workspace metadata + drop dirs dep |

R-B audit retirement: `e5bb6afccb` (docs(audit): retire 10 resolved/FP items + record session-10 completions)

R-D verified done (no new commit needed — Option 2 was implemented in session 2).

**All R-A/B/C/D tracks complete as of 2026-05-25.**
