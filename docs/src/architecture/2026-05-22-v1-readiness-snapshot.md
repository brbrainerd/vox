# Vox v1.0 Readiness Snapshot — 2026-05-22

**Captured at:** 2026-05-22T00:59:09Z
**HEAD:** `5f4f19e84` (chore: lockfile + audit snapshots + held-out test + gitignore polish)
**Branch:** `cc_bdesktop2/jovial-buck-e93ac0` (worktree, local only)
**Cumulative ahead of upstream `main`:** 48 commits

This snapshot is the measured state of every CR-L / CR-P / CR-E / CR-A / CR-D
gate from the [v1.0 release criteria](v1-release-criteria.md), captured
against on-disk artifacts at the moment of writing. Every claim cites a
real artifact path; sub-bar items disclose the gap; pending items name
their blocker.

It is the **canonical "what's true today" reference** for the v1.0
honest-completion plan ([2026-05-21](../../superpowers/specs/2026-05-21-v1-honest-completion-plan.md)).
Future readers (or future-me) should compare against the artifacts named
here before re-asserting any of these claims.

## TL;DR

- **Block-GA umbrella exits 0** from a cold-cache run:
  `cargo run -p vox-cli -- audit --gate all --strict-block-ga` → `exit=0`.
- **All five block-GA criteria met:** CR-L0, CR-L5, CR-L6, CR-L7, CR-L8.
- **Two LLM-panel gates also met** (not block-GA but flagship metrics):
  CR-L1 humaneval (0.939), CR-L3 repair-corpus (0.800).
- **Three measured-sub-bar with honest evidence:** CR-A1 cyclomatic
  (max=53 vs 15 budget), CR-D3 CLI examples (11.8%), CR-L4 plan-fidelity
  (0.400 vs 0.85).
- **One probe-only:** CR-L2 (MENS endpoint not running locally; runner
  ready, emits real `BackendUnavailable` with verbatim error + remediation).
- **Three true infra-pending:** CR-P1, CR-P2, CR-E2 (OCI / bundle pipeline).
- **OpenRouter spend across two sessions:** ~$2.00 of a $25 cap.
- **`vox-arch-check`: clean** (zero arch cycles; evidence-ledger consistent).

## Gate-by-gate measurement

All artifacts are dated `2026-05-22` unless noted. Cumulative-cost
column is the total OpenRouter spend across **all** runs that contributed
to that gate this session (cache hits → $0 on repeat runs).

### Block-GA layer (all met)

| Gate | Pass rate | Target | Met | Artifact | Cumulative cost |
|---|---|---|---|---|---|
| CR-L0 spec-to-app | **0.667** | 0.60 | ✓ | `contracts/reports/spec-to-app/2026-05-22.json` | $0.20 |
| CR-L5 aci-default | 1.000 | 1.0 | ✓ | `contracts/reports/aci-default/2026-05-22.json` | $0 |
| CR-L6 retirement | 1.000 | 1.0 | ✓ | `contracts/reports/retirement/2026-05-22.json` | $0 |
| CR-L7 deploy (full legs) | 1.000 | 1.0 | ✓ | `contracts/reports/deploy/2026-05-22.json` | $0 |
| CR-L8 corpus-feedback | 1.000 | 1.0 | ✓ | `contracts/reports/corpus-feedback/2026-Q2.json` | $0 |

**CR-L0 detail:** 3 specs × 2 OpenRouter members (claude-sonnet-4-6,
gpt-5.4) under 5-iteration refinement loop. Per-member: gpt-frontier 100%
(3/3), claude-sonnet 33% (1/3). Median = 0.667. Trajectory same-day:
0% (no calibration) → 41.7% (calibrated prompt, single-shot) → 50% (3-iter)
→ 66.7% (5-iter; default). Runner at
`crates/vox-audit/src/subcommands/spec_to_app_panel.rs`.

**CR-L7 detail:** Full wallclock trio (new + deploy --dry-run + doctor)
across 3 marquee fixtures including the new slot-1 todo-auth app. Trio
= 0.01s vs 120s CR-P3 budget (12000× under budget; CR-P3 co-measured
here).

