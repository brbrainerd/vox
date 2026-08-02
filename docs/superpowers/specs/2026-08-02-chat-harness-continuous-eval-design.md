# Chat Harness Continuous Eval & Regression Tracking — Design Spec

**Status:** Approved (brainstormed and approved in-session; see conversation history for the
question-by-question decisions this spec encodes).

## 1. Problem statement

This session's code review of the `claude/axis-chat-fixes` branch found several real bugs
(composer session-id pollution, send-lock ordering, an approval-gate bypass for chat-tagged
tasks, a multi-path privacy-filter bypass) that no existing test caught — because no existing
test exercises the real chat harness end-to-end. Investigation confirmed:

- **Test coverage today is entirely "was the right IPC command called," with fully mocked data.**
  `App.test.tsx`/`ChatSurface.test.tsx` mock `invoke` and assert on call arguments; nothing drives
  the real `Orchestrator`/`AiTaskProcessor`/MCP chat-tool stack. No test asserts the harness
  produces a *correct* answer.
- **`vox harness eval` (this session's own Task 20/25 work) is fully hermetic.** Its 7 golden
  tasks are pure functions or in-process checks; the one task that could exercise a real model
  (`live-model-smoke`) is a stub that always skips or deliberately fails. Nothing is persisted
  between runs — every invocation starts fresh with no history to compare against — and it is not
  wired into CI at all.
- **Telemetry (`vox-telemetry`/`vox-telemetry-otlp`) is mostly scaffolding.** Model-selection
  decisions are recorded to a local `research_metrics` table, but only when run through `vox-cli`
  (never `vox-gui`), and the chat-turn path itself emits nothing. The OTLP upload path is a
  literal stub.
- **Graphify is the wrong tool for this.** It is a static code-structure graph (files, calls,
  imports) with zero runtime/temporal dimension. A prior research doc
  (`docs/src/architecture/agent-harness-testing-and-regression-gating-research-2026-07-30.md`)
  considered graphify for exactly this use case and explicitly routed away from it toward
  OTel-shaped telemetry instead. This spec does not add a graphify corpus for runtime tracking;
  it builds a purpose-built eval-history data model instead (see §6).

**What this spec builds:** a system that (a) closes the specific test-coverage gap that let this
session's bugs through, and (b) gives Vox a persistent, queryable, historical record of chat
harness quality and model-selection behavior — so that changes to the harness or model catalog
can be verified as improvements (or caught as regressions) without a human manually chatting with
Vox and eyeballing the replies.

## 2. Goals

1. Add real-backend integration tests (no mocked `invoke`, no webview) that exercise the exact
   bug classes this session's review found, so they can never silently regress again.
2. Stand up a small, curated, live-model-calling eval corpus (~15-20 golden conversational tasks)
   covering plain chat replies, tool-calling/agentic tasks, privacy-mode enforcement, and
   cost-tier selection.
3. Score every eval run with a deterministic-check-first, LLM-judge-only-when-necessary hybrid.
4. Persist every eval run (and every model-selection decision made during it) to `vox-db`, so
   trends are queryable historically — not just "did this run pass," but "how has this trended."
5. Specifically track model cost-tier appropriateness over time (is Vox actually picking
   free/cheap models when a task doesn't need a premium one, or has that silently drifted).
6. Run the live eval nightly via a scheduled GitHub Actions workflow on the existing self-hosted
   runner, and make the results visible to any developer's local Vox Axis GUI and CLI without
   requiring a live connection to the CI runner.
7. When a regression is detected, surface enough metadata (git SHA, changed files since the last
   run, config/model-catalog version) that a human can quickly locate the cause — without
   automatically bisecting (that's future scope, see §11).

## 3. Non-goals

- **Not** building a general-purpose observability/APM platform. This is scoped to the chat
  harness and model selection specifically.
- **Not** finishing `vox-telemetry-otlp`'s OTLP upload stub or wiring a remote collector/Grafana.
  That's explicitly deferred (§11) — this spec's persistence layer is `vox-db`, not OTel.
- **Not** building full Tauri E2E (`tauri-driver`/WebDriver driving the real GUI binary + webview).
  Phase 1 is real-backend-Rust-only; full Tauri E2E is named as an explicit, non-blocking follow-up
  (§11).
- **Not** automatic regression bisection (re-running eval against intermediate commits to pinpoint
  a cause). Metadata capture + manual diff view only, per the approved design.
- **Not** covering every `TaskCategory` variant in the initial golden corpus — scoped to
  chat/model-selection scenarios (§7).

## 4. Architecture overview

Four phases, each independently shippable, sharing one data model:

```
Phase 1: Backend integration tests (crates/vox-integration-tests)
    -> real Orchestrator + AiTaskProcessor + MCP chat tools, no mocks, no webview
    -> regression tests only (pass/fail), no historical tracking needed

Phase 2: Live eval harness (crates/vox-cli's `vox harness eval`, extended)
    -> new golden-task category: real model calls against a curated corpus
    -> hybrid scoring: deterministic checker first, LLM-judge ensemble as fallback
    -> writes results through vox-db (Phase 3's tables)

Phase 3: Persistence (crates/vox-db)
    -> harness_eval_run / harness_eval_task_result / model_selection_event tables
    -> modeled directly on the existing research_eval_runs/research_eval_samples pattern

Phase 4: Scheduling + sync
    -> new nightly GitHub Actions workflow, self-hosted runner, `vox harness eval --live`
    -> git-committed JSONL export/sync so local dev machines see CI's history via `git pull`
       (no new server dependency, consistent with this repo's local-first conventions)

Phase 5: Surfacing
    -> `vox harness eval report` / `vox harness eval history` CLI commands
    -> new "Harness Health" surface in Vox Axis GUI, reading the same vox-db tables
```

## 5. Phase 1 — Backend integration tests

**Location:** `crates/vox-integration-tests/tests/chat_harness_regression_test.rs` (new file).
This crate already hosts exactly this style of test — `orchestrator_e2e_test.rs` (682 lines)
already spins up a real `Orchestrator` in-process, with forensic per-test logging under
`target/e2e-logs/`, a watchdog thread, and documented multi-worker-thread `#[tokio::test]`
gotchas. The new file follows that file's established conventions directly (forensic logger,
`E2E_TEST_TIMEOUT`/`PHASE_TIMEOUT`/`WATCHDOG_INTERVAL` constants, `worker_threads >= 2`) rather
than inventing new test infrastructure.

(Note: `chatbot_integration_test.rs`, despite its name, tests the **Vox language compiler** on a
sample "chatbot" program — unrelated to the GUI chat feature. Do not confuse the two or extend
that file.)

**What it tests**, each as a real end-to-end pass through the actual production code paths (not
re-implementing the logic in the test):

1. **Session-id isolation.** Submit a background task the way `/spawn`/Deploy-skill/the
   composer's background-task mode do (distinct `bg-task-*` session id) and assert the
   orchestrator's `chat_history:{session_id}` context store for the *real* active chat session is
   never touched.
2. **Send-lock ordering.** Two concurrent `chat_send_message`-equivalent calls for the same
   session; assert exactly one persisted user-message row with no orphan, and the second caller
   observes a clean rejection rather than a silently dropped write.
3. **Gate-cascade applies uniformly.** Submit a `TaskCategory::Chat` task whose completion
   attestation would fail the approval/trust/harness gates under any other category, and assert
   it is gated identically — not skipped.
4. **Privacy filter holds across all selection paths.** Under `VOX_INFERENCE_PRIVACY=local_only`,
   with no local model registered, assert task submission fails closed (per this session's
   `runtime.rs` fix) rather than falling back to Cascade; separately, with a local model
   registered, assert the premium-alias and `decide()` paths both still exclude a cloud
   candidate (not just `best_for_internal`'s direct callers).

Each of these tests is a **direct regression test for a bug this session found and fixed** — the
explicit intent is that if any of these four fixes were reverted, this file would fail immediately
in CI, unlike today.

**Test-double policy:** the orchestrator, task dispatch, model registry, and gate logic are all
real (in-process, same as `orchestrator_e2e_test.rs`). The only permitted test double is the LLM
provider boundary itself (a wiremock-style stub HTTP server, matching the existing pattern already
used in `vox-orchestrator-mcp`'s own `chat_message_envelope_includes_latency_ms`-style tests) —
this keeps Phase 1 hermetic and CI-safe (every commit, not just nightly), while still exercising
every layer of real orchestration logic around that one boundary.

## 6. Phase 2 — Live eval harness

**Location:** extends `crates/vox-cli/src/commands/harness/eval.rs`. Adds a new golden-task kind
alongside the existing hermetic ones — these make real calls through the real chat harness
(`vox_orchestrator_mcp::chat_tools::chat`, the same code Phase 1 drives, but here against real
providers) rather than in-process library calls.

### 6.1 Golden task corpus (initial scope: ~15-20 tasks)

- **Plain chat replies (≈5-6 tasks):** short factual Q&A with a checkable answer (e.g. "what is
  2+2 in one word" → deterministic string/regex match on the reply). Exercises the synchronous
  `chat_send_message` path exactly as the GUI composer's "Quick chat" mode does.
- **Tool-calling / agentic tasks (≈5-6 tasks):** a task requiring at least one real tool call with
  a checkable end-state (e.g. "read file X and report its line count" → deterministic check against
  the actual file). Exercises the `AiTaskProcessor` pipeline, matching what `TaskCategory::Chat`
  and background-task-mode submissions now both run through post-this-session's-fixes.
- **Privacy-mode tasks (≈2-3 tasks):** run once under `VOX_INFERENCE_PRIVACY=local_only`, assert
  the model actually used (recorded via the `model_selection_event`, §7) is a local provider —
  a live-model version of Phase 1's item 4, catching drift that only manifests against real
  provider/model catalog state (e.g. a newly-added cloud model accidentally matching a local-only
  candidate filter).
- **Cost-tier tasks (≈2-3 tasks):** a deliberately trivial task (low complexity, no
  precision-critical signal) submitted with no explicit tier override; assert the selected model's
  cost tier (§8) is Free or Cheap, not Premium.

Each task is authored as a small Rust struct: `{ id, prompt, category, checker }`, where `checker`
is an enum `Deterministic(fn(&EvalTurnResult) -> CheckOutcome) | LlmJudge { rubric: &str }`. This
keeps the corpus statically typed and colocated with the harness code, mirroring the existing
`golden_tasks()` pattern in the same file rather than inventing a new YAML/JSON DSL.

### 6.2 Scoring — deterministic + LLM-judge hybrid

Per the harness-testing research doc's own recommendation (§4 of that doc):

- **Deterministic tasks** (most of the corpus, by design — see §6.1) are checked with a plain Rust
  predicate against the harness's actual output/end-state. No judge involved, no judge-bias risk.
- **Open-ended tasks with no deterministic check** (a minority — none in the initial ~15-20, but
  the scoring path supports it for future expansion) are scored by an **ensemble of judge calls**
  (odd number, e.g. 3) with majority vote, plus a style-invariance check (the same rubric applied
  to a paraphrased/reordered version of the same reply) to catch a judge swinging on stylistic
  artifacts rather than substance — directly per the research doc's finding that judges can swing
  up to 98% on style alone.
- Every task additionally runs `--samples N` times (reusing the existing pass^k mechanism already
  built in this file) — a task only counts as passing if **all N samples pass**, not just one.

### 6.3 Cost & safety controls

- Live tasks only run when explicitly requested (`vox harness eval --live`), never as part of the
  default hermetic gate — mirrors the existing `VOX_HARNESS_EVAL_LIVE` gating convention but as an
  explicit CLI flag rather than only an env var, so CI's scheduled workflow (§9) can invoke it
  directly and unambiguously.
- If required API keys/local models aren't available for a given task, that task reports `Skipped`
  (not `Failed`) — consistent with the existing `Skipped` status semantics in this file, and with
  how `key_gate`/`premium_alias` selection tests already treat missing keys elsewhere in this
  codebase.
- A hard per-run cost ceiling (config value, default **$0.50/run**) aborts the remaining live
  tasks if exceeded mid-run, logging which tasks were skipped as a result — prevents a runaway
  loop (a buggy retry, a misconfigured `--samples`) from generating unbounded spend. Chosen to be
  comfortably above the expected real cost of a ~15-20-task, mostly-free/cheap-model corpus, while
  still catching a genuine runaway.

## 7. Phase 3 — Persistence (`vox-db`)

New tables in `crates/vox-db/src/schema/domains/scientia.rs` (same domain file as the closely
analogous `research_eval_runs`/`research_eval_samples` pair this schema is modeled on):

```sql
-- One row per `vox harness eval --live` invocation.
CREATE TABLE IF NOT EXISTS harness_eval_run (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL UNIQUE,
    triggered_by        TEXT    NOT NULL,      -- 'ci-nightly' | 'local' | 'ci-manual'
    git_sha             TEXT    NOT NULL,
    git_branch          TEXT    NOT NULL,
    changed_files_json  TEXT,                  -- files changed since the previous run's git_sha
    config_version      TEXT,                  -- model-catalog / routing.yaml version marker
    samples_per_task    INTEGER NOT NULL,
    task_count          INTEGER NOT NULL,
    pass_count          INTEGER NOT NULL,
    fail_count          INTEGER NOT NULL,
    skip_count          INTEGER NOT NULL,
    total_cost_usd      REAL    NOT NULL DEFAULT 0.0,
    started_at_ms       INTEGER NOT NULL,
    finished_at_ms      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_harness_eval_run_time
    ON harness_eval_run(started_at_ms);

-- One row per golden task per run (per-sample detail rolled into pass_samples/total_samples).
CREATE TABLE IF NOT EXISTS harness_eval_task_result (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    task_id             TEXT    NOT NULL,      -- matches the Rust golden-task struct's `id`
    category            TEXT    NOT NULL,      -- 'chat' | 'tool_calling' | 'privacy' | 'cost_tier'
    checker_kind         TEXT    NOT NULL,      -- 'deterministic' | 'llm_judge'
    status              TEXT    NOT NULL,      -- 'pass' | 'fail' | 'skip'
    pass_samples        INTEGER NOT NULL,
    total_samples        INTEGER NOT NULL,
    latency_p50_ms       INTEGER,
    cost_usd             REAL,
    failure_detail        TEXT,                 -- present when status='fail'; first failing sample's reason
    recorded_at_ms       INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_harness_eval_task_result_run
    ON harness_eval_task_result(run_id, task_id);

-- One row per model-selection decision made during an eval run (also usable outside eval later).
CREATE TABLE IF NOT EXISTS model_selection_event (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    task_id             TEXT    NOT NULL,
    model_id            TEXT    NOT NULL,
    cost_tier           TEXT    NOT NULL,      -- 'free' | 'cheap' | 'premium' (see 8.1)
    selection_reason     TEXT    NOT NULL,      -- SelectionReason::Display() string
    was_privacy_gated     INTEGER NOT NULL,      -- 0/1: was VOX_INFERENCE_PRIVACY=local_only active
    recorded_at_ms       INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_model_selection_event_run
    ON model_selection_event(run_id, model_id);
```

`run_id` is a stable string (e.g. `{git_sha short}-{timestamp}`), matching the existing
`research_eval_runs.run_id TEXT UNIQUE` convention, so JSONL sync (§9) can upsert idempotently by
primary key without a separate dedup pass.

`changed_files_json` is computed once per run as `git diff --name-only <previous_run's git_sha>..<this_sha>`
— this is the "metadata capture" half of regression cause-tracking (§10); no automatic bisection.

## 8. Model-selection cost-tier tracking

### 8.1 Cost-tier classification

A pure function `cost_tier_for(model: &ModelSpec) -> CostTier` (new, in
`crates/vox-orchestrator/src/models/`, alongside `SelectionReason`):

- `Free` — `model.is_free == true`.
- `Cheap` — not free, but `cost_per_1k` below a configured threshold (reuses the existing
  cost-preference threshold conventions already present in `models/scoring.rs` rather than
  inventing a new number).
- `Premium` — everything else (matches `select_via_premium_alias`'s existing notion of a
  "premium" pick).

### 8.2 What gets tracked and how drift is surfaced

Every `model_selection_event` row records the tier and the `SelectionReason` string for that
decision. The GUI/CLI surface (§10) computes, per time window (e.g. trailing 7/30 runs):

- **Free/cheap ratio** across all non-privacy-forced selections — the headline "are we still
  picking economical models when appropriate" metric.
- **Ratio trend** — flagged as a regression candidate when the free/cheap ratio drops by more than
  a configurable threshold between consecutive runs (paired with the run's `changed_files_json`
  for investigation, per §10).
- **Per-cost-tier-task pass rate** — did the dedicated cost-tier golden tasks (§6.1) themselves
  pass (i.e. did a trivial task actually get a non-premium pick), independent of the ratio-drift
  signal above.

## 9. Phase 4 — Scheduling + sync

**Scheduled workflow:** new `.github/workflows/harness-eval-nightly.yml`, cron-triggered, running
on the existing self-hosted runner (same runner infra already used by this repo's other CI jobs).
Steps: checkout, build `vox-cli`, run `vox harness eval --live`, which writes directly to the
runner's local `vox-db` SQLite file.

**Local visibility via git-committed JSONL sync (no new server dependency):** after the eval run,
a `vox harness eval publish` step exports every `harness_eval_run`/`harness_eval_task_result`/
`model_selection_event` row created since the last publish into an append-only, auto-generated
JSONL file at `docs/harness-eval-history/runs.jsonl` (clearly marked as auto-generated, per this
repo's existing convention of never hand-editing generated docs — see AGENTS.md), and commits +
pushes it via the same bot-commit mechanism other automated repo updates already use. Any
developer's local `vox-gui`/`vox harness eval history` then ingests this file (idempotent upsert
keyed by `run_id`) on the next `git pull`/GUI launch — no live network call to the CI runner is
ever required. This keeps the design consistent with this repo's stated local-first philosophy
(existing `docs/src/architecture/project_local_first_ci_*` line of work) instead of introducing a
dependency on the separately-tracked, not-yet-production `vox-server` centralized-telemetry effort.

**Local ad-hoc runs:** `vox harness eval --live` also works unpublished, purely locally (e.g. a
developer sanity-checking a branch before merge) — it still writes to the local DB and is visible
in the local GUI/CLI immediately, it just isn't published to the shared JSONL unless the developer
explicitly runs `vox harness eval publish` (kept manual for local runs to avoid noisy/duplicate
history entries from every dev's ad-hoc testing).

## 10. Phase 5 — Surfacing

### 10.1 CLI

- `vox harness eval history [--limit N] [--category X]` — tabular list of recent runs with
  pass/fail/skip counts, cost, free/cheap ratio.
- `vox harness eval report [--since <run_id|date>]` — a single run's full detail, or a trend
  summary across a range: pass-rate trend, free/cheap-ratio trend, and (if a regression is
  detected per §8.2/§10.3) the flagged run(s) with their `changed_files_json` and `git_sha` range.

### 10.2 GUI — "Harness Health" surface

New surface in Vox Axis (same tier as existing surfaces like Policies/Coverage), reading the same
`harness_eval_*`/`model_selection_event` tables via the existing `GuiDbPool` pattern (no new IPC
plumbing style — mirrors how chat sessions/model scoreboard data already reach the GUI). Contents:

- A trend chart: pass rate and free/cheap model-selection ratio over the last N runs.
- A recent-runs table (mirrors the CLI's `history` output).
- A regression banner when the most recent run's pass rate or free/cheap ratio dropped beyond
  threshold vs. the prior run, showing the git SHA range and changed-files list for that gap.
- A per-task-category breakdown (chat / tool-calling / privacy / cost-tier) so a category-specific
  regression (e.g. only privacy-mode tasks failing) is visible at a glance, not buried in an
  aggregate pass rate.

### 10.3 Regression detection logic

Computed at read time (no separate background job) by the CLI/GUI query layer, comparing the two
most recent runs (or a requested range):

- Pass-rate drop beyond a configured threshold (default: any newly-failing task, or an aggregate
  drop >10 percentage points) → flagged.
- Free/cheap ratio drop beyond a configured threshold (default: >15 percentage points) → flagged.
- Each flagged regression surfaces: the two run's `git_sha`s, `changed_files_json` for the range,
  and the specific `harness_eval_task_result` rows that flipped from pass to fail (or a tier that
  flipped from Free/Cheap to Premium) — this is the "simple diff view," not automatic bisection.

## 11. Explicitly deferred (named, not silently dropped)

- **Full Tauri E2E** (`tauri-driver`/WebDriver driving the real GUI binary + webview) — Phase 1 is
  real-backend-only; this is a real gap Phase 1 does not close (a bug purely in frontend→IPC
  wiring, e.g. a button not calling the right command at all, could still slip through). Scoped as
  a follow-up project, not blocking this spec.
- **Finishing `vox-telemetry-otlp`'s OTLP upload stub** / wiring a remote collector or Grafana —
  this spec's persistence is `vox-db`, independent of that unfinished pipeline.
- **Automatic regression bisection** — deferred per the approved "metadata capture + simple diff
  view" scope; re-running eval against intermediate commits automatically is real future value but
  real added cost/complexity.
- **Broader `TaskCategory` coverage** in the golden corpus (Review, Debugging, etc. beyond the
  chat/model-selection scope) — the corpus can grow incrementally using the same infrastructure;
  not blocking initial ship.
- **Full selection-candidate audit trail** (recording every candidate considered and rejected, not
  just the winner) — the approved scope is winner-only (`model_selection_event`); richer
  candidate-level recording is a natural, additive future extension of the same table shape.

## 12. Testing strategy for this system itself

- Phase 1's own tests are, by construction, the regression coverage for the bugs that motivated
  this spec (§5) — no additional meta-testing needed there.
- Phase 2's scoring logic (deterministic checkers, judge-ensemble majority vote, pass^k rollup) is
  unit-tested with fixture `EvalTurnResult`s — no live calls needed to test the scoring math itself,
  matching how `score_eval`'s existing fold logic is already tested in `eval.rs`.
- Phase 3's schema gets round-trip tests in `vox-db` (write a run + task results + selection
  events, read them back), mirroring the existing `research_eval_runs` test conventions in that
  crate.
- Phase 4's JSONL export/publish and ingest-on-pull logic gets a unit test proving idempotent
  upsert (publishing the same run twice, or ingesting the same JSONL twice, produces no duplicate
  rows) — this is the correctness property the whole local-first sync design depends on.
- Phase 5's CLI/GUI regression-detection logic (§10.3) gets unit tests against fixture run pairs
  covering: no regression, pass-rate regression, cost-tier-ratio regression, and both at once.
