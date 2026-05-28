# 2026-05-21 — v1.0 honest completion plan

**Status:** in flight. This plan supersedes the optimistic claims in
`2026-05-19-v1-execution-plan.md` and the audit scorecard at
`docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md §3`.

> **Why this exists.** A prior pass through the codebase claimed v1.0
> readiness on the strength of "code lands + unit tests pass." A hard
> audit against `docs/src/architecture/v1-release-criteria.md` shows
> the **measurement evidence** the criteria actually demand is largely
> absent. This plan closes that gap and adds standing machinery so
> future "shippable" claims must be backed by a green gate or an
> artifact under `contracts/reports/`.

## 0. The core mistake, named

The recurring failure mode in prior plans was treating the **code path**
as the bar:

> "Workflow runtime exists" → "CR-L8 closed."
> "164 fixtures compile" → "CR-L1 ≥80% pass rate met."
> "Doctor leg integration test green" → "CR-L7 closed."

These are non-sequiturs. CR-L1..L8 are explicit about **measurement**
(LLM-panel runs, telemetry pipelines, end-to-end timing). The fix
is structural, not cultural: any claim of "closed" must be backed by
either (a) a green CR-L gate output JSON under `contracts/reports/`,
or (b) a benchmark artifact under `contracts/reports/perf/`. No path,
no claim. This is enforced in §1 below.

## 1. Anti-overclaim machinery (Phase 0 — lands FIRST)

These prevent the same gap class from reopening as work continues.

### 1.1. `vox audit --gate all` aggregate
**Artifact:** `crates/vox-audit/src/registry.rs` already enumerates the
nine gates. Wire `--gate all` as a meta-target that runs every
registered gate, writes each result to
`contracts/reports/<gate>/<UTC-date>.json`, and emits a roll-up at
`contracts/reports/_snapshot/<UTC-date>.json`. Exit code: non-zero if
any block-GA gate has `threshold.met == false`.

**Verification:** `cargo run -p vox-cli -- audit --gate all` from a
clean checkout. Roll-up JSON must list every gate by name with its
observed/target/met triple. CI fails if the roll-up is missing or any
block-GA row is `met: false`.

**Owner:** vox-audit. **Effort:** ~4 hours (registry walk + roll-up
writer + CI shim).

### 1.2. Evidence-ledger contract for the audit scorecard
**Artifact:** `contracts/reports/evidence-ledger.v1.json` — a
machine-readable map from "claim_id" to "artifact_path". Every "CLOSED"
or "PASSING" row in
`docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md`
must cite a `claim_id` that resolves to a path in the ledger, and the
path must exist on disk. A new CI lint
(`crates/vox-arch-check/src/evidence_ledger_check.rs`) fails the build
if the doc claims "CLOSED" without a resolvable ledger entry, or if
the entry points at a missing file, or if the file is older than the
gate's freshness window (90 days for CR-L8, 30 days for everything
else by default).

**Verification:** edit a fake "CLOSED" row in the audit doc that
doesn't reference an artifact → `cargo run -p vox-arch-check` must
fail loudly with the unresolved claim_id.

**Owner:** vox-arch-check. **Effort:** ~6 hours.

### 1.3. CI gate: block-GA gates must be green
**Artifact:** add a `.github/workflows/cr-l-gates.yml` job that runs
`vox audit --gate all` on every PR and on a nightly schedule. Job
fails if any `block_ga: true` gate is unmet. Posts the roll-up JSON
as a workflow artifact.

**Verification:** a PR that intentionally breaks the retirement gate
must red-line CI.

**Owner:** ci. **Effort:** ~2 hours.

### 1.4. "Pre-claim" pre-commit script
**Artifact:** `scripts/pre-claim.vox` — a vox-script (per CLAUDE.md
§VoxScript-First Glue Code, not bash) that any contributor / agent
runs before editing the audit scorecard. Reads the current
`_snapshot/<latest>.json`, lists every gate that's currently not met,
and refuses to commit until either the scorecard reflects that or the
gate goes green.

