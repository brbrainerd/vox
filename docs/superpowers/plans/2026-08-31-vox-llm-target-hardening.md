# Vox LLM-Target Hardening — Measure, Restore, Arm

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Revision 2.** Revision 1 was audited along seven tracks against the source tree and did not
> survive: six of its tasks rebuilt shipped code, two rested on false premises, three would have
> caused incidents, and its ordering was derived from a misread exit code. Every task below is
> re-scoped against `file:line` evidence. **Read the spec's §10 before starting** — it records
> what Revision 1 claimed and why each claim failed, and several tasks here exist only to undo
> that reasoning.

**Goal:** Restore the measurement that was silently amputated, make the gates that already exist
capable of failing, and pay for both on a fast tier that is currently **175–200s against a 135s
fail wall**.

**Architecture:** Almost nothing new is built. Phase 0 commits measurements from meters that
already run. Phase 1 restores deleted harnesses and repairs gates whose failure paths are
unreachable. Phase 2 buys back CI time. Phase 3 extends `canonical-map.v1.yaml` rather than
inventing a registry. Phase 4 touches the compiler, behind measurement.

**Spec:** [`2026-08-31-vox-llm-target-hardening-design.md`](../specs/2026-08-31-vox-llm-target-hardening-design.md)

**Sibling plan — coordinate:**
[`2026-08-31-build-time-measurement-activation.md`](2026-08-31-build-time-measurement-activation.md)
owns `build-crate-summary` and `build-bench-baseline`. Task 0.2 is theirs; check before doing it.

**Standing rule for this plan:** every task states what it measures before it changes anything.
A task that cannot state its blast radius does not start.

---

## Phase 0 — Measure (days; near-zero risk; unblocks everything)

### Task 0.1 — Persist the CI cost meter that already runs

- [ ] `vox ci job-timings` (`job_timings.rs:1-22`) already runs after every CI run via
      `ci-timings.yml:36 --annotate`, and its output is discarded as ephemeral check annotations.
      Add a commit step writing `contracts/reports/perf/ci-job-timings.v1.json` on main runs,
      mirroring the pattern at `ci.yml:1753-1763`.
- [ ] Persist the per-generator `ssot-drift` timings that have existed since `17c976664`
      (2026-06-26, `docs.rs:560-568`) — they print to stderr and are thrown away. Emission is
      done; only capture is missing.
- [ ] Run `vox ci pre-push --report-json` on a clean tree and commit the artifact. The schema
      (`contracts/reports/pre-push-report.v1.schema.json`) is committed; **no instance is.**
- [ ] Regenerate `contracts/budgets/test-tier-budgets.v1.yaml` from that artifact. The current
      `measured_ms: 90000` is a hand-typed comment; measured reality is 175–200s.

**Verification:** three artifacts committed; the budgets file cites them instead of a comment.

**Do this first.** Every cost decision in the spec is unfalsifiable without it, and it is the
cheapest task in the plan.

### Task 0.2 — Populate `build-bench-baseline` (check the sibling plan first)

- [ ] `contracts/ci/build-bench-baseline.v1.json` is 6 records, all `wall_ms: 0`, untouched since
      `2ff335fce`. `build_bench.rs:181 baseline_is_unpopulated` already detects this and bails at
      line 290, and `ci.yml:1013` runs it blocking — **so this should be failing on main now.**
      Confirm that, then populate.
- [ ] Do **not** add `continue-on-error` to make it pass. `ci.yml:1013` carries the comment
      *"Blocking on purpose. `continue-on-error: true` here is what let the baseline sit at
      all-zeros for months."*

### Task 0.3 — Fix the F1 bench so precision measurement is real

- [ ] `bench.rs:95-103` scores a zero-fixture rule at precision 1.0 / recall 1.0 / **F1 1.0**,
      which passes `--min-f1 0.70` (`ci.yml:1320`). Make zero fixtures a **hard error**.
- [ ] `bench.rs:51-56` splits the rule id at the first `/`, looking for
      `fixtures/security/hardcoded-secret-aws-key_pos` while the file on disk is
      `fixtures/security-hardcoded-secret/aws_key_pos.txt`. Either read the per-rule `fixtures:`
      declarations in `rules.v1.yaml` (currently never read) or match the on-disk convention.
- [ ] Report the real F1 for all 45 rules. **33 currently score vacuously**, including every
      `stub/*`, `scaling/*`, `ai-laziness/*`, and `security/hardcoded-secret/*`.

**Verification:** rule count with real fixtures rises from 12 to 45, or the gate fails naming the
rules that still have none.

---

## Phase 1 — Restore and arm

### Task 1.1 — Restore the five CR-L harnesses (root-cause a silent merge revert)

