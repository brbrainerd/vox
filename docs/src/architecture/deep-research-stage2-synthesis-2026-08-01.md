---
title: "Deep Research Stage 2 Synthesis & Priorities (2026-08-01)"
description: "Synthesizes the four Stage 2 docs (cross-domain methods survey, domain-agnosticism audit, GUI representation design, implementation-divergence audit) into one prioritized gap list, feeding the Stage 2 implementation plan. Headline findings: a live ranking bias favoring code sites over general-authority sources, the post-hoc citation audit feature silently dropped during Stage 1 execution, and the dominant Stage 1 failure mode was under-specified integration surface, not wrong algorithms — directly shaping how Stage 2's plan must be written."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative synthesis and prioritization for the deep-research Stage 2 program; direct input to its implementation plan."
---

# Deep Research Stage 2 Synthesis & Priorities

**Date:** 2026-08-01
**Inputs:** [deep-research-cross-domain-methods-survey-2026-08-01.md](deep-research-cross-domain-methods-survey-2026-08-01.md), [deep-research-domain-agnosticism-audit-2026-08-01.md](deep-research-domain-agnosticism-audit-2026-08-01.md), [deep-research-gui-representation-design-2026-08-01.md](deep-research-gui-representation-design-2026-08-01.md), [deep-research-implementation-divergence-audit-2026-08-01.md](deep-research-implementation-divergence-audit-2026-08-01.md), plus the graphify structural map of `vox-scientia`/`vox-research-shim`/`vox-search`/`vox-research-events`/`vox-publisher` (6216 nodes, 12457 edges, 536 communities). Builds on [deep-research-synthesis-and-priorities-2026-08-01.md](deep-research-synthesis-and-priorities-2026-08-01.md) (Stage 1).

---

## Where Stage 1 landed vs. what Stage 2 found

Stage 1 shipped 13 tasks across two plans (trust/novelty core, multi-provider routing) plus 10 code-review fixes — genuinely substantial, verified working. Stage 2's job was to test that work against reality on two axes: does it generalize beyond code/academic queries, and did it actually deliver everything the docs claimed. The answer to both is "mostly, with specific, fixable gaps" — not a redo, a continuation.

## Prioritized gap list

### P0 — Fix now, near-zero risk

1. **`web_dispatcher.rs`'s source-authority ranking bias.** `github.com`/`docs.rs` are boosted (1.15x) alongside arXiv/PubMed/doi.org, while Wikipedia/Reuters/BBC/AP get no boost at all (1.0x, same as a random blog). This is a live, measurable bias today — not latent. Fix: extend the authority tier to include major general-reference and wire-service domains at a comparable weight. *(Domain-agnosticism audit §5)*
2. **Remove `ANTI_LAZINESS_RIDER` from research judge/synthesis prompts.** Verbatim coding-agent boilerplate ("stubs," "TODO blocks," "implement ALL requested logic") injected into the *research answer synthesis and quality-judging* prompts — nonsensical noise for "summarize the causes of the French Revolution," and a plausible source of code-like completeness framing bleeding into prose synthesis. *(Domain-agnosticism audit §6)*
3. **Give the post-hoc citation audit pass its own numbered implementation task.** Stage 1's design spec named it (competitive-landscape doc's #1 differentiation finding), but it was prose-only inside a plan document, not a numbered task — and prose-only items are exactly what falls off an execution list under time pressure, per the divergence audit's own pattern finding. This item is P0 not because it's cheap, but because *failing to give it a task number* is what caused it to be skipped once already.

### P1 — High impact, directly closes Stage 1's real gaps