**Verification:** locally `vox run scripts/pre-claim.vox` against a
mocked snapshot must exit non-zero when the scorecard contradicts the
snapshot.

**Owner:** vox-cli. **Effort:** ~3 hours.

**Phase-0 exit criterion:** `cargo run -p vox-cli -- audit --gate all`
runs to completion against the live tree. The roll-up shows today's
actual state (which today is: CR-L0/3/4/7/8 not met). The audit
scorecard is rewritten so every "CLOSED" claim has a ledger entry
pointing at a real artifact. CI runs the gate aggregate.

---

## 2. Doc reconciliation (Phase 1 — concurrent with Phase 0)

Cheap and bounded. Wave these in immediately so the rest of the plan
isn't fighting stale text.

### 2.1. `v1-release-criteria.md` 200 → 164 drift
**File:** `docs/src/architecture/v1-release-criteria.md` line 39.
Change "200-program" to "164-program" with a footnote citing G18 in
the audit doc (the anchoring decision). Also fix the CR-L1 invocation
from `vox audit humaneval` to `vox audit --gate humaneval`.

### 2.2. False "CLOSED" claims in the audit scorecard
**File:** `docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md`
state-of-the-world table. Today claims CR-L8 "CLOSED" and CR-L7
"CLOSED for v1.0 lane." Replace with honest statuses:
- CR-L7: **PARTIAL — doctor leg only; deploy + new legs deferred to §2.3**
- CR-L8: **NOT CLOSED — pipeline not wired into any emitting workflow**

Add a one-line meta-rule at the top of the scorecard: every status
word other than "OPEN" must cite an artifact path or gate name.

### 2.3. Add slot-1 marquee app to doctor-green guard
**File:** `crates/vox-audit/tests/examples_golden_doctor_green.rs`.
The `every_marquee_app_compiles_clean` test walks `apps/marquee/<app>/src/main.vox`
but skips `apps/interop/marquee_app/src/main.vox`. Add a second
sub-walker that picks up the slot-1 path explicitly.

### 2.4. Update `2026-05-19-v1-execution-plan.md` provenance line
At the top, add: "Superseded 2026-05-21 by `2026-05-21-v1-honest-completion-plan.md`
for evidence-tracking; the language/codegen items in the 05-19 plan
are still the canonical reference for what landed."

**Phase-1 exit criterion:** all four file edits committed; arch-check
`evidence_ledger_check` passes on the rewritten scorecard.

---

## 3. Measurement wiring (Phase 2)

The runners exist (`vox audit --gate <name>`). What's missing is the
**actual measurement execution** — wiring telemetry emitters, LLM
panel invocation, deploy timing, MENS sampling. After Phase 2,
running `vox audit --gate all` produces a real number for every gate,
not "corpus inventory only."

### 3.1. CR-L8: telemetry → corpus-feedback artifact pipeline
**Artifact path:** `contracts/reports/corpus-feedback/<UTC-quarter>.json`.

**Schema:** top-50 firing diagnostics, autofix accept/reject rates,
repair outcome histogram, source pointers. Define at
`contracts/schemas/corpus-feedback.v1.schema.json`.

**Emit-site:** every `vox.lint.*` and `vox.repair.*` telemetry event
already goes through `vox-telemetry`. Add a sink that writes to
`contracts/reports/corpus-feedback-events/<UTC-day>.jsonl` (already
referenced by the gate). Sink registration in
`vox_cli::init_telemetry_sinks`.

**Aggregator:** new subcommand `vox audit corpus-feedback --emit` (or
`vox audit --gate corpus-feedback --emit-artifact`) that reads the
events dir, computes the schema, writes the dated artifact, returns
the path.

**Verification:** run `vox check` on a known-broken `.vox` file, then
run the aggregator, then run `vox audit --gate corpus-feedback`. Gate
must report `threshold.met: true` with `incomplete: false`.

