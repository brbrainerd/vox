---
title: "LLM/AI SSOT — Remaining Design Passes + Cost-Tracking SSOT"
description: "Persistent roadmap decomposing the remaining LLM/AI single-source-of-truth work into discrete, TDD-scoped design passes — with cost measurement → cost routing → GUI cost-surfacing as the headline cross-cutting thread. Each pass is independently spec→plan→executable, with workflow / parallel-subagent execution notes."
category: "Architecture SSOTs"
---

# LLM/AI SSOT — Remaining Design Passes + Cost-Tracking SSOT

**Status:** Roadmap (persistent planning artifact). Each pass below gets its own `brainstorm → spec → plan → execute` cycle; this doc scopes them and the cross-cutting cost thread.
**Branch:** `llm-ssot-united`. **Builds on:** Band A SSOT + the completed `vox-llm-egress` initiative (Phases 1–6 — single sanctioned egress, enforced by the `llm_provider_call` detector).
**Implementer target:** Claude Sonnet 4.6 (TDD; workflows + parallel subagents for fan-out).

## 0. Guiding invariant (applies to every pass)

> **One value, one home, surfaced reactively.** Every LLM/AI setting *and every cost number* has a single authoritative source; consumers are views; changes propagate to the GUI through the existing `vox://llm-config-changed` / event bus rather than being recomputed divergently. New code is added test-first; no second copy of a number that already exists.

## 1. Cost-Tracking SSOT (HEADLINE — cross-cutting)

**Why now:** the egress consolidation made cost *measurable* in one place (`EgressChatResponse.cost_usd`), but cost is still **computed, recorded, routed, and displayed in four separate subsystems** that can drift. As we finish the SSOT, cost must become single-source, accurate (provider-reported preferred over estimate), and reactively surfaced to the user.

**Current reality (verified 2026-06-15):**
- **Measure:** `vox-llm-egress::chat_once` returns `cost_usd` = provider-reported (`usage.total_cost`/`cost`, else `x-response-cost` header). The facade (`vox-actor-runtime/llm/chat.rs`) falls back to a `cost_per_1k` estimate when `None`.
- **Record:** facade → `record_telemetry_outcome` → `vox_db::store::ops_agents::record_unified_llm_turn` (table column `cost_usd` + `cumulative_cost_usd` on `model_scoreboard`).
- **Route:** `vox-orchestrator/src/models/{scoring,select,policy,admission}.rs` use cost (`cost_inverse`, `EFFICIENCY_COST_SCALER`, `max_cost_usd_per_call`, exploration budget) for model selection.
- **Surface:** GUI shows budget *caps* (`daily_budget_usd`, `per_session_budget_usd` in `user_config.rs`) but **cumulative actual spend is not clearly surfaced reactively** — the gap.

**Scope (the pass):**
1. **Single cost-measurement home.** Make `vox-llm-egress::EgressChatResponse.cost_usd` the *only* provider-cost producer; the `cost_per_1k` *estimate* lives in exactly one place (a `vox-config` or `vox-llm-config` accessor keyed off the model registry) — not inlined in the facade. The streaming path (`stream_once`) must also surface cost (the deferred egress streaming-cost extension — see §2) so gamify/streaming cost is recorded, not dropped.
2. **Single recording path.** All inference (facade + any future caller) records via one `record_llm_cost(outcome)` entrypoint in `vox-db` — already nearly true; assert it with a test that no other code path writes `cost_usd`.
3. **Single spend aggregate.** One query/view computes cumulative spend (session/day/total) from the recorded turns; budgets (caps) and spend (actuals) read the same source.
4. **Reactive GUI surfacing.** Add a `get_llm_spend()` Tauri command (session/day/total + remaining-vs-budget) and emit a `vox://llm-spend-changed` event after each recorded turn, mirroring the Band-A reactive pattern. The Runtime/Orchestrator settings panel shows live spend against the budget caps it already displays.
5. **Cost routing reads the same numbers.** The orchestrator's cost-based scoring reads the *recorded* per-model cost aggregates (the scoreboard), so "what we route on" == "what we charge" == "what we show".

**TDD scope:** unit tests for the single estimate accessor; a parity test asserting only one writer of `cost_usd`; a `vox-db` aggregate test (session/day/total); a GUI test that `get_llm_spend` reflects recorded turns and the event fires; a routing test that selection reads the scoreboard aggregate.
**Execution:** mostly sequential (measure→record→aggregate→surface→route), but the GUI surfacing (4) and routing-read (5) are independent once the aggregate (3) exists → parallel subagent tracks.
**SSOT check:** after this pass, grep proves exactly one `cost_per_1k` estimate site and one `cost_usd` writer.

