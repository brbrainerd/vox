---
title: "Deep Research Domain-Agnosticism Audit (2026-08-01)"
description: "Code-level audit of Vox's deep-research pipeline for academic-paper or code/technical bias. Finds a live ranking bias favoring github.com/docs.rs over Wikipedia/Reuters/BBC in web_dispatcher.rs, coding-agent boilerplate (ANTI_LAZINESS_RIDER) leaking into research judge/synthesis prompts, an unguarded per-hit OpenAlex title search, and a worthiness taxonomy that would force-bucket all non-academic sources into one low-trust class once wired in. Confirms novelty/dedup, the planner, and the confidence gate are already genuinely domain-neutral."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative code-level bias audit for the deep-research domain-agnosticism work; identifies concrete, file:line-cited fixes."
---

# Deep Research Domain-Agnosticism Audit

**Date:** 2026-08-01 (Stage 2)
**Scope:** Does Vox's deep-research pipeline structurally assume an academic/scholarly or code/technical research query? Audited against the actual current code (not docs). Companion to [deep-research-cross-domain-methods-survey-2026-08-01.md](deep-research-cross-domain-methods-survey-2026-08-01.md).

---

## Summary

**Genuinely domain-neutral (confirmed, no bias found):** novelty/dedup (`vox-search/src/novelty.rs`, `vox-scientia/src/producers/novelty_lexical.rs`), the query planner's system prompt (`planner.rs`), and the confidence gate's fusion math (`gate.rs`) — none contain domain-specific language or logic.

**Confirmed biases, ranked by actual current impact:**
1. **`web_dispatcher.rs`'s source-authority ranking boost** — live, measurable bias today.
2. **`ANTI_LAZINESS_RIDER`** coding-agent boilerplate injected into research judge/synthesis prompts — cosmetic/latent bias.
3. **`TrustScorer`'s unconditional per-hit OpenAlex title search** — wasteful and theoretically fragile, but currently neutralized downstream.
4. **`worthiness.rs`'s Journal/Preprint/Repository/Social taxonomy** — a real design flaw, honestly disclosed as not yet wired into production.

## 1. `crates/vox-search/src/trust.rs` — TrustScorer

`score_hit_trust(title, doi)` (line 185) is called unconditionally for *every* web hit in `web_gather.rs:33`, regardless of source type. `check_retraction` (line 47) only matches `doi.org`/`dx.doi.org` URLs via `extract_doi_from_url` (line 169) — for a Wikipedia/`.gov`/newspaper/blog hit, this path is simply skipped, correctly fail-open. But `venue_type`/`reputation_multiplier` (lines 89, 133) does an OpenAlex **full-text title search** using the hit's title text — not URL or domain — for literally every hit, academic or not.

**Trace for a non-academic URL:** for a Wikipedia or NYT article, there's no domain-based short-circuit; the code searches OpenAlex by title text regardless. Most such titles won't match an indexed scholarly work, so `venue_type` returns `None` and `reputation_multiplier` falls through to the neutral `1.0` default (line 138) — this fail-open behavior works correctly for the common case.

**Two real issues:** it's not domain-gated (every hit, including a HuffPost blog post, triggers a live OpenAlex call keyed on a fuzzy title-text match, with no domain guard against coincidental title collisions misclassifying a hit as "journal" venue type), and it's wasteful (an unconditional network call per hit regardless of source type). Mitigating factor: `pipeline.rs:404-411` caps each hit's trust contribution at `min(1.0)`, so a spurious "journal" boost can only ever be neutral, never inflate the gate score above what plain citation counting would give — the bias exists but its downstream effect is currently dampened, not harmful.

## 2. `novelty_lexical.rs` / `novelty.rs` — confirmed domain-neutral

Pure 4-gram character shingling + FNV1a hashing on lowercased text (`novelty.rs:14`) and plain Jaccard similarity over shingle sets (`novelty_lexical.rs:12`) — no keyword lists, no code-specific tokenization, no academic assumptions. Tests use non-technical text ("sourdough bread fermentation"). No bias found.

## 3. `worthiness.rs` — WorthinessProfile taxonomy

`soft_gate_peer_review_signal` (line 38) maps OpenAlex `venue_type` to `Journal`/`Repository`/`Preprint`; **everything else — unrecognized string or `None` — collapses to `WorthinessProfile::Social`** with `passed: false, score: 0.2` (lines 55-61). A well-corroborated investigative news piece and a random forum post are both "Social" with an identical failed soft-gate — no differentiation by actual quality, corroboration, or authority for non-academic sources.

However, the file's own doc comment (lines 4-8) states plainly this infrastructure "has no production callers yet." Confirmed via grep: not wired into the finding-promotion pipeline. **Real design flaw when eventually activated, but zero live effect today.**

## 4. `planner.rs` / `gate.rs` — confirmed domain-neutral