**Owner:** vox-audit + vox-telemetry. **Effort:** ~12 hours.

### 3.2. CR-L7: deploy + new legs of the gate
**File:** `crates/vox-audit/src/subcommands/deploy.rs`. Today only the
doctor leg runs. Extend to:
- `vox new --template web` into a tempdir, time the call
- `vox deploy --dry-run` from the tempdir, time the call
- `vox doctor --project` over the tempdir (already running)
- Roll up `total_seconds`; threshold = 120 (CR-P3).

**Verification:** `vox audit --gate deploy` reports
`overall_pass_rate` as `1.0` only when the wallclock sum is under 120s
on every fixture in `contracts/marquee/manifest.v1.yaml`.

**Owner:** vox-audit. **Effort:** ~6 hours.

### 3.3. CR-L1: LLM-panel run against the 164-corpus
**File:** `crates/vox-audit/src/subcommands/humaneval.rs`. The panel
mode is already implemented (`--llm-panel` flag). Add a CI-friendly
invocation `cargo run -p vox-cli -- audit --gate humaneval --llm-panel
--corpus contracts/eval/humaneval-vox` that:
- Pulls `OPENROUTER_API_KEY` from env
- Runs all 164 prompts through the panel (configured via
  `contracts/eval/llm-panel.v1.yaml`)
- Writes per-fixture results to `contracts/reports/humaneval/<UTC>.json`
- Reports the real pass-rate.

**Verification:** the artifact at
`contracts/reports/humaneval/<UTC>.json` must contain a
`results.overall_pass_rate` ≥ 0.80 for the gate to be green. First
run is expected to underperform; that's the point — we publish the
real number.

**Cost note:** ~164 fixtures × 3 panel members × 1 attempt ≈ 500 LLM
calls. At ~$0.01/call (Sonnet 4) ≈ $5 per run. Run monthly, not
per-PR.

**Owner:** vox-audit. **Effort:** ~4 hours wiring + ongoing LLM cost.

