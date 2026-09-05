---
title: "P2/P4 execution ledger — toolchain SSOT and CI lanes"
category: "Architecture SSOTs"
status: living
date: 2026-09-05
---

# P2/P4 — Toolchain SSOT & CI Lanes: execution ledger

**Written:** 2026-09-05, from the `claude/plan-p2p4-toolchain-ci-0e5cdb` worktree.
Every number below was measured on the tree it describes, and the tree is named wherever it
matters. Where something is inferred rather than measured, it says so.

Source plans: [`2026-09-05-p2-toolchain-ssot.md`](../plans/2026-09-05-p2-toolchain-ssot.md),
[`2026-09-05-p4-ci-lanes.md`](../plans/2026-09-05-p4-ci-lanes.md),
index [`2026-09-05-00-INDEX.md`](../plans/2026-09-05-00-INDEX.md).

This file exists because the working ledger lived in `.superpowers/sdd/`, which is
`.gitignore`d and worktree-local — it was invisible to every other session and would have
died with `git worktree remove`.

---

## 1. Corrections to the plans (each measured, each would have caused damage)

| Plan claim | Reality |
|---|---|
| "55 toolchain sites (45 `@stable` + 8 `@master`)" | **53** `uses:` steps (43 `@stable` + 10 `@master`) + 3 comment lines = 56 total mentions. The plan's own components sum to 53; "55" was arithmetic, not observation. The 8th claimed `@master` site, `release-binaries.yml:240`, is `id: sbom`. |
| "`crates/voxup/src/profiles.rs:105`" is one row | **Two coupled lines**: `:105` fixture and `:120` `assert_eq!` reading it back. Rewriting one alone fails `parses_minimal_manifest`. |
| "`scripts/ci/voxcirunnerscale.task.xml` referenced by NOTHING — delete" | **FALSE.** Live infrastructure: `runner-autoscaling.md:138` names it as the versioned task definition, plus a `.cmd` companion and `install-runner-schedule.vox` installer. An implementer refused the instruction and was right. Only `voxcihealthwatchdog.task.xml` was genuinely dead. |
| ALL-CANCELLED means "slower than the commit cadence — make it faster" | **Wrong remedy for 9 of 10.** There is exactly ONE self-hosted runner and its labels are `[self-hosted, Linux, ARM64]`. 25 jobs requested `x64`, which matches nothing — they were never *scheduled*, not slow. Optimising an unschedulable job changes nothing. `ci.yml` is the one exception (its required path was always schedulable; its cancellation is benign-by-design). |
| P4 Task 3: "extend ci.yml's artifact pattern across files" | Not implementable as written — see §5. |
| Critique row: "`vox-plugin-cloud` also workspace-excluded, same rot as the shim" (assigned P1) | **Moot** — the crate does not exist on disk. |

**Meta-lesson:** the plans' *research* was strong; their *facts* needed checking one at a
time. An absolute claim ("referenced by nothing") derived from a scoped search is the
recurring failure shape.

## 2. What landed

Merge target `claude/plan-p2p4-toolchain-ci-0e5cdb` — 23 commits, `cargo test -p vox-cli-ci
--all-targets` = **370 passed, 0 failed**.

- **Release safety.** `draft: true` + `prerelease: true` on all three publish paths;
  `nightly-tag.yml` deleted (it pushed `v*`-matching tags on a daily cron, which fanned out
  to four tag-triggered workflows, one of which published a non-draft release).
- **Toolchain SSOT.** All 53 sites on `./.github/actions/setup-rust`; zero hand-written
  versions; all 10 hard-coded `1.96.0` release pins gone.
- **Caching.** Toolchain in every key; `hashFiles('Cargo.lock')` not `**/Cargo.lock` (the
  glob also matched `server/telemetry/Cargo.lock`, a separate excluded workspace); 26 cache
  blocks deduped to 10.
- **Fleet.** 25 unschedulable `x64` jobs fixed; `mutation-pr` deleted (95 min median,
  duplicated by `mutation-nightly`); two concurrency-group collisions and one cron collision
  fixed.
- **Coverage gaps from P7.** `--features gui` now compiled (it was in NO lane);
  `cargo nextest run -p vox-config --features llm-egress` added beside the default run —
  **204 vs 210 tests, six had silently stopped running** while nextest still exited 0.

Four guards keep the classes closed rather than the instances fixed:
`release-draft-guard`, `toolchain-ssot`, `toolchain-workflow-lint`, `cache-key-lint`.

Stack `claude/plan-p2p4-followup` — 3 further commits: `grep -oP` in workflows to **zero**
(retiring `ci.yml`'s GNU-only drift guard for `vox ci toolchain-ssot`, and wiring in the
version-SSOT check that previously ran only in a workflow with **0 runs ever**), plus the
first-ever CI lane for `vox-cargo-shim`.

## 3. The 1.98.1 bump — not applied, and the traps waiting for whoever does it

**Decision: adopt 1.98.1.** Both candidates satisfy the no-`.0` rule, so that is not a
discriminator; the wave is bounded and enumerated; choosing 1.96.1 defers the identical wave
rather than avoiding it.