The measurement spine was amputated, not degraded.

- [ ] Restore from `40a798545`:
      `crates/vox-audit/src/subcommands/{repair_corpus,plan_fidelity,spec_to_app_panel,mens_on_distribution,deploy}.rs`
      (3,858 lines across six files). They were present in **both** parents of merge
      `3c7b3b917` (2026-05-27, "harmonization pass 3") and absent in the result.
- [ ] Re-register them at `lib.rs:265-273`, replacing `RepairCorpusStub`, `PlanFidelityStub`,
      `SpecToAppStub`, `MensOnDistributionStub`, `DeployStub`.
- [ ] `panel.rs` (1,028 lines, the OpenRouter client) is currently unreferenced — confirm it is
      reachable again.
- [ ] **Delete the test that locks in the regression.**
      `stubs.rs` `every_stub_returns_infrastructure_error_with_incomplete_report` asserts the
      amputated state.
- [ ] Add a **stub-count guard** in `ga.rs`: a snapshot in which any product gate is a stub must
      not be published as a measurement. Today a fully-stubbed umbrella emits JSON
      indistinguishable from a real red.
- [ ] Add a regression test: no gate whose corpus manifest is `minimum-viable` may resolve to a
      stub.

**Verification:** `vox audit --gate all` returns exit 0 or 1 for these five, never 2.

**Note on exit codes** (`report.rs:44-54`): 0 = met, 1 = `BarMissed`, 2 = `InfrastructureError`
(*not measured*, non-blocking by contract), 3 = `InvalidInput`. `-1` from `ga.rs:128-151` means
the binary was not found — a `required-features` packaging issue fixed by `1153557e8`. Do not
re-diagnose it.

### Task 1.2 — Re-measure the CR-L gates (maintainer task; costs money)

- [ ] Build the product bins first: `cargo build -p vox-audit --bins --features ci-gates`. Plain
      `cargo build` does not produce them.
- [ ] Run the panel gates with `VOX_OPENROUTER_API_KEY`. The 2026-05-23 run cost **$2.56** against
      a $25 cap. **This is a secrets + spend decision, not an engineering step** — it can never be
      a CI default or a pre-push gate.
- [ ] Commit the artifacts. Add `incomplete: false` to the acceptance criterion, or the step is
      satisfiable by committing stubs.
- [ ] `mens-on-distribution` additionally needs a `mens-current` panel member; its blocker is a
      CUDA `CUDA_ERROR_INVALID_IMAGE` recorded in `evidence-ledger.v1.json → blocked_claims`.
      Leave it blocked and say so.

**Expectation to test, not assume:** the last real run had repair-corpus **0.775** (bar 0.70),
plan-fidelity **0.910** (bar 0.85), spec-to-app **0.683** (bar 0.60), deploy **1.0** — all met.
If those reproduce, **there is no repair deficit** and Phase 4's ranking must be re-justified.

### Task 1.3 — Make evidence staleness actually blocking (two edits, not one)

- [ ] Change `FindingKind::Stale` severity from `"WARN"` to `"ERROR"` at `evidence_ledger.rs:70`.
      Without this, `--strict-evidence` fails **zero** claims, because `main.rs:434-438` filters on
      `== "ERROR"`.
- [ ] Pass `--strict-evidence` in `cr-l-gates.yml` (currently bare `cargo run -p vox-arch-check`,
      so warn-only via `main.rs:1637`).
- [ ] Do both only **after** Task 1.2 refreshes artifacts. 14 claims are currently stale.
- [ ] Move `code.cr_l3_cr_l4_runners` to `blocked_claims`. Its note reads *"Runners are real.
      Measurements are not."* — as of `3c7b3b917` the first half is false.
- [ ] `cr-l-gates.yml` holds `permissions: contents: read` and only uploads workflow artifacts, so
      it **cannot** commit even if a step were added. Widening it is a deliberate decision; if
      taken, scope the commit step to `contracts/reports/` evidence artifacts only.

**Verification:** before refreshing, `--strict-evidence` fails with 14 findings; after, zero.
The current command already passes, so it proves nothing.

**Hazard — do not copy this pattern anywhere else:**
`cr-l8-corpus-feedback.yml:401-405` `cp`s the current pass-set over `scripts-pass-baseline.txt`
with no monotonicity check. It is inert only because no commit step follows. Adding one would
launder regressions permanently.

**Note:** `evidence_ledger.rs` parses the **filename** date (`parse_artifact_date`, line 170 —
Revision 1 said 182). That is correct and non-obvious; git does not preserve mtimes.

### Task 1.4 — Arm the TOESTUB gate (measure, then flip, in one commit)

