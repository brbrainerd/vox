---
title: "VoxDB Audit & Condensation — Implementation Plan"
description: "Step-by-step plan to re-baseline vox-db's schema from 219 live tables to the 116 with real callers, and quarantine the 103 dead/dormant ones (4 of which are declared via a separate Collection-spec mechanism, not CREATE TABLE DDL; 5 are already-orphaned with no declaration anywhere), per 2026-08-01-voxdb-audit-condensation-design.md."
category: "architecture"
status: "roadmap"
training_eligible: false
training_rationale: "Implementation plan; transient artifact."
---

# VoxDB Audit & Condensation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-baseline `vox-db`'s schema down to the 116 tables with real callers (design §2.2), quarantine the 103 dormant/dead ones behind an opt-in Cargo feature (not delete — see design §3.2), resolve the two data-bearing exceptions and the "finished-but-unwired" subsystem clusters by hand rather than by default, and update or delete the tests that assert against now-quarantined tables (design §2.4's own table lists **9 test functions across 7 files** — `local_tests.rs` (3), `schema/domains/history.rs`, `schema/mod.rs`, `tests/news_approval_tests.rs`, `tests/schema_contract_tests.rs`, `tests/ops_skill_tests.rs`, `toestub_store.rs`; an earlier prose figure of "16 tests across 8 files" did not match that table and must not be used as a pass/fail target — see Task 1.2). Add a durable, re-runnable audit tool so this doesn't silently drift back over time.

**Declaration-mechanism note (Task 4 must account for this):** the 219 live tables are declared through **two different mechanisms**, confirmed by re-querying `.vox\store.db` and cross-referencing every table name against source: 210 via literal `CREATE TABLE` DDL under `crates/vox-db/src/schema/` (domain files, embedded `.sql`, and the DDL constants in `schema/spec/mod.rs`), and 4 more (`provider_usage`, `attention_events`, `agent_trust_scores`, `handoff_payloads`) via `CollectionInfo` entries in `schema::spec::orchestrator_schema_digest()`, backing schemaless tables created at runtime by `crate::collection::Collection::ensure_table()` rather than baseline DDL. The remaining 5 (`archive_membership`, `chunk_members`, `context_window_items`, `context_windows`, `zstd_dictionaries`) have **no declaration anywhere in current source** (confirmed via a repo-wide search, not just `schema/`) — they are pure leftover rows/tables from an already-removed feature. This means Task 4.1 ("move DDL to `quarantine.rs`") applies as written to 99 of the 103 quarantine candidates; `handoff_payloads` needs its `CollectionInfo` entry removed/relocated instead of DDL text, and the 5 fully-orphaned tables need no schema-file change at all — only the `DROP TABLE` in Task 5. See design §2.1 for the full reconciliation.

**Architecture:** Nine sequential tasks. Task 1 makes the audit durable and re-runnable (VoxScript, not the throwaway Python used for investigation). Task 2 resolves the ambiguous cases *before* anything is quarantined, since those decisions change what Tasks 3-5 touch. Tasks 3-5 are the actual schema/test/migration surgery. Tasks 6-9 are cleanup, guardrails, and verification. Tasks 7-8 are opportunistic documentation-only additions that surfaced during the audit but are not part of the schema-condensation deliverable itself — they carry no dependency on Tasks 1-6 and can be dropped without affecting the rest of the plan if time is short.

### Task dependency / parallelism