### LLM-panel gates (not block-GA but flagship metrics)

| Gate | Pass rate | Target | Met | Artifact | Cumulative cost |
|---|---|---|---|---|---|
| CR-L1 humaneval (full panel) | **0.939** | 0.80 | ✓ | `contracts/reports/humaneval/2026-05-22.json` | $1.16 |
| CR-L3 repair-corpus (panel) | **0.800** | 0.70 | ✓ | `contracts/reports/repair-corpus/2026-05-22.json` | $0.03 |
| CR-L4 plan-fidelity (panel) | 0.400 | 0.85 | ✗ | `contracts/reports/plan-fidelity/2026-05-22.json` | $0.37 |
| CR-L2 MENS sampling (probe) | n/a | 0.95 | ✗ (incomplete) | `contracts/reports/mens-on-distribution/2026-05-22.json` | $0 |

**CR-L1 detail:** 164-fixture canonical corpus × 2 OpenRouter members.
gpt-frontier 96.3% (158/164), claude-sonnet 91.5% (150/164). Per
ratified panel YAML §scoring_rule: median-of-members = 0.939.
mens-current excluded (not OpenRouter-routable).

**CR-L3 detail:** 5-project broken-source corpus × 2 members. Both
members repaired 4/5. Pass = post-repair source compiles with zero
errors. §4.2 corpus growth 5 → 50 remains future polish; the 5-project
measurement is publishable evidence as-is.