- [ ] **Write the `should_fail_build` unit test first.** `grep should_fail_build` returns three
      call sites and zero tests; that absence is why the bug survived.
- [ ] Fix the scan root: `ci.yml:751-752` runs the `FULL=true` branch with no roots, and
      `matrix.rs:556-558` defaults that to `crates/vox-repository` — **22 of 3,889 files.** Pass
      `crates` explicitly. Without this the severity change is cosmetic on full runs.
- [ ] Measure the blast radius before flipping:
      `cargo run -q -p vox-code-audit --bin toestub -- --mode audit --format json --min-severity error crates`.
      Best committed proxy is 549 (`scaling-audit/findings-latest.json`); a fresh proxy is 327
      (Rust files >500 non-blank lines excluding `@generated`). **98 % is `arch/god_object`.**
- [ ] Flip `EnforceWarn` from `>= Critical` to `>= Error` (`engine.rs:450-453`) **and handle
      `god_object` in the same commit** — demote to Warning or add a counted baseline. Fix the
      remaining ~10 Errors outright. Revision 1's stop condition (">~200 ⇒ remediation project")
      is already tripped; the flip alone is not viable.
- [ ] **Arm at the scope the measurement covered first** — `full` / `merge_group` only — for one
      cycle, then extend to the scoped PR path. `enforce-warn` is also the pre-push mode, so a
      blind flip breaks every developer's local gate simultaneously, on a repo whose AGENTS.md
      tells agents to treat pre-push green as the verdict.
- [ ] Correct the stale comment at `ci.yml:741`: "all 23 detectors, Warnings+ block" against a
      registry of 54 and a floor of Error.

**Scope note:** only the scoped TOESTUB CI step is neutered. `Legacy` mode fails on `>= Error`
and the pre-commit `tdd-guard` runs `enforce-strict`, so the same detectors do block elsewhere.

### Task 1.5 — Prove gates can fail, cheapest mechanism first

- [ ] **Review rule:** every new gate ships a negative unit test. Already the pattern in 9 of 11
      sampled gates — `crate_edges.rs:420 new_edge_fails`, `:486 upward_layer_edge_fails`,
      `:514 missing_layer_fails` are the canaries Revision 1 proposed to invent, already written
      against temp workspaces.
- [ ] Extend `cargo mutants` scope to `crates/vox-code-audit/src/engine.rs`. It already runs on
      PRs touching `vox-compiler`/`vox-codegen`; **one config line would have killed Task 1.4's
      bug outright.**
- [ ] Ship exactly **one** cross-process canary — `toestub.enforce-warn` — because that failure
      path crosses into shell/YAML exit-code wiring where no unit test observes it.
- [ ] Require **one canary per severity tier** the gate claims to enforce. Revision 1's proposed
      content would have caught the real bug only by accident, since the hardcoded secret happens
      to be the single Critical rule.
- [ ] Put canary running in `complete`, **not** `fast`. Realistic cost is +30–90s, not the ~2s
      Revision 1 estimated: these gates are whole-workspace walkers whose cost is the walk, and
      most take no scoped root.
- [ ] **Do not build** canaries for `config-hygiene` (no path argument), `crate-edges` or
      `arch-check` (both shell `cargo metadata`, seconds), or `where-things-live`
      (`main.rs:1488 wtl_parity_warns` is warn-only and cannot fail).

**Verification:** temporarily revert Task 1.4; the canary must fail. Restore. Do not skip this —
a canary that has never been observed failing is itself detection theater.

---

## Phase 2 — Buy back CI time

### Task 2.1 — The cheap wins (no new machinery)

- [ ] **Demote `command_compliance` (33.5s) and `completion_quality` (24.8s) from `fast` to
      `complete`.** 58s, 46 % of `ssot-drift`, neither a fast-feedback concern. **Largest single
      reduction in the plan.**
- [ ] Exclude `contracts/reports/**` and `*.generated.json` from the full-gate sledgehammer
      (`affected.rs:22-27`). Two lines; kills roughly a third of the trigger set. Note the
      sledgehammer fires on only **11–12 %** of the last 200 commits — behind `Cargo.lock` and
      `.github/workflows/**` — so this does not need a registry and Revision 1's dependency on one
      is dropped.
- [ ] Delete dead ratchets: `contracts/hir/legacy-baseline.toml` (zero consumers,
      `last_reviewed: 2026-03-25`).

### Task 2.2 — Gate caching (generalize what works; opt-in only)

