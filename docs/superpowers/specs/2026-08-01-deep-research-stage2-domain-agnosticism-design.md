---
title: "Deep Research Stage 2 — Domain-Agnosticism & GUI Trust Surfacing Design"
description: "Design spec for the P0/P1 items from the Stage 2 synthesis: fix web_dispatcher's code-site ranking bias, drop the coding-agent boilerplate from research prompts, give the citation audit a numbered task, add corroboration counting as the universal trust fallback, domain-gate TrustScorer's OpenAlex call, wire self_consistency into the confidence gate, bring ResearchView up to the app's existing trust-UI standard, and complete NoveltyEvidenceBundle population."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative design spec for the deep-research Stage 2 domain-agnosticism and GUI trust-surfacing work; direct input to its implementation plan."
---

# Deep Research Stage 2 — Domain-Agnosticism & GUI Trust Surfacing Design

**Date:** 2026-08-01
**Inputs:** [deep-research-stage2-synthesis-2026-08-01.md](../../src/architecture/deep-research-stage2-synthesis-2026-08-01.md) and its four supporting docs (cross-domain methods survey, domain-agnosticism audit, GUI representation design, implementation-divergence audit). Covers synthesis items #1–#8 (P0/P1) only; P2/P3 are explicitly deferred — see "Deferred" below.

---

## Problem statement

Stage 1 built real trust/novelty/worthiness infrastructure, but Stage 2's audit found it's shaped for academic/technical queries and its output never reaches the user: `web_dispatcher.rs` ranks GitHub above Reuters, `ResearchView` shows raw markdown with zero trust UI, and a promised citation-audit feature was silently dropped because it was never a numbered task. This spec fixes the ranking bias, gives every citation an honest trust signal that works for any subject (not just DOI-bearing papers), and surfaces all of it in the GUI — while building in the call-site-inventory discipline the divergence audit says Stage 1 lacked.

## Design

### 1. Fix `web_dispatcher.rs`'s source-authority bias (P0-1)

**File:** `crates/vox-search/src/web_dispatcher.rs`, `source_authority_score` (lines 206-224).

Add a general-authority tier alongside the existing `.gov`/`.edu` tier (1.25x), so wire services and major reference sites aren't stuck at the same 1.0x as a random blog:

```rust
fn source_authority_score(url: &str) -> f64 {
    let key = url.to_ascii_lowercase();
    if key.contains(".gov/") || key.contains(".edu/")
        || key.contains("wikipedia.org/") || key.contains("reuters.com/")
        || key.contains("apnews.com/") || key.contains("bbc.co") { 1.25 }
    else if key.contains("arxiv.org/") || key.contains("doi.org/")
        || key.contains("pubmed.ncbi.nlm.nih.gov/")
        || key.contains("docs.rs/") || key.contains("github.com/") { 1.15 }
    else { 1.0 }
}
```

Update the existing test `rank_and_dedupe_prefers_authoritative_free_sources` (line 241) to also assert a `wikipedia.org` hit outranks a plain blog, and add a new test asserting `reuters.com` ranks at the same tier as `.gov`. This is a hardcoded list, same shape as today's — item #4 (corroboration counting) is the longer-term replacement for hardcoded domain lists, not a blocker for this fix.

**Call-site inventory:** `source_authority_score` has exactly one call site, inside `rank_and_dedupe_results` in the same file — confirmed by grep. No fan-out risk.

### 2. Remove `ANTI_LAZINESS_RIDER` from research prompts (P0-2)

**File:** `crates/vox-research-shim/src/research/orchestrator/config.rs` (constant, lines 11-15), consumed at `stages.rs:87` (judge prompt) and `stages.rs:230` (synthesis prompt).

**Call-site inventory (grep-verified):** exactly 2 consumption sites, both in `stages.rs`. Delete the constant and both injection points. Replace with research-appropriate completeness language at the same 2 sites: `"Cite every material claim. Do not omit contradicting evidence. Do not pad the summary with unsupported filler."` — same completeness intent, no code-generation vocabulary ("stubs," "TODO blocks").

No other crate references `ANTI_LAZINESS_RIDER` — confirmed by grep before implementation.

### 3. Give the citation audit its own numbered task (P0-3)

