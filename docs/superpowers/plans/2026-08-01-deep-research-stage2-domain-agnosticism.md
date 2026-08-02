# Deep Research Stage 2 — Domain-Agnosticism & GUI Trust Surfacing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the deep-research pipeline's code/academic bias (ranking, prompts, trust scoring), add a subject-agnostic corroboration-counting trust fallback, wire the already-computed `self_consistency` signal into the confidence gate, and bring `ResearchView` up to the GUI trust-UI standard the rest of the app already has.

**Architecture:** Nine tasks, ordered so each is independently testable and shippable: 3 small backend fixes (ranking bias, prompt cleanup, citation-audit task stub), a new domain-agnostic trust module (corroboration counting), a domain gate on the existing academic trust scorer, a gate-scoring wire-up (with a pre-existing naming collision resolved first), then three GUI tasks that consume all of the above (trust chip, claim accordion, headline banner). Every "wire X into Y" task below includes the actual grep output for every site X needs to reach, captured during this planning pass — see the [implementation-divergence audit](../../src/architecture/deep-research-implementation-divergence-audit-2026-08-01.md)'s finding that Stage 1's under-specified integration surface was the dominant source of code-review fix-commits.

**Tech Stack:** Rust (`vox-search`, `vox-research-shim`), React/TSX (`vox-gui/ui`), existing test harnesses (`cargo test`, Vitest if configured — confirmed absent for this surface; TSX tasks include manual verification steps instead).

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/vox-search/src/web_dispatcher.rs` | `source_authority_score` — ranking bias fix (Task 1) |
| `crates/vox-research-shim/src/research/orchestrator/config.rs` | `ANTI_LAZINESS_RIDER` removal (Task 2) |
| `crates/vox-search/src/corroboration.rs` (new) | Domain-agnostic corroboration counting (Task 4) |
| `crates/vox-search/src/trust.rs` | Academic-domain gate on `score_hit_trust` (Task 5) |
| `crates/vox-research-shim/src/research/verifier.rs` | `ClaimVerdict.self_consistency` → `resample_stability` rename (Task 6a) |
| `crates/vox-research-shim/src/research/orchestrator/pipeline.rs` | Gate wiring for `resample_stability` (Task 6b) |
| `crates/vox-gui/ui/src/components/surfaces/Research/TrustChip.tsx` (new) | 3-tier citation trust chip (Task 7) |
| `crates/vox-gui/ui/src/components/surfaces/Research/ResearchClaimAccordion.tsx` (new) | Expandable claim list (Task 8) |
| `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx` | Headline banner + accordion wiring (Task 9) |

Task 3 (numbered citation-audit placeholder) and the `NoveltyEvidenceBundle` item from the spec are handled as documented backlog entries, not code in this plan — see Task 3 and the note at the end of Task 9.

---

### Task 1: Fix `web_dispatcher.rs` source-authority ranking bias

**Files:**
- Modify: `crates/vox-search/src/web_dispatcher.rs:206-224` (`source_authority_score`)
- Test: `crates/vox-search/src/web_dispatcher.rs` (inline `#[cfg(test)] mod tests`, existing file — confirmed by grep, no separate test file for this module)

**Call-site inventory:** `source_authority_score` has exactly one call site — inside `rank_and_dedupe_results` in the same file (grep-verified: `grep -rn "source_authority_score" crates/vox-search/src` returns only the definition and its one call). No fan-out needed.

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `crates/vox-search/src/web_dispatcher.rs` (after the existing `rank_and_dedupe_prefers_authoritative_free_sources` test, which starts at line 241):

```rust
    #[test]
    fn rank_and_dedupe_boosts_general_authority_sources() {
        let mut results = vec![
            result("https://blog.example/post", 0.8),
            result("https://en.wikipedia.org/wiki/Research", 0.8),
            result("https://www.reuters.com/world/some-article", 0.8),
        ];

        rank_and_dedupe_results(&mut results);

        let wiki_pos = results
            .iter()
            .position(|r| r.url.contains("wikipedia.org"))
            .expect("wikipedia result present");
        let reuters_pos = results
            .iter()
            .position(|r| r.url.contains("reuters.com"))
            .expect("reuters result present");
        let blog_pos = results
            .iter()
            .position(|r| r.url.contains("blog.example"))
            .expect("blog result present");

        assert!(wiki_pos < blog_pos, "wikipedia.org should outrank an unboosted blog");
        assert!(reuters_pos < blog_pos, "reuters.com should outrank an unboosted blog");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-search rank_and_dedupe_boosts_general_authority_sources -- --nocapture`
Expected: FAIL — all three results tie at the same 1.0x score, so `rank_and_dedupe_results`' stable sort leaves them in ties/insertion order and the assertions fail (wiki/reuters not guaranteed to sort ahead of the blog).

- [ ] **Step 3: Implement the fix**

Replace lines 206-224 of `crates/vox-search/src/web_dispatcher.rs`:

```rust
fn source_authority_score(url: &str) -> f64 {
    let key = canonical_url_key(url);
    if key.contains(".gov/")
        || key.ends_with(".gov")
        || key.contains(".edu/")
        || key.ends_with(".edu")
        || key.contains("wikipedia.org/")
        || key.contains("reuters.com/")
        || key.contains("apnews.com/")
        || key.contains("bbc.co")
    {
        1.25
    } else if key.contains("arxiv.org/")
        || key.contains("doi.org/")
        || key.contains("pubmed.ncbi.nlm.nih.gov/")
        || key.contains("docs.rs/")
        || key.contains("github.com/")
    {
        1.15
    } else {
        1.0
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-search rank_and_dedupe_boosts_general_authority_sources rank_and_dedupe_prefers_authoritative_free_sources -- --nocapture`
Expected: PASS for both tests (the pre-existing test still passes since `docs.rs`/blog relative ordering is unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-search/src/web_dispatcher.rs
git commit -m "fix: boost Wikipedia/Reuters/AP/BBC ranking to match .gov/.edu authority tier"
```

---

### Task 2: Remove `ANTI_LAZINESS_RIDER` coding-agent boilerplate from research prompts

**Files:**
- Modify: `crates/vox-research-shim/src/research/orchestrator/config.rs:11-15` (delete constant)
- Modify: `crates/vox-research-shim/src/research/orchestrator/stages.rs:2,87,230` (remove import + both injection sites)
- Test: `crates/vox-research-shim/src/research/orchestrator/stages.rs` (inline test module)

**Call-site inventory (grep-verified):** `grep -rn "ANTI_LAZINESS_RIDER" crates/` returns exactly 4 lines, all inside `vox-research-shim`: the `pub(super) const` definition in `config.rs:11`, the `use` import in `stages.rs:2`, and two consumption sites in `stages.rs:87` (judge prompt) and `stages.rs:230` (synthesis prompt). No other crate references it — confirmed, the earlier grep in this session's research phase found the same 4 sites plus unrelated matches in `vox-orchestrator-mcp` that are a *different* symbol (`chat_tools` files matched on an unrelated substring during the broader search, not this constant — re-verify with the exact string above before editing to be certain no 5th site exists).

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-research-shim/src/research/orchestrator/stages.rs`'s existing `#[cfg(test)]` module (find it via `grep -n "mod tests" crates/vox-research-shim/src/research/orchestrator/stages.rs`):

```rust
    #[test]
    fn judge_prompt_has_no_code_generation_boilerplate() {
        let sys_prompt = build_judge_system_prompt();
        assert!(
            !sys_prompt.contains("TODO"),
            "judge prompt should not contain code-generation vocabulary: {sys_prompt}"
        );
        assert!(
            sys_prompt.contains("Cite every material claim"),
            "judge prompt should use research-appropriate completeness language: {sys_prompt}"
        );
    }
```

(If `build_judge_system_prompt` is not the exact function name at line 87's call site, use the actual function name found via `grep -n "fn.*judge.*prompt\|sys_prompt" crates/vox-research-shim/src/research/orchestrator/stages.rs` before writing this step — adjust the test to call whatever function currently produces the string that gets `.replace("{}", ANTI_LAZINESS_RIDER)`'d at line 87.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-research-shim judge_prompt_has_no_code_generation_boilerplate -- --nocapture`
Expected: FAIL — the prompt still contains "TODO blocks" from `ANTI_LAZINESS_RIDER` and does not yet contain "Cite every material claim".

- [ ] **Step 3: Implement the fix**

In `crates/vox-research-shim/src/research/orchestrator/config.rs`, replace the `ANTI_LAZINESS_RIDER` constant (lines 11-15) with:

```rust
pub(super) const RESEARCH_COMPLETENESS_RIDER: &str = "
<research_completeness_rider>
Cite every material claim. Do not omit contradicting evidence. Do not pad the summary with unsupported filler.
</research_completeness_rider>
";
```

In `crates/vox-research-shim/src/research/orchestrator/stages.rs`, change line 2's import to `use super::config::RESEARCH_COMPLETENESS_RIDER;`, and replace both occurrences of `ANTI_LAZINESS_RIDER` (lines 87 and 230) with `RESEARCH_COMPLETENESS_RIDER`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-research-shim judge_prompt_has_no_code_generation_boilerplate -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the full `vox-research-shim` test suite to confirm no other test asserted on the old rider text**

Run: `cargo test -p vox-research-shim`
Expected: PASS. If any pre-existing test asserted on `"stubs"` or `"TODO blocks"` text from the old rider, update that assertion to match the new `RESEARCH_COMPLETENESS_RIDER` text — do not skip the test.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-research-shim/src/research/orchestrator/config.rs crates/vox-research-shim/src/research/orchestrator/stages.rs
git commit -m "fix: replace coding-agent ANTI_LAZINESS_RIDER with research-appropriate completeness prompt"
```

---

### Task 3: Citation audit pass — status corrected, no longer a backlog item

**Files:** None — this task is process/doc-correction, not code.

**Correction (2026-08-01, during Stage 2 GUI work):** this task originally existed to give the post-hoc citation audit pass a tracked backlog entry, on the premise (from an earlier draft of the divergence audit) that it had been silently dropped during Stage 1 implementation (the "P1-7 silent-drop pattern"). That premise was wrong: `audit_citations()` in `vox-research-shim`'s `pipeline.rs` (~line 731-770) exists and is wired into `ResearchMetadata.citation_audit`, producing a real `CitationAuditResult`. See [divergence audit §4](../../src/architecture/deep-research-implementation-divergence-audit-2026-08-01.md) for the corrected finding.

What *is* still a legitimate, smaller follow-up: the shipped `audit_citations()` is a lighter-weight design than the original spec — it checks each citation's snippet for quote-overlap against the verifier's already-fetched evidence spans, not a true post-hoc re-fetch of each cited URL to independently re-verify against the live page. Upgrading it to a real re-fetch remains open, but is a scoped enhancement to existing, working code — not a "make this exist" task, and does not need this task's original tracked-backlog framing.

- [x] **Step 1: Confirm this task is tracked outside prose**

Superseded — no backlog entry needed; the feature already exists in code.

---

### Task 4: Add independent-source corroboration counting

**Files:**
- Create: `crates/vox-search/src/corroboration.rs`
- Modify: `crates/vox-search/src/lib.rs` (add `pub mod corroboration;` — confirm exact `mod` list location via `grep -n "^pub mod" crates/vox-search/src/lib.rs` before editing)
- Test: inline in `crates/vox-search/src/corroboration.rs`

**Call-site inventory:** this is new functionality with no pre-existing callers to inventory. Its own consumers are created in Task 6 (gate) and Task 7 (GUI trust chip) — both listed there.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-search/src/corroboration.rs`:

```rust
//! Independent-source corroboration counting: a domain-agnostic trust
//! fallback for hits that have no DOI/academic venue signal. Counts the
//! number of *distinct domains* whose retrieved evidence supports a claim,
//! so three pages on the same site don't count as three corroborations.

use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct CorroboratingHit {
    pub url: String,
    pub supports_claim: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorroborationCount {
    pub claim_id: String,
    pub supporting_domains: Vec<String>,
}

impl CorroborationCount {
    pub fn count(&self) -> usize {
        self.supporting_domains.len()
    }
}

/// Extracts the registrable domain from a URL for dedup purposes (e.g.
/// `https://www.reuters.com/world/x` and `https://reuters.com/y` both key
/// to `reuters.com`). Strips a leading `www.` only; does not attempt full
/// public-suffix-list registrable-domain parsing (YAGNI for this feature —
/// good enough to dedup same-site hits, not to resolve `co.uk`-style TLDs).
fn domain_of(url: &str) -> Option<String> {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = without_scheme.split('/').next()?;
    Some(host.strip_prefix("www.").unwrap_or(host).to_ascii_lowercase())
}

pub fn count_corroboration(claim_id: &str, hits: &[CorroboratingHit]) -> CorroborationCount {
    let supporting_domains: Vec<String> = hits
        .iter()
        .filter(|h| h.supports_claim)
        .filter_map(|h| domain_of(&h.url))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    CorroborationCount {
        claim_id: claim_id.to_string(),
        supporting_domains,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_distinct_domains_only() {
        let hits = vec![
            CorroboratingHit { url: "https://www.reuters.com/a".into(), supports_claim: true },
            CorroboratingHit { url: "https://reuters.com/b".into(), supports_claim: true },
            CorroboratingHit { url: "https://apnews.com/c".into(), supports_claim: true },
            CorroboratingHit { url: "https://blog.example/d".into(), supports_claim: false },
        ];

        let count = count_corroboration("claim-1", &hits);

        assert_eq!(count.count(), 2, "reuters.com counted once despite www./bare variants, apnews.com counted once, blog excluded as non-supporting");
    }

    #[test]
    fn zero_supporting_hits_yields_zero_count() {
        let hits = vec![CorroboratingHit { url: "https://example.com/a".into(), supports_claim: false }];
        assert_eq!(count_corroboration("claim-2", &hits).count(), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-search corroboration:: -- --nocapture`
Expected: FAIL with a compile error — `crates/vox-search/src/corroboration.rs` is not yet declared as a module in `lib.rs`, so `cargo test` won't find these tests until Step 3.

- [ ] **Step 3: Register the module**

Edit `crates/vox-search/src/lib.rs`: add `pub mod corroboration;` alongside the other `pub mod` declarations (find the exact insertion point via `grep -n "^pub mod trust;" crates/vox-search/src/lib.rs` — insert alphabetically near `trust`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-search corroboration:: -- --nocapture`
Expected: PASS for both `counts_distinct_domains_only` and `zero_supporting_hits_yields_zero_count`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-search/src/corroboration.rs crates/vox-search/src/lib.rs
git commit -m "feat: add independent-source corroboration counting as a domain-agnostic trust signal"
```

---

### Task 5: Domain-gate `TrustScorer`'s OpenAlex call

**Files:**
- Modify: `crates/vox-search/src/trust.rs` (add `is_plausibly_academic`, gate `score_hit_trust` at line 185)
- Test: inline in `crates/vox-search/src/trust.rs`'s existing `#[cfg(test)]` module

**Call-site inventory:** `score_hit_trust` (line 185) has exactly one call site — `crates/vox-research-shim/src/research/orchestrator/web_gather.rs:33` (grep-verified: `grep -rn "score_hit_trust" crates/` returns the definition at `trust.rs:185` and one call at `web_gather.rs:33`). This task changes `score_hit_trust`'s internal behavior only; the call site's signature (`score_hit_trust(title, doi) -> f64`) is unchanged, so `web_gather.rs` needs no edit.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-search/src/trust.rs`'s existing `#[cfg(test)]` module (near `reputation_multiplier_favors_journal_venue` at line 245):

```rust
    #[test]
    fn is_plausibly_academic_gates_correctly() {
        assert!(is_plausibly_academic("https://doi.org/10.1000/xyz123"));
        assert!(is_plausibly_academic("https://arxiv.org/abs/2401.00001"));
        assert!(is_plausibly_academic("https://www.ncbi.nlm.nih.gov/pmc/articles/PMC123"));
        assert!(!is_plausibly_academic("https://en.wikipedia.org/wiki/Research"));
        assert!(!is_plausibly_academic("https://www.reuters.com/world/article"));
        assert!(!is_plausibly_academic("https://blog.example/post"));
    }

    #[tokio::test]
    async fn score_hit_trust_skips_openalex_for_non_academic_url() {
        // No mock server configured for this hit — if score_hit_trust attempted
        // an OpenAlex call for a non-academic hit, this test would hang/timeout
        // rather than return promptly, since score_hit_trust doesn't take a URL
        // today. This test documents the NEW signature added in this step.
        let score = score_hit_trust_for_url(
            "Some Blog Post Title",
            None,
            "https://blog.example/post",
        )
        .await;
        assert_eq!(score, 1.0, "non-academic URL should short-circuit to neutral without an OpenAlex call");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-search is_plausibly_academic_gates_correctly score_hit_trust_skips_openalex_for_non_academic_url -- --nocapture`
Expected: FAIL — neither `is_plausibly_academic` nor `score_hit_trust_for_url` exist yet (compile error).

- [ ] **Step 3: Implement the domain gate**

Add to `crates/vox-search/src/trust.rs`, near the existing `score_hit_trust` function (line 185):

```rust
/// Cheap domain check gating the OpenAlex title-search call in
/// `score_hit_trust_for_url` — skips the network call and title-collision
/// misclassification risk for hits that are clearly not scholarly sources.
/// Fail-open: an unrecognized domain returns `false` (skip OpenAlex), which
/// is the same neutral 1.0 result a genuine no-match would have produced
/// anyway, so this never suppresses a real signal, only wasted calls.
pub fn is_plausibly_academic(url: &str) -> bool {
    let key = url.to_ascii_lowercase();
    key.contains("doi.org/")
        || key.contains("arxiv.org/")
        || key.contains(".edu/")
        || key.contains("pubmed.ncbi.nlm.nih.gov/")
        || key.contains("ncbi.nlm.nih.gov/")
        || key.contains("researchgate.net/")
        || key.contains("springer.com/")
        || key.contains("sciencedirect.com/")
        || key.contains("jstor.org/")
}

/// URL-aware wrapper around `score_hit_trust` that skips the OpenAlex
/// reputation lookup entirely for non-academic domains. This is the new
/// entry point `web_gather.rs` should call; `score_hit_trust` itself is
/// left unchanged for any other caller that doesn't yet have a URL handy.
pub async fn score_hit_trust_for_url(title: &str, doi: Option<&str>, url: &str) -> f64 {
    if !is_plausibly_academic(url) {
        return 1.0;
    }
    score_hit_trust(title, doi).await
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-search is_plausibly_academic_gates_correctly score_hit_trust_skips_openalex_for_non_academic_url -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Wire the new entry point into the one real call site**

Modify `crates/vox-research-shim/src/research/orchestrator/web_gather.rs:32-33`:

```rust
    let doi = vox_search::trust::extract_doi_from_url(&hit.path);
    let trust_score =
        vox_search::trust::score_hit_trust_for_url(&hit.title, doi.as_deref(), &hit.path).await;
```

- [ ] **Step 6: Run the full `vox-search` and `vox-research-shim` suites**

Run: `cargo test -p vox-search -p vox-research-shim`
Expected: PASS. This confirms Task 5's change doesn't break `web_gather.rs`'s existing tests around `ResearchHit.trust_score` population.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-search/src/trust.rs crates/vox-research-shim/src/research/orchestrator/web_gather.rs
git commit -m "fix: domain-gate TrustScorer's OpenAlex call to skip non-academic hits"
```

---

### Task 6: Wire `self_consistency` into the confidence gate

**Files:**
- Modify: `crates/vox-research-shim/src/research/verifier.rs:88-99` (rename field on `ClaimVerdict`)
- Modify all `ClaimVerdict` construction sites (inventory below)
- Modify: `crates/vox-research-shim/src/research/orchestrator/pipeline.rs:385-411` (consume the renamed field in gate scoring)
- Test: `crates/vox-research-shim/src/research/verifier.rs`'s existing test module, plus a new pipeline-level test

**Naming-collision context:** `crates/vox-orchestrator/src/orchestrator_policy.rs` and `crates/vox-orchestrator/src/confidence_fusion.rs` also reference `self_consistency` — grep-confirmed as a **separate, pre-existing, unrelated field** (weight 0.20, `contradiction_hints`-derived, part of a different confidence-fusion scheme). This task renames only `ClaimVerdict.self_consistency` (the `vox-research-shim` one) to `resample_stability`; `vox-orchestrator`'s field is untouched.

**Call-site inventory (grep-verified: `grep -rn "self_consistency" crates/vox-research-shim/`):**
1. `verifier.rs:98` — field definition on `ClaimVerdict`
2. `verifier.rs:339` — construction in a helper (likely a "no verification needed" fast-path default)
3. `verifier.rs:386` — construction/assignment inside `verify_with_resampling` (the real computed-value site)
4. `verifier.rs:398` — construction in another fast-path/default
5. `discovery_bridge.rs:289` — construction, a bridge/adapter path outside the main verifier flow
6. `pipeline.rs:944` — construction, likely another fallback/default path
7. `pipeline.rs:386-389` — the currently-dead comment noting this field is "not wired yet" — this is the consumption site this task activates
8. `tests/scientia_research_discovery_bridge.rs` — a test file referencing the field; must still compile after the rename

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-research-shim/src/research/orchestrator/pipeline.rs`'s existing test module (find via `grep -n "mod tests" crates/vox-research-shim/src/research/orchestrator/pipeline.rs`):

```rust
    #[test]
    fn supported_claim_weighted_by_resample_stability() {
        use super::super::verifier::{Claim, ClaimVerdict, Verdict};

        let stable_claim = ClaimVerdict {
            claim: Claim { claim_id: "c1".into(), text: "Stable claim".into() },
            verdict: Verdict::Supported,
            confidence: 0.9,
            supporting_count: 3,
            contradicting_count: 0,
            evidence_spans: vec![],
            resample_stability: 1.0,
        };
        let flaky_claim = ClaimVerdict {
            claim: Claim { claim_id: "c2".into(), text: "Flaky claim".into() },
            verdict: Verdict::Supported,
            confidence: 0.6,
            supporting_count: 1,
            contradicting_count: 0,
            evidence_spans: vec![],
            resample_stability: 0.34,
        };

        let weighted_stable = weighted_supported_claim_score(&[stable_claim.clone()]);
        let weighted_flaky = weighted_supported_claim_score(&[flaky_claim.clone()]);

        assert!(
            weighted_stable > weighted_flaky,
            "a claim whose verdict was stable across resamples must contribute more to the gate than a flaky one"
        );
        assert_eq!(weighted_stable, 1.0);
        assert_eq!(weighted_flaky, 0.34);
    }
```

(Adjust the `Claim`/`ClaimVerdict` field list above to match whatever `verifier.rs` actually defines beyond what was grep-verified in this planning pass — read `crates/vox-research-shim/src/research/verifier.rs` lines 1-100 in full before writing this step to confirm every field name on `Claim` and `ClaimVerdict`, since only `self_consistency` was directly verified during planning.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-research-shim supported_claim_weighted_by_resample_stability -- --nocapture`
Expected: FAIL — `resample_stability` doesn't exist yet (compile error, field is still named `self_consistency`), and `weighted_supported_claim_score` doesn't exist yet.

- [ ] **Step 3: Rename the field at its definition and all 6 construction sites**

In `crates/vox-research-shim/src/research/verifier.rs:98`, rename `pub self_consistency: f64,` to `pub resample_stability: f64,` and update the doc comment above it (lines 95-97) to match. Then, one at a time, visit each of the 6 construction sites from the inventory above (`verifier.rs:339`, `verifier.rs:386`, `verifier.rs:398`, `discovery_bridge.rs:289`, `pipeline.rs:944`) and rename the field-init key from `self_consistency:` to `resample_stability:` — do not change any values, only the key name. After all renames, run:

Run: `cargo build -p vox-research-shim 2>&1 | grep -i "self_consistency\|resample_stability"`
Expected: no output (a clean build confirms every construction site was found and renamed — if the compiler errors on a missed site, that error output IS the remaining inventory item; fix it and re-run).

- [ ] **Step 4: Add the weighting function and wire it into gate scoring**

In `crates/vox-research-shim/src/research/orchestrator/pipeline.rs`, replace the comment block at lines 385-393:

```rust
    // ── (e) Confidence gate → routing decision ────────────────────────────────
    let supported_claim_count = weighted_supported_claim_score(&claim_verdicts);
```

Add the function near the top of `pipeline.rs` (or in a location matching existing helper-function placement in that file — check for a `mod helpers` or free-function section via `grep -n "^fn \|^pub fn " crates/vox-research-shim/src/research/orchestrator/pipeline.rs` first):

```rust
/// Weights each `Supported` claim's contribution to the gate's citation
/// coverage signal by how stable its verdict was across LLM resamples — a
/// claim that flipped between Supported/Contested across resamples
/// (low `resample_stability`) should count for less than one that was
/// consistently Supported, rather than every Supported verdict counting
/// identically regardless of how reliable it was.
fn weighted_supported_claim_score(verdicts: &[super::super::verifier::ClaimVerdict]) -> f32 {
    verdicts
        .iter()
        .filter(|v| matches!(v.verdict, super::super::verifier::Verdict::Supported))
        .map(|v| v.resample_stability as f32)
        .sum()
}
```

Then, wherever `supported_claim_count` (now a weighted `f32` rather than a `usize` count) feeds into `GateInput` construction below line 412, confirm the field it's assigned to accepts a float — if `GateInput`'s field is typed `usize`, check `crates/vox-research-shim/src/research/gate.rs` for the exact field type via `grep -n "struct GateInput" -A 15 crates/vox-research-shim/src/research/gate.rs` and adjust either the field type to `f32` (preferred — preserves the fractional weighting) or round `weighted_supported_claim_score`'s result before assignment, whichever requires touching fewer downstream call sites. Re-run `cargo build -p vox-research-shim` after this change and fix any type errors surfaced.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-research-shim supported_claim_weighted_by_resample_stability -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test -p vox-research-shim`
Expected: PASS — this confirms the rename didn't break `tests/scientia_research_discovery_bridge.rs` (inventory item 8) or any other existing test asserting on `self_consistency`. Fix any remaining reference found by the compiler or test failures.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-research-shim/src/research/verifier.rs crates/vox-research-shim/src/research/orchestrator/pipeline.rs crates/vox-research-shim/src/research/discovery_bridge.rs
git commit -m "feat: wire ClaimVerdict resample-stability into confidence gate's citation score"
```

---

### Task 7: Build the 3-tier citation trust chip (GUI)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Research/TrustChip.tsx`
- Read (do not modify yet): `crates/vox-gui/ui/src/components/surfaces/Research/ClaimsView.tsx` — confirm the exact `VerdictBadge` import path and color-token pattern before writing `TrustChip.tsx`, so the new component visually matches rather than inventing a new style language.

- [ ] **Step 1: Read `ClaimsView.tsx` to confirm the existing badge/chip pattern**

Run: `grep -n "VerdictBadge\|import.*Badge\|className=" crates/vox-gui/ui/src/components/surfaces/Research/ClaimsView.tsx | head -30`

Use the actual color classes / component library found here (e.g. Tailwind classes, a shared `Badge` primitive) — do not guess a styling approach independent of what's already there.

- [ ] **Step 2: Write `TrustChip.tsx`**

```tsx
export type TrustSignal =
  | { kind: "formal"; venueType: string; retracted: boolean }
  | { kind: "corroborated"; sourceCount: number }
  | { kind: "uncorroborated" };

export function TrustChip({ signal }: { signal: TrustSignal }) {
  if (signal.kind === "formal") {
    if (signal.retracted) {
      return <span className="trust-chip trust-chip--warning">RETRACTED</span>;
    }
    return (
      <span className="trust-chip trust-chip--formal">
        {signal.venueType} · not retracted
      </span>
    );
  }
  if (signal.kind === "corroborated") {
    return (
      <span className="trust-chip trust-chip--corroborated">
        Confirmed by {signal.sourceCount} independent source
        {signal.sourceCount === 1 ? "" : "s"}
      </span>
    );
  }
  return (
    <span className="trust-chip trust-chip--uncorroborated">
      Single source — not independently corroborated
    </span>
  );
}
```

Replace the `trust-chip*` class names with whatever styling primitive Step 1 found `ClaimsView.tsx` actually using (e.g. if it uses a shared `<Badge variant="...">` component instead of raw class names, restructure `TrustChip.tsx` to use that same component rather than the sketch above — the sketch establishes the 3-branch logic, not the literal styling API).

- [ ] **Step 3: Manual verification — render the component in isolation**

Since no test harness exists for `vox-gui/ui` components in this codebase (confirmed by the absence of a Vitest/Jest config near this directory — check with `find crates/vox-gui/ui -maxdepth 2 -iname "vitest.config*" -o -iname "jest.config*"` before assuming; if one exists, write a snapshot test instead of skipping this step), verify by temporarily importing `TrustChip` into `ResearchView.tsx` behind a dev-only conditional, running the GUI dev server (`vox_run_dev` or whatever `crates/vox-gui`'s existing dev-run instructions specify — check `crates/vox-gui/README.md` or `package.json` scripts), and visually confirming all three `TrustSignal` variants render distinctly. Remove the temporary import before Step 4's commit — Task 9 wires it in for real.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Research/TrustChip.tsx
git commit -m "feat: add 3-tier TrustChip component for research citations"
```

---

### Task 8: Build the expandable claim accordion

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Research/ResearchClaimAccordion.tsx`
- Read: `crates/vox-gui/ui/src/components/surfaces/Research/ClaimsView.tsx` (import `VerdictBadge` from its actual export — confirm exact export name/path via `grep -n "export.*VerdictBadge" crates/vox-gui/ui/src/components/surfaces/Research/ClaimsView.tsx`)

- [ ] **Step 1: Confirm `VerdictBadge`'s exact export signature**

Run: `grep -n "VerdictBadge" crates/vox-gui/ui/src/components/surfaces/Research/ClaimsView.tsx`

Read the matched lines to get the exact prop shape `VerdictBadge` expects (e.g. `verdict: string` vs `verdict: Verdict` enum) before Step 2.

- [ ] **Step 2: Write `ResearchClaimAccordion.tsx`**

```tsx
import { useState } from "react";
import { VerdictBadge } from "./ClaimsView"; // adjust to the exact export path confirmed in Step 1
import { TrustChip, type TrustSignal } from "./TrustChip";

export interface ResearchClaimRow {
  claimId: string;
  text: string;
  verdict: string;
  confidence: number;
  resampleStability: number;
  citations: Array<{ url: string; trust: TrustSignal }>;
}

export function ResearchClaimAccordion({
  claims,
  sourceCount,
}: {
  claims: ResearchClaimRow[];
  sourceCount: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const contestedCount = claims.filter((c) => c.verdict === "Contested").length;

  return (
    <div className="research-claim-accordion">
      <button
        type="button"
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        {claims.length} claims verified · {contestedCount} contested · {sourceCount} sources
      </button>
      {expanded && (
        <ul>
          {claims.map((claim) => (
            <li key={claim.claimId}>
              <VerdictBadge verdict={claim.verdict} />
              <span>{claim.text}</span>
              <span>confidence {Math.round(claim.confidence * 100)}%</span>
              <span>
                {claim.resampleStability >= 0.67
                  ? `Stable across resamples (${Math.round(claim.resampleStability * 100)}%)`
                  : "Verdict flipped in resampling — treat with care"}
              </span>
              <ul>
                {claim.citations.map((cite) => (
                  <li key={cite.url}>
                    <a href={cite.url}>{cite.url}</a>
                    <TrustChip signal={cite.trust} />
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

Adjust the `VerdictBadge` prop name (`verdict={claim.verdict}`) to match whatever prop signature Step 1 found — this sketch assumes a `verdict: string` prop; if `ClaimsView.tsx` instead expects a typed enum or a whole `ClaimVerdict`-shaped object, restructure accordingly.

- [ ] **Step 3: Manual verification**

Same approach as Task 7 Step 3 — temporarily render with fixture data (3-4 fake `ResearchClaimRow` entries covering all three `TrustSignal` variants and both stable/flaky `resampleStability` values) via the dev server, confirm expand/collapse works and all rows render, then remove the temporary fixture wiring before committing (Task 9 wires it in for real, from live data).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Research/ResearchClaimAccordion.tsx
git commit -m "feat: add expandable claim accordion for ResearchView"
```

---

### Task 9: Wire headline banner + claim accordion into `ResearchView`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx`

**Call-site inventory:** `ResearchView.tsx` is the sole consumer being modified — per the GUI representation design doc, no other surface currently renders research-run detail data, so there is no fan-out risk for this change.

- [ ] **Step 1: Locate the current raw-markdown-dump render**

Run: `grep -n "report_markdown\|artifact_json" crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx`

Read the surrounding ~30 lines to find the exact detail-pane render function/JSX block this task modifies.

- [ ] **Step 2: Add the headline verdict banner**

Above the existing report-markdown render (found in Step 1), add:

```tsx
function HeadlineVerdictBanner({
  confidenceTier,
  corroboratingSources,
  contestedClaims,
  totalClaims,
}: {
  confidenceTier: "Direct" | "Light" | "DeepResearch";
  corroboratingSources: number;
  contestedClaims: number;
  totalClaims: number;
}) {
  const contestedRatio = totalClaims > 0 ? contestedClaims / totalClaims : 0;
  if (contestedRatio > 0.3) {
    return (
      <div className="research-headline-banner research-headline-banner--contested">
        Contested — {contestedClaims} of {totalClaims} claims have conflicting evidence
      </div>
    );
  }
  if (contestedClaims === 0) {
    return (
      <div className="research-headline-banner research-headline-banner--high">
        High confidence — {corroboratingSources} corroborating sources, no contested claims
      </div>
    );
  }
  return (
    <div className="research-headline-banner research-headline-banner--mixed">
      Mixed evidence — {contestedClaims} of {totalClaims} claims contested, treat with care
    </div>
  );
}
```

Wire it into the detail pane's JSX, above the existing report render, passing whatever fields the research-run detail object already exposes for confidence tier and claim counts (confirm exact field names on the detail-pane's data type via `grep -n "routing_tier\|confidence" crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx` and the TypeScript type it's built from — if the research API response doesn't yet expose per-run claim/citation data to the GUI at all, that is a backend API gap outside this plan's 9 tasks; flag it explicitly as a follow-up rather than fabricating a fake data shape).

- [ ] **Step 3: Wire in `ResearchClaimAccordion`**

Below the unchanged report render, add `<ResearchClaimAccordion claims={...} sourceCount={...} />`, importing it from `./ResearchClaimAccordion`, mapping the research-run detail object's claim/citation data into the `ResearchClaimRow[]` shape defined in Task 8. If the backend doesn't yet expose per-claim `resample_stability` or per-citation trust signals through the research API response consumed by this component, that is the same backend-API-gap flag as Step 2 — note it, do not fabricate placeholder data in the shipped component.

- [ ] **Step 4: Manual verification via the dev server**

Start the GUI dev server per `crates/vox-gui`'s existing instructions, run an actual research query end-to-end, and confirm: (a) the headline banner renders with real data, (b) the accordion expands and shows real claims/citations, (c) the report body renders unchanged below both. Take a screenshot for the PR/commit description if the project's convention includes one (check recent GUI-change commits via `git log --oneline -- crates/vox-gui/ui | head -5` for precedent).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx
git commit -m "feat: bring ResearchView up to the app's trust-UI standard (headline banner + claim accordion)"
```

---

## Deferred (explicitly out of scope for this plan)

- `NoveltyEvidenceBundle` population (spec item #9). **Correction found during plan-writing:** the divergence audit claimed zero construction sites exist; this planning pass's grep found `NoveltyEvidenceBundle {` constructed in `crates/vox-publisher/src/scientia_novelty_assess.rs` and `scientia_prior_art.rs`, with 3 dedicated test files (`novelty_bundle_contract_parity.rs`, `novelty_golden_harness.rs`, `scientia_novelty_acceptance.rs`) already exercising it. Before writing a follow-up plan for this item, re-run the audit's population check against current code — the audit doc may be stale on this specific point, or "populated" may mean something narrower (e.g. populated in `vox-publisher`'s SCIENTIA path but not in the `vox-research-shim` research-run path this plan touches). This discrepancy itself is exactly the kind of "verify before trusting a prior finding" case the meta-finding warns about — do not carry the audit's claim forward into a new plan without re-checking.
- All P2/P3 synthesis items (evidence-tier tagging, CourtListener/ClinicalTrials.gov APIs, domain reputation scoring, disputed-narrative framing beyond Task 9's simple ratio stub, `WorthinessSignalsV2` production wiring, recency-decay, fact-check-org lookup, Stage-1 P2-P4 carryovers).
- The full citation-audit re-fetch-and-verify implementation (Task 3 only tracks it as a numbered follow-up, doesn't build it).

## Self-review

**Spec coverage:** All 8 P0/P1 synthesis items map to a task — #1→Task 1, #2→Task 2, #3→Task 3, #4→Task 4, #5→Task 5, #6→Task 6, #7→Tasks 7-9, #8→Deferred (with correction noted).

**Placeholder scan:** No "TBD"/"TODO"/"implement later" left in any step. Several steps explicitly instruct verifying an exact name/type/path against current code before finalizing a literal (marked inline, e.g. Task 6 Step 1's `Claim` field list, Task 9's data-shape gap) — these are flagged verification steps, not placeholders, consistent with the meta-finding's call-site-inventory discipline; each names exactly what to check and why.

**Type consistency:** `resample_stability: f64` (Task 6) flows consistently from `ClaimVerdict` through `weighted_supported_claim_score` (returns `f32`, matching `GateInput`'s existing `trust_weighted_citation_score: f32` pattern at `pipeline.rs:410`) into `GateInput`. `TrustSignal` (Task 7) is consumed identically by `TrustChip` and `ResearchClaimAccordion` (Task 8) via the same type import. `CorroborationCount` (Task 4) is defined once and is the type Task 6/Task 7's TODO-marked backend-wiring gap would consume, though that exact wiring (`CorroborationCount` → GUI `TrustSignal::Corroborated`) is left as the Task 9 Step 3 backend-API-gap flag rather than a fabricated task — a genuine scope boundary, not an inconsistency.

**Cross-cutting invariant audit tasks satisfied:** fail-open behavior is explicit in Task 5's `is_plausibly_academic` doc comment and Task 9's contested-ratio stub (both default to the more-permissive/neutral branch on ambiguous input). `ClaimVerdict` construction-site inventory is Task 6 Step 3's explicit 6-site list plus a compiler-driven verification loop. `NoveltyEvidenceBundle` construction-site inventory was attempted during this plan's writing (see Deferred section) and surfaced a real discrepancy against the prior audit — exactly the outcome this discipline exists to catch.
