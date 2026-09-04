---
title: "ADR 046 — Pareto-Frontier Reporting for Model Surfaces"
description: "Present model scoreboards as a Pareto set over reliability, cost, and latency instead of one composite rank — and why this is reporting-only, not a routing change."
date: "2026-09-03"
status: "current"
category: "Architecture Decisions (ADRs)"
---

# ADR 046 — Pareto-Frontier Reporting for Model Surfaces

## Context

Task M3 ("Surface") asks, under a heading titled *Surface*, for a "Pareto set + budget-constrained
argmax, not a weighted sum" — one item among five unambiguous reporting items in that plan. Before
this change, `vox model scoreboard` and `vox model explain` collapsed reliability, cost, and latency
into a single composite rank, hiding the fact that these axes trade against each other: a model that
is cheaper but slower, or faster but less reliable, has no way to show up as a legitimate alternative
next to the top-ranked row.

## Decision

Implement Pareto-frontier marking as a reporting feature only. Nothing in this change alters model
selection or routing.

- `crates/vox-orchestrator/src/models/pareto.rs` defines `ParetoPoint { quality, cost_usd, latency_ms }`
  and `pareto_frontier(&[ParetoPoint]) -> Vec<usize>`, exported from `models/mod.rs`.
- `crates/vox-cli/src/commands/model/scoreboard.rs` renders frontier membership (`frontier_marker`),
  an advisory `--budget <USD/success>` recommendation (`budget_recommendation`, `render_budget_line`),
  and a `pareto_legend` explaining the axes.
- `crates/vox-cli/src/commands/model/explain.rs` partitions candidates by rank confidence
  (`partition_by_rank_confidence`) and renders a `selection_note` plus per-candidate sections
  (`render_candidate_sections`) so a low-sample-count row is shown as unranked rather than silently
  outranked. Ranked candidates carry a `[pareto-optimal]` marker over the same three axes, followed
  by `scoreboard.rs`'s `pareto_legend` verbatim so the two surfaces cannot explain the same mark
  differently. The candidate list is ordered by the router's composite priority score, and says so;
  it is not ordered by observed performance.
  **Known limitation:** `explain` reads scores through `ModelRegistry::inject_scoreboard`, whose
  registry-side API is keyed by `model_id` alone, so each model's counts are whichever
  `(task_category, strength_tag)` row landed last. The rendered output discloses this and points at
  `vox model scoreboard`, which renders the triples separately. Re-keying the registry API would
  reach into selection, which this ADR's reporting-only scope excludes.
- The quality axis fed into `ParetoPoint` is the 95% Wilson lower bound on the observed success rate,
  not `model_scoreboard.quality_score` (a constant `1.0` in practice, and deliberately not surfaced).
  No rendered column or legend is headed "Quality" — the axis is labelled reliability throughout,
  because "success" measures provider-response availability, not answer correctness (see below).

**The load-bearing design decision:** on a lower-is-better axis (cost, latency), a `None` value —
no measurement — is treated as *incomparable*, not as "worst possible" and not as "neutral".
"Worst possible" would permanently dominate out every unmeasured model. "Neutral" is non-transitive:
`Some(5) ≼ None ≼ Some(1)` while `Some(5) ⋠ Some(1)`, which admits domination cycles and can produce
an empty frontier for non-empty input. Treating `None` as incomparable keeps the order transitive,
which is what guarantees `pareto_frontier` never returns empty for non-empty input.

Two independent thresholds gate rank confidence for different purposes and are deliberately not
unified: `MIN_CALLS_FOR_CONFIDENT_RANK = 5` (`models/registry.rs`) governs when a model's rank is
shown as confident at all, while `COST_PER_SUCCESS_MIN_SUCCESSES = 10` (`scoreboard.rs`) governs when
a cost-per-success figure is shown as a number rather than "insufficient data".

The `--budget` advisory is gated on **both**, via `recommendable_positions`, which narrows the
frontier to rows clearing both thresholds before `budget_recommendation` sees them. Without the
first gate the advisory could name a row the same table pointedly leaves unmarked; without the
second it could match on a cost the same table prints as "insufficient data". When only
sub-threshold rows are affordable, the line reads "no Pareto-optimal row qualifies".

## Why not a routing change

Four independent reasons, each verified against source on 2026-09-03:

1. **`success` is not answer quality.** `success` in the LLM call record means "the provider returned
   a non-error response," not "the answer was correct" (`crates/vox-actor-runtime/src/llm/chat.rs:200`,
   `:433` record `success: true`; `false` is recorded only at the corresponding error paths, `:252`,
   `:469`). The MCP chat surface goes further and hardcodes `success: true` unconditionally
   (`crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:1357`). A router optimizing this signal
   optimizes provider uptime, not usefulness. `quality_score` was already retired from routing
   consideration for being a constant `1.0` in practice — the same reason it stays off this report.
2. **The Wilson bound is a rich-get-richer signal, not an explorer.** The 95% Wilson lower bound rises
   monotonically with sample count at a fixed observed rate, at a decelerating rate — 0.596 → 0.826 →
   0.880 for 9/10, 90/100, 900/1000 successes. An incumbent model's bound improves purely by being
   called more, independent of whether it is actually better. A frontier built on this axis, if fed
   back into frontier-restricted exploration, would compound the incumbent's advantage rather than
   surface underexplored alternatives.
3. **The one live routing path does not go through the type this ADR touches.** `ModelSelectionEngine`
   is constructed at exactly one non-test call site — inside `resolve_model_with_registry_fallbacks`,
   which itself has zero production callers (its only callers are two test files and two re-exports).
   Live model selection runs through `models::select::decide` (`models/select.rs:87`),
   `best_for_task_with_filter` (`registry.rs:738`), `FreeTierRouter`, and explicit pins. A frontier or
   budget knob added to `ModelSelectionEngine` would change no production traffic while reading, to a
   reviewer, like a global routing switch.
4. **`RoutingPolicy.algorithm` has no live rollback path.** `RoutingPolicy` is baked in at compile time
   via `include_str!` (`crates/vox-orchestrator/src/routing/policy.rs:233-239`). A Clavis override
   exists for `epsilon_ceiling` but not for `algorithm`. Wiring a routing decision to this ADR's
   reporting axes would mean any rollback is a rebuild-and-redeploy, not a config push — an operational
   cost this reporting-only change does not take on.

## Consequences

- The reported frontier is over reliability (Wilson lower bound), cost, and latency, and every rendered
  surface labels it that way — never "Quality."
- An unmeasured axis is incomparable rather than worst-possible or neutral, which keeps the frontier
  order transitive and non-empty for non-empty input.
- Models with no observations at all are excluded from the frontier and listed separately, rather than
  being folded into either "best" or "worst."
- Nothing feeds `models::select::decide`, `best_for_task_with_filter`, `FreeTierRouter`, pins, or
  `ModelSelectionEngine`. Rollback of this feature is reverting the reporting commits; it carries no
  routing-behavior risk.

## Supersedes

`docs/src/architecture/model-autonomic-system-2026.md:19,177` describes
`resolve_model_with_registry_fallbacks` as a live selection path and states an intent to keep it "as a
thin wrapper … so older callers don't break." As of 2026-09-03, `resolve_model_with_registry_fallbacks`
has zero production callers (two test files, two re-exports). This ADR supersedes that
characterization: the function is dead code on the production path, not a maintained routing surface.

## Follow-ups

See the plan's §Blocked Follow-Ups:
[`docs/superpowers/plans/2026-09-03-m3-surface-pareto-reporting.md`](../../superpowers/plans/2026-09-03-m3-surface-pareto-reporting.md).