- [ ] Generalize the **two existing** implementations rather than inventing `.vox/cache/gates/`:
      `visus_review/mod.rs:92-110` keys on `screenshot_sha256 ‖ model ‖ prompt_version`, persists
      to `contracts/reports/gui-visual-review/cache.v1.json`, commits back to main
      (`ci.yml:1753-1763`) and prunes dead keys; `vox-arch-check/src/cache.rs` keys on SHA-256 of
      `Cargo.lock` + `layers.toml` + `where-things-live.md`.
- [ ] Use the **existing** `gate_version` identity — `vox ci` already refuses to run when the
      binary's build commit ≠ working-tree commit. A hand-maintained per-gate version is exactly
      the wiring step this repo skips.
- [ ] Make `cacheable` **opt-in with a mandatory declared input set.**
- [ ] Add a **clock-perturbation test**: run each gate twice with `SOURCE_DATE_EPOCH` advanced 400
      days. Differing verdicts ⇒ uncacheable. Without this, caching memoizes a green verdict
      across the moment a deadline passes — `check-links`, `retirement-audit`,
      `ignored-test-age`, `evidence_ledger`, arch-check Rule 6 are all clock-dependent, and
      Task 3.3's expiry work creates more.
- [ ] Add a **cache-bypass canary run**: every canary runs twice, cache off and cache on. A canary
      that passes under the cache means the cache is unsound. A cached PASS is a disabled gate.
- [ ] Start with `affected_cmd::check_graph` — 15.2s → ~0, inputs are exactly `is_sentinel`'s set.
      Then `command_compliance` + `check_docs_ssot` (52s combined) **if** their input sets are
      enumerable.
- [ ] **Never cache:** the 17 existential scanners keyed on files-read (a new violating file is
      invisible), anything reading `VOX_CI_RETIRED_SYMBOL_SCAN_CRATES` (it narrows the scan set),
      and `gui-smoke`/`backend-tests` (their env switches gate whether the check runs at all — a
      cached PASS from a skipped run is fabricated).
- [ ] Measure the real hit rate on the last 20 merged PRs **before** citing a number. The 60–80 %
      literature figure does not transfer; this gate mix is unusually cache-hostile.

### Task 2.3 — Arm and unify the budget gate (do not add a third)

- [ ] Two implementations already read `test-tier-budgets.v1.yaml` with duplicated parsing:
      `tier_budget_check.rs:68-125` (JUnit, CI) and `pre_push.rs:415-445` (wall clock, local).
      The split-brain is documented in-source at `tier_budget_check.rs:57`. **Extract one shared
      reader.** Revision 1's a-priori ledger-sum would have been a third opinion contradicting two
      measured ones.
- [ ] Flip `enforce_budgets` default-on (`pre_push.rs:264`) — today the budget gate never runs.
- [ ] Remove `continue-on-error: true` from `ci.yml:1123-1126` ("until tightened budgets land"),
      or the rule lands advisory by inheritance.
- [ ] **Re-baseline first (Task 0.1).** Arming against `fail_ms: 135000` while reality is
      175–200s bricks every push.
- [ ] Have any ledger `cost_ms_p50` be *written from* these measurements, so the sum and the
      measured total are the same numbers by construction.

**Zero-sum rule:** under current numbers, Task 2.1 must land before any new gate merges.

### Task 2.4 — Gate actuation ledger (gap analysis first)

- [ ] **Before designing anything**, diff against `contracts/policy/policy-registry.v1.yaml` —
      2,261 generated lines already carrying `severity`, `blocking`, `runs_on: [pre-push, ci]`,
      and a `source: {kind, ref, detail}` producer pointer per policy. It is plausibly 70 % of the
      ledger.
- [ ] Add only the genuinely missing columns there: `cost_ms_p50`, `canary`, `cacheable`,
      `last_effective_failure`.
- [ ] Separately: `contracts_index.rs:38-43` never deserializes `enforced_by`, and 92 of 189
      entries (49 %) name `vox ci contracts-index` — a file-exists check — as their sole enforcer.
      Same pathology as `expires_after`. Either enforce the annotation or delete the field.

---

## Phase 3 — SSOT (extend, don't invent)

### Task 3.1 — Extend `canonical-map.v1.yaml`

- [ ] **Do not create `contracts/ssot/facts.v1.yaml` and do not add `vox ci facts-parity`.**
      `canonical-map.v1.yaml` + `canonical_docs.rs:15-37` already provide `spec_paths`,
      `canon_doc`, `generated_docs`, `aliases`, and `owning_crate_globs`, gated by
      `vox ci check-docs-ssot` already inside `ssot-drift`.
- [ ] Fix the seed row at line 144: the producer is inverted (AGENTS.md is listed as an *alias
      of* `agent-instruction-architecture.md`), and `.cursor/rules/` is absent entirely. Set
      `canon_doc: AGENTS.md`, add the Cursor rule as a `generated_docs` consumer.