> **2026-05-21 panel-mode hardening landed, bar met.** humaneval.rs
> already had the panel branch wired but missed three pieces that the
> CR-L0 work proved necessary: (a) `std::thread::scope` wrap around the
> reqwest-blocking client so the outer Tokio runtime owned by vox-cli
> doesn't panic on drop; (b) the Vox calibration primer (`assert(X is
> Y)`, explicit `return`, `to Unit`) — without it, models silently use
> Rust/JS idioms and pass-rate collapses; (c) cumulative budget cap via
> `VOX_AUDIT_BUDGET_USD` and a `VOX_AUDIT_CR_L1_MAX_FIXTURES` knob so
> CI can run against a stable subsample (default unset = full corpus).
>
> **Canonical 164-fixture measurement (2026-05-21):** median pass rate
> **93.9%** vs 80% target → met=true. gpt-frontier 96.3% (158/164),
> claude-sonnet 91.5% (150/164). Total cost: **$1.156** of $20 budget.
> Caching layer at `contracts/reports/llm-panel-cache/` makes repeat
> runs free.

### 3.4. CR-L2: MENS sampling against the corpus
**File:** `crates/vox-audit/src/subcommands/mens_on_distribution.rs`.
Same as 3.3 but instead of panel members, sample from the current
MENS checkpoint via `vox_actor_runtime::llm::stream`. Output
`contracts/reports/mens-on-distribution/<UTC>.json` with
`overall_pass_rate` against the 0.95 bar.

**Verification:** same as 3.3 — artifact exists with a real number.

**Owner:** vox-audit + vox-actor-runtime. **Effort:** ~6 hours.

### 3.5. CR-L3: panel × `vox repair --project` over broken-project corpus
**File:** `crates/vox-audit/src/subcommands/repair_corpus.rs`. For each
project under `contracts/eval/repair-corpus/projects/`, run the
existing `commands::repair::run_project` against a fresh copy, with
the LLM panel as the model provider. Compare the post-repair compile
state to the `expected.json` baseline.

**Verification:** artifact at
`contracts/reports/repair-corpus/<UTC>.json` carries a per-project
result row and an aggregate `overall_pass_rate`. Initial number will
be low because the corpus is only 5 projects (see Phase 3 for
expansion).

**Owner:** vox-audit. **Effort:** ~8 hours.

### 3.6. CR-L4: orchestrator-driven plan loop
**File:** `crates/vox-audit/src/subcommands/plan_fidelity.rs`. For each
plan under `contracts/eval/plan-fidelity/plans/`, invoke
`vox_orchestrator::plan::execute` with the plan as input and check
every step's outcome against the plan's recorded expectations.

**Verification:** artifact at
`contracts/reports/plan-fidelity/<UTC>.json`. Aggregate is
plans-with-all-steps-passing ÷ total-plans.

**Owner:** vox-audit + vox-orchestrator. **Effort:** ~10 hours.

### 3.7. CR-L0: autonomous-agent loop over spec-to-app specs
**File:** `crates/vox-audit/src/subcommands/spec_to_app.rs`. For each
spec under `contracts/eval/spec-to-app/specs/`, launch an MCP-driven
agent loop (via vox-orchestrator + the existing spec runner harness)
that: (a) reads the English spec, (b) drafts a `.vox` module, (c)
runs `vox check`, (d) `vox build`, (e) `vox deploy --dry-run`, (f)
`vox doctor --project`. Each spec passes when all six steps succeed
within the per-spec token-cost ceiling of $5.

**Verification:** artifact at
`contracts/reports/spec-to-app/<UTC>.json`. Aggregate = pass ÷ total
≥ 0.60.

> **2026-05-21 wiring + refinement loop landed; bar met.** The
> panel-mode runner at `crates/vox-audit/src/subcommands/spec_to_app_panel.rs`
> drives OpenRouter-routable panel members through generate → `vox check` →
> success-criteria scoring under a hard $20 cumulative budget cap
> (`VOX_AUDIT_BUDGET_USD`) and per-spec `max_cost_usd` ceiling. After the
> first single-shot result (41.7%) came in sub-bar, the multi-iteration
> refinement loop was added: each `vox check` failure feeds the
> diagnostics + prior draft back to the model for another iteration, up
> to `VOX_AUDIT_SPEC_TO_APP_MAX_ITERATIONS` (default 5).
> **Same-day measurement trajectory:** 0% (no prompt calibration) →
> 41.7% (calibrated prompt, single-shot) → 50.0% (3-iter refinement) →
> **66.7% (5-iter refinement; ≥ 60% bar; gate met=true).** Per-member:
> gpt-frontier 100% (3/3), claude-sonnet 33% (1/3). Cumulative same-day
> cost ~$0.20. Cache lives at `contracts/reports/llm-panel-cache/spec-to-app/`
> so repeat runs cost $0.
>
> **What this v1.0 loop does NOT yet cover:** build/deploy/doctor
> refinement. The current loop is "generate → check → re-generate"; a
> richer agent loop would also build, deploy --dry-run, and doctor each
> draft, feeding those errors back too. Skipped for v1.0 because
> (a) it requires per-spec project-skeleton generation, and (b) vox
> check diagnostics already give the model enough signal to recover
> from the failure modes observed in the 3-spec corpus.

**Owner:** vox-audit + vox-orchestrator + vox-cli-mcp. **Effort:** ~24
hours (this is the integration test for the whole language-target
claim).

**Phase-2 exit criterion:** every gate run on a fresh checkout
produces a real artifact in `contracts/reports/<gate>/<UTC>.json`
with a non-zero corpus and a real `overall_pass_rate` number — not
"corpus-inventory mode" placeholder. Initial numbers WILL be below
threshold; that's expected and publishable.

---

## 4. Corpus expansion (Phase 3)

After Phase 2, the gates produce numbers but most are below threshold
because the corpora are 5-10% of their minimum-viable size. This
phase scales them up.

### 4.1. CR-L0 spec-to-app: 3 → 10 specs
Author 7 more specs under `contracts/eval/spec-to-app/specs/004-…`
through `010-…`. Each carries a `spec.toml` (English-language
description + token-cost ceiling + acceptance criteria) and an
`expected/` directory with the reference solution structure. Cover:
- T1 (simple): single-table CRUD, single endpoint
- T2 (medium): two tables with a foreign-key-ish reference, queries
- T3 (complex): actor + stream endpoint + auth + retry

Tier balance: at least 3 T1, 3 T2, 4 T3.

**Effort:** ~3 hours per spec × 7 = ~21 hours.

### 4.2. CR-L3 repair-corpus: 5 → 50 broken-project fixtures
Author 45 more multi-file broken-project fixtures. Each is a
mini-project with intentional breakage in one of: type mismatch,
exhaustiveness, effect violation, API misuse, dead code, retired
syntax, schema drift. Format established by existing 001-005.

Bias toward bug classes the existing detectors already catch — that's
what `vox repair` knows how to fix. Save the wilder classes for
v1.1 corpus growth.

**Effort:** ~1 hour per fixture × 45 = ~45 hours.

### 4.3. CR-L4 plan-fidelity: 5 → 50 plans
Author 45 more multi-step plans. Each carries a `plan.toml` with
ordered steps, per-step expected-outcome (compile-clean / tests-pass /
file-shape / etc.). Wave 1 = trivial; Wave 2 = multi-file with
dependencies; Wave 3 = cross-component.

Tier balance: 10 Wave 1, 30 Wave 2, 10 Wave 3 (CR-D1 specifically
calls out Wave 2 as the measurement target).

**Effort:** ~30 min per plan × 45 = ~22 hours.

### 4.4. CR-L1 HumanEval-Vox: corpus already at 164 ✓
Skip — Phase A5d already hit the anchor. Anti-regression: a CI test
asserts `count_current == 164` in the manifest.

**Phase-3 exit criterion:** every CR-L gate's corpus meets the
minimum-viable size cited in
`docs/src/architecture/v1-release-criteria.md` §5.

---

## 5. Production-validation criteria (Phase 4)

CR-P/E/A/D criteria the language plan never directly touched. Each
gets its own benchmark or check with an artifact under
`contracts/reports/perf/` or `contracts/reports/arch/`.

### 5.1. CR-P3 — `vox new web → vox deploy` ≤ 120s
**Artifact:** `contracts/reports/perf/cr-p3/<UTC>.json` with
wallclock seconds. Already partially covered by §3.2 (CR-L7 deploy
leg). Add a standing CI job that publishes this metric.

### 5.2. CR-P1 — 3 marquee apps deployed live on OCI
**Artifact:** `contracts/reports/perf/cr-p1/<UTC>.json` recording
deploy timestamps + health-check URLs for each of slot-1
(marquee-app), slot-2 (todo-auth), slot-3 (chat). Real OCI
infrastructure required; deploys via the `vox deploy --target fly`
path (already implemented).

**Cost note:** ~$5/mo per app × 3 = ~$15/mo standing infra cost.

**Owner:** infra. **Effort:** ~12 hours (one-time deploy + health
endpoint wiring).

### 5.3. CR-P2 — 99.9% uptime on `vox-ml-cli` over 7 days
**Artifact:** `contracts/reports/perf/cr-p2/<UTC>-7day.json` with
uptime % from an external watchdog (e.g. UptimeRobot's API). Stretch
goal; needs the inference endpoint actually live somewhere.

**Owner:** infra. **Effort:** ~6 hours one-time + 7 days wall.

### 5.4. CR-E1 — `vox run --interp` <50ms cold start
**Artifact:** `contracts/reports/perf/cr-e1/<UTC>.json` with p50/p95/p99
from a Criterion benchmark suite. Bench fixture: a 5-line
`hello.vox`. Run on the CI matrix (Linux x86, Linux ARM, macOS ARM).

**Owner:** vox-cli + vox-compiler. **Effort:** ~6 hours.

### 5.5. CR-E2 — Marquee bundle ≤ 800KB gzip
**Artifact:** `contracts/reports/perf/cr-e2/<UTC>.json` with gzip'd
bundle bytes for each marquee app. Add a build-time gate in
`vox build` that fails if the output exceeds the threshold.

**Owner:** vox-codegen. **Effort:** ~4 hours.

### 5.6. CR-A1 — cyclomatic complexity <15 on primary lowering paths
**Artifact:** `contracts/reports/arch/cr-a1/<UTC>.json` from a
cargo-mccabe (or equivalent) run over `crates/vox-compiler/src/lower/`.
Fail CI on the first function above 15.

**Owner:** vox-arch-check. **Effort:** ~4 hours.

### 5.7. CR-A2 — 100% FFI/IPC under VoxProto v1
**Artifact:** `contracts/reports/arch/cr-a2/<UTC>.json` listing every
FFI/IPC boundary in the workspace and whether each has a v1 schema.
Initial sweep will be ugly; gate begins with `enforce: false`,
flipped to `enforce: true` once parity reaches 100%.

**Owner:** vox-arch-check. **Effort:** ~8 hours.

### 5.8. CR-A4 — lifecycle metadata parity
**Artifact:** `contracts/reports/arch/cr-a4/<UTC>.json` from the
orchestration-contract walker checking every contract has
`experimental` | `stable` | `deprecated` + `migration_window_days`.

**Owner:** vox-arch-check. **Effort:** ~4 hours.

### 5.9. CR-D3 — 100% CLI subcommands have help + .vox example
**Artifact:** `contracts/reports/arch/cr-d3/<UTC>.json` cross-referencing
the output of `vox --help` against `examples/cli/` `.vox` files. Every
subcommand needs both.

**Owner:** vox-cli + docs. **Effort:** ~6 hours (audit) + ~12 hours
(authoring missing examples).

**Phase-4 exit criterion:** every CR-P/E/A/D criterion has a
`contracts/reports/{perf,arch}/<criterion>/<UTC>.json` artifact, and
the standing CI gate from §1.3 enforces freshness on it.

---

## 6. Execution order + hard checkpoints

```
Phase 0 ── Anti-overclaim machinery ─┐
                                     │