- **Hard sequential spine:** `0 (preflight) → 1 (census tool) → 2 (decisions) → ... → 9 (final verification)`. Task 0 must complete first (baseline must already be green) and Task 9 must run last (whole-workspace gates). Task 1's outputs (`table_usage_report.json`, `quarantine_test_findings.json`) feed Task 2 directly — not parallelizable with it.
- **Task 2 is the hard gate:** its outputs (the adjusted LIVE/DORMANT/DEAD split, and specifically whether `history_entries`, TOESTUB, Scientia, external-review, publication, MENS/training, and `skill_executions` stay live or get quarantined) determine which tables Tasks 3, 4, and 5 actually touch. None of 3/4/5 can start until 2.1-2.3 are resolved — their file scopes are undefined before that.
- **Task 4 → Task 5 must be sequential, not parallel:** Task 5.1's migration targets the version and table list Task 4.4 just finalized; running 5 concurrently with 4 risks targeting a `BASELINE_VERSION` or DROP list that hasn't settled yet.
- **Task 3 and Task 4 are NOT safely parallel by default**, despite touching "conceptually different" concerns (tests vs. DDL). `schema/domains/history.rs` plausibly holds both the `history_entries` DDL Task 4 moves *and* the test Task 3.3 disposes of in the same file; `toestub_store.rs` similarly couples Task 3.7's test disposition to Task 4's DDL move if the TOESTUB cluster is quarantined. Whether this collision is real depends entirely on Task 2's decision (if `history_entries` stays live and TOESTUB has an active doc, there's no overlap). **Check file-overlap again after Task 2 concludes, before dispatching 3 and 4 in parallel** — don't assume disjointness from task numbering alone.
- **Task 6** touches a different crate (`vox-db-types`, read-mostly) and depends only on Task 2's finalized DEAD list; it is file-disjoint from 3/4/5's targets and can run in parallel with that cluster once Task 2 is done.
- **Tasks 7 and 8** are pure documentation, touch files unrelated to `vox-db` source, and depend on neither the census nor Task 2's decisions — safe to hand off as background work at any point.
- **Steps too ambiguous for an unsupervised subagent without a clarifying question first:** Task 2.1-2.3 (judgment calls that change downstream file scope), Task 3.7 (explicit "flag it rather than silently deleting" decision point), Task 8.1 ("skip rather than inventing new documentation" is itself a judgment call), Task 5.2 (must re-confirm the exact DROP list against Task 2's final decisions before running any destructive-pattern migration test, even against a copy).

**Tech Stack:** Rust 2024, `cargo`, VoxScript (`vox run`), `turso`/SQLite, Markdown.

**Spec:** [2026-08-01-voxdb-audit-condensation-design.md](./2026-08-01-voxdb-audit-condensation-design.md)

**Authoritative ground truth (do NOT update from memory — re-read before editing):**
- `crates/vox-db/src/schema/domains/*.rs` (actual current DDL)
- `crates/vox-db/src/schema/manifest.rs` (actual current `BASELINE_VERSION`)
- `crates/vox-db/Cargo.toml` (actual current feature flags)
- The design doc's audit tables (§2.2-2.4) — but re-run Task 1's tool rather than trusting the numbers if any schema file has changed since 2026-08-01

**Project rules to honor (from CLAUDE.md / AGENTS.md):**
- Project automation MUST be `.vox` files via `vox run`, never `.ps1`/`.sh`/`.py` glue (Task 1).
- `archive/` and `docs/src/archive/` are tombstoned — do NOT read or modify.
- Use `Edit`/`Write`/`Read`/`Glob`/`Grep` tools, not `cat`/`sed`/`awk`/`echo`.
- Auto-generated docs MUST be regenerated, never hand-edited.
- Never `cargo fmt --all` on Windows — use `vox run scripts/fmt.vox` or `cargo fmt -p vox-db`.

---

## Pre-flight (do once before starting)

- [ ] **Step 0.1: Confirm baseline build and tests are clean.**

  Run:
  ```
  cargo build -p vox-db -p vox-db-types
  cargo test -p vox-db
  ```
  Expected: both clean. This is the reference point — if `cargo test -p vox-db` is already red before you start, stop and investigate that first; don't let pre-existing failures get attributed to this work.

- [ ] **Step 0.2: Read the spec end-to-end.** `docs/src/architecture/2026-08-01-voxdb-audit-condensation-design.md`, especially §2.3 (the two data-bearing exceptions) and §2.4 (the affected tests — read the table itself, not the "16 across 8" prose line above it; the table lists 9 functions across 7 files and is the one Task 1.2 must reproduce) before touching any code.

---

## Task 1: Build the durable VoxScript audit tool

**Why first:** everything downstream depends on an accurate, re-runnable table census. The design doc's numbers came from a one-off Python scratchpad script (not committed, not re-runnable by anyone else) — this task turns that into the real tool referenced by design §3.6, so the plan isn't executing against a snapshot that may have already drifted.

- [ ] **1.1a (RED, write first):** Before writing any classification logic, create a small fixture set — a handful of throwaway `.rs` files (or an in-repo `tests/fixtures/db_table_census/` directory) containing a few `CREATE TABLE` statements with known-correct LIVE/DORMANT/DEAD outcomes (e.g. one table referenced from a fixture file outside `vox-db`, one referenced only inside `vox-db`, one referenced nowhere). Write the test that asserts the classifier sorts these correctly, and run it — it must fail (no script exists yet). This is the driving test for 1.1, not a nice-to-have.

- [ ] **1.1: Write `scripts/db-table-census.vox`.** Port the classification logic from the investigation (walk `.rs` files under `crates/`, `apps/`, `server/`, `tools/`, `tests/`, `examples/`, `scripts/`, `ci/`; regex-match table names from `sqlite_master` against file contents; exclude `codex_legacy.rs`, `circuit_breaker.rs`, `schema/manifest.rs` from the "usage" signal per design §2.2 — **document this exclusion list inline in the script with a comment explaining why each file is excluded**, since it's a hardcoded, non-structural signal that a future file of the same shape won't automatically inherit; classify LIVE/DORMANT/DEAD by whether references exist outside vs. only inside `vox-db`/`vox-db-types`). Output: `graphify-out/table_usage_report.json` (table, rows, status, referencing files) plus a human-readable summary printed to stdout. **Sensitive-table handling:** for tables whose name matches a secrets/identity pattern (at minimum `clavis_account_secrets`, `user_identities` — grep the domain files for other candidates before finalizing the list), emit status and referencing-file-count only, never row content, and flag them in the summary as "sensitive — do not auto-quarantine without manual review" regardless of their computed LIVE/DORMANT/DEAD status (see Task 2.4 below). Make 1.1a's fixture test pass (GREEN) before moving on.

  Run: `vox run scripts/db-table-census.vox -- --db .vox/store.db`

  Expected: 1.1a's fixture test passes. Against the live workspace, this tool's output is the *authoritative* count for Task 2 — it should closely match design §2.2's 219/116/82/21 (confirmed by direct query against `.vox\store.db` and full cross-reference at the time the design doc was written), but treat any small drift as real workspace change since then, not a known reconciliation gap; a large drift (more than a handful of tables) means something about the classification logic itself needs re-checking before trusting it.

- [ ] **1.2a (RED, write first):** Create a synthetic fixture — a couple of fake `#[test]` functions in a throwaway file that reference known DEAD/DORMANT table names (from 1.1's fixture, not the real schema) — with an asserted expected extraction (which functions, which files). Run it before the scanner exists; it must fail.

- [ ] **1.2: Write `scripts/db-test-census.vox`.** Same idea for design §2.4 — scan `#[test]`/`#[tokio::test]` function bodies for references to tables classified DEAD/DORMANT by 1.1's output. Output: `graphify-out/quarantine_test_findings.json`. Make 1.2a's fixture test pass (GREEN).

  Run: `vox run scripts/db-test-census.vox`

  Expected: 1.2a's fixture test passes. Against the live workspace, cross-check the output against design §2.4's table (9 functions / 7 files, **not** the doc's "16 across 8" prose figure — see the Goal section above) as a sanity check, but the tool's own output — not the design doc's table — is what Task 3 actually works from, since the tool may find things the manual audit missed or vice versa.

- [ ] **1.3: Commit both scripts and their outputs.** These become the recurrence-prevention mechanism (design §3.6) — a future CI hook or periodic check can diff against `graphify-out/table_usage_report.json` to catch new tables added without a caller. **Before committing `table_usage_report.json` and `quarantine_test_findings.json`, confirm they contain no row content for `clavis_account_secrets`/`user_identities`-class tables** (status/counts only, per 1.1's sensitive-table handling) — this report is meant to be periodically regenerated and re-committed, so this check applies every time it's re-run, not just once. Wiring an actual CI gate is optional/future work and is **not** delivered by this plan — note explicitly in the commit/PR description that "recurrence prevention" currently means "a re-runnable tool exists," not "drift is automatically caught"; nobody is yet obligated to re-run it before landing future schema changes.

---

## Task 2: Resolve the ambiguous cases before quarantining anything

**Why before Tasks 3-5:** whatever this task decides to keep live changes the DDL that stays in the default baseline and the tests that get UPDATE vs. left alone. Doing this first avoids quarantining something in Task 4 only to un-quarantine it in a follow-up PR.

- [ ] **2.1: `developer_journey_definitions` (1 row, DEAD by code-reference).** Do **not** start from `crates/vox-db/src/codex_legacy.rs` — that file is not where this row comes from. Instead read `crates/vox-db/src/schema/domains/developer_journeys.rs` (the `CREATE TABLE` DDL there carries an `INSERT OR IGNORE` seed row baked directly into the DDL string, which fires unconditionally every time `baseline_sql()` runs — i.e. on every fresh `VoxDb::connect()` — independent of any JSONL import path) and `crates/vox-db/src/store/ops_developer_journeys.rs` (confirm whether it reads `developer_journey_definitions` at all, or only `developer_journey_steps`). If confirmed to be a write-only schema-init seed with no reader anywhere: this is a distinct case from "generic incidental import residue" — treat it explicitly as a deterministic seed, not a legacy-import artifact, when writing the quarantine.rs comment (Task 4.1) and the DROP-migration exception list (Task 5.1), and proceed to quarantine as planned, excluded from the auto-DROP migration pending a one-time manual row export if the row has any value. If some real code path does read it that this re-check missed: pull it out of the DEAD list, treat as LIVE-adjacent, do not quarantine.

- [ ] **2.2: `history_entries` (1 row, DORMANT, paired round-trip + schema tests).** Read `crates/vox-db/src/history_store.rs` and both tests (`local_tests.rs::history_entries_round_trip`, `schema/domains/history.rs::history_entries_schema_round_trip`). Determine whether this is finished-but-simply-unwired functionality (recommended default: **keep in the live baseline**, not quarantine — a fully tested feature with real data is a poor quarantine candidate regardless of caller count) or genuinely abandoned. Record the decision and one-sentence rationale in this plan's status callout when done.

- [ ] **2.3: Cross-check subsystem clusters against other active-development docs.** Design §2.2 enumerates 11 subsystem clusters, not 5 — check **all of them**, not just a subset, since an unchecked cluster is quarantined by default and a missed check is exactly how active WIP gets silently dropped: Scientia (11 tables), external review/CodeRabbit (6), Codex conversation graph (7), publication/scholarly (6), TOESTUB (4), MENS/training (4), skill ecosystem (2 — confirm against design §2.2 whether this is the same cluster as any "skill_executions"/"skill-execution" table referenced elsewhere in this plan; reconcile the naming before treating them as the same thing), news publishing (3), planning graph (3), gamification/vox-mesh trust (4), CI completion detector (2). Grep `docs/src/architecture/` for recent design docs covering each (e.g. search for "scientia", "external review", "toestub", "skill", "mens", "codex conversation", "news publishing", "planning graph", "gamification", "ci completion"). **MENS/training in particular has multiple current, non-archived design docs in `docs/src/architecture/` (e.g. mens-training-ssot, mens-training-pipeline-audit-and-improvement-plan, mesh-mens-distributed-training-and-execution-plan, voxmens-* research/decision docs) — treat this as a strong signal the cluster is active, and confirm before quarantining any of its 4 tables.** For any cluster with an active/roadmap-status doc describing in-progress wiring, keep that cluster's tables in the default baseline (do not quarantine) even though the code-usage census marked them DORMANT — quarantining active WIP just creates rework for whoever's mid-flight on it. For clusters with no such doc, proceed with quarantine as planned.

  Record the final adjusted LIVE / DORMANT+DEAD split (starting point: 116/103, per Task 1.1's confirmed output) after this step — this is the actual list Tasks 3-5 operate on, not the raw census from Task 1.

- [ ] **2.4: Sensitive-table safeguard (independent of the code-reference census).** For `clavis_account_secrets`, `user_identities`, and any other table Task 1.1 flags as sensitive: regardless of what the census computes (LIVE/DORMANT/DEAD) today or on any future re-run, require an explicit human confirmation step before such a table is ever added to a quarantine or DROP list — a mechanical "zero non-quarantine callers" signal is not sufficient on its own for tables of this kind, because a later refactor could remove their last outside-`vox-db` caller and let the same tooling reclassify them with no one noticing. Record this as a standing rule (e.g. a comment in `scripts/db-table-census.vox` and in this plan) rather than a one-time check, since Task 1's tool is meant to be re-run indefinitely per design §3.6.

---

## Task 3: Test disposition

Work through the test functions from Task 1.2's output (design §2.4's table: 9 functions across 7 files — see the Goal-section note above; do **not** target "16 across 8"), using Task 2's decisions to determine which tables are actually leaving the baseline.

**Sequencing note (applies to every subtask below):** editing an assertion list *before* Task 4 has removed any DDL is a no-op change from the test's perspective — the quarantined tables still physically exist in the schema at that point, so "trim the list, run tests, see green" proves nothing (it was never red). The real red/green cycle for each test below happens in two passes:
1. **Now (Task 3):** make the edit described in each subtask, but do not yet treat a passing `cargo test -p vox-db` as confirmation — the tables haven't been removed yet, so this can only catch compile errors.
2. **After Task 4.3 removes the DDL, and again after 4.4 bumps the version:** re-run `cargo test -p vox-db`. This is the actual red/green checkpoint — if you temporarily revert a given test's Task-3 edit at that point and it now fails (compile error or assertion failure) against the post-quarantine schema, that confirms the edit was both necessary and targets the right table. Task 4's "Run" block (below) has been updated to include this step explicitly; don't skip it by treating Task 4's plain `cargo build` as sufficient.

- [ ] **3.1: `local_tests.rs::baseline_schema_includes_chat_and_search_tables`** — UPDATE: remove any now-quarantined table names from its assertion list (subject to Task 2.3's TOESTUB/chat-search cluster decision); keep it asserting the tables that remain live. **Meaningfulness check:** after trimming, confirm the test still asserts more than one or two tables — if Task 2.3 quarantined most of the groups this test originally checked, consider whether the remaining assertion is still worth keeping as a distinct test or should be folded elsewhere; a test that trivially passes on a single leftover table is low-signal.
- [ ] **3.2: `local_tests.rs::legacy_jsonl_roundtrips_gamification_and_coordination`** — UPDATE: remove `distributed_locks` (and any other now-quarantined table it iterates) from the roundtrip set it exercises. **Verify post-Task-4.3** that this test would fail if you left `distributed_locks` in the iterated set (i.e. it's actually exercising the removed table, not passing regardless).
- [ ] **3.3: `local_tests.rs::history_entries_round_trip` and `schema/domains/history.rs::history_entries_schema_round_trip`** — disposition per Task 2.2's decision: if kept live, no change needed. If quarantined: move both tests behind the same decision (delete or mark `#[ignore]` with the rationale). **Before deciding, verify post-Task-4.3** that these tests do in fact fail against the post-quarantine schema (compile error, since `history.rs` holds both the DDL Task 4 moves and this test — see the parallelism note above) — that failure is the red step confirming the disposition is necessary, not a guess.
- [ ] **3.4: `schema/mod.rs::chat_search_and_codex_in_fragments`** — UPDATE: trim to tables still in the default baseline after Task 2.3. **Verify post-Task-4.3** the untrimmed version would fail (fragment no longer contains the quarantined table's DDL) before finalizing the trim.
- [ ] **3.5: `tests/news_approval_tests.rs::mark_news_published_column_order_matches_github_twitter_oc...` and `tests/schema_contract_tests.rs::published_news_uses_news_id_primary_key`** — disposition per whether the publication/scholarly cluster was kept live in Task 2.3. Same post-4.3 red-check requirement as 3.3-3.4 if quarantined.
- [ ] **3.6: `tests/ops_skill_tests.rs::unpublish_skill_removes_row`** — **this test only exercises `skill_manifests`** (via `publish_skill`/`unpublish_skill`/`get_skill_manifest`, all of which issue statements against `skill_manifests`, not `skill_executions`) — its disposition should follow whatever Task 2.3 decided for the skill-manifests table, not `skill_executions`. If the intent was specifically to cover `skill_executions`, the correct target tests are `list_skill_executions_returns_newest_first`, `list_skill_executions_limit_is_honoured`, and `list_skill_executions_empty_when_no_rows` — in the same file, `tests/ops_skill_tests.rs`, not a different one — confirm against Task 1.2's actual output (not this plan's original assumption) which test(s) really reference `skill_executions`, and disposition those per whether the "skill ecosystem" cluster (design §2.2) was kept live in Task 2.3.
- [ ] **3.7: `toestub_store.rs::ensure_tables_is_idempotent_and_creates_usable_tables`** — first check whether `toestub_store.rs`'s public functions have *any* caller outside the module (if the whole module is unused, this is a larger dead-code question than one test — flag it rather than silently deleting the test and leaving dead production code behind; this is a decision point, not a scoped edit — surface it before proceeding rather than resolving it unsupervised). Disposition per Task 2.3.

  Run after all of 3.1-3.7 (first pass, DDL still present): `cargo test -p vox-db`
  Expected: clean, edits compile. This does **not** yet confirm the dispositions were correct — see the mandatory re-run after Task 4.3/4.4 below.

---

## Task 4: Schema re-baseline

- [ ] **4.1:** Create `crates/vox-db/src/schema/domains/quarantine.rs`. For each table in the final quarantine list (Task 2.3's output) that has literal `CREATE TABLE` DDL, move it there verbatim from its current domain file, with a comment noting status (DEAD/DORMANT), the ops-file trail from Task 1.1's output (or "none" for DEAD), and subsystem cluster. **Note:** until 4.3 runs, the DDL now exists in both `quarantine.rs` and the original domain file — this is an expected transient duplicate-definition state, not yet a build error (nothing assembles `quarantine.rs` into `baseline_sql()` until 4.2), but don't run a full schema-creation test between 4.1 and 4.3 or it will double-create tables. **Two exceptions to the DDL-move pattern** (see the "Declaration-mechanism note" in the Goal section, and design §2.1): if `handoff_payloads` ends up on the final quarantine list, its only declaration is a `CollectionInfo` entry in `schema::spec::orchestrator_schema_digest()` — remove or relocate that entry instead of moving DDL. `archive_membership`, `chunk_members`, `context_window_items`, `context_windows`, and `zstd_dictionaries` have no declaration anywhere in current source — skip them entirely in 4.1; they need no schema-file change, only the `DROP TABLE` in Task 5.

- [ ] **4.2a (RED, write first):** The real edit site for gating is `crates/vox-db/src/schema/manifest.rs` (lines ~52-175), **not** `schema/domains/mod.rs` — `domains/mod.rs` contains only `pub mod ...;` declarations with no fragment-assembly logic; the `SCHEMA_FRAGMENTS` const array and `pub fn baseline_sql()` that actually loop over fragments and build the DDL string live entirely in `manifest.rs` (re-exported via `schema/mod.rs`). Before touching `manifest.rs`, write a test that calls `baseline_sql()` with the `quarantine` feature off and asserts a quarantined table's `CREATE TABLE` string/name is **absent**, plus a second variant of the same test (or a `#[cfg(feature = "quarantine")]`-gated one) asserting it's **present** when built with `--features quarantine`. Run the feature-off variant now — it should fail (the DDL hasn't moved/been gated yet), confirming this is a real red step and not a check of already-true behavior. A plain `cargo build` cannot substitute for this: build succeeding says nothing about whether the SQL string was actually excluded from the binary.

- [ ] **4.2:** Add a `quarantine` feature to `crates/vox-db/Cargo.toml` (default off — but do **not** copy the `local` feature as your "off by default" precedent: `local` is actually `default = ["local"]` in the current `Cargo.toml`; only `replication` is off-by-default. Model `quarantine` on `replication`, not `local`, and do not add `quarantine` to the `default = [...]` array). Wire `manifest.rs` so the quarantine fragment is only appended in `baseline_sql()` when the feature is enabled (there is no existing SQL-gating pattern in this crate to "mirror" — `local`/`replication` gate Rust connection/config code paths in `config.rs`, `facade/connect.rs`, `pool.rs`, `store/open.rs`, never a conditionally-included DDL fragment, so treat this as a new pattern for the crate, not a copy of an established one). Make 4.2a's test pass (GREEN) for both feature configurations.
- [ ] **4.3:** Remove the quarantined tables' DDL from their original domain files (this resolves 4.1's transient duplicate-definition state).

  Run immediately after 4.3, before 4.4: `cargo test -p vox-db`
  Expected: this is the real red/green checkpoint for all of Task 3's edits (see Task 3's sequencing note) — the quarantined DDL is now actually gone from the default build, so a green result here means Task 3's trimmed assertions are genuinely exercised, not vacuously passing. If anything fails here, it's either a Task 3 edit that missed a table or a Task 4.1/4.3 DDL-move mistake — resolve before proceeding to 4.4.

- [ ] **4.4:** Bump `BASELINE_VERSION` 84 → 85 in `schema/manifest.rs`, with a one-line changelog comment matching the existing style (see lines 10-18 of that file).

  Run:
  ```
  cargo build -p vox-db
  cargo build -p vox-db --features quarantine
  cargo test -p vox-db
  cargo test -p vox-db --features quarantine
  cargo build --workspace
  ```
  Expected: all clean. The two plain `cargo build` legs alone are not sufficient — `cargo test -p vox-db --features quarantine` is required to actually exercise the quarantined DDL's runtime behavior (a broken FK reference or DDL typo in the quarantined fragment could compile but fail to create a working schema, and `cargo build` would not catch that). The final `cargo build --workspace` leg exists specifically to catch Cargo feature unification: because this workspace uses `resolver = "2"`, if any other crate in the workspace ever adds `vox-db` as a normal (non-dev) dependency with the `quarantine` feature enabled, that feature gets unified into every normal-dependency use of `vox-db` in the same build, including production binaries — `cargo build -p vox-db --features quarantine` run in isolation would never surface that. If this workspace-wide build ever picks up quarantined DDL unexpectedly, treat it as a build-graph bug and fix the offending crate's `Cargo.toml`, the same class of issue as the recent `vox-orchestrator-mcp` wiremock/eval-gate fix.

---

**Out of scope for this plan:** design §3.5 ("Graduation path" — the procedure to revive a quarantined table later) has no corresponding task here and is not implemented by Tasks 1-9. If a quarantined table needs to come back, that's a follow-up piece of work following design §3.5's 4-step procedure (move DDL out of `quarantine.rs`, resolve test disposition, bump `BASELINE_VERSION` again, land the real caller) — not something this plan's tasks produce automatically.

## Task 5: Existing-DB migration

**Blocker-level correction — read before implementing:** the originally-described mechanism ("a `Migration` entry that runs `SELECT COUNT(*)` and aborts, else `DROP TABLE`, via `migration.rs`'s existing mechanism") cannot work as stated. `Migration` is `{ version: i64, name: String, up_sql: String }` with no callback/hook field, and `apply_migrations` executes `up_sql` via `connection().execute_batch(...)` — a flat batch with no host-language branching and no way to surface a query result back into Rust for a conditional decision. `migration.rs`'s own module doc says `up_sql` "must not contain row-returning statements (no standalone SELECT)" for exactly this reason. A `SELECT COUNT(*)`-then-branch cannot live inside `up_sql`. The safety rail has to be Rust-side orchestration that runs **before** `apply_migrations` is ever called for this migration, not inside it. Additionally, `migration.rs`'s own doc warns "use custom `Migration` rows only on ephemeral DBs, tests, or with a plan to re-baseline the file" — read that warning in full before writing 5.1.

**Second blocker — version numbering:** do not add this migration "at the next version" past `BASELINE_VERSION`. `store/open.rs`'s normal `connect()` path treats `current_version > BASELINE_VERSION` as a fatal, non-recoverable `StoreError::LegacySchemaChain` with no automatic recovery. Task 4.4 bumps `BASELINE_VERSION` to 85; if this migration lands at version 86, the instant it's applied to any DB, that DB's `schema_version` (86) exceeds `BASELINE_VERSION` (85) and every subsequent plain `connect()` on it fails forever. The migration that drops the quarantined tables for existing DBs must bring `schema_version` to exactly 85 — the same version fresh installs reach via `baseline_sql()` (which already excludes the quarantined DDL by construction) — not to some version beyond it. Coordinate the exact version number with whatever migration already exists (if any) that advances old DBs toward 84/85, so there is exactly one migration path to version 85, not two.

- [ ] **5.1a (RED, write first, persisted in `migration.rs`'s existing test harness — not a one-time manual check):** Write two failing tests before any implementation exists:
  - (a) Given a DB copy with a non-empty to-be-quarantined table, running the migration path aborts with the named error and leaves every table (including the non-empty one) intact, and `schema_version` does **not** advance. This must fail now because neither the pre-check function nor the migration exists yet.
  - (b) Given a DB copy where all to-be-quarantined tables are empty, running the migration path drops them and `schema_version` advances to 85. Also failing now for the same reason.

  Both tests must also cover the case where a quarantined table **doesn't exist at all** in the DB copy (older baseline that predates that table) — assert this is treated as "0 rows, safe to skip," not an unhandled SQLite "no such table" error. Recall from design §2.7 that at least ~19 other `.vox/store.db` copies exist across worktrees at potentially different baseline versions; if only the main checkout's DB is available locally, simulate an older/partial schema for this case rather than skipping it.

- [ ] **5.1: Implement the two-phase mechanism to make 5.1a green:**
  - **Phase 1 (Rust-side, runs before any migration is applied):** a plain async function that opens a normal (non-batch) query connection, runs `SELECT COUNT(*) FROM <table>` for each quarantined table individually, treating a "no such table" error as count-0 (safe), and either aborts with a named error listing every non-empty table (no migration applied, DB untouched) or proceeds.
  - **Phase 2 (the actual `Migration` entry, via the existing mechanism):** only reached if Phase 1 passed — a plain, unconditional `DROP TABLE IF EXISTS <table>` batch for every table Phase 1 confirmed empty, at version 85 (see version-numbering note above).
  - **Rollback/backup:** `Migration` has no `down_sql`/reverse-migration field anywhere in the crate, so there is no automated rollback if something goes wrong after the DROP batch runs (partial batch failure, wrong table on the list, needing to undo after the fact). Document this explicitly in the migration's code comment and require whatever invokes this migration in practice to keep (or instruct the user to keep) a copy of `.vox/store.db` taken immediately before the upgrade — do not silently rely on "it's just a DROP, it's fine."
  - **Abort-recovery path:** because `migrate()` runs automatically on every local `connect()` (design §2.5), a Phase-1 abort will recur on every subsequent launch until the offending table's rows are cleared or exported by hand. Add a clear-error message that names the exact table(s), states that the install is pinned below the new baseline until resolved, and points at a one-line remediation (export the row(s), then either delete them or move them to their new home, then retry). Do not leave this as "aborts with a clear error" with no further guidance — that leaves a real local install stuck with no stated recovery.

- [ ] **5.2:** Test the migration against a **copy** of the real audited DB (`.vox/store.db` from the main checkout) — never the live file directly. Confirm: (a) it drops all quarantined tables when run against a copy with the two exceptions from Task 2 already excluded from the DROP list, (b) it correctly refuses and names the table if you deliberately insert a row into a to-be-dropped table first. **This is in addition to 5.1a's persisted tests, not a replacement for them** — 5.1a is the committed regression test for the safety rail; 5.2 is a one-time exploratory sanity check against real data. Re-confirm the exact DROP list against Task 2's final decisions immediately before running this (per the parallelism note above, don't run this from a stale table list). Also note: since this migration is only exercised here against a copy of the main checkout's DB, and no other worktree's `.vox/store.db` is tested, treat any other *active* worktree as untested until its own next normal `vox` invocation applies the migration — if that's a concern, run this same procedure against one or two of the other known worktree DB copies as an extra check before considering the migration safe to rely on broadly. Stale/abandoned worktrees are handled by pruning (Task 8.1), not by this migration.

  Run:
  ```
  copy .vox\store.db .vox\store.db.test-copy
  ```
  then run the migration against the copy via `migration.rs`'s existing test harness (extend it with 5.1a's two persisted tests rather than only doing this manually), and inspect the resulting table list.

  Expected: table count drops from 219 to the Task 2.3-adjusted LIVE count; `schema_version` reflects 85; no data loss on any table that had rows; a table missing from an older-baseline copy is handled per 5.1a, not treated as a hard failure.

---

## Task 6: `vox-db-types` cleanup for DEAD tables only

- [ ] **6.1 (green-before-red-analog: build green before deletion, delete, build still green after):** For the final DEAD-table list from **Task 2.3's adjusted split** (not a hardcoded "21 originally-DEAD" — Task 2.1/2.3 may have pulled tables out of or into that set, e.g. if `developer_journey_definitions` turns out not to be genuinely dead-adjacent), grep `crates/vox-db-types/src/store_types/*.rs` for any corresponding Rust struct. Design §2.2's original census found zero references anywhere for true DEAD tables, so this should turn up nothing — treat any hit as a signal the census under-counted and re-run Task 1.1 rather than deleting a type that's actually referenced somewhere the scan missed.
- [ ] **6.2:** If 6.1 confirms zero references for a struct: this task is subtractive only, so there's no red test to write first — instead, confirm `cargo build -p vox-db-types` is green **before** deleting anything (establishing the pre-deletion baseline), delete the confirmed-dead struct(s), then run `cargo build -p vox-db-types` (and `cargo build --workspace`) again as the green checkpoint proving the deletion was safe. Do not stop at "grep found nothing" — that alone deletes nothing; the actual removal and this before/after build check are the deliverable of this task.

---

## Task 7: Storage-boundary guardrail (documentation only)

- [ ] **7.1:** Add a short note to `docs/src/architecture/where-things-live.md` (or create a small dedicated doc if that file isn't the right home) documenting the threshold from design §3.7: `embeddings.vector` and any future large-BLOB columns move to file/object storage with a path+hash pointer once real row count or per-row size crosses a stated number. No code change.

---

## Task 8: File-sprawl addendum (documentation only, low priority)

- [ ] **8.1:** One-line addition to wherever the existing worktree-cleanup practice is documented (or your own habit, if it's not written down anywhere yet — in that case, skip rather than inventing new documentation for it): stale worktree removal also reclaims its `.vox/store.db`.

---

## Task 9: Final verification

- [ ] **9.1:** Full workspace build and `vox-db` test suite:
  ```
  cargo build --workspace
  cargo test -p vox-db
  cargo build -p vox-db --features quarantine
  cargo test -p vox-db --features quarantine
  ```
  Expected: all clean. (This repeats Task 4's final "Run" block as a whole-workspace re-confirmation, including the `--features quarantine` test leg — not just the build legs.)
- [ ] **9.2:** Re-run `vox run scripts/db-table-census.vox -- --db .vox/store.db` (Task 1.1's tool). Expected: LIVE count matches Task 2.3's final adjusted number; DORMANT+DEAD count is 0 in the default (non-quarantine) baseline's declared tables (quarantined tables now live only in `quarantine.rs`, outside the default `baseline_sql()` scan path).
- [ ] **9.3:** Confirm file-size impact. **`vox init` does not create a `.vox/store.db`** — it only scaffolds `Vox.toml`/`src/main.vox`/`.vox_modules/` (`crates/vox-cli/src/commands/init.rs`'s `run()` calls `scaffold_vox_project_at()`, which never touches `VoxDb`). `store.db` is created lazily the first time something actually calls `VoxDb::connect()` — before running this step, identify which real command or test path first opens the store (e.g. a command that touches lock/sync/search state, or `vox-db`'s own connect-test harness against a fresh scratch path) and use that instead of `vox init`. Run it in a scratch directory and measure `.vox/store.db`'s size. **Acceptance bar:** don't just check "smaller than 5.9MB" — that would pass on a marginal, buggy 1% reduction. Since the table-count reduction is now whatever Task 1.1/9.2 actually measured (not a fixed 219→116), compute the expected size reduction as roughly proportional to that ratio at this point in the plan (Task 9.2 has just re-run the census — use its LIVE count vs. the pre-condensation count) and confirm the measured size is in that ballpark, not merely "any amount smaller."