- [ ] Promote `aliases` from `array<string>` to `array<string | {path, owner, reason,
      expires_after}>`, string form retained for compat.
- [ ] Extend `verify_alias_rules` (`canonical_docs.rs:133`) to enforce the object form.

### Task 3.2 — Retired surfaces: one generator, two targets

- [ ] `vox ci sync-retired-surfaces` from AGENTS.md's 17 rows to **both**
      `.cursor/rules/retired-surfaces.mdc` (currently 7 rows, **10 missing**) and
      `contracts/documentation/retired-symbols.v1.yaml` (15 entries, a *different* set). Targeting
      one hard-codes a two-thirds fix.
- [ ] Auto-stage in `lefthook.yml`.
- [ ] Do **not** model it on `sync-ignore-files` — that is a verbatim whole-file copy with a
      header swap (`sync_ignore_files.rs:23-45`). No mechanism transfers.

**Verification:** all 10 missing rows appear; `retired-symbol-check` gains patterns for
`vox-dashboard`, `vox-oratio`, `vox-bootstrap`, `vox-dei-shim`, `vox-sherpa-transcribe`.

### Task 3.3 — Fix the four live rule-file contradictions by hand

Generators cannot fix semantic inversions. These are in `alwaysApply: true` files.

- [ ] `documentation-policy.mdc` tells agents to use `{{#include}}`; AGENTS.md §Markdown Hygiene
      forbids it.
- [ ] `documentation-policy.mdc` requires `last_updated` frontmatter; AGENTS.md forbids hand-adding
      it.
- [ ] `voxscript-first-automation.mdc` cites `docs/src/architecture/vox-as-glue-research-2026.md`,
      which moved to `docs/src/archive/research-2026-q1/`.
- [ ] Same file says `vox-runtime` where AGENTS.md says `vox-actor-runtime`.
- [ ] Then add a **contradiction lint** for the eight non-derivable rule files: every referenced
      path resolves, every named crate exists. (Only 2 of 10 files are derivable from AGENTS.md;
      5 are fully independent — `build-environment.mdc` is CUDA paths and linker lore.
      `GEMINI.md` already links to AGENTS.md as normative rather than copying, which is the
      correct pattern.)
- [ ] Register the eight independent files in `canonical-map.v1.yaml` against their **real**
      producers (`runner-contract.md`, `data-storage-ssot-2026.md`, `cli-design-rules-ssot.md`),
      not AGENTS.md.

### Task 3.4 — `where-things-live` link lint (not a generator)

**Revision 1's "28 phantom crates" is retracted: 0 are stale.** 19 sit under
`## Planned but not yet landed`, 3 are inline `_(planned)_`, 6 are deliberate `(was X)` notes, and
every markdown *link* resolves. A generator would delete the six historical notes, which are
exactly what stops an agent re-inventing a retired crate.

- [ ] Rule 1: every `[`vox-*`](../../../crates/X/)` link resolves. *(passes today — free canary)*
- [ ] Rule 2: every bare-backticked `vox-*` is in the planned section, inline-tagged, or in a
      `(was …)` clause. *(passes today)*
- [ ] Rule 3: no crate under `## Planned` has a directory. **Fails today on `vox-cli-ci`**, which
      ships 80+ modules.
- [ ] Rule 4: no AGENTS.md retired name appears as a `Planned crate`. **Fails today on
      `vox-dashboard`** (deleted 2026-05-12 per ADR-037).
- [ ] Rule 5: every directory in `crates/` appears somewhere in the file — the real coverage gap,
      currently unmeasured.

### Task 3.5 — Config default parity (registry-vs-registry)

**Revision 1's read-site scan finds nothing:** of 321 non-test env read sites, **0** carry a
same-line `unwrap_or`; the idiom hoists defaults into named resolvers. `VOX_SEARCH_BM25_K1` —
Revision 1's own example — has no `env::var` site at all.

- [ ] Extend `config_registry_parity.rs:9-18` from name-set parity to **(name, default) parity**
      across `CONFIG_KEYS` (124), `OperatorEnvSpec` (119), and `registry.v1.yaml` (99). It
      currently *unions* the three, so nothing can ever flag. ~30 lines against code that already
      loads all three.
- [ ] Land the 2 real divergences at `Error` immediately — `VOX_CIRCUIT_BREAKER_CONTRACT` and
      `VOX_GAMIFY_ECONOMY_PATH` (string vs `null`). Sample of 2, precision 100 %.
- [ ] Land the coverage gap at `Info` with a ratchet: only **3** keys are shared; 96 live only in
      the YAML, 121 only in Rust.