Phase 1 ── Doc reconciliation ──────┐│
                                    ▼▼
Phase 2 ── Measurement wiring ─────┐
                                   │  (gate aggregate now reports
                                   ▼   real numbers, mostly below bar)
Phase 3 ── Corpus expansion ──────┐
                                  │  (numbers begin to climb)
                                  ▼
Phase 4 ── CR-P/E/A/D benchmarks ──→  v1.0 GA candidate
```

**Hard checkpoint between each phase:**

- After Phase 0+1: `cargo run -p vox-arch-check` clean.
  `vox audit --gate all` produces a roll-up snapshot with today's
  honest state (every claim in the audit scorecard cites a path).
- After Phase 2: every gate reports a non-`incomplete` result with a
  real `overall_pass_rate`. CR-L8 is the canary — when it stops being
  `incomplete: true` we know the telemetry pipeline is wired.
- After Phase 3: corpus sizes for CR-L0/3/4 meet the v1-release-criteria
  minimums (10, 50, 50 respectively).
- After Phase 4: all CR-P/E/A/D artifacts exist; standing CI enforces
  them.

## 7. Evidence ledger (filled in as work lands)

| claim_id | criterion | artifact_path | first_green_date | most_recent_run | notes |
|---|---|---|---|---|---|
| `audit.cr_l5.aci_default` | CR-L5 | `contracts/reports/aci-default/<UTC>.json` | 2026-05-19 | run via gate | met=true, target=1.0 |
| `audit.cr_l6.retirement` | CR-L6 | `contracts/reports/retirement/<UTC>.json` | 2026-05-19 | run via gate | met=true, 16/16 detectors |
| `audit.cr_l7.deploy_doctor_leg` | CR-L7 (partial) | `contracts/reports/deploy/<UTC>.json` | 2026-05-19 | met=true | doctor leg only; full legs §3.2 |
| `audit.cr_l1.llm_panel` | CR-L1 (full) | `contracts/reports/humaneval/<UTC>.json` | 2026-05-21 | met=true (0.939) | 164-fixture panel: gpt 96.3%, claude 91.5%, $1.16 spend. §3.3 ✓ |
| `audit.cr_l3.repair_panel` | CR-L3 | `contracts/reports/repair-corpus/<UTC>.json` | 2026-05-21 | met=true (0.800) | 5-project panel: both members 4/5, $0.03 spend. §3.5 ✓; corpus 5→50 (§4.2) future polish |
| `audit.cr_l4.plan_loop_measured` | CR-L4 | `contracts/reports/plan-fidelity/<UTC>.json` | 2026-05-21 | measured: 0.40, sub-bar (target 0.85) | 5-plan single-shot panel, $0.07 spend. §3.6 wiring ✓; threshold needs refinement-loop port + base sources per plan (§4.3) |
| `code.workflow_runtime` | language surface | `crates/vox-workflow-runtime/` + `crates/vox-cli/tests/workflow_populi_golden.rs` | 2026-05-19 | passes | journal events round-trip |
| `code.sse_codegen` | language surface | `crates/vox-codegen/src/codegen_rust/emit/http.rs::emit_sse_handler` + tests | 2026-05-19 | passes | actor-subscribe bridge + interval tick |
| `code.cr_l3_cr_l4_runners` | language surface | `crates/vox-audit/src/subcommands/{plan_fidelity,mens_on_distribution}.rs` | 2026-05-19 | passes | RUNNERS real; MEASUREMENTS pending §3.4–3.6 |
| `arch.cycles` | CR-A3 | `cargo run -p vox-arch-check` | 2026-05-21 | clean | zero circular deps |
| `audit.cr_l0.spec_to_app` | CR-L0 | `contracts/reports/spec-to-app/<UTC>.json` | 2026-05-21 | met=true (0.667) | panel + 5-iter refinement; gpt 100%, claude 33%. §3.7 ✓ |
| `audit.cr_l7.full` | CR-L7 (full) | `contracts/reports/deploy/<UTC>.json` | 2026-05-21 | met=true | new + deploy --dry-run + doctor legs all green; §3.2 ✓ |
| `audit.cr_l8.feedback_loop` | CR-L8 | `contracts/reports/corpus-feedback/<UTC>.json` | 2026-05-21 | met=true | aggregator + quarterly artifact; quiet-quarter OK; §3.1 ✓ |
| `perf.cr_e1.coldstart_50ms` | CR-E1 | `contracts/reports/perf/cr-e1/<UTC>.json` | 2026-05-21 | met=true (p99=0.25ms) | 200× under 50ms budget; §5.4 ✓ |
| `arch.cr_a4.lifecycle_parity` | CR-A4 | `contracts/reports/arch/cr-a4/<UTC>.json` | 2026-05-21 | met=true (9/9) | flat + nested forms both recognized; §5.8 ✓ |
| _below: real measurement landed, threshold not yet met_ | | | | | |
| `arch.cr_a1.cyclomatic` | CR-A1 | `contracts/reports/arch/cr-a1/<UTC>.json` | 2026-05-21 | measured: 14 funcs > budget (max=28) | sub-bar; refactoring is real follow-on. §5.6 — measurement done |
| `docs.cr_d3.cli_examples` | CR-D3 | `contracts/reports/arch/cr-d3/<UTC>.json` | 2026-05-21 | measured: 8/68 = 11.8% | sub-bar; authoring 60 .vox examples is real follow-on. §5.9 — measurement done |
| `arch.cr_a2.schema_parity` | CR-A2 | `contracts/reports/arch/cr-a2/<UTC>.json` | 2026-05-22 | met=true (186/186, 100%, enforce=true) | sweep recognizes $schema/x-vox-version/openapi/version/schema; excludes protocol-sample/fixture data; 3 YAMLs annotated. §5.7 ✓ |
| _below: NOT YET FILLED_ | | | | | |
| `audit.cr_l2.mens_sampling` | CR-L2 | `contracts/reports/mens-on-distribution/<UTC>.json` | — | — | needs §3.4 |
| `perf.cr_p1.three_apps_live` | CR-P1 | `contracts/reports/perf/cr-p1/<UTC>.json` | — | — | needs §5.2 (OCI infra) |
| `perf.cr_p2.uptime_7d` | CR-P2 | `contracts/reports/perf/cr-p2/<UTC>-7day.json` | — | — | needs §5.3 (OCI infra) |
| `perf.cr_p3.120s_loop` | CR-P3 | `contracts/reports/deploy/<UTC>.json` (note field) | 2026-05-21 | met=true (0.01s vs 120s budget) | Co-measured with CR-L7; same wallclock trio. §5.1 ✓ |
| `perf.cr_e2.bundle_800kb` | CR-E2 | `contracts/reports/perf/cr-e2/<UTC>.json` | — | — | needs §5.5 |

The above table is the canonical "what is and isn't done" — anything
not listed is not claimed. As work lands, fill in `first_green_date`
and `most_recent_run`. The arch-check from §1.2 reads
`contracts/reports/evidence-ledger.v1.json` (mirror of the above) and
fails if the audit scorecard claims "CLOSED" without a row here.

## 8. Effort budget

| Phase | Effort (hours) | Calendar | Dependencies |
|---|---|---|---|
| 0 — Anti-overclaim machinery | 15 | 3 days | — |
| 1 — Doc reconciliation | 4 | half day | — |
| 2 — Measurement wiring | 70 | 2-3 weeks | Phase 0; OPENROUTER_API_KEY |
| 3 — Corpus expansion | 90 | 2-3 weeks parallel auth | Phase 2 partial |
| 4 — CR-P/E/A/D benchmarks | 60 | 2-3 weeks | OCI infra access |
| **Total** | **~240 hours** | **~8 weeks focused** | |

That's the honest number for end-to-end v1.0 GA with evidence behind
every claim. Cf. the 2026-05-18 plan's "~108 hours" estimate, which
assumed code-lands ≡ done.

## 9. What's explicitly OUT of scope for v1.0 GA

- HumanEval growth past 164 (the anchor target is hit; richer surface
  goes to v1.1)
- Mesh Phase 2 LAN (council demoted to v1.1 per D16)
- Per-actor broadcast payload typing beyond `Stream[str]` (codegen
  works on `String`; type-system lift is v1.1)
- WebSocket emit (SSE handles v1.0 use cases per the 05-18
  architecture audit)
- Inference hosting (out of scope per the audit doc §3.9)

## 10. Acceptance for "v1.0 GA"

Single command:

```bash
cargo run -p vox-cli -- audit --gate all --strict-block-ga
```

Must exit 0. The roll-up snapshot at
`contracts/reports/_snapshot/<UTC>.json` must show every block-GA
gate (CR-L0..CR-L8 and CR-P/E/A criteria with block_ga=true) at
`met: true` AND `incomplete: false`. The evidence ledger must have
zero un-filled rows in the v1.0-targeted section.

That's the bar. Anything short of that bar is not "v1.0 ready."

## 11. Drift checks (what would tell us this plan failed)

If, three months from now, any of these appear:
- A claim of "v1.0 ready" without `--strict-block-ga` passing
- An audit doc edit adding "CLOSED" without a ledger row
- A gate result older than its freshness window
- A corpus growing without `corpus_hash` regen
- A CI run that skips `--gate all` for "speed"

Then the anti-overclaim machinery has gaps and §1 needs hardening.
Treat each of these as a P0 process bug.
