---
title: "Beyond Crossref/OpenAlex: Cross-Domain Research Methods Survey (2026-08-01)"
description: "Surveys how legal, medical, historical, and journalistic research validate sources/claims without a DOI-keyed citation graph, extracting 11 concrete, adoptable techniques (citation-validity graphs, evidence-tier tagging, corroboration counting, domain reputation scoring) for Vox's trust/novelty pipeline, which is currently scholarly-paper-shaped."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative cross-domain research-methods survey grounding the deep-research Stage 2 domain-agnosticism work; identifies concrete, implementable techniques with data-source notes."
---

# Beyond Crossref/OpenAlex: Cross-Domain Research Methods Survey

**Date:** 2026-08-01 (Stage 2)
**Scope:** How legal, medical, historical, journalistic, and general/consumer research validate sources and claims without a DOI-keyed academic citation graph — the exact gap in Vox's current `TrustScorer` (Crossref retraction + OpenAlex venue reputation), which only functions for scholarly-paper-shaped sources. Companion to [deep-research-domain-agnosticism-audit-2026-08-01.md](deep-research-domain-agnosticism-audit-2026-08-01.md).

---

Vox's current trust/novelty scoring pipeline treats every source as if it were a journal article: Crossref answers "has this been retracted?" and OpenAlex answers "is this a reputable venue (journal/repository/conference/preprint)?" Both depend on a DOI. The moment a query touches history, law, current events, or consumer products, both signals go silent — not because the source is untrustworthy, but because the infrastructure that scores it doesn't exist in DOI-space. Below are eleven concrete, structurally adoptable techniques from five fields that solve exactly this problem, each with an implementation note.

## 1. Legal research: citation validity graphs + jurisdiction weighting