- [ ] Enforce the invariant already written at `config_key.rs:21` — *"MUST equal the in-code
      constant"* — documented and never checked.

**Out of scope:** a Rust `let`-rebinding detector. Clippy's `shadow_unrelated` covers it.

### Task 3.6 — Suppression expiry (after Task 3.7 measures it)

- [ ] Implement the **first** expiry enforcement in this repo. There is no reference to copy:
      `check_links.rs:79-87` prints `WARN … (still skipping)` and returns `true`.
- [ ] Fix `check_links` in the same change, or ship a second decorative field.
- [ ] Make `expires_after` `required` with a `pattern` in `suppression.v1.schema.json`.
- [ ] Extend `validate_toestub_suppression_contracts` (`suppression.rs:99`) to cover both
      baselines — neither is schema-validated today.
- [ ] Load `suppressions.v1.json` in the runners meant to honour it:
      `ToestubConfig::suppression_path` defaults to `None` (`engine.rs:82`) and `lefthook.yml:48`
      passes no `--suppressions`.
- [ ] Expire **per file, densest-first** — not by churn. Measured churn is **0 % cold**, median
      3–5 commits, so churn has no discriminating power. Per-file has a real tail
      (`vox-compiler/src/eval/builtins.rs` holds 45 entries) and matches the review unit.
- [ ] Pin the ordering snapshot as a committed input with its own SHA, or a time-dependent
      computation makes `ssot-drift` report perpetual drift on a file nobody edited.
- [ ] Escalate on **the first push that touches the suppressed file**, not on wall clock. A
      date-triggered red at 00:00 UTC lands on an unrelated push with no owner.
- [ ] Drop "advisory → blocking": `run_silent_drop_gate` / `run_weak_test_gate` are count-based
      with no severity rung (`core_gates.rs:83-89, 118-124`).

### Task 3.7 — Wire `semcov-gates` and de-pin line numbers (prerequisite to 3.6)

- [ ] `bin/semcov-gates.rs:28-29` is the sole production reader of both toestub baselines and is
      **never invoked** — absent from every workflow, from `lefthook.yml`, and from
      `core_gates.rs:177-190`'s trio. `ci.yml:1243` asserts it "IS blocking"; correct that comment.
- [ ] Declare it as a `[[bin]]` and run it **non-blocking for one week**; publish the real
      beyond-baseline count. The 1,642 entries suppress a gate that has never run, so every number
      in §3.3 is currently unmeasured.
- [ ] **De-pin `line` before arming.** `suppression.rs:170-176` matches `path_glob` **and exact
      line**. All 1,642 are line-pinned and 100 % sit in code edited within six months (hottest
      file: 90 commits). Line numbers have drifted; drifted entries no longer match; findings
      resurface as *new*. Arming as-is fails on an unknown large fraction of 1,642,
      indistinguishable from real regressions. Replace with a per-file monotone remaining-count
      budget.

---

## Phase 4 — Language and compiler (each task starts with a measurement that can kill it)

### Task 4.1 — DAF: build the injector the corpus does not provide

- [ ] **Revision 1's premise was wrong** — `repair-corpus/problems/*` are hand-authored
      `buggy.vox`/`fixed.vox` pairs with pre-existing, sometimes multi-line bugs across five
      classes including **`logic`**, which compiles clean and would score DAF 0, flattering the
      average and tripping the stop condition on corpus composition.
- [ ] Build a fault injector over known-good fixtures (`examples/golden/**`), restricted to
      syntactic and type faults.
- [ ] Wire the free second data source: `projects/*/expected.json` already carries
      `expected_diagnostic_count_before` and **nothing reads it**.
- [ ] **Split parse from typecheck.** `descent/mod.rs:571 recover_to_top_level()` already
      resynchronizes at declaration boundaries with brace-depth tracking, so parse-side DAF is
      likely ~1 and "improve parser recovery" is probably the wrong fix. `run_frontend_str`
      accumulates `Vec<Diagnostic>` with no dedup and no `caused_by` suppression — that is where a
      cascade lives.
- [ ] **Tier: nightly.** Injecting and compiling per fixture is minutes.
- [ ] Report the number before writing any ratchet. If DAF ≤ 1.3 for both stages, stop.

### Task 4.2 — Wire one `vox-constrained-gen` call site

- [ ] The crate has **zero call sites.** Three crates declare the dependency; nothing references
      `GrammarMode`, `build_sampler`, or `mask_logits`. The smoke gate is a `println!`
      (`mens.rs:33-35`).
- [ ] Wire `GrammarMode::Vox` into one real path and make `vox ci constrained-gen-smoke` run the
      sampler.
