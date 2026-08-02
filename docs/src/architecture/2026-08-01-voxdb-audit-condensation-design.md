---
title: "VoxDB Audit & Condensation — Schema Re-baseline and Quarantine Design"
description: "Live-data audit of the canonical local VoxDB store plus a design to re-baseline the schema to observed-live tables and quarantine the rest, driven by a code-usage census instead of guesswork."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
training_rationale: "Implementation plan; transient artifact."
---

# VoxDB Audit & Condensation — Schema Re-baseline and Quarantine Design

Source-of-truth spec for the companion implementation plan: [2026-08-01-voxdb-audit-condensation-plan.md](2026-08-01-voxdb-audit-condensation-plan.md).

## 1. Scope

Audit target: `C:\Users\Owner\vox\.vox\store.db` (the main checkout's local embedded VoxDB — 5.9MB), chosen as the representative sample of real day-to-day usage. Out of scope for this pass: the ~19 other `.vox/store.db` copies scattered across worktrees/crates (addressed briefly in §2.7/§3.8 as a housekeeping note, folded into existing worktree-cleanup practice, not a new mechanism), and `vox-server`'s separate Postgres-backed service (different stack, different migration system, not part of `vox-db`).

## 2. Audit findings

### 2.1 The DB is schema-heavy, not data-heavy

**219 tables** is confirmed directly from the live file (`PRAGMA` / `sqlite_master` query against `.vox\store.db`, re-run for this doc): `SELECT COUNT(*) FROM sqlite_master WHERE type='table'` returns exactly 219. That number is real; what needed reconciling was where those 219 tables are *declared* in source, since a plain `CREATE TABLE` grep undercounts:

- **210** tables come from literal `CREATE TABLE` DDL text under `crates/vox-db/src/schema/` (domain files, embedded `.sql` fragments, and the DDL constants in `schema/spec/mod.rs`).
- **4 more** (`provider_usage`, `attention_events`, `agent_trust_scores`, `handoff_payloads`) are declared through a *second*, separate mechanism: `schema::spec::orchestrator_schema_digest()`'s `CollectionInfo` list, backing schemaless `_id`/`_data` JSON-document tables created at runtime via `crate::collection::Collection::ensure_table()` (`db.collection("provider_usage")` etc.), not baseline DDL. `210 + 4 = 214`.
- **The remaining 5** (`archive_membership`, `chunk_members`, `context_window_items`, `context_windows`, `zstd_dictionaries`) have **zero declaration and zero reference anywhere in the current source tree** — confirmed via a repo-wide search (`rg`, all `.rs`/`.sql`, `target`/`.worktrees` excluded), not just the `schema/` subtree. They exist only as physical leftovers in the live file from a schema version that predates the current codebase; nothing today creates, reads, or writes them. `214 + 5 = 219`.

This changes what "quarantine" means for those two groups in §3: the 4 `CollectionInfo`-backed tables need their digest entry (not a DDL string) removed/moved if quarantined, and the 5 fully-orphaned tables need no source change at all — only the `DROP TABLE` in the Task 5 migration, since there's no declaration to relocate. The LIVE(116)/DORMANT(82)/DEAD(21) split in §2.2 is unaffected by this reconciliation: that census was computed directly from the live DB's 219 real table names cross-referenced against actual code usage everywhere (not from a schema-declaration grep), so it already correctly placed `provider_usage`/`attention_events`/`agent_trust_scores` as LIVE (real `vox-orchestrator` callers) and `handoff_payloads` plus the 5 orphans as DEAD.

Live query against the real file:

- Only **8 of 219 tables have any rows at all** (`agent_exec_history`=278, `agent_events`=276, `developer_journey_steps`=8, `schema_version`=4, `user_preferences`=2, `conversations`=1, `developer_journey_definitions`=1, `history_entries`=1) — re-confirmed by a fresh query against the live file for this document, identical to the original audit.
- **571 total rows** in a 5.9MB (5,931,008-byte) file. `PRAGMA page_count` × `PRAGMA page_size` = 1,448 × 4,096 = 5,931,008 — an exact match to the on-disk size, confirming the file is 100% accounted for by SQLite pages, no slack space. The live file has 219 tables + 386 indexes = 605 schema objects sharing those 1,448 pages, ~2.39 pages/object — with only 571 rows total, nearly all of that page count is empty b-tree root/leaf overhead, not stale data.
- Every new worktree pays this tax immediately on `vox` init (observed 1.4-13.3MB per copy across 19 on-disk instances via direct filesystem stat, not estimated), regardless of whether it ever uses more than a handful of tables.

**Consequence for the plan:** "condensation" here means shrinking the *declared surface*, not deleting rows — there's almost nothing to delete at the row level.

### 2.2 Code-usage census (in lieu of a full Graphify semantic pass)

The user's ask was to use Graphify to trace what calls the database. A full Graphify LLM-semantic extraction over this ~3,900-file, 100+-crate workspace would be slow and costly for a question that's actually an exact-match lookup ("does any Rust code reference table X"), not a semantic one. Graphify's own AST path is deterministic/free but wouldn't capture SQL table names anyway — they're string literals inside `turso::Connection::query/execute` calls, not Rust symbols. A targeted static scan across all 3,861 `.rs` files under `crates/`, `apps/`, `server/`, `tools/`, `tests/`, `examples/`, `scripts/`, `ci/` answers the actual question directly and cheaply.

Three files were excluded from the "usage" signal because they mention nearly every table name generically (JSONL legacy import/export, a circuit-breaker doc-comment enumerating the whole feature surface, and the schema manifest's required-table lists) rather than performing real per-table CRUD: `codex_legacy.rs`, `circuit_breaker.rs`, `schema/manifest.rs`. Without this exclusion, 211/219 tables would falsely appear "used."

| Status | Count | Definition |
|---|---|---|
| **LIVE** | 116 (53%) | Referenced by code outside `vox-db`/`vox-db-types` (orchestrator, GUI, MCP servers, CLI, etc.) |
| **DORMANT** | 82 (37%) | `vox-db` has CRUD/ops code for it (facade fn, `store/ops_*.rs`, or a type in `vox-db-types`), but nothing outside the crate ever calls it |
| **DEAD** | 21 (10%) | No non-generic code anywhere — not even inside `vox-db` — touches it |

**DEAD (21):** `archive_membership`, `artifact_reviews`, `builder_sessions`, `chunk_members`, `codex_projection_versions`, `codex_query_snapshots`, `codex_subscriptions`, `context_window_items`, `context_windows`, `developer_journey_definitions`, `handoff_payloads`, `package_deps`, `populi_reviews`, `scholarly_publication_records`, `scientia_citations`, `scientia_prereg`, `scientia_publication_attempts`, `session_turns`, `syndication_events`, `typed_stream_events`, `zstd_dictionaries`.

**DORMANT (82):** full list and per-table ops-file trail will live in `graphify-out/table_usage_report.json`, generated by the VoxScript audit tool built in plan task 1 (§3.6) — that tool is the durable source, not this document. Representative clusters by subsystem: Scientia research pipeline (11 tables), external review/CodeRabbit (6), Codex conversation graph (7), publication/scholarly (6), TOESTUB build cache (4), MENS/training (4), skill ecosystem (2), news publishing (3), planning graph (3), gamification/vox-mesh trust (4), CI completion detector (2), plus scattered singles.

### 2.3 Two exceptions that must NOT be treated as routine quarantine candidates

Row-count cross-check against the 103 DEAD+DORMANT tables found **two with real (if minimal) data**:

- `developer_journey_definitions` (DEAD by code-reference, 1 row) — a row exists despite zero code referencing it outside the excluded generic files. This is **not** legacy-import residue: the row comes from a static `INSERT OR IGNORE` baked directly into the `CREATE TABLE` DDL string in `schema/domains/developer_journeys.rs` (lines 23-31), which fires unconditionally every time `baseline_sql()` runs — i.e. on every fresh `VoxDb::connect()` — independent of any JSONL import path. Confirmed by the identical single row (`journey_id = 'canonical_journey.v1.greenfield_vox_mens_devloop'`) present in 10/10 independently-sampled `.vox/store.db` files spanning different ages and branches. `store/ops_developer_journeys.rs` (the only code that touches this journey system) queries `developer_journey_steps` directly and never reads `developer_journey_definitions` at all — it is a write-only schema-init seed, not an orphaned import artifact, and the code-usage census's "referenced by code" methodology has no way to distinguish that from a genuinely orphaned row.
- `history_entries` (DORMANT, 1 row) — has a paired round-trip test (`local_tests.rs::history_entries_round_trip`) **and** a schema round-trip test (`schema/domains/history.rs::history_entries_schema_round_trip`), plus real data. This looks like finished, tested functionality that simply hasn't been wired to a caller yet, not abandoned scaffolding.

These two are called out explicitly in the plan as manual-review items, not automatic quarantine/drop candidates.

### 2.4 Tests that will break under re-baselining

Grepped every `#[test]`/`#[tokio::test]` function body (not just file-level references) across the workspace for the 103 DEAD+DORMANT table names. 9 test functions across 7 files are affected (an earlier count of "16 across 8" did not match the enumerated list below and has been corrected):

| File | Test(s) | Touches |
|---|---|---|
| `crates/vox-db/src/local_tests.rs:51` | `baseline_schema_includes_chat_and_search_tables` | `conversation_edges`, `conversation_versions`, `processing_run_steps`, `processing_runs`, `search_indexing_jobs`, `topic_evolution_events` |
| `crates/vox-db/src/local_tests.rs:380` | `legacy_jsonl_roundtrips_gamification_and_coordination` | `distributed_locks` |
| `crates/vox-db/src/local_tests.rs:305` | `history_entries_round_trip` | `history_entries` (see §2.3) |
| `crates/vox-db/src/schema/domains/history.rs:21` | `history_entries_schema_round_trip` | `history_entries` (see §2.3) |
| `crates/vox-db/src/schema/mod.rs:23` | `chat_search_and_codex_in_fragments` | `behavior_events`, `codex_capability_map`, `search_indexing_jobs` |
| `crates/vox-db/tests/news_approval_tests.rs:78` | `mark_news_published_column_order_matches_github_twitter_oc...` | `published_news` |
| `crates/vox-db/tests/schema_contract_tests.rs:57` | `published_news_uses_news_id_primary_key` | `published_news` |
| `crates/vox-db/tests/ops_skill_tests.rs:68` | `unpublish_skill_removes_row` | `skill_manifests` |
| `crates/vox-db/src/toestub_store.rs:203` | `ensure_tables_is_idempotent_and_creates_usable_tables` | `toestub_suppressions` (and sibling toestub tables) |

Per your instruction: these are **not** gated behind a feature flag and left in place. Each gets an explicit update-or-eliminate decision — see plan §3 for the per-test disposition and rationale (some, like the `published_news` and `history_entries` pairs, look like finished-but-unwired features and are flagged for a keep/quarantine judgment call rather than default deletion).

### 2.5 Migration/mutation stack: already lean, no action needed

Two mechanisms exist:
1. **Baseline manifest** (`schema/manifest.rs`) — single `BASELINE_VERSION` (currently 84) snapshot regenerated whenever schema changes; this is what nearly all schema evolution goes through.
2. **Legacy user-defined `Migration` struct** (`migration.rs`) — only **16 `Migration::new(` call sites** in the entire workspace, concentrated in `migration.rs` itself, one test file (`semcov_wave18_tests.rs`), and one plugin (`vox-gamify/src/db/helpers.rs`). That count is construction sites only; it does not include `facade/migrations.rs` (the real consumer — defines `apply_migrations(&self, migrations: &[Migration])`) or the 5 `Migration { ... }` literal constructions in `tests/migration_tests.rs`, both genuine references this list omits. The "no pile of migration debt" conclusion below still holds — the mechanism is small and centralized — but "concentrated in exactly these three files" undersells where the type is actually used.
3. **`auto_migrate`** — diff-based, additive-only (`CREATE TABLE`/`ADD COLUMN`/`CREATE INDEX`); by explicit design it never drops anything automatically (confirmed in its own doc comment).

There is no pile of migration debt to compress. This finding changes the framing from the original ask ("compress our stack of mutations") — the mutation stack isn't the bloat source; the schema surface is.

### 2.6 Storage boundary: one BLOB table worth a documented threshold, no fix needed yet

`embeddings.vector` is a `BLOB NOT NULL` column storing vectors inline in SQLite. At today's volume (0 real rows in the audited DB) this is a non-issue. No code change proposed now — §3.7 documents a threshold at which this should move to file/object storage with the DB holding only a path+hash pointer, so it doesn't quietly become a problem later.

### 2.7 File-level sprawl (secondary finding, deprioritized)

19 separate `.vox/store.db` files exist across worktrees and crate dirs (`Get-Item` sizes: 1.42-13.32MB each, one clear outlier at 13.32MB in `axis-frontend-remediation` worth a quick look if that worktree is still active). Deep audit was explicitly scoped to the main repo's copy only; this is addressed as a one-line addendum to the existing "prune stale worktrees" habit already in practice for this project, not a new mechanism.

## 3. Design: re-baseline + quarantine

### 3.1 Schema re-baseline

Bump `BASELINE_VERSION` 84 → 85. `schema/domains/*.rs` retains DDL for only the 116 LIVE tables. This becomes the default shape for every fresh `VoxDb::connect()`.

### 3.2 Quarantine module

New `crates/vox-db/src/schema/domains/quarantine.rs` holds the DDL for all 103 DEAD+DORMANT tables, each annotated with a comment: status (dead/dormant), the ops-file trail from the audit (or "none" for DEAD), and the subsystem cluster it belongs to. Gated behind a new Cargo feature on `vox-db`, `quarantine` (default off, matching `replication` in `Cargo.toml`; note `local` is actually **on** by default there, so it is not a default-off precedent to point to). This is a new pattern for the crate, not a copy of an established one: `local`/`replication` gate Rust connection/config code paths (`config.rs`, `facade/connect.rs`, `pool.rs`, `store/open.rs`) — no existing `cfg(feature = ...)` in the crate gates a conditionally-included SQL DDL fragment. `baseline_sql()` only includes the quarantine fragment when the feature is enabled.

Because the workspace uses `resolver = "2"`, Cargo unifies feature flags per (package, dep-kind, target), not per consuming crate — `default off` does not by itself guarantee the feature stays out of a production build. If any other workspace crate ever adds `vox-db` as a normal (non-dev) dependency with `features = ["quarantine"]`, that feature is unified into every normal-dependency use of `vox-db` in the same build, including production binaries built in the same `cargo build` invocation. Verifying `cargo build -p vox-db` and `cargo build -p vox-db --features quarantine` separately does not catch this; a workspace-wide build check (and ideally a restriction to dev-dependency-only use, or a CI check) is needed.

**This is a demotion, not a deletion.** The DDL isn't lost — it's one `git mv`-sized diff away from being live again, which lowers the collision risk for a branch actively wiring up, say, the Scientia pipeline: it just needs to enable the feature locally (or move its tables out of quarantine as part of landing that PR) rather than losing work. (This document does not compare quarantine against the simpler alternative of deleting the DDL outright and relying on `git log`/`git revert` to resurrect it — that comparison, and whether the ongoing Cargo-feature machinery is worth its cost over plain deletion, is not documented here.)

### 3.3 Test disposition (updated per your instruction — no blanket gating)

Every one of the 9 affected test functions (§2.4) gets an explicit disposition in the plan: **UPDATE** (trim the table list it asserts against, core test value remains for the still-live tables it also covers), **DELETE** (the test only validated now-quarantined schema with no residual value), or **MANUAL REVIEW — do not touch yet** (evidence of finished-but-unwired functionality: paired round-trip + schema tests, or real data rows). No `#[cfg(feature = "quarantine")]` test gating.

### 3.4 Existing-DB migration

Dropping all 103 quarantined tables from an existing local DB requires hand-written DDL (`auto_migrate` never drops automatically by design, confirmed in §2.5), but it **cannot be implemented as a single `Migration` entry** the way earlier drafts of this design assumed, for two independent reasons:

1. **The `COUNT(*)=0` safety check can't run inside `Migration.up_sql`.** `up_sql` is executed via `execute_batch`, which forbids row-returning statements — `migration.rs`'s own doc comment states up_sql "must not contain row-returning statements (no standalone SELECT)" for exactly this reason. The pre-check must instead be Rust-side orchestration: query `SELECT COUNT(*)` per quarantined table *before* any DROP is issued, and abort with a clear error naming the offending table(s) if any is non-zero, rather than embedding the check in the migration's SQL. Given §2.3, this pre-check is expected to actually fire on `developer_journey_definitions` and `history_entries` in the audited DB — those two are excluded from the DROP list entirely rather than tripping the check at run time, pending the manual-review decision.
2. **A custom `Migration` at a version above `BASELINE_VERSION` would permanently break normal connects.** `store/open.rs`'s `migrate()` treats any `schema_version` greater than `BASELINE_VERSION` as a fatal `StoreError::LegacySchemaChain` with no automatic recovery. `migration.rs`'s own module doc warns custom `Migration` rows should be used "only on ephemeral DBs, tests, or with a plan to re-baseline the file" for exactly this reason. So the DROP statements cannot be shipped as a separate, later-numbered `Migration` entry — they must be folded into the version-85 re-baseline transition itself (§3.1), applied once, ahead of or alongside the pre-check above, not as an ongoing user-defined `Migration`.

Neither the `Migration` struct nor `auto_migrate` has a rollback/reverse-migration mechanism — `migration.rs`'s `Migration` struct has only `version`, `name`, `up_sql`, with no `down_sql`/reverse field. If the DROP is misapplied there is no automated undo. Operationally, since local-first installs run this migration path automatically on startup, this document does not yet specify what happens to an install where the pre-check aborts (does the app refuse to start, retry every launch, or something else) — that recovery/support path is an open question the plan needs to answer, not something already mitigated.

### 3.5 Graduation path

To revive a quarantined table for real use: move its DDL out of `quarantine.rs` into the appropriate domain file, resolve any test dispositions marked MANUAL REVIEW back to UPDATE, bump the baseline version, and land the real caller in the same PR.

### 3.6 Recurrence prevention

A **VoxScript** tool (per this repo's automation policy — no new `.py`/`.sh`) that re-runs the LIVE/DORMANT/DEAD classification from §2.2 and reports newly-added tables with zero non-quarantine callers, in the same shape as the existing crate-edges ratchet. Exact wiring (CI gate vs. periodic report) is decided in the plan.

**Sensitive-table handling.** The report necessarily covers all 219 declared tables, including `clavis_account_secrets` (encrypted secrets vault) and `user_identities` (identity binding). Because the report is committed to git and re-generated periodically (§1), it must exclude — or at minimum aggregate-only, with no per-row content — these sensitive tables before being written to `graphify-out/table_usage_report.json`, so an uncontrolled row-count trend for the secrets vault or identity table doesn't leak into version-controlled history over time even without exposing ciphertext.

Because classification is driven purely by code-reference counting, a future refactor that removes the last outside-`vox-db` caller of a sensitive table would let the same mechanical tooling reclassify it DORMANT/DEAD and sweep it toward quarantine/drop with no human involved. The tool (and the manual-review step in §2.3) needs an explicit, name-based exception list for sensitive tables — starting with `clavis_account_secrets` and `user_identities` — that requires human sign-off before quarantine/drop, independent of the code-reference signal.

### 3.7 Storage-boundary guardrail (documentation only)

Document in `docs/src/architecture/where-things-live.md` (or a dedicated note) that `embeddings.vector` and any future large-BLOB columns should migrate to file/object storage with a path+hash pointer once real row count or per-row size crosses a stated threshold — not fixed now, since there's nothing to fix yet.

### 3.8 File sprawl (documentation-only addendum)

One-line addition to the existing worktree-cleanup practice: when pruning a stale worktree, its `.vox/store.db` goes with it — no separate mechanism.

## 4. Non-goals

- No wholesale deletion of the `store/ops_*.rs` Rust implementation code for DORMANT tables' CRUD functions. That's a separate, much larger dead-code sweep and out of scope here — this pass targets the schema surface only.
- No changes to `vox-server`'s Postgres-backed schema/migrations.
- No changes to the legacy `Migration` mechanism itself (§2.5 found it healthy).
- No new abstraction, feature flag matrix, or config system beyond the single `quarantine` Cargo feature.

## 5. Risks

| Risk | Mitigation |
|---|---|
| A branch in flight is actively wiring up a table this plan quarantines | Quarantine ≠ delete (§3.2); graduation path (§3.5) is a small diff, not lost work |
| Migration silently drops real data | Rust-side `COUNT(*)=0` pre-check per table before DROP, run outside the `Migration`/`execute_batch` mechanism since that mechanism can't run row-returning SQL (§3.4); two known exceptions pre-excluded (§2.3) |
| A custom `Migration` entry for the DROPs would leave `schema_version` above `BASELINE_VERSION`, permanently breaking normal `connect()` (`StoreError::LegacySchemaChain`) | Fold the DROPs into the version-85 re-baseline transition itself instead of a separate later-numbered `Migration` (§3.4) |
| The DROP is misapplied, or the pre-check aborts on a real install | **Not yet mitigated.** No rollback/reverse-migration mechanism exists in `migration.rs`, and this document does not specify what a local-first install does when the pre-check aborts (refuse to start, retry, or something else) — open question for the plan (§3.4) |
| A "dormant" table is actually finished-but-unwired, not abandoned | Manual-review bucket in test disposition (§3.3) catches the two clearest signals (paired tests, real data). The cross-check against `docs/src/architecture/` design docs must cover all 11 subsystem clusters listed in §2.2, not just a named subset — MENS/training in particular has multiple confirmed-active docs (e.g. `mens-training-ssot.md`, `voxmens-hub-and-spoke-ssot-research-2026-06-18.md`) and needs the same before-drop check as any other cluster |
| Sensitive tables (`clavis_account_secrets`, `user_identities`) drift into DORMANT/DEAD via a future refactor and get auto-quarantined | Not yet mitigated by the code-reference signal alone; needs an explicit name-based exception list requiring human sign-off (§3.6) |
| Audit script bit-rots and nobody re-checks before landing | Formalized as a VoxScript tool (§3.6), not a one-off — becomes the recurrence-prevention mechanism too |