4. **Independent-source corroboration counting as the universal trust fallback.** The highest-leverage recommendation from the cross-domain methods survey: needs no external API, and is the single technique that makes trust scoring work for history, journalism, current events, and general research — exactly the domains `TrustScorer` currently can't reach. Feeds directly into the GUI's new "Confirmed by N independent sources" citation indicator.
5. **Domain-gate `TrustScorer`'s OpenAlex call.** Skip the per-hit title search for hits whose domain is clearly non-academic — closes the wasteful-and-fragile issue found in the domain-agnosticism audit (§1) and is a prerequisite for corroboration-counting to be the *primary* signal for non-academic hits rather than a fallback competing with a noisy academic lookup.
6. **Wire `ClaimVerdict.self_consistency` into the confidence gate.** Computed and stored since Stage 1, never consumed — genuinely live gap, not stale documentation, confirmed by direct code read. Also: rename or clearly disambiguate from `orchestrator_policy.rs`'s pre-existing, unrelated `self_consistency` field before wiring, to avoid the naming collision the divergence audit flagged.
7. **Bring `ResearchView` up to the GUI standard already set by `ClaimsView`/`NoveltyEvidencePanel`.** Headline verdict banner + expandable claim accordion (validated design, see the GUI representation doc), reusing `VerdictBadge` and the signal-grid pattern rather than inventing new components. This is the single most user-visible item in this whole synthesis — today's `ResearchView` shows literally nothing beyond raw markdown.
8. **Complete `NoveltyEvidenceBundle` population.** Confirmed still empty despite the lexical-dedup work landing — the schema this item was meant to feed was never actually fed. Once populated, the GUI work is already solved (`NoveltyEvidencePanel`'s existing signal grid is directly reusable, per the GUI doc).

### P2 — Real value, moderate effort

9. **Evidence-tier tagging per claim, decoupled from source-tier** (cross-domain methods survey technique 3, generalizing the historical primary/secondary/tertiary distinction too) — an LLM-classifiable field, no new API needed.
10. **CourtListener (legal) and ClinicalTrials.gov/openFDA (medical) validity/status checks**, wired alongside Crossref using the same "check if this record has been superseded" pattern, both free/no-auth APIs.
11. **Domain/outlet-level reputation scoring for non-academic sources** (E-E-A-T-style LLM rubric) as the non-academic analog to OpenAlex venue reputation.
12. **"Disputed narrative" report framing** for genuinely contested/corroboration-thin topics — render competing credible positions with their own citation sets instead of forcing single-answer synthesis, surfaced in the GUI as "Contested — N credible perspectives" on the headline banner.
13. **Wire `WorthinessSignalsV2` into the real SCIENTIA finding-promotion pipeline** (confirmed still zero callers) — but only after item 4's corroboration-counting and item 11's domain-reputation scoring land, so non-academic sources don't all get force-bucketed into `WorthinessProfile::Social` regardless of actual quality, per the domain-agnosticism audit's finding.

### P3 — Lower urgency, still real

14. Recency-decay weighting keyed to topic volatility (cross-domain methods survey technique 11).
15. Fact-check-org verdict lookup for checkable political/viral claims (technique 8) — no free unified API, needs targeted WebFetch, keep scoped narrow.
16. All ten Stage-1 P2-P4 carryovers confirmed still untouched: cross-encoder reranking, `synthesis_max_tokens` bump (still hardcoded 1200), semantic result caching, durable async jobs, per-hop observability, Reflexion-style self-revision, an internal eval harness, skills packaging, GUI settings-search indexing for API keys.

## The meta-finding: how Stage 2's plan must be written differently

The implementation-divergence audit's most important finding isn't any single dropped feature — it's the *pattern*: nearly every Stage 1 code-review fix was "the logic was applied to only one of N equivalent call sites" or "a cross-cutting invariant wasn't audited across sibling paths," not wrong algorithms. This directly shapes how Stage 2's implementation plan must differ from Stage 1's:

- **Every "wire X into Y" task must include a grep-verified inventory of every call/construction site X needs to reach**, established while writing the plan — not discovered during code review.
- **Cross-cutting invariants get their own audit task** (e.g. "does the corroboration-counting fallback apply everywhere `TrustScorer` is called, not just the dominant site") rather than being scoped implicitly inside one feature task.
- **Every design-spec item gets a numbered task**, even ones that "sound like one function" — P0-3's citation-audit near-miss is the concrete proof this matters.

## Recommended next step

This synthesis is the complete input for Stage 2's implementation plan. Per the brainstorming skill's terminal-state rule, translate items #1-#8 (P0/P1) into a concrete plan via the writing-plans skill next — applying the meta-finding above as a hard constraint on how each task is scoped, not just what it covers.