- [ ] **Do not add `GrammarMode::TwoPhase` yet.** The A/B has no baseline until `Vox` reaches a
      model, and `RevisionSampler` cannot serve as the phase boundary — `revision.rs:60-105` never
      checkpoints, never rewinds, and `max_depth` is unread (`revision.rs:39`).
- [ ] Either implement the checkpoint stack or delete `RevisionConfig`.

### Task 4.3 — Diagnostic payload delta (~80 % already ships)

- [ ] Already in `typeck/diagnostics.rs`: `VoxCompilerDiagnosticPayload`, `SuggestedFix`,
      `DiagnosticFix`, `SpanPayload`, `MinimalRepro`, ~72 codes with a compile-time sync test,
      `contracts/diagnostics/registry.v1.yaml`, and `--json` / `--for-llm` at `check.rs:84-92`.
      **Do not "add `vox check --json`".**
- [ ] Add only: `one_fix_site`, `confidence`, `caused_by`, schema version pin.
- [ ] Keep span+replacement — it is strictly better than a unified diff for machine application.
- [ ] **Re-cost separately:** "one repair fixture per diagnostic code" is 72+ fixtures against a
      corpus of **15** (`manifest.v1.yaml: count_current: 15, count_target: 50`). Larger than
      every other Phase 4 task combined; it is its own plan.
- [ ] Also fix `corpus_hash: "blake3:0000…0000"` in that manifest — the `wall_ms: 0` pattern in an
      integrity field.

### Task 4.4 — DELETED: token-economy ratchet

`vox ci source-token-budget` exists (`syntax_k.rs:146`), fails on regression, and runs via
`pipeline_parity.rs:47` at **tolerance 0.0** in `ci.yml:880`. Revision 1's ">10 % regression"
would have loosened a shipped gate tenfold.

- [ ] Optional follow-up only: add a `bpe_tokens` field. `syntax_k.rs:141-142` states these are
      *"structural lexer tokens … NOT model BPE tokens"*, so the context-window argument was
      applied to the wrong metric. **Do not touch the tolerance.**

### Task 4.5 — `Profile` axis on `feature_matrix` (not a new subsystem)

- [ ] Fix the compiler's own deprecation text first: `head.rs:46` says `"use @tool instead"` while
      AGENTS.md says the canonical form is bare `tool`. One line, and it is a live split-brain
      between a diagnostic and the policy SSOT.
- [ ] Add `Profile { Compat, Standard, Strict }` as a third axis on `feature_matrix.rs`'s existing
      exhaustive table — no `_` arm, so a new `Feature` fails the build until its profile cell is
      declared (`support`, line 649). A parallel subsystem beside it would be split-brain.
- [ ] Thread one `Profile` field through `PipelineOptions` (`pipeline.rs:24`).
- [ ] `strict` rejects `ParseErrorClass::Tombstoned` warnings as errors, plus `@v0` and
      `@mcp.tool`. Note `@v0` is currently **silently dropped from HIR with no diagnostic at all**
      (`hir/lower/mod.rs:417-419`).
- [ ] `syntax_version` is a comment regex in one detector and is invisible to the compiler
      (`detectors/syntax_version.rs:27`). Delete it or repoint it at the profile.
- [ ] Emit the profile-filtered grammar as **EBNF / Lark / XGrammar-2 — never GBNF**, which
      `grammar-export/src/lib.rs:97` refuses repo-wide over CVE-2026-2069.
- [ ] **Do not require `strict` for corpus generation in the same release.** The generator would
      reject its own output and silently narrow the training set.

### Task 4.6 — Wiring: intra-module only; cross-file needs a compiler feature

**Revision 1's load-bearing claim was false.** Compilation is per-file: `pipeline.rs:96` is
`run_frontend_str(source, file_path)`; `build.rs:156` compiles one file; `app_contract.rs:92`
projects from a singular module; `hir/lower/mod.rs:195-199` defers import resolution to
`Interpreter::run_module`. A `route` in `handlers.vox` mounted from `app.vox` is invisible at
check time.

- [ ] **4.6a** — intra-module only: a `route` in a file whose own `routes` block does not mount it
      and which exports nothing; a `state_machine` state with no inbound transition (pure AST; the
      outbound case already ships).
- [ ] Land as an **Error-severity lint with an allowlist**; measure the false-positive rate on
      `examples/golden/**` before considering promotion.
- [ ] **4.6b** — cross-file requires `vox check --project`: resolve `HirImport::local_file_path`
      at check time and merge `AppContractModule`s. **Its own phase, weeks.** Note the hazard:
      making an early stage depend on the emitted MCP manifest is circular or a forced two-pass
      compile.