**Technique 1 — Shepardizing/KeyCite-style validity signal.** Legal researchers don't just check if a case exists; they check its *current* precedential status by walking the graph of everything that has cited it since. Shepard's (LexisNexis) and KeyCite (Westlaw) assign color-coded signals — green ("good law"), yellow ("distinguished/criticized"), red ("overruled/reversed") — computed by classifying every subsequent citing document's treatment of the cited case ([BYU Law Library](https://guides.law.byu.edu/c.php?g=315332&p=2106927), [Westlaw KeyCite guide](https://librarians.blog/keycite-shepardizing-signal-flags-validity-5200)). Adoptable structure: **don't just check if a source is flagged (retracted); check the *citation-treatment* of everything downstream of it** — has anything superseded, criticized, or reversed the claim since. *Data*: [CourtListener / Free Law Project REST API](https://www.courtlistener.com/help/api/rest/) is free, no-auth, and exposes exactly this — 9M+ case law decisions with a citation-network API. For non-legal domains, no equivalent free citation-treatment API exists — needs an LLM heuristic classifying each citing/discussing source as affirming, contradicting, or superseding.

**Technique 2 — Primary vs. secondary source hierarchy with jurisdiction-aware weighting.** Legal research ranks statute/constitution/regulation (primary, binding) above a law review article or treatise (secondary, persuasive only), further weighted by jurisdictional scope — a controlling appellate decision in the same circuit outranks an out-of-circuit or lower-court opinion ([Monmouth Univ. Library](https://guides.monmouth.edu/legal_research/shepardizing)). Adoptable structure: **source-type tier + scope-of-authority weighting**, not tier alone. *Data*: no clean free API for jurisdiction inference; needs LLM classification of "what authority/scope does this source claim."

## 2. Medical research: evidence-hierarchy tiers + non-journal retraction signals

**Technique 3 — Tag every claim with an evidence-tier, not just a source-tier.** Clinical guidelines score the *study design* backing a specific claim (systematic-review/meta-analysis > RCT > cohort > case-control > case report > expert opinion), then apply GRADE to rate confidence per outcome, downgrading for risk of bias/imprecision/inconsistency/indirectness/publication-bias and upgrading for large effect size ([GRADE overview, PMC](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC2735783/)). This decouples "is the venue reputable" from "is the *evidence type* for *this specific claim* strong" — the same journal can carry a case report (weak) and a meta-analysis (strong). Adoptable structure: **classify each extracted claim by evidence-type tier, independent of venue reputation, surfacing both scores separately.** *Data*: no API; LLM heuristic reading study-design language ("randomized," "case report," "systematic review of N studies").

**Technique 4 — Retraction/safety signals outside the DOI graph.** Most medically consequential corrections never touch Crossref: a drug recall, a trial terminated for safety, an FDA communication. ClinicalTrials.gov's v2 API returns a trial's current status (terminated/withdrawn/suspended) by NCT ID, and openFDA exposes drug/device recall and adverse-event data, both free/no-auth ([openFDA](https://open.fda.gov/apis/drug/), [ClinicalTrials.gov](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC11975776/)). Adoptable structure: **when a source cites a clinical trial or drug, cross-check trial/recall status as an independent negative-signal check, parallel to Crossref.**

## 3. Historical research: corroboration-as-trust-signal + provenance chain

**Technique 5 — N-source corroboration in place of a retraction database.** There is no "Crossref" for a 200-year-old letter. Historians treat convergence across *independent* sources as the trust signal itself — a claim gains credibility when sources that couldn't have copied from each other agree, and loses it when it rests on a single, uncorroborated account ([UMass Dartmouth](https://guides.lib.umassd.edu/c.php?g=1072033&p=7805616)). This is the single most portable technique here because it needs **no external API** — it's a property of the retrieved set itself. Adoptable structure: **for any claim with no DOI/retraction path, count independent corroborating sources (with an independence check — same author/outlet/wire-service origin doesn't count as a second source) and let corroboration count substitute for the trust score Crossref/OpenAlex would otherwise supply.**

**Technique 6 — Primary/secondary/tertiary tiering + provenance/chain-of-custody.** Historians rank an original artifact above an interpretive work above a summary-of-summaries ([UMN Crookston](https://crk.umn.edu/library/primary-secondary-and-tertiary-sources)), separately tracking *provenance* — a transcribed/translated copy is a strictly weaker witness than the original even if both are "primary" in genre. Adoptable structure: **tag sources on two independent axes — genre tier and copy-distance-from-original — since OpenAlex's single venue-reputation axis conflates both.**

## 4. Journalism: independent-source counting + outlet-level correction track record

**Technique 7 — The two-source rule as a formalized corroboration threshold.** Traditional journalism requires a claim (especially a sensitive one) to be confirmed by at least two independent sources before publication. Same structure as Technique 5 but with an explicit minimum-count gate: **do not surface a claim as high-confidence unless it clears a corroboration-count threshold**, rising for high-stakes claim types.

**Technique 8 — Structured verdict methodology from fact-checking orgs.** PolitiFact and Snopes publish transparent methodologies; a comparison study found the two orgs agreed on all but one of 749 matching claims after normalizing rating scales — independently-run verdict processes converging is itself evidence the process is sound ([Ballotpedia](https://ballotpedia.org/The_methodologies_of_fact-checking)). Adoptable structure: **when a fact-checking org has already rated a specific claim, surface that verdict as a first-class trust signal** — the closest non-academic analog to a citation. *Data*: no unified free API; needs targeted WebFetch against known fact-check domains for checkable (political/viral) claims.

**Technique 9 — Outlet-level correction-track-record as a trust prior.** Transparent, frequent correction of errors is a positive trust signal for an outlet; a pattern of uncorrected errors is negative — the news-domain equivalent of retraction, scoped to the *publisher* not the article. NewsGuard operationalizes this: trained journalists score 2,100+ sites 0–100 across nine criteria including "publishing transparent corrections" ([NewsGuard](https://www.newsguardtech.com/ratings/rating-process-criteria/)). *Data*: commercial/licensed, not free-tier — flag as a paid-API option or replace with an LLM heuristic checking a domain's public track record.

## 5. General/consumer research: domain-reputation heuristics without any citation graph

**Technique 10 — E-E-A-T-style multi-factor scoring for sources with zero formal citation infrastructure.** Google's Search Quality Rater Guidelines formalize what to check when there's no DOI, no peer review, often no byline: Experience (demonstrable first-hand use), Expertise (accurate terminology, structured depth), Authoritativeness (referenced by other independent, already-trusted sources), and Trustworthiness — stated as "the most important member of the family" ([SEJ E-E-A-T guide](https://www.searchenginejournal.com/google-e-e-a-t-how-to-demonstrate-first-hand-experience/474446/)). Adoptable structure: **replace the single OpenAlex "venue type" score with a multi-factor LLM-scored rubric** for any non-academic source. *Data*: no clean API — explicitly not machine-computed even at Google; an LLM-prompting heuristic over source text + metadata.

**Technique 11 — Recency-sensitivity as an explicit claim property.** Consumer/current-events research treats "how fast does this fact decay" as a property distinct from source quality — a 3-year-old product review about a discontinued model is stale regardless of outlet authority, whereas a historical primary source doesn't decay at all. Vox's current pipeline treats publication date as roughly constant-value metadata rather than a decay function tied to subject volatility. *Data*: no API; a query-time classification ("fast-decaying topic — prices, availability, current events — or slow-decaying — history, established facts?") setting a recency-weight multiplier, reusing date metadata already collected.

## Synthesis: top 5 additions for Vox's trust scoring

Vox's current scoring answers exactly one question — "is this a legitimate, unretracted piece of the academic literature" — and goes silent for anything without a DOI, which is most of history, law, journalism, and consumer research. Priority order:

1. **Independent-source corroboration counting as the universal fallback trust signal** (Techniques 5 & 7). Highest leverage: needs no external API and covers every domain Crossref/OpenAlex can't reach. Count independent, non-derivative corroborating sources when there's no DOI, with an LLM independence-check filtering out sources sharing a common wire-service/press-release origin.
2. **Per-claim evidence-tier tagging, decoupled from source-tier** (Technique 3, generalizing Technique 6). Score the evidence type behind each specific claim (primary/eyewitness vs. secondary analysis vs. tertiary summary vs. opinion), not just the venue.
3. **Domain/outlet-level reputation and corrections-track-record scoring for non-academic sources** (Techniques 9 & 10). A cacheable per-domain LLM-rubric reputation score as the non-academic analog to OpenAlex venue reputation.
4. **Domain-specific validity/status APIs wired in alongside Crossref, not instead of it** (Techniques 1 & 4 — CourtListener for legal, ClinicalTrials.gov/openFDA for medical). Both free, no-auth REST APIs slotting into the existing "check if this record has been superseded/invalidated" pattern Crossref already implements, keyed to a different identifier (case citation, NCT number, drug name) than a DOI.
5. **A recency-decay weight keyed to claim/topic volatility** (Technique 11), cheap to add via query-time LLM topic-volatility classification reusing date metadata already collected.

## Sources

- [BYU Law Library — Introduction to Shepard's Citators](https://guides.law.byu.edu/c.php?g=315332&p=2106927)
- [Westlaw KeyCite Signal Flags Guide](https://librarians.blog/keycite-shepardizing-signal-flags-validity-5200)
- [Monmouth University — Shepardizing/Legal Research Guide](https://guides.monmouth.edu/legal_research/shepardizing)
- [CourtListener / Free Law Project REST API Documentation](https://www.courtlistener.com/help/api/rest/)
- [Grading Quality of Evidence and Strength of Recommendations: A Perspective (PMC)](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC2735783/)
- [Grading quality of evidence and strength of recommendations — Allergy (Wiley)](https://onlinelibrary.wiley.com/doi/10.1111/j.1398-9995.2009.01973.x)
- [open.fda.gov Drug API Endpoints](https://open.fda.gov/apis/drug/)
- [The importance of ClinicalTrials.gov in informing trial design, conduct, and results (PMC)](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC11975776/)
- [University of Minnesota Crookston — Primary, Secondary, and Tertiary Sources](https://crk.umn.edu/library/primary-secondary-and-tertiary-sources)
- [UMass Dartmouth — Critical Assessment of Primary and Secondary Sources](https://guides.lib.umassd.edu/c.php?g=1072033&p=7805616)
- [Ballotpedia — The Methodologies of Fact-Checking](https://ballotpedia.org/The_methodologies_of_fact-checking)
- [PolitiFact — The Principles of the Truth-O-Meter](https://politifact.com/article/2018/feb/12/principles-truth-o-meter-politifacts-methodology-i/)
- [NewsGuard — Website Rating Process and Criteria](https://www.newsguardtech.com/ratings/rating-process-criteria/)
- [NewsGuard — Press release on rating adoption](https://www.newsguardtech.com/press/over-2100-websites-improve-journalism-practices-newsguard-rating/)
- [Search Engine Journal — Google E-E-A-T Guide](https://www.searchenginejournal.com/google-e-e-a-t-how-to-demonstrate-first-hand-experience/474446/)
- [Yoast — What is E-E-A-T?](https://yoast.com/what-is-e-e-a-t/)
