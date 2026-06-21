# Execution Constraints — Claude Sonnet 4.6 (Install/Release/Publish Program)

> **What this is:** the constraints + operating contract for a Claude Sonnet 4.6 agent
> executing any plan in this program (Track 0 already done by Gemini Flash;
> Tracks A–D and follow-ups go to Sonnet 4.6). Pair this with the specific track plan
> being executed. It is the Sonnet-4.6 analog of `2026-06-19-track0-FLASH-HANDOFF.md`.
>
> **Why Sonnet 4.6 differs from the Flash handoff:** Flash needed spoon-fed facts and a
> hard "STOP on any contradiction" rule. Sonnet 4.6 holds more context, verifies against
> the codebase itself, and may *resolve* a contradiction within bounds (below) — but the
> **policy constraints are identical and non-negotiable**.

---

## 0. Read-first

- `AGENTS.md` (normative cross-tool policy) and `docs/src/architecture/where-things-live.md`.
- `CLAUDE.md` (Claude-specific overlay).
- The specific plan you are executing + its design spec under `docs/superpowers/specs/`.
- `docs/superpowers/plans/2026-06-19-install-release-publish-INDEX.md` (program map).
- `docs/superpowers/plans/2026-06-19-track0-ACCEPTANCE-REVIEW.md` (the bar your output is judged against).

## 1. Hard constraints (identical to Flash — violating these fails review)

1. **VoxScript-only automation.** No new `.ps1`/`.sh`/`.py`. Automation = Rust + VoxScript (`.vox`) + workflow YAML only.
2. **Never `cargo fmt --all`** (banned, CI-enforced). Format only the crates you touched: `cargo fmt -p <crate>`.
3. **Windows process-orphan hazard.** NEVER pipe `cargo` to `head`/`grep`/`tail` — it leaks thousands of processes + tens of GB RAM. Redirect to a file: `cargo test -p <crate> > test-out.txt 2>&1`, then read the file. Recovery if it happens: `taskkill /F /IM cargo.exe` (repeat).
4. **Scratch files are git-ignored** (`*-out.txt`, `test-out.txt`, `build-out.txt`, `voxup-deps.txt`). Never `git add -A` blindly; add named files only.
5. **`docs/src/**` requires frontmatter** (`title`/`description`/`category`/`status`); architecture docs use `category: "Architecture SSOTs"` or `"architecture"`.
6. **Commit per task** with the plan's exact message. Many small commits over one large one. Keep `Cargo.lock` committed in the same series as any dependency change (CI runs `--locked`).
7. **Integration tests (`tests/`) may name only the crate's public API + dev-dependencies** — not regular deps. Put parsing/IO in lib helpers returning std types. (This is what kept Track 0's tests compiling.)
8. **No admin-merge / no skipping hooks** unless the user explicitly authorizes. Run `cargo clippy -p <touched-crate> -- -D warnings` before proposing a merge (fast pre-push skips clippy).

## 2. Branch hygiene — MANDATORY (the Track 0 lesson)

The Track 0 run drifted: the working tree switched to another session's branch mid-flight, and Flash's commits interleaved with three concurrent sessions on a shared branch. **Before doing any work:**

- [ ] `git rev-parse --abbrev-ref HEAD` — confirm you are on the intended branch. If not, STOP and tell the user; do **not** start work on the wrong branch.
- [ ] `git status --short` — if the working tree has substantial unrelated uncommitted changes from another session, STOP. Do not commit into a dirty shared tree.
- [ ] **Work in an isolated worktree** off the intended base: use the `superpowers:using-git-worktrees` skill. One track = one worktree = one branch. This is the default, not the exception.
- [ ] After each commit, re-confirm the branch hasn't moved under you (merge queues + parallel sessions move `origin/main` and can switch checkouts).
- [ ] When done, isolate the series: the track's commits must be cleanly cherry-pickable / reviewable as a unit. If they interleaved, rebase/cherry-pick onto a clean track branch before handing back.

## 3. Method — TDD, verification, autonomy bounds

- **TDD, as written in the plan.** Failing test → minimal impl → green → commit. Do not batch.
- **Verification before completion** (`superpowers:verification-before-completion`): run the actual gate command and paste real output before claiming green. Evidence before assertion. "Should pass" is not "passed." A red test is never "done."
- **Autonomy bounds — when you may resolve vs must STOP:**
  - *May resolve and note it:* a wrong file path in the plan, an off-by-one in a code snippet, a renamed symbol, a missing `use`, a stale dependency instruction — fix it, and record the deviation in your hand-back.
  - *Must STOP and ask the user:* anything that changes a **locked design decision** (the 5 in the design spec), adds a new crate/dependency not in the plan, touches a CI gate's pass/fail semantics, alters an SSOT's meaning, or would require admin-merge. Surface it; do not silently redesign.
- **Reconcile, don't duplicate.** Before building anything publish-related (Track C), audit the existing crates.io program (`project_gamify_gui_pluginization_plan_2026_06_18`: hakari-aware publish machinery, R18 publishability gate). Wire into it; do not build a second mechanism.
- **Stay in scope.** Touch only files the plan names. If you find an unrelated problem, note it for a follow-up (or `spawn_task`) — do not fold it in.

## 4. Codebase gotchas (verify, don't assume — they drift)

- Toolchain version SSOT: `contracts/toolchain/workspace-toolchain.v1.yaml` → key `versions.rust` (currently `1.96.0`). Keep `rust-toolchain.toml`, `Cargo.toml` `rust-version`, and Dockerfiles in sync if you touch it.
- Publish set SSOT: `crates/_public.toml` (`crates = [...]`) reconciled into `contracts/distribution/profiles.v1.yaml` `publish.crates`.
- `serde_yaml` in this workspace is the maintained `serde_yaml_ng` fork via package-rename (0.9 is RUSTSEC-deprecated and banned).
- `vox-gui` breaks `clippy --all-targets` (Tauri build script) — exclude it from workspace clippy sweeps; lib-only check passes.
- `agy` (Antigravity) is an optional shelled-out runtime binary in `vox-orchestrator-mcp` — never a hard build/install/publish dependency. `vox-orchestrator-mcp` is non-publishable.

## 5. Definition of done (every plan)

- All plan tasks committed on an isolated, clean track branch with the exact messages.
- The plan's verification commands run with **pasted real output**, all green; `cargo fmt -p <crate> -- --check` clean.
- New CI gate (if any) present and, where the plan says "required," flagged for the user to wire branch-protection (an agent cannot set repo settings).
- Hand-back written: what landed, real verification numbers, every deviation from the plan and why, anything deferred, and the branch/commit range. Log it to `docs/superpowers/antigravity-handoff-ledger.md` (AGH-#### entry) following the existing schema.
- Do NOT claim done if any gate is red or unrun.

## 6. Hand-back template

```yaml
plan: "<path>"
executor: "Claude Sonnet 4.6"
branch: "<isolated track branch>"
commit_range: "<base>..<tip>"
delivered: [ ... ]
verification:
  tests: "<command> → <real result>"
  clippy: "<command> → <real result>"
  fmt: "<command> → clean|diff"
deviations: [ "<plan said X; reality was Y; did Z because…>" ]
deferred: [ ... ]
needs_human: [ "make <gate> a required check", ... ]
outcome: "green|partial|failed"
```