**Complete lint wave, measured on a tree containing `c1f9d1851`:** 5 errors, all clippy-only.

```
crates/vox-db/src/learning.rs:376:25, :381:25          redundant & in format!
crates/vox-db/src/store/ops_memory/embedding.rs:91:18  chunks_exact
crates/vox-plugin-speech/src/audio.rs:83:18            chunks_exact
crates/vox-quantize/src/write.rs:96:26                 chunks_exact
```

**`cargo check` exits 0 on every one of these.** P2 Task 1 specified `cargo check` as the
gate; it would have declared 1.98.1 clean and turned main red on the first CI run. This is
AGENTS.md's #1 perennial ("toolchain-bump lint waves") reproducing exactly.

**TRAP 1 — `ssot_probe --write` does NOT rewrite the toolchain rows.** It reports drift only
(`ssot_probe.rs:74-76`). `release-prepare.yml` invokes `--write`, so it is the natural thing
to reach for and it will silently skip all 9 toolchain rows. Use
`toolchain_ssot::rewrite_all`, which exists and is tested but was never wired to the probe's
write path.

**TRAP 2 — the bump must land atomically.** `crates/voxup/tests/distribution_parity.rs`
asserts the toolchain contract equals `rust_version` in
`contracts/distribution/profiles.v1.yaml`, and `crates/voxup/src/profiles.rs` restates it a
third time across **two coupled lines**. Bump alone ⇒ main red.

**TRAP 3 — re-measure the wave after rebasing.** It was 9 before `c1f9d1851` and 5 after. I
predicted the reduction, was wrong at the time because I measured a tree that did not contain
the fix, and only `git merge-base --is-ancestor <sha> HEAD` settled it.

**Also unfinished:** the 1.96.0 rustdoc baseline. `RUSTDOCFLAGS='-D warnings' cargo doc
--workspace --no-deps --exclude vox-gui` (byte-identical to `ci.yml:703`) reports 35 errors on
1.98.1. All are intra-doc-link lints predating 1.98 and one is in main's just-merged pareto
code, so they are near-certainly pre-existing — **but that is inference.** Run the same
command on 1.96.0 and diff the error sets.

## 4. Incidents worth not repeating

**Concurrent git operations under a running build — three times, by two different sessions.**
(1) The integrator rebased this worktree mid-build, producing phantom "missing
`pareto_frontier`" rustdoc errors that had to be withdrawn as evidence. (2) I checked out a
branch while an agent was still committing, and a commit landed on a branch I had declared
frozen and signalled for merge. (3) I switched branches under `cargo test`, invalidating the
run. Each time the diagnosis was correct *after* the fact.

The mechanical rule: **never `git checkout` in a worktree with a build or agent running.** If
two branch contexts are needed, that is what `git worktree add` is for. A clean `git status`
answers "nothing uncommitted", not "nothing running" — only the second question matters here.

**A rebase can compose two independently-correct diffs into an invalid file — twice.** Both
times a duplicate YAML key: last-wins for GitHub, but a hard error for `serde_yaml`, which is
what the repo's guards parse with. A duplicate key therefore *blinds the guard* rather than
failing loudly, and it happened in `release-binaries.yml`, the most safety-critical file.

**A stand-in check must be at least as strict as the thing it stands in for.** I validated
workflows with PyYAML's `safe_load` and reported "all parse cleanly" for most of the session.
**PyYAML silently accepts duplicate keys; `serde_yaml` rejects them.** My checker was weaker
than the guards it substituted for, and had been passing a file with a duplicated `draft:`.
Replaced with a duplicate-key-detecting loader. Verify strictness explicitly; do not assume it.

**A near-miss on the forbidden outcome.** `draft: true` was found missing from
`release-binaries.yml` — uncommitted debris from a "prove it fails red" step that was never
restored. `prerelease: true` and the comment were still present, so the step read as correct
at a glance. Committed and merged, the next `v*` tag would have published a public release.
A red/green proof on a safety-critical file is itself a hazard; grep for the exact key
afterwards, not for the step.

## 5. Deliberately not done, with reasons

**P4 Task 3 (dedup the 8 identical `vox-cli` builds) — ruled against.** The 8 sites span
**7 workflows**; `upload-artifact`/`download-artifact` share within a *run*, so ci.yml's
pattern does not transfer — it would require a `workflow_call` reusable workflow the plan
never costed. All 7 now get `target/` caching via `setup-rust` keyed on toolchain + lockfile,
which captures most of the claimed ~2.3 runner-hours. And their triggers do not co-occur
(`mobile-e2e-android` is `pull_request`-only and 30/30 skipped; `mobile-e2e-ios` is
`push`/`schedule` and 30/30 failed at "Setup Node"), so a shared producer artifact adds a new
failure mode to the two least healthy lanes.

## 6. Filed back — not this work stream's to fix

- **`vox-langtool` is absent from `RELEASE_PACKAGES`** (`release_build.rs:31`,
  `&["vox-cli", "vox-ml-cli"]`) while being the `minimal` tier's **only** binary
  (`profiles.v1.yaml`) — so the language-user tier ships no artifact at all. That file
  asserts membership before packaging, so this is a Rust change in `vox-cli`.