- [ ] Correct the spec's status quo claim: `unwired_module` and `reachability` are **Rust/TS only**
      (`reachability.rs:161`, `unwired_module.rs:26-36`). Nothing polices unwired Vox surfaces
      today.

### Task 4.7 — Rust core: the 5 real call sites

- [ ] Migrate the **5** genuine production secret reads to `resolve_secret`:
      `vox-actor-runtime/src/builtins/mod.rs:1162`, `vox-orchestrator-mcp/src/agent_tools.rs:31`,
      `vox-plugin-webhook/src/lib.rs:74,135`, `voxup/src/channel.rs:44`. That takes the detector's
      true-positive rate to zero.
- [ ] **Keep `env_secret_shape`.** No crate can ban `std::env::var`; "delete the detector" is
      unreachable. If you want it cheaper, move it to `clippy.toml` `disallowed-methods`.
- [ ] **Drop `#[non_exhaustive]` entirely.** It *requires* downstream wildcard arms, so adding a
      variant compiles cleanly everywhere — the opposite of the claimed effect. Instead: lint
      against stray `_` arms in cross-crate matches, and against applying `#[non_exhaustive]` to
      workspace-internal enums. The pattern that works is `feature_matrix.rs:639-651`.
- [ ] HTTP is a dependency change, not a type: newtype `vox_http_client::client()`'s return, drop
      `reqwest` from the ~20 consumer manifests, enforce via the already-armed `crate-edges`
      ratchet. Heaviest first: `vox-publisher` (17), `vox-orchestrator-mcp` (15), `vox-gamify` (10).

---

## Phase 5 — The unexamined surface

### Task 5.1 — LSP CI coverage

- [ ] **`vox-lsp` appears zero times in `.github/workflows/`.** No job builds, runs, or
      smoke-tests the language server; its only tests are 15 in `lib.rs`. Add a build + smoke job.
- [ ] It already reuses compiler diagnostics (`lib.rs:9-11`) and ships
      `code_action_provider: true` with quick-fixes from `data.fixes` (`lib.rs:139-178`) — so
      Task 4.1's DAF and Task 4.3's payload reach editors for free. Say so in the docs.
- [ ] **Measure which harness this repo's agents actually consume** before ranking CLI vs LSP work
      any further. Neither document has this data.

### Task 5.2 — Restore what Revision 1 dropped

- [ ] Probe-and-refine: make "resolve the target file via `graphify query` before editing" a
      contract rather than a session-hook nudge.
- [ ] AGENTS.md size A/B through `vox eval` (48 KB, ~12k tokens, loaded every session; the
      literature is split on whether that helps or hurts).
- [ ] Both were in the source audit and silently dropped by Revision 1 — a regression against the
      document being extended, and agent navigation cost applies to every task in this repo.

---

## Sequencing

Phase 0 → Phase 1 → Phase 2 → (Phase 3 ∥ Phase 4 ∥ Phase 5). Task 3.7 gates 3.6. Task 2.1 gates
every new gate under the zero-sum rule. Task 1.2 gates any re-ranking of Phase 4.

**Stop and re-plan if:**

- Task 1.2 reproduces the 2026-05-23 numbers → there is no repair deficit; Phase 4's ranking needs
  re-justification from scratch.
- Task 1.4's blast radius exceeds ~550 → remediation programme, own plan.
- Task 2.2's measured hit rate is below 25 % → Phase 4 is unaffordable at current budgets.
- Task 0.1 shows `setup` and `docker-vox-image-smoke` dominate → the cost programme moves to build
  caching and Task 2.1's 58s is noise.

**Most dangerous to abandon half-done: Task 3.6.** Partial expiry-stamping with enforcement live
means some entries expire and some are permanent, and "required for new suppressions" pushes
contributors toward the *other* baselines that have no expiry field at all
(`config-hygiene-baseline.txt` 379, `config-registry-baseline.txt` 348, `baseline-freeze.json`
6,091). Displacement, not cleanup — measurably worse than not starting.

## Definition of done

- Every CR-L gate returns 0 or 1, never 2, and every artifact has `incomplete: false`.
- No product gate resolves to a stub, enforced by a guard.
- `detect-rules-bench` reports a real F1 for all 45 rules, or fails naming the rules without
  fixtures.
- The scoped TOESTUB gate fails on `Error`, scans `crates`, and has a canary observed failing.
- `vox ci` reports zero dead gates against the extended `policy-registry`.
- Every evidence artifact is within `max_age_days`, with staleness at `ERROR` severity and
  `--strict-evidence` passed.
- No suppression lacks an enforced `expires_after`, and no baseline pins a line number.
- The fast tier is under its **re-baselined** budget, measured, with `enforce_budgets` on.
- `vox-lsp` has a CI job.