Not a design decision — an execution-discipline fix. The Stage 2 implementation plan (written next, via `writing-plans`) must include a standalone numbered task titled exactly "Post-hoc citation audit pass," not folded as a bullet inside a larger task. Scope: after synthesis, re-fetch each cited source and verify the citation actually supports the claim attributed to it (already-designed in Stage 1's spec, never executed — see divergence audit §4). This spec item exists purely to prevent the same silent drop from recurring.

### 4. Independent-source corroboration counting (P1-4)

**New module:** `crates/vox-search/src/corroboration.rs`.

The pipeline already clusters hits for CRAG evidence-gathering (`web_gather.rs`) — this reuses that clustering rather than adding new retrieval. For a given claim, count the number of **distinct-domain** hits whose retrieved evidence text supports it (using the same NLI/verifier judgment `verifier.rs` already produces per hit — no new classification model needed). Distinct-domain, not distinct-URL, so 3 pages on the same site don't count as 3 corroborations.

```rust
pub struct CorroborationCount {
    pub claim_id: String,
    pub supporting_domains: Vec<String>,
}

pub fn count_corroboration(claim_id: &str, verified_hits: &[VerifiedHit]) -> CorroborationCount {
    let supporting_domains: Vec<String> = verified_hits.iter()
        .filter(|h| h.verdict == Verdict::Supported)
        .map(|h| domain_of(&h.url))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    CorroborationCount { claim_id: claim_id.to_string(), supporting_domains }
}
```

**Call-site inventory:** consumed by (a) `gate.rs`'s fusion score as a new signal — becomes the primary trust signal for hits with no DOI (see item #5), and (b) the GUI's per-citation trust chip (item #7) via the research API response. Both are new consumption sites created by this spec, not pre-existing sites needing a fan-out audit.

### 5. Domain-gate `TrustScorer`'s OpenAlex call (P1-5)

**File:** `crates/vox-search/src/trust.rs`, `score_hit_trust` (line 185), called unconditionally from `web_gather.rs:33`.

Add a domain check before the OpenAlex title search:

```rust
fn is_plausibly_academic(url: &str) -> bool {
    let key = url.to_ascii_lowercase();
    key.contains("doi.org/") || key.contains("arxiv.org/") || key.contains(".edu/")
        || key.contains("pubmed.ncbi.nlm.nih.gov/") || key.contains("ncbi.nlm.nih.gov/")
        || key.contains("researchgate.net/") || key.contains("springer.com/")
        || key.contains("sciencedirect.com/") || key.contains("jstor.org/")
}
```

`score_hit_trust` skips `venue_type`/`reputation_multiplier` (OpenAlex call) entirely when `is_plausibly_academic` is false, returning the neutral `1.0` multiplier immediately — same fail-open result as today's coincidental-no-match case, but without the wasted network call or title-collision misclassification risk. `check_retraction`'s existing `doi.org` URL match is untouched (already correctly gated).

**Call-site inventory:** `score_hit_trust` has one call site (`web_gather.rs:33`) — confirmed by grep. This is an internal short-circuit, no fan-out.

For hits that fail the academic gate, `pipeline.rs`'s trust-weighted citation score (item #5's consumer) falls back to item #4's corroboration count as the primary signal instead of a neutral 1.0 — this is the actual value: non-academic hits get a *real* trust signal (corroboration) rather than just "not penalized."

### 6. Wire `self_consistency` into the confidence gate (P1-6)

**File:** `crates/vox-research-events` (or wherever `ClaimVerdict` is defined — confirm exact path during plan-writing) and `crates/vox-research-shim/src/research/gate.rs`, `pipeline.rs`'s currently-unconsumed weighting comment.

**Naming-collision fix first:** `orchestrator_policy.rs` has a pre-existing, unrelated `self_consistency` field (weight 0.20, `contradiction_hints`-derived). Before wiring `ClaimVerdict.self_consistency`, rename one of the two fields to remove ambiguity — proposed: rename `ClaimVerdict.self_consistency` to `ClaimVerdict.resample_stability` (it measures verdict stability across LLM resamples, which is a more precise name anyway) and leave `orchestrator_policy.rs`'s field as-is since it's an unrelated, older signal.

**Call-site inventory (grep-verified before implementation):** every construction site of `ClaimVerdict` must be checked to confirm `resample_stability` is populated there, not just at the primary verifier call site — this is exactly the kind of "wire X into Y, but which Y sites" gap the divergence audit flagged. The implementation plan's task must include this grep as an explicit sub-step, not assume one call site.

Gate change: `pipeline.rs`'s `supported_claim_count` weighting multiplies each supported claim's contribution by its `resample_stability` (0.0-1.0) instead of counting all supported claims equally — a claim that flipped verdict across resamples contributes less to the gate score than one that was stable.

### 7. Bring `ResearchView` up to the GUI trust standard (P1-7)

**Files:** `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx` and a new `ResearchClaimAccordion.tsx` component (co-located in the same directory), reusing `ClaimsView.tsx`'s `VerdictBadge` import.

Per the GUI representation design doc's validated mockup (Option C + headline banner):

1. **Headline verdict banner** — new component at the top of the detail pane, above the existing report render. Reads `routing_tier_for` output + `ConfidenceSignal.score` (already computed by `gate.rs`, exposed via the research API response — confirm exact field name during plan-writing) and renders one of three framings: `"High confidence — N corroborating sources, no contested claims"` / `"Mixed evidence — N of M claims contested, treat with care"` / `"Contested — N credible perspectives"` (this third framing only fires when the pipeline's contested-claim ratio crosses a threshold, established alongside item #9's disputed-narrative framing in a later phase — for this spec, ship the first two framings only and stub the contested-narrative detection as a simple ratio check: `contested_claims / total_claims > 0.3`).
2. **Report body** — unchanged, full-width markdown render as today.
3. **Claim accordion** — collapsed by default, header text `"{n} claims verified · {m} contested · {k} sources"`. Expanding renders one row per claim: `VerdictBadge` (imported from `ClaimsView.tsx`, not reimplemented) + confidence + `resample_stability` (from item #6, rendered as `"Stable across N resamples"` / `"Verdict flipped in resampling — treat with care"`) + a citation list, each citation showing the 3-tier trust chip from item #8.

**Call-site inventory:** `ResearchView.tsx` is the only consumer of the research-run detail data added here — no fan-out to other GUI surfaces required for this item.

### 8. Per-citation 3-tier trust chip (P1-7, continued)

New shared component `TrustChip.tsx` (co-located with `ResearchClaimAccordion.tsx`, reusable by other surfaces later per the GUI design doc's "one widget, multiple signal sources" principle):

- **Formal signal** (has DOI/academic venue from `TrustScorer`): `"{venue_type} · not retracted"` or `"RETRACTED"` in a warning color.
- **Corroborated** (item #4's count ≥ 2, no DOI): `"Confirmed by {n} independent sources"`.
- **Uncorroborated** (single source, no DOI): `"Single source — not independently corroborated"`, styled as a neutral caveat, not a low score.

This is a pure display component — no new backend call, consumes fields already produced by items #4/#5.

### 9. Complete `NoveltyEvidenceBundle` population (P1-8)

**File:** wherever `NoveltyEvidenceBundle` is defined (confirm exact path — `vox-scientia` per the divergence audit) and the dedup pipeline that currently only does lexical-shingle similarity (`novelty_lexical.rs`).

**Call-site inventory (grep-verified before implementation):** the divergence audit confirmed zero construction sites exist today. The plan must grep for every place a novelty/dedup verdict is currently produced and add `NoveltyEvidenceBundle` construction at each one — do not assume there's a single natural site.

Populate the bundle's OpenAlex/Crossref/Semantic Scholar prior-art fields from data the pipeline already fetches for `TrustScorer` (item #5) where available — avoid a second independent API-call path for the same lookups. Once populated, `NoveltyEvidencePanel.tsx`'s existing signal-grid UI is reused as-is for a research run's novelty display, per the GUI design doc — no new frontend component needed for this item.

## Deferred (P2/P3, per synthesis doc)

Evidence-tier tagging, CourtListener/ClinicalTrials.gov/openFDA integration, domain/outlet reputation scoring, disputed-narrative report framing beyond the simple ratio stub in item #7, `WorthinessSignalsV2` production wiring, recency-decay weighting, fact-check-org lookup, and all ten Stage-1 P2-P4 carryovers. Explicitly out of scope for the plan this spec feeds.

## Cross-cutting invariant audit tasks (required in the implementation plan)

Per the divergence audit's meta-finding, the plan must include these as their own tasks, not implicit sub-steps:

- **Fail-open consistency**: every new gating point (item #5's academic-domain gate, item #7's contested-narrative stub) must default to the more-permissive branch on ambiguous input, matching the existing `TrustScorer` fail-open pattern — audited across all 3 new gates in one task, not per-item.
- **`ClaimVerdict` construction-site inventory** (item #6) and **`NoveltyEvidenceBundle` construction-site inventory** (item #9) each get their own grep-and-list sub-task before the wiring code is written, per the divergence audit's explicit recommendation.

## Self-review

- No placeholders/TODOs left in this spec's code sketches — all are illustrative, not literal, and the plan is expected to confirm exact field/path names via grep before implementing (called out inline at 3 points above where a name wasn't confirmed in this research pass).
- Every P0/P1 synthesis item (1-8) has a corresponding numbered section.
- Scope check: GUI, backend ranking, trust scoring, and gate wiring are all covered per the original ask ("consider the GUI for all aspects... auditing and search pipeline").
- Ambiguity check: the `ClaimVerdict` rename (item #6) and the `NoveltyEvidenceBundle` exact path (item #9) are the two genuine unknowns flagged for plan-time verification, not implementation-time surprises.

## Recommended next step

Invoke `superpowers:writing-plans` to turn this spec into a numbered, grep-verified implementation plan — applying the cross-cutting invariant audit tasks above as mandatory plan entries, not optional extras.