## 2. Egress streaming-cost extension (small; unblocks §1 + gamify streaming)

**Why:** `stream_once` doesn't surface `x-response-cost`, so gamify's streaming `cost_reporter` was kept as a documented local egress (Phase 4). To finish single-egress and accurate cost, extend the core.
**Scope:** `stream_once` returns the response cost (header at response time + any final `usage` chunk) via a side channel — either an `on_cost: impl Fn(f64)` callback param or a `StreamHandle { stream, cost: watch::Receiver<Option<f64>> }`. Then migrate gamify `stream_openrouter` onto it and remove its `// vox-arch-check: allow llm-egress` exemption.
**TDD:** a wiremock SSE test asserting the cost callback fires with the header value; a gamify test that the migrated stream still reports cost.
**Execution:** single track; ~1–2 tasks.

## 3. `llm_bridge` consolidation (its own spec — largest remaining)

**Why:** `vox-orchestrator-mcp/src/llm_bridge/` is a **second multi-provider egress facade** (~800 LoC: Gemini/Anthropic/OpenAI/Ollama adapters) that still bypasses `vox-llm-egress`. Until it routes through the core, single-egress is not total.
**Scope (own brainstorm→spec):** route its OpenAI-compatible adapters through `vox_llm_egress::chat_once`/`stream_once`; preserve the 8 entangled concerns the egress map flagged — **cost estimation + DeepSeek off-peak discounts (ties into §1)**, mesh reputation, ChatML collapse, vision/attachments, budget gating, Anthropic tool-fallback, custom headers, probe caches. Keep genuinely-local paths (VoxLocal `/generate`, PopuliMesh, health probes) local + annotated.
**TDD:** per-adapter parity tests (request shape + cost reconciliation); a test that budget gating still fires; the detector should then flag any remaining un-annotated bridge egress.
**Execution:** **parallel subagent fan-out** — one track per provider adapter (Gemini/Anthropic/OpenAI/Ollama) against the stable core, plus a synthesis pass for the cross-cutting concerns (cost/budget/mesh). This is the prime workflow candidate.

## 4. Band B — orchestrator settings under the registry (its own brainstorm→spec→plan)

**Why:** Band A registered the provider/endpoint/model/tuning/budget keys; the orchestrator's **routing/cascade/scoring/calibration knobs** (the ~25 `scoring.rs` consts, `tier_cascade` thresholds, `calibration` bandit params, `VOX_ROUTING_*`/`VOX_CAPABILITY_*` secrets, `SelectionAxes` presets — all inventoried in `llm-config-key-manifest.md`) are still scattered.
**Scope:** bring them under the `vox-llm-config` registry as an `orchestrator.*` namespace; surface in a GUI advanced panel; reactive via the same event bus. **Cost tie-in:** routing-cost knobs (`max_cost_usd_per_call`, exploration budget, cost weights) become registry-driven and GUI-visible, closing the loop with §1.
**TDD:** parity tests per subsystem (routing/cascade/autonomic/scoring/budgets); a test that a routing knob changed in the GUI affects selection.
**Execution:** sub-phased by independent subsystem (6a routing / 6b cascade / 6c autonomic+bandit / 6d scoring weights / 6e budgets) → **parallel subagent tracks** against the stable registry.

## 5. Band A Phase 5 — snapshot-cache perf (independent)

**Why:** config accessors re-read env per call. Snapshot-cache resolution behind the existing `vox-config::snapshot` watch channel; invalidate on write. **Cost tie-in:** none directly, but reduces per-call overhead on the hot inference path that also computes cost.
**Execution:** single track.

## 6. Land it — PR `llm-ssot-united` → origin/main

Once §1–2 (and ideally §3) land: open the PR (Band A docs already merged via #311; backup tag `backup/llm-ssot-band-a-20260615` + the superseded branch retained). Run `/code-review` over the full branch diff first.

## Dependency / sequencing summary

| Pass | Depends on | Parallelizable | Workflow candidate |
|---|---|---|---|
| §2 egress streaming-cost | egress core (done) | no | no |
| §1 cost SSOT | §2 (for streaming cost) | partially (steps 4+5) | medium |
| §3 llm_bridge | egress core (done) | yes — per-adapter | **strong** |
| §4 Band B | Band A registry (done) | yes — per-subsystem | **strong** |
| §5 perf | Band A (done) | no | no |
| §6 PR | §1–3 | — | review fan-out |

**Recommended order:** §2 → §1 (the cost-SSOT headline) → §3 (llm_bridge, parallel) → §4 (Band B, parallel) → §5 → §6. §1 is the highest-value next pass: it makes cost accurate + single-source + GUI-surfaced, which is the user's stated priority and rides directly on the egress work just completed.