**CR-L4 sub-bar detail:** Documented empirical finding (the refinement
loop that lifted CR-L0 from 41% → 67% does NOT help here — same 40%
at 5× cost). Plan-fidelity failures are semantic plan-misunderstandings
that vox-check diagnostics can't fix. Closing the gap needs §4.3 base
sources per plan (so the model isn't guessing what to modify) OR a
richer agent loop with test-execution feedback. Default iter reduced to
3 after the experiment to save future cost.

**CR-L2 probe detail:** Runner does an actual `reqwest::blocking GET
{VOX_MENS_ENDPOINT}/health` (default `http://127.0.0.1:7863`) on a
dedicated OS thread. In this session: probe failed → real
`BackendUnavailable` artifact records the URL + connect error verbatim;
note carries remediation `python scripts/vox_inference.py --serve`. When
the server is reachable in any measurement env, the wiring is ready for
the VoxLocal RPC sampling protocol (non-OpenAI-compat) to land as the
next step.

### Non-LLM v1.0 criteria

| Gate | Measurement | Target | Met | Artifact |
|---|---|---|---|---|
| CR-E1 cold-start p99 | 0.25ms | ≤ 50ms | ✓ (200× under) | `contracts/reports/perf/cr-e1/2026-05-21.json` |
| CR-A2 schema-parity | 186/186 (100%) | 100% | ✓ (ENFORCE=true) | `contracts/reports/arch/cr-a2/2026-05-22.json` |
| CR-A3 zero cycles | clean | 0 cycles | ✓ | (`vox-arch-check` runs clean) |
| CR-A4 lifecycle parity | 9/9 (100%) | 100% | ✓ | `contracts/reports/arch/cr-a4/2026-05-22.json` |
| CR-P3 ≤120s `new→deploy` | 0.01s | ≤ 120s | ✓ | (co-measured in CR-L7 deploy artifact) |
| CR-A1 cyclomatic complexity | max=53 (was 78) | ≤ 15 per fn | ✗ (sub-bar) | `contracts/reports/arch/cr-a1/2026-05-21.json` |
| CR-D3 CLI example coverage | 8/68 (11.8%) | 100% | ✗ (sub-bar) | `contracts/reports/arch/cr-d3/2026-05-21.json` |

**CR-A1 sub-bar detail:** Refactor pass on `check_expr` (78 → 53, -32%)
and `check_expr_field_access` (45 → below 15, off the over-budget
list). 21 functions remain over the 15-budget threshold. Next-worst:
`emit_expr_with` at 51, `lower_fn` at 31, `extract_db_query_chain` at
29. Real engineering effort (~30-60 min per function); risk of
regressions. v1.x polish track.

**CR-D3 sub-bar detail:** Coverage measured as "`vox <subcommand>`
referenced from at least one `.vox` file in `examples/` or `scripts/`".
Authoring the remaining 60 example files is real labor (~10-20h).
Mechanical "comment-only mentions" would game the metric without
serving users.

### True infra-pending

| Gate | Blocker | What it would need |
|---|---|---|
| CR-P1 three apps live | OCI infra | Provider account, domain, SSL, 3 deployments |
| CR-P2 7-day uptime | OCI infra + time | Provider + 7-day soak monitor |
| CR-E2 bundle ≤ 800KB | Bundle pipeline | Marquee app build + gzip + size measurement (~2h engineering) |

## Engineering wins from this session (commits 1b80d24b4 → 5f4f19e84)

Fourteen logical commits on `cc_bdesktop2/jovial-buck-e93ac0`:

1. `chore(gitignore)`: ignore `/contracts/reports/llm-panel-cache/`
2. `feat(audit): anti-overclaim machinery` — evidence ledger + arch-check
   Rule 14 + nightly CI workflow + pre-claim guard + honest plan + doc
   reconciliation
3. `fix(secrets)`: round-trip-verify keyring writes + surface env-vs-auth.json
   shadowing (caught while wiring the OpenRouter user journey)
4. `feat(audit): umbrella infra` — `--strict-block-ga` + snapshot writer +
   shared `same_day_canonical_with_panel` evidence-preservation helper +
   `--llm-panel` CLI flag
5. `feat(audit): CR-L7 deploy gate full legs` — new + deploy --dry-run +
   doctor + slot-1 marquee fixture
6. `chore(reports): CR-L8 Q2 + daily corpus-feedback artifacts`
7. `feat(audit): CR-E1/A1/A4/D3/A2 measurement binaries` + lifecycle &
   schema annotations
8. `feat(audit): CR-L0 spec-to-app` — panel runner + 5-iter refinement loop
9. `feat(audit): CR-L1/L3/L4 panel modes` — humaneval, repair-corpus,
   plan-fidelity
10. `feat(audit): CR-L2 MENS endpoint probe` + honest BackendUnavailable
    artifact
11. `refactor(typeck): CR-A1` — `check_expr` complexity 78 → 53 via
    extracting MethodCall / Lambda / Try arms and collapsing
    Std-namespace dispatch
12. `feat(eval): HumanEval-Vox corpus 30 → 164` + held-out manifest refresh
13. `feat(compiler+codegen): language-surface improvements` across typeck,
    HIR, codegen (workflow runtime, SSE codegen, broadcast, polymorphic
    instantiation, expected-type propagation, etc.)
14. `chore`: lockfile + audit snapshots + held-out test + gitignore polish

### Reusable engineering patterns

These patterns were proven across multiple panel gates this session and
are now load-bearing infrastructure:

- **`std::thread::scope` wrap around `reqwest::blocking`** so the outer
  Tokio runtime owned by `vox-cli` doesn't panic on drop. Used by
  spec-to-app, humaneval, repair-corpus, plan-fidelity, and the MENS probe.
- **`CachingPanelClient` + `ProtectedPanelClient` + `OpenRouterPanelClient`
  composition** with budget cap (`VOX_AUDIT_BUDGET_USD`, default $20).
  Cache makes repeat runs free.
- **`vox_audit::same_day_canonical_with_panel(workspace_root, gate)`**
  shared evidence-preservation guard. Corpus-only branches refuse to
  clobber a same-day panel artifact. Skipped when `args.corpus` is
  explicitly overridden (tests' escape hatch).
- **Calibrated Vox system prompt** — the difference between 0% and 41.7%
  on CR-L0 was a prompt that explicitly named `assert(X is Y)` (not
  `assert_eq`), `to T` (not `->`), `to Unit` for void, explicit `return`.
- **Multi-iteration refinement loop** — proven to lift CR-L0 from 41% →
  67% (+25 pts). Empirically does NOT help CR-L4 (plan-misunderstandings
  aren't compiler errors). Cost-budget-aware: stops when per-spec or
  cumulative cap would trip.

## Honest gaps surfaced (not blockers, real-engineering follow-ons)

These are documented for the next reader to act on, not hidden:

- **CR-A2 found 12 malformed JSON schema files** in `contracts/` —
  `serde_json::from_str` fails on them. The CR-A2 text-scan fallback
  still detects their `x-vox-version` markers so parity isn't affected,
  but they're real defects. Spawned-task chip captured the fix scope.
- **CR-L4 empirical finding** recorded in code + ledger: refinement
  iterations don't help when the model's failure mode is semantic
  misunderstanding, not compile errors.
- **vox-mesh test compile error** (`unresolved import crate::types` in
  `models/registry.rs:103`) — pre-existing on the branch, untouched by
  any of the 14 commits. Documented here for the next mesh-touching
  pass to fix.
- **Stale "completed" task #36** ("P5.7 — CR-A2 VoxProto schema parity
  sweep") had no binary, no artifact, nothing. Caught via the
  `verify audit-agent retirement claims by hand` memory rule and
  replaced with a real implementation (task #43).

## Reproduction commands

To re-verify any claim in this snapshot from a fresh checkout:

```bash
# Block-GA umbrella (the canonical v1.0 acceptance switch)
cargo run -p vox-cli -- audit --gate all --strict-block-ga
# → exit 0 means block-GA layer green

# Per-gate panel run (requires OPENROUTER_API_KEY via `vox secrets set
# openrouter <token>` OR env var)
cargo run -p vox-cli -- audit --gate spec-to-app   --llm-panel contracts/eval/llm-panel.v1.yaml
cargo run -p vox-cli -- audit --gate humaneval     --llm-panel contracts/eval/llm-panel.v1.yaml
cargo run -p vox-cli -- audit --gate repair-corpus --llm-panel contracts/eval/llm-panel.v1.yaml
cargo run -p vox-cli -- audit --gate plan-fidelity --llm-panel contracts/eval/llm-panel.v1.yaml
cargo run -p vox-cli -- audit --gate mens-on-distribution --llm-panel contracts/eval/llm-panel.v1.yaml

# Standalone measurement binaries
cargo run -p vox-audit --bin cr-e1   # cold-start p99
cargo run -p vox-audit --bin cr-a1   # cyclomatic-complexity sweep
cargo run -p vox-audit --bin cr-a2   # schema-parity sweep (enforce=true)
cargo run -p vox-audit --bin cr-a4   # lifecycle metadata parity
cargo run -p vox-audit --bin cr-d3   # CLI example coverage

# Arch lint (zero cycles + evidence-ledger consistency)
cargo run -p vox-arch-check

# Budget knobs (env)
#   VOX_AUDIT_BUDGET_USD                 - cumulative cap, default 20
#   VOX_AUDIT_SPEC_TO_APP_MAX_ITERATIONS - default 5
#   VOX_AUDIT_CR_L1_MAX_FIXTURES         - default unset (full 164)
#   VOX_AUDIT_CR_L3_MAX_PROJECTS         - default unset (full 5)
#   VOX_AUDIT_CR_L4_MAX_ITERATIONS       - default 3
#   VOX_MENS_ENDPOINT                    - default http://127.0.0.1:7863
```

## What's NOT this snapshot

- **Push / PR status.** The 14 commits sit on the local worktree branch.
  No remote push, no PR opened, per user direction.
- **Cost projection for full CI.** Each panel gate run consumes
  OpenRouter credits at the per-gate budget cap. Caching makes repeat
  identical runs free. Per-PR LLM cost depends on what changed; nightly
  runs against the canonical corpus cost ≤ $2 from cold cache.
- **Test-coverage numbers.** Out of scope for this snapshot. Per-crate
  lib tests verified green across every crate touched this session
  (vox-audit 115, vox-compiler 298, vox-codegen 188, vox-secrets 25,
  vox-arch-check 16).

## Cross-references

- [Honest completion plan (canonical)](../../superpowers/specs/2026-05-21-v1-honest-completion-plan.md)
- [v1.0 release criteria](v1-release-criteria.md)
- [Vox-as-LLM-target audit & plan](vox-as-llm-target-audit-and-plan-2026.md)
- [Evidence ledger](../../../contracts/reports/evidence-ledger.v1.json)