Planner's system prompt (`planner.rs:52-58`) is `"Decompose the user's research question into 3-{max_subqueries} precise web/local retrieval subqueries"` — generic, no code/docs/technical language. Gate is purely numeric fusion (citation coverage 0.35, claim-support 0.30, domain diversity 0.20, retrieval floor 0.15) — no LLM prompt at all. No bias found in either.

## 5. `web_dispatcher.rs` — confirmed, live ranking bias

`source_authority_score` (lines 206-224), used by `rank_and_dedupe_results` to multiply-boost ranking scores before hits are returned:

```rust
if key.contains(".gov/") || key.contains(".edu/") { 1.25 }
else if key.contains("arxiv.org/") || key.contains("doi.org/")
     || key.contains("pubmed.ncbi.nlm.nih.gov/")
     || key.contains("docs.rs/")
     || key.contains("github.com/") { 1.15 }
else { 1.0 }
```

This is an explicit, hardcoded preference boost for `github.com` and `docs.rs` (Rust crate documentation) at the **same tier as arXiv/PubMed/doi.org** — code-hosting and code-documentation sites get a ranking bump identical to peer-reviewed literature aggregators. Meanwhile broadly authoritative general-interest sources — Wikipedia, Reuters, BBC, NYT, AP — get **no boost at all**, falling to the same `1.0` default as any random blog.

**Concrete example:** for the query "what caused the 2008 financial crisis," a `github.com` repo containing an unrelated toy finance model would outrank a `wikipedia.org` or `reuters.com` article on the same topic, purely due to this multiplier, all else equal. Confirmed by the existing test at line 241 (`rank_and_dedupe_prefers_authoritative_free_sources`), which explicitly asserts `docs.rs` outranks a blog — the bias is intentional, just scoped too narrowly to code-adjacent domains when it should also cover general-authority domains.

`tavily_research.rs`, `searxng.rs`, and `duckduckgo.rs` contain no similar biasing logic themselves — only `web_dispatcher.rs` post-processes/ranks their combined results.

## 6. `stages.rs` (judge/synthesis) and `verifier.rs` (claim verification)

**Verifier** (`verifier.rs:226-228`) system prompt: `"Classify whether retrieved evidence supports the claim. Output only JSON: {"verdict":"Supported|Contradicted|Contested|Unverified",...}"` — a strict per-claim support/contradict verdict, reasonable for factual/technical lookup but not a perfect fit for open-ended historical/legal questions where the right answer is often "multiple credible sources corroborate a contested narrative" rather than a clean binary — though the `Contested` verdict is a partial accommodation. No explicit code/technical language, so this is a design-shape limitation, not wording bias.

**Judge** (`stages.rs:73-99`) rubric (`factual_accuracy`/`citation_density`/`coverage`) is generic, no technical-domain language.

**Real finding — `ANTI_LAZINESS_RIDER`** (`orchestrator/config.rs:11-15`), injected into both the judge prompt (`stages.rs:87`) and the synthesis prompt (`stages.rs:230`):

```
<anti_laziness_rider>
DO NOT summarize or skip steps. DO NOT provide stubs, placeholders, or 'TODO' blocks. Implement ALL requested logic in full detail.
If providing a plan, ensure it is exhaustive and execution-ready. Laziness will be penalized with a 0 quality score.
</anti_laziness_rider>
```

This is verbatim coding-agent boilerplate ("stubs," "placeholders," "TODO blocks," "implement ALL requested logic") reused from a software-engineering system prompt and pasted into the *research answer synthesis and quality-judging* prompts. For a query like "summarize the causes of the French Revolution," this text is nonsensical noise injected into both the synthesizer's instructions and the LLM-judge's scoring rubric — it doesn't actively break non-technical answers, but it's a clear vestige of code-generation prompting bleeding into the general-research pipeline, and could subtly bias the model toward exhaustive/code-like completeness framing rather than natural prose synthesis.

## Recommended fixes (feeding Stage 2 synthesis)

1. Expand `web_dispatcher.rs`'s `source_authority_score` to include general-authority domains (major news wire services, Wikipedia, national statistical/archival sites) at a comparable tier to `.gov`/`.edu`, not just code-adjacent domains — or replace the hardcoded domain list with the corroboration-counting and domain-reputation techniques from the cross-domain methods survey.
2. Remove or condition `ANTI_LAZINESS_RIDER` out of the judge/synthesis prompts for non-code research stages — replace with (or supplement with) research-appropriate completeness language ("cite every material claim," "do not omit contradicting evidence") rather than code-generation-specific instructions.
3. Add a domain/URL gate to `TrustScorer` before triggering the OpenAlex title search — skip the call entirely for hits whose domain is clearly non-academic (news, government, reference sites), both for efficiency and to eliminate the title-collision misclassification risk.
4. When `worthiness.rs` is eventually wired into production (already flagged as future work in the trust/novelty-core plan), do not ship the current binary Journal/Preprint/Repository/Social taxonomy as-is — extend it with the domain-reputation and corroboration-count signals from the cross-domain methods survey so non-academic sources aren't uniformly force-bucketed into the lowest trust tier regardless of actual quality.
