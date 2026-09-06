---
title: "Deep Research GUI Representation Design (2026-08-01)"
description: "Design for surfacing trust, citation, novelty, and verification signals in Vox's ResearchView GUI, currently a raw markdown/JSON dump with none. Reuses established patterns already built for ScientiaSurface/ClaimsView/NoveltyEvidencePanel (VerdictBadge, signal grids, prior-art citation lists) rather than inventing new UI language, and extends them to be legible across subject types, not just academic/technical queries."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative GUI design for surfacing deep-research trust/citation/verification signals; direct input to the Stage 2 implementation plan."
---

# Deep Research GUI Representation Design

**Date:** 2026-08-01 (Stage 2)
**Scope:** How research runs, citations, trust/novelty/worthiness signals, and audit trails should be shown to a user — across subject types, not just code/technical queries. Companion to [deep-research-domain-agnosticism-audit-2026-08-01.md](deep-research-domain-agnosticism-audit-2026-08-01.md).

---

## The gap

`ResearchView.tsx` (`crates/vox-gui/ui/src/components/surfaces/Research/`) — the GUI's main deep-research surface — currently renders a query box, a pipeline-stage timeline while running, a session history list, and a detail pane that dumps `report_markdown ?? artifact_json` as raw preformatted text. **No trust score, citation, or verification UI at all.**

This is a real gap, but not a design vacuum: three sibling surfaces in the same app already solved this problem for adjacent workflows, and their patterns should be reused rather than re-invented:

- **`ClaimsView.tsx`** has a `VerdictBadge` component (Supported/Contested/Contradicted/Abstain, color-coded) plus confidence, verifiability score, and verifier model per claim.
- **`NoveltyEvidencePanel.tsx`** (in `DiscoveryReview.tsx`) is the most citation/trust-rich surface in the codebase: a verdict chip (Novel/Possibly novel/Not novel/Contradicted/Insufficient evidence), a signal grid (max semantic/lexical similarity, near-hit count, sources succeeded), a "closest prior art" link with similarity score, a prior-art list (title/year/citation count/semantic score), and an "Evidence conflicts" section splitting supporting vs. contradicting hits with excerpts.
- **`PublicationsView.tsx`** shows extracted claims with verdict_label + confidence in a drill-down panel.

The design below is explicitly "bring `ResearchView` up to the standard the rest of the app already has," extended to work when the underlying signals are corroboration-count-based (per the cross-domain methods survey) rather than only citation-count/DOI-based.

## Layout: headline verdict + expandable claim accordion

Two decisions, validated with the user via mockup comparison:

**1. A headline verdict banner at the top of every research result**, before any report text — reusing the confidence-tier concept already computed by `gate.rs`'s `routing_tier_for` (Direct/Light/DeepResearch) and `ConfidenceSignal.score`. Framed as "how sure should you be," not a bare number: e.g. "High confidence — 9 corroborating sources, no contested claims" or "Mixed evidence — 2 of 14 claims contested, treat with care." This gives a user an immediate, honest read on the whole run before they invest in reading the report — directly answering the competitive-landscape finding that no reviewed competitor (Google/Claude/OpenAI/Perplexity) does this well; most either omit confidence framing entirely or bury it.

**2. Report renders full-width as today, unchanged** — the flowing prose stays clean and readable. Below it, a collapsed-by-default accordion: `"12 claims verified · 2 contested · 47 sources"`. Expanding reveals each extracted claim as a row: `VerdictBadge` + confidence + **`self_consistency`** (once wired into the gate per the implementation-divergence audit's finding — this is a genuinely new, honest signal none of the competitors surface: "this verdict was stable across 3 independent resamples" vs. "this verdict flipped between samples") + its supporting citations, each showing a trust indicator.

This was chosen over (a) inline hover-badges (loses the "at a glance" summary the headline banner already provides, and inline badges get lost in long reports) and (b) a persistent side panel (costs permanent horizontal space for information most users only want to drill into occasionally, not for every single read).

## Per-citation trust indicator: adapting to subject type

The domain-agnosticism audit found `TrustScorer` is scholarly-paper-shaped (Crossref retraction + OpenAlex venue reputation). The GUI must not just wait for those APIs to expand — it should degrade its own *display* gracefully today, and be built to show richer signals as the cross-domain methods survey's recommendations land:

- **Has a DOI / academic venue signal** (current `TrustScorer` output): show venue-type chip (Journal/Repository/Preprint) + retraction status, matching `WorthinessProfile`'s existing taxonomy.
- **No DOI, but corroborated by N independent sources** (the survey's #1 recommendation — works today with zero new APIs): show a corroboration count badge, `"Confirmed by 4 independent sources"`, sourced from the existing hit-clustering the pipeline already does for CRAG evidence gathering — this is the single cheapest, highest-coverage trust signal to surface, since it requires no new external dependency, only exposing data the pipeline already computes internally.
- **No DOI, no corroboration count available** (a single uncorroborated source): show this honestly as `"Single source — not independently corroborated"` rather than a numeric score that implies false precision. This is the domain-agnostic equivalent of the current `WorthinessProfile::Social` fallback, but framed as an honest caveat rather than a silently-low hidden score.

This gives every citation a legible trust indicator regardless of subject — a legal query's CourtListener validity signal, a medical query's evidence-tier tag, or a historical query's corroboration count all render through the same 3-tier chip pattern (Formal signal / Corroborated / Uncorroborated), so the GUI doesn't need N different citation-widget variants per domain — one widget, multiple possible signal sources feeding it.

## Novelty/worthiness signals in the GUI, once wired

Per the implementation-divergence audit, `WorthinessSignalsV2` and `NoveltyEvidenceBundle` are built but not yet wired into any production path. When they land (a Stage 2 implementation task, not this doc's scope to build), the GUI surface is already solved: `NoveltyEvidencePanel.tsx`'s existing signal-grid pattern is directly reusable for a research run's "is this actually new information, or does the pipeline think it already knew this" signal — no new component needed, just a new data source feeding the existing panel shape.

## Cross-subject-type considerations for the report body itself

Beyond citation/trust widgets, two report-body changes fall out of the domain-agnosticism audit:

- The `ANTI_LAZINESS_RIDER` coding-boilerplate found in the synthesis/judge prompts should not just be removed from the prompt (a backend fix, out of this doc's scope) — its removal should be visible in the GUI as more natural prose synthesis for non-technical queries, which is worth a quick before/after spot-check once that backend fix lands, not a GUI change of its own.
- For genuinely contested/corroboration-thin topics (per the historical/journalistic research patterns), the report body should be able to render a **"disputed narrative" framing** — presenting the strongest version of each credible position with its own citation set, rather than forcing a single synthesized answer the way a factual/technical query naturally can be. This is a report-generation change (synthesis prompt), surfaced in the GUI simply by rendering the existing headline-verdict banner as "Contested — N credible perspectives" instead of a single confidence tier when the pipeline detects this shape (multiple `Contested`-verdict claims backed by roughly equal corroboration on each side).

## Out of scope for this doc

Actual component implementation (React/TSX), the backend wiring for corroboration-count computation and exposure via the research API, and the `WorthinessSignalsV2`/`NoveltyEvidenceBundle` production wiring itself are all implementation-plan work, not design-doc work — this doc establishes the layout and signal-taxonomy decisions the plan will implement against.