- **Root `Cargo.toml`'s `exclude` has two stale entries** pointing at paths that do not
  exist: `crates/_corpus_verify_tmp` and `crates/vox-plugin-cloud`. `ci.yml:456`'s
  `is_excluded_crate()` filter also names `vox-plugin-cloud`; both halves should move
  together. INDEX §3 requires an explicit hand-off for that file's `exclude` section.
- **`vox-cli` does not build** with its declared features — `vox-orchestrator-mcp` has ~15
  pre-existing compile errors (`record_live_chat_turn`, `TaskEnqueueHints.chat_session_id`,
  `spawn_dynamic_agent_with_parent` arity). `cargo clippy --workspace` misses it because that
  unifies *default* features; this is AGENTS.md's under-declared-feature trap observed live.
  Consequence: no guard in this work stream could be verified by running the real `vox`
  binary; implementers called the wired library functions instead, and said so.

## 7. Measured reference data

- **Runners:** exactly one, `[self-hosted, Linux, ARM64]`. Any `x64` request is unschedulable.
- **Branch protection:** a single required context, `Check, Build, and Test (Rust)`, defined
  in both `ci.yml` and `ci-fallback-hosted.yml`; `strict: false`, `enforce_admins: false`,
  no required reviews.
- **Excluded crates:** `ci.yml:456` names nine; **exactly one is a Rust crate**
  (`vox-cargo-shim`). Seven are skill directories with no `Cargo.toml`; one does not exist.
- **Workflow health** (46 workflows, last 30 runs, cancelled split from failed):
  18 healthy / 10 all-cancelled / 10 broken / 3 flaky / 3 never-run / 2 all-skipped.
  Raw table and per-workflow failing steps were captured during execution.

## 8. Workspace test sweep — the 20 failures, measured and attributed

The plan asked for this to be recorded in-repo because it "existed only in a conversation".
Command (nextest, so the counts are comparable to CI's):

    cargo nextest run --workspace --exclude vox-gui --no-fail-fast

Run on the settled branch tip:

    11582 tests run: 11567 passed, 15 failed, 143 skipped

**All 15 are pre-existing.** They cluster into related families, which is what genuine
breakage looks like — not scattered flakiness:

| count | area | tests |
|---|---|---|
| 4 | `vox-compiler::emission_ladder_test` | ladder_{auth_patterns,crud_api,db_native_ir}_compiles_as_rust_script, ladder_contract_drives_each_fixture_target |
| 4 | `vox-codegen::emit_compile_harness` | golden_option_type_compiles, ladder_{auth_patterns,crud_api,db_native_ir}_golden_compiles |
| 3 | `vox-cli` integration | ci_workflow_contract linux_ci_runs_workspace_tests…, command_catalog_paths_match_baseline, db_migrate_semantics migrate_dry_run_defaults_to_local_codex… |
| 2 | `vox-arch-check::integration` | arch_check_description_rule_fixture, arch_check_smoke_fixture |
| 2 | `vox-orchestrator::models::select` | select_with_empty_policy_falls_through_to_cascade, select_with_premium_alias_honors_alias_when_intelligence_high |

The two `select` failures were the only ones this branch could plausibly have caused — its
clippy pass rewrote `and_then(|o| if p(o) { Some(o) } else { None })` to `filter(p)` in that
file. Ruled out by differential test: main's own `select.rs`, dropped into this tree, fails
the identical two (31 passed / 2 failed). The rewrite is also semantically equivalent for
`Option`. Pre-existing.

### Two regressions this branch DID cause, found only by the sweep

Both were in `vox-cli`. Every scoped check run during this work used `-p vox-cli-ci`, so
every green result reported was true and none of them reached the crate that broke.

1. `release_workflows_pin_the_toolchain` — asserts that a release workflow mentioning
   `dtolnay/rust-toolchain` must also pin the version. release-gui.yml stopped USING the
   action but still NAMED it in a comment I had deliberately kept as "still accurate". A
   substring check cannot separate prose from configuration. The adjacent comment also read
   "channel 1.96.0", stale the moment this branch's own bump landed.
2. `merge_group_fanout_guard::ci_yml_merge_group_required_lane_fits_runner_ceiling` — buckets
   merge_group jobs by label set and looked up `linux,self-hosted,x64`. Removing the
   unschedulable `x64` label emptied that bucket; the guard panicked with its own
   anticipatory message, "label set renamed?". It was right.

Both fixed in c06ce0aa8. A first attempt at (2) used a blanket string replace and also
rewrote a unit test's synthetic fixture, where the old label set was correct — a textual
replace cannot distinguish a fact about the repo from a fact about a fixture.

### The pattern behind all three
Each defect came from applying a locally-correct edit uniformly: keeping a comment that read
as accurate, removing a label everywhere, replacing a string everywhere. Each was right where
I was looking and wrong somewhere I was not. It is the same shape as the plan errors catalogued
in §1 — an absolute claim derived from a scoped look — which suggests it is a property of this
kind of work rather than of any one agent.
