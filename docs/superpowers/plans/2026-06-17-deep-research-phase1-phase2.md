# Deep Research — Phase 1 & 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Vox's existing deep research pipeline with LLM-driven CRAG query expansion, per-hit novelty scoring, multi-signal confidence gate fusion, a working OpenRouter free-tier cascade fallback, and uncapped synthesis token budget.

**Architecture:** All changes target two crates: `vox-search` (CRAG expansion + novelty scorer) and `vox-research-shim` (confidence gate fusion + cascade config). A new `novelty.rs` file in `vox-search` provides a stateful `NoveltyScorer` that filters redundant hits during multi-hop retrieval. The CRAG expansion stays in `crag.rs` but gains a new `expand_queries_with_llm_or_heuristic` combinator that callers in `vox-research-shim` drive with an async LLM call. The free-tier cascade is a single env flag (`VOX_RESEARCH_FREE_TIER`) wired into `ResearchConfig`.

**Tech Stack:** Rust stable (workspace toolchain), `cargo test -p <crate>`, ripgrep (`rg`) for exploration. No new dependencies. Tests live in `#[cfg(test)]` modules in the same file.

---

## Background

### Key files you will touch

| File | What it does |
|------|-------------|
| `crates/vox-search/src/crag.rs` | `CragRouter` — CRAG query expansion, 152 lines |
| `crates/vox-search/src/research.rs` | `run_multi_hop_web_research()` — multi-hop loop, 124 lines |
| `crates/vox-search/src/policy.rs` | `SearchPolicy` — all retrieval tunables |
| `crates/vox-search/src/lib.rs` | Crate root — add `pub mod novelty;` here |
| `crates/vox-actor-runtime/src/llm/cascade.rs` | `apply_stage_defaults()` — hard-sets max_tokens per stage |
| `crates/vox-research-shim/src/research/gate.rs` | `score_with_config()` — Phase 0a stub to replace, 128 lines |
| `crates/vox-research-shim/src/research/orchestrator/config.rs` | `ResearchConfig` — free-tier fields go here |
| `crates/vox-research-shim/src/research/orchestrator/web_gather.rs` | LLM expansion helper goes here |
| `crates/vox-secrets/src/spec.rs` | `SecretId` enum + `SecretSpec` entries for new env vars |

### How to run tests

```powershell
cargo test -p vox-search           # Tasks 1, 2, 5
cargo test -p vox-actor-runtime    # Task 3
cargo test -p vox-research-shim    # Tasks 4, 6, 7
cargo test -p vox-secrets          # Tasks 2, 6
```

### Rules from AGENTS.md

- **All LLM calls go through `vox_actor_runtime::llm`** — never a direct HTTP client.
- **Format with `cargo fmt -p <crate>`** — never `cargo fmt --all` (breaks on Windows).
- **Test with `cargo test -p <crate>`** for a single crate.

---

## Task 1: Novelty Scorer — New File

**Files:**
- Create: `crates/vox-search/src/novelty.rs`
- Modify: `crates/vox-search/src/lib.rs`

- [ ] **Step 1.1: Create `crates/vox-search/src/novelty.rs`**

Create the file with this exact content:

```rust
//! Per-hit novelty scoring via 4-gram character shingling.

use std::collections::HashSet;

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

fn shingle_hashes(content: &str, n: usize) -> Vec<u64> {
    let lower = content.to_ascii_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < n {
        return vec![fnv1a(&lower)];
    }
    chars
        .windows(n)
        .map(|w| fnv1a(&w.iter().collect::<String>()))
        .collect()
}

/// Tracks seen content fingerprints across a research session.
#[derive(Debug, Default)]
pub struct NoveltyScorer {
    seen: HashSet<u64>,
}

impl NoveltyScorer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Score content novelty: fraction of 4-gram shingles NOT yet in the seen-set.
    /// Returns 0.0 (all seen) to 1.0 (all new).
    #[must_use]
    pub fn score(&self, content: &str) -> f64 {
        let hashes = shingle_hashes(content, 4);
        if hashes.is_empty() {
            return 0.0;
        }
        let new_count = hashes.iter().filter(|h| !self.seen.contains(h)).count();
        new_count as f64 / hashes.len() as f64
    }

    /// Commit content to the seen-set.
    pub fn accept(&mut self, content: &str) {
        for h in shingle_hashes(content, 4) {
            self.seen.insert(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scorer_scores_any_content_as_fully_novel() {
        let scorer = NoveltyScorer::new();
        assert_eq!(scorer.score("the quick brown fox jumped over the lazy dog"), 1.0);
    }

    #[test]
    fn exact_duplicate_after_accept_scores_zero() {
        let mut scorer = NoveltyScorer::new();
        let text = "the quick brown fox jumped over the lazy dog";
        scorer.accept(text);
        assert_eq!(scorer.score(text), 0.0);
    }

    #[test]
    fn partial_overlap_scores_between_zero_and_one() {
        let mut scorer = NoveltyScorer::new();
        scorer.accept("the quick brown fox");
        let score = scorer.score("the quick brown lazy dog");
        assert!(score > 0.0 && score < 1.0, "score={score}");
    }

    #[test]
    fn short_content_treated_as_single_shingle() {
        let scorer = NoveltyScorer::new();
        assert_eq!(scorer.score("hi"), 1.0);
        let mut s = scorer;
        s.accept("hi");
        assert_eq!(s.score("hi"), 0.0);
    }

    #[test]
    fn empty_content_scores_zero() {
        let scorer = NoveltyScorer::new();
        assert_eq!(scorer.score(""), 0.0);
    }
}
```

- [ ] **Step 1.2: Run tests — expect failure (file not in lib.rs yet)**

```powershell
cargo test -p vox-search novelty
```

Expected: `error[E0583]: file not found for module 'novelty'`

- [ ] **Step 1.3: Register module in `crates/vox-search/src/lib.rs`**

Find the line `pub mod memory_hybrid;` and add before it:

```rust
pub mod novelty;
```

- [ ] **Step 1.4: Run tests — expect 5 passing**

```powershell
cargo test -p vox-search novelty
```

Expected: `test result: ok. 5 passed; 0 failed`

- [ ] **Step 1.5: Commit**

```powershell
cargo fmt -p vox-search
git add crates/vox-search/src/novelty.rs crates/vox-search/src/lib.rs
git commit -m "feat(vox-search): add NoveltyScorer with 4-gram shingling"
```

---

## Task 2: Wire Novelty into Multi-Hop Loop

**Files:**
- Modify: `crates/vox-search/src/research.rs`
- Modify: `crates/vox-search/src/policy.rs`
- Modify: `crates/vox-secrets/src/spec.rs`

- [ ] **Step 2.1: Add `novelty_min_score` to `SearchPolicy` struct**

In `policy.rs`, find the end of the `SearchPolicy` struct. The last field is `pub persist_web_hits: bool,`. Add after it (inside the struct, before the closing `}`):

```rust
    /// Minimum novelty score (0.0-1.0) for a hit to appear in synthesis context.
    /// Env: `VOX_SEARCH_NOVELTY_MIN_SCORE`. Default: 0.15.
    #[serde(default = "default_novelty_min_score")]
    pub novelty_min_score: f64,
```

Add the default fn near the other `default_*` functions at the top:

```rust
#[inline]
fn default_novelty_min_score() -> f64 { 0.15 }
```

- [ ] **Step 2.2: Add to `Default::default()` for `SearchPolicy`**

Find `persist_web_hits: !parse_truthy_env(...)` initializer and add after its closing comma:

```rust
            novelty_min_score: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchNoveltyMinScore,
            )
            .expose()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|x| x.is_finite() && *x >= 0.0 && *x <= 1.0)
            .unwrap_or_else(default_novelty_min_score),
```

- [ ] **Step 2.3: Add `VoxSearchNoveltyMinScore` to `vox-secrets`**

In `crates/vox-secrets/src/spec.rs`, find `VoxSearchPersistWebHitsDisabled` in the `SecretId` enum, add after it:

```rust
VoxSearchNoveltyMinScore,
```

Find the `SecretSpec` for `VoxSearchPersistWebHitsDisabled` and add after it (match the surrounding struct shape exactly):

```rust
SecretSpec {
    id: SecretId::VoxSearchNoveltyMinScore,
    env_var: "VOX_SEARCH_NOVELTY_MIN_SCORE",
    description: "Minimum novelty fraction (0.0-1.0) for a web hit to be included in synthesis context. Default: 0.15.",
    required: false,
    redact: false,
},
```

- [ ] **Step 2.4: Update `run_multi_hop_web_research` in `research.rs`**

Replace the entire `run_multi_hop_web_research` function (lines 36-98) with:

```rust
pub async fn run_multi_hop_web_research(
    policy: &SearchPolicy,
    initial_queries: &[String],
    quality_target: f64,
    anchor_query: &str,
) -> Vec<String> {
    use crate::novelty::NoveltyScorer;

    let mut research_results = Vec::new();
    let mut hops_remaining = policy.web_search_max_hops;
    let mut active_queries: Vec<String> = initial_queries.to_vec();
    let mut visited_urls = HashSet::new();
    let mut running_top_score = 0.0_f64;
    let mut novelty_scorer = NoveltyScorer::new();
    let novelty_threshold = policy.novelty_min_score;

    while hops_remaining > 0 && !active_queries.is_empty() {
        let mut hop_hits: Vec<HybridSearchHit> = Vec::new();
        info!(
            hop = policy.web_search_max_hops - hops_remaining + 1,
            query_count = active_queries.len(),
            "starting research hop"
        );

        for query in &active_queries {
            match WebSearchDispatcher::search(query, policy).await {
                Ok(hits) => {
                    for hit in hits {
                        if visited_urls.insert(hit.path.clone()) {
                            running_top_score = running_top_score.max(hit.score.clamp(0.0, 1.0));
                            let novelty = novelty_scorer.score(&hit.content_snippet);
                            if novelty >= novelty_threshold {
                                novelty_scorer.accept(&hit.content_snippet);
                                let engine = hit
                                    .provenance
                                    .iter()
                                    .find_map(|p| p.strip_prefix("engine:"))
                                    .unwrap_or("unknown");
                                research_results.push(format!(
                                    "[autonomous_research:{}] {} (score: {:.3}; engine: {}; novelty: {:.2}) - {}",
                                    hit.path,
                                    hit.title,
                                    hit.score,
                                    engine,
                                    novelty,
                                    hit.content_snippet.replace('\n', " ")
                                ));
                            }
                            hop_hits.push(hit);
                        }
                    }
                }
                Err(e) => {
                    warn!(query = %query, error = %e, "research query failed");
                }
            }
        }

        let current_quality =
            web_research_crag_quality(policy, running_top_score, visited_urls.len());

        if !CragRouter::should_continue(current_quality, quality_target, hops_remaining) {
            break;
        }

        active_queries = CragRouter::expand_queries_from_partial_evidence(anchor_query, &hop_hits);
        hops_remaining -= 1;
    }

    research_results
}
```

- [ ] **Step 2.5: Add test to `research.rs`**

Add to the `#[cfg(test)]` block in `research.rs`:

```rust
    #[test]
    fn novelty_scorer_filters_duplicate_content() {
        use crate::novelty::NoveltyScorer;
        let mut scorer = NoveltyScorer::new();
        let text = "Rust ownership model prevents data races at compile time.";
        assert_eq!(scorer.score(text), 1.0, "first time: fully novel");
        scorer.accept(text);
        assert_eq!(scorer.score(text), 0.0, "second time: duplicate");
        let new_text = "Python uses garbage collection.";
        assert!(scorer.score(new_text) > 0.8, "unrelated: still novel");
    }
```

- [ ] **Step 2.6: Run and verify**

```powershell
cargo check -p vox-search
cargo check -p vox-secrets
cargo test -p vox-search
```

Expected: all pass.

- [ ] **Step 2.7: Commit**

```powershell
cargo fmt -p vox-search
cargo fmt -p vox-secrets
git add crates/vox-search/src/research.rs crates/vox-search/src/policy.rs crates/vox-secrets/src/spec.rs
git commit -m "feat(vox-search): wire NoveltyScorer into multi-hop loop; add VOX_SEARCH_NOVELTY_MIN_SCORE"
```

---

## Task 3: Synthesis Token Budget Fix

**Files:**
- Modify: `crates/vox-actor-runtime/src/llm/cascade.rs` (lines 147-164)

- [ ] **Step 3.1: Write failing test**

Add to the `mod tests` block in `cascade.rs`:

```rust
    #[test]
    fn synthesis_stage_does_not_force_1800_max_tokens() {
        use crate::model_resolution::RouteResolutionInput;
        let candidates = cascade_with_optional_manual(
            ResearchStage::Synthesis,
            &RouteResolutionInput::default(),
            None,
            None,
            None,
        );
        if let Some(c) = candidates.first() {
            assert_ne!(
                c.max_tokens,
                Some(1_800),
                "Synthesis max_tokens must not be hard-coded; got {:?}",
                c.max_tokens
            );
        }
    }
```

- [ ] **Step 3.2: Run — expect failure**

```powershell
cargo test -p vox-actor-runtime synthesis_stage_does_not_force_1800_max_tokens
```

Expected: `FAILED`

- [ ] **Step 3.3: Fix `apply_stage_defaults`**

Replace the full `apply_stage_defaults` function (lines 147-164):

```rust
fn apply_stage_defaults(stage: ResearchStage, cfg: &mut LlmConfig) {
    cfg.telemetry_task_category = Some("research".to_string());
    cfg.telemetry_strength_tag = Some(format!("{stage:?}").to_ascii_lowercase());
    cfg.temperature = Some(match stage {
        ResearchStage::Planner => 0.2,
        ResearchStage::ClaimExtraction | ResearchStage::Verification | ResearchStage::Judge => 0.0,
        ResearchStage::Synthesis => 0.2,
        ResearchStage::SelfVerification => 0.0,
    });
    // Synthesis max_tokens is NOT set here — controlled by ResearchConfig::synthesis_max_tokens.
    if stage != ResearchStage::Synthesis {
        cfg.max_tokens = Some(match stage {
            ResearchStage::Planner => 700,
            ResearchStage::ClaimExtraction => 900,
            ResearchStage::Verification => 500,
            ResearchStage::Judge => 400,
            ResearchStage::SelfVerification => 700,
            ResearchStage::Synthesis => unreachable!("guarded by outer if"),
        });
    }
}
```

- [ ] **Step 3.4: Run — expect pass**

```powershell
cargo test -p vox-actor-runtime
```

Expected: all pass.

- [ ] **Step 3.5: Commit**

```powershell
cargo fmt -p vox-actor-runtime
git add crates/vox-actor-runtime/src/llm/cascade.rs
git commit -m "fix(vox-actor-runtime): Synthesis stage no longer overrides max_tokens in cascade defaults"
```

---

## Task 4: Confidence Gate — Multi-Signal Fusion

**Files:**
- Modify: `crates/vox-research-shim/src/research/gate.rs`
- Modify: `crates/vox-research-shim/src/research/orchestrator/pipeline.rs`
- Modify: `crates/vox-research-shim/src/research/orchestrator/stages.rs`

- [ ] **Step 4.1: Write failing tests**

Replace the entire `#[cfg(test)]` block in `gate.rs` (lines 87-128) with:

```rust
#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::research::claims::Claim;
    use crate::research::types::RoutingTier;

    fn dummy_claims(n: usize) -> Vec<Claim> {
        (0..n)
            .map(|i| Claim {
                text: format!("claim {i}"),
                claim_id: i as u64,
                is_numeric: false,
                is_recent: false,
                is_named_event: false,
            })
            .collect()
    }

    fn full_config() -> GateConfig {
        GateConfig {
            min_citations_for_full_score: Some(5),
            min_domains_for_full_score: Some(4),
        }
    }

    #[test]
    fn routing_tier_for_high_score_returns_direct() {
        let s = ConfidenceSignal { score: 0.9 };
        assert!(matches!(s.routing_tier_for(0.7, 0.4, 0.2), RoutingTier::Direct));
    }

    #[test]
    fn routing_tier_for_mid_score_returns_light() {
        let s = ConfidenceSignal { score: 0.5 };
        assert!(matches!(s.routing_tier_for(0.7, 0.4, 0.2), RoutingTier::Light));
    }

    #[test]
    fn routing_tier_for_low_nonzero_score_returns_deep_research() {
        let s = ConfidenceSignal { score: 0.1 };
        assert!(matches!(s.routing_tier_for(0.7, 0.4, 0.2), RoutingTier::DeepResearch));
    }

    #[test]
    fn routing_tier_for_exact_direct_threshold_returns_direct() {
        let s = ConfidenceSignal { score: 0.7 };
        assert!(matches!(s.routing_tier_for(0.7, 0.4, 0.2), RoutingTier::Direct));
    }

    #[test]
    fn routing_tier_for_exact_light_threshold_returns_light() {
        let s = ConfidenceSignal { score: 0.4 };
        assert!(matches!(s.routing_tier_for(0.7, 0.4, 0.2), RoutingTier::Light));
    }

    #[test]
    fn zero_evidence_scores_zero() {
        let config = full_config();
        let input = GateInput {
            claims: &[],
            citation_count: 0,
            supported_claim_count: 0,
            distinct_domain_count: 0,
            no_retrieval_hits: true,
            answer_is_empty: false,
        };
        assert_eq!(score_with_config(&input, &config).score, 0.0);
    }

    #[test]
    fn full_evidence_scores_near_one() {
        let claims = dummy_claims(4);
        let config = full_config();
        let input = GateInput {
            claims: &claims,
            citation_count: 5,
            supported_claim_count: 4,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let s = score_with_config(&input, &config);
        assert!(s.score > 0.95, "expected >0.95, got {}", s.score);
    }

    #[test]
    fn partial_evidence_scores_in_deep_research_band() {
        let claims = dummy_claims(1);
        let config = full_config();
        let input = GateInput {
            claims: &claims,
            citation_count: 2,
            supported_claim_count: 0,
            distinct_domain_count: 1,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let s = score_with_config(&input, &config);
        assert!(
            s.score > 0.0 && s.score < 0.7,
            "expected between 0 and 0.7, got {}",
            s.score
        );
    }
}
```

- [ ] **Step 4.2: Run — expect compile error**

```powershell
cargo test -p vox-research-shim semcov_wave2_tests
```

Expected: compile error — missing fields on `GateConfig` and `GateInput`.

- [ ] **Step 4.3: Expand `GateConfig`**

Replace lines 14-17 in `gate.rs`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    pub min_citations_for_full_score: Option<usize>,
    pub min_domains_for_full_score: Option<usize>,
}
```

- [ ] **Step 4.4: Expand `GateInput`**

Replace the `GateInput` struct (lines 37-44):

```rust
#[derive(Debug)]
pub struct GateInput<'a> {
    pub claims: &'a [Claim],
    pub citation_count: usize,
    pub supported_claim_count: usize,
    pub distinct_domain_count: usize,
    pub no_retrieval_hits: bool,
    pub answer_is_empty: bool,
}
```

- [ ] **Step 4.5: Replace `score_with_config`**

Replace lines 74-85:

```rust
#[must_use]
pub fn score_with_config(input: &GateInput<'_>, config: &GateConfig) -> ConfidenceSignal {
    if input.no_retrieval_hits {
        return ConfidenceSignal { score: 0.0 };
    }
    let min_cit = config.min_citations_for_full_score.unwrap_or(5) as f32;
    let min_dom = config.min_domains_for_full_score.unwrap_or(4) as f32;
    let citation_score = (input.citation_count as f32 / min_cit).clamp(0.0, 1.0);
    let claim_support_score = if input.claims.is_empty() {
        0.5
    } else {
        (input.supported_claim_count as f32 / input.claims.len() as f32).clamp(0.0, 1.0)
    };
    let diversity_score = (input.distinct_domain_count as f32 / min_dom).clamp(0.0, 1.0);
    let score = citation_score * 0.35
        + claim_support_score * 0.30
        + diversity_score * 0.20
        + 1.0_f32 * 0.15; // retrieval_score always 1.0 here (guarded above)
    ConfidenceSignal { score: score.clamp(0.0, 1.0) }
}
```

- [ ] **Step 4.6: Make `registrable_domain` pub(super) in `stages.rs`**

In `crates/vox-research-shim/src/research/orchestrator/stages.rs`, find `fn registrable_domain(url: &str)` and change to `pub(super) fn`.

- [ ] **Step 4.7: Fix `GateInput` construction in `pipeline.rs`**

Find the `GateInput {` block in `pipeline.rs` and replace:

```rust
    let supported_claim_count = claim_verdicts
        .iter()
        .filter(|v| matches!(v.verdict, super::super::verifier::Verdict::Supported))
        .count();
    let distinct_domain_count = {
        use std::collections::HashSet;
        let mut domains = HashSet::<String>::new();
        for hit in &all_hits {
            if let Some(host) = super::stages::registrable_domain(&hit.url) {
                domains.insert(host);
            }
        }
        domains.len()
    };
    let gate_input = GateInput {
        claims: &draft_claims,
        citation_count: all_hits.len().min(3),
        supported_claim_count,
        distinct_domain_count,
        no_retrieval_hits: all_hits.is_empty(),
        answer_is_empty: false,
    };
```

- [ ] **Step 4.8: Run all shim tests**

```powershell
cargo check -p vox-research-shim
cargo test -p vox-research-shim
```

Expected: all pass.

- [ ] **Step 4.9: Commit**

```powershell
cargo fmt -p vox-research-shim
git add crates/vox-research-shim/src/research/gate.rs crates/vox-research-shim/src/research/orchestrator/pipeline.rs crates/vox-research-shim/src/research/orchestrator/stages.rs
git commit -m "feat(vox-research-shim): replace Phase 0a gate stub with multi-signal confidence fusion"
```

---

## Task 5: LLM-Driven CRAG Query Expansion

**Files:**
- Modify: `crates/vox-search/src/crag.rs`
- Modify: `crates/vox-research-shim/src/research/orchestrator/web_gather.rs`

- [ ] **Step 5.1: Add `expand_queries_with_llm_or_heuristic` to `CragRouter`**

In `crag.rs`, after `expand_queries_from_partial_evidence`, add:

```rust
    /// Use LLM-generated queries when available; fall back to heuristic.
    /// Caps output at 4 queries. Pass `None` or `Some(&[])` to use heuristic.
    #[must_use]
    pub fn expand_queries_with_llm_or_heuristic(
        original_query: &str,
        hits: &[HybridSearchHit],
        llm_queries: Option<&[String]>,
    ) -> Vec<String> {
        let provided: Vec<String> = llm_queries
            .unwrap_or(&[])
            .iter()
            .filter(|q| !q.trim().is_empty())
            .cloned()
            .collect();
        if !provided.is_empty() {
            let mut out = provided;
            out.truncate(4);
            return out;
        }
        CragRouter::expand_queries_from_partial_evidence(original_query, hits)
    }
```

- [ ] **Step 5.2: Write tests for the combinator**

Add to the `tests` module in `crag.rs`:

```rust
    #[test]
    fn expand_with_llm_uses_llm_when_nonempty() {
        let llm = vec!["RAG NLI 2025".to_string(), "evidence grounding".to_string()];
        let result = CragRouter::expand_queries_with_llm_or_heuristic(
            "deep research", &[hit(0.2, "weak", false)], Some(&llm));
        assert_eq!(result, llm);
    }

    #[test]
    fn expand_with_llm_falls_back_when_empty() {
        let result = CragRouter::expand_queries_with_llm_or_heuristic(
            "deep research citation grounding",
            &[hit(0.2, "small weak snippet", false)],
            Some(&[]));
        assert!(!result.is_empty());
        assert!(result.iter().any(|q| q.contains("primary source evidence")));
    }

    #[test]
    fn expand_with_llm_caps_at_four() {
        let many: Vec<String> = (0..10).map(|i| format!("q{i}")).collect();
        let result = CragRouter::expand_queries_with_llm_or_heuristic("test", &[], Some(&many));
        assert!(result.len() <= 4);
    }

    #[test]
    fn expand_with_llm_filters_blank_entries() {
        let queries = vec!["".to_string(), "  ".to_string(), "valid".to_string()];
        let result = CragRouter::expand_queries_with_llm_or_heuristic("test", &[], Some(&queries));
        assert_eq!(result, vec!["valid"]);
    }
```

- [ ] **Step 5.3: Run tests — expect all pass**

```powershell
cargo test -p vox-search crag
```

Expected: all pass.

- [ ] **Step 5.4: Add `try_llm_query_expansion` to `web_gather.rs`**

Add after the existing conversion helpers, before `fn research_hits_from_search_execution`:

```rust
/// Attempt LLM-driven CRAG query expansion. Returns `None` on any failure.
pub(super) async fn try_llm_query_expansion(
    original_query: &str,
    top_snippets: &[String],
    config: &super::config::ResearchConfig,
) -> Option<Vec<String>> {
    use vox_actor_runtime::ActivityOptions;
    use vox_actor_runtime::llm::LlmChatMessage;
    use vox_actor_runtime::llm::cascade::{ResearchStage, cascade_with_optional_manual, chat_with_cascade};
    use vox_actor_runtime::llm::types::LlmMessageRole;
    use vox_actor_runtime::model_resolution::RouteResolutionInput;

    let snippets_text = top_snippets
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, s)| format!("{}. {}", i + 1, s.chars().take(300).collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n");

    let user_msg = format!(
        "Research question: {original_query}\n\nEvidence so far:\n{snippets_text}\n\n\
        Identify 2-4 specific follow-up search queries covering the most important missing \
        aspects. Output ONLY valid JSON:\n{{\"followup_queries\": [\"query 1\", \"query 2\"]}}"
    );

    let messages = vec![
        LlmChatMessage {
            role: LlmMessageRole::System,
            content: "You are a research gap analyst. Generate precise follow-up search \
                      queries to fill knowledge gaps. Output only valid JSON.".to_string(),
        },
        LlmChatMessage {
            role: LlmMessageRole::User,
            content: user_msg,
        },
    ];

    let candidates = cascade_with_optional_manual(
        ResearchStage::Planner,
        &RouteResolutionInput::default(),
        config.llm_endpoint.as_deref(),
        config.api_key.as_deref(),
        Some(&config.planner_model),
    );

    let opts = ActivityOptions::default();
    let Ok(response) = chat_with_cascade(
        &opts, messages, candidates, Some(ResearchStage::Planner),
    ).await else {
        return None;
    };

    let text = response.content.trim();
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start > end { return None; }
    let json_str = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct Expansion { followup_queries: Vec<String> }
    let parsed: Expansion = serde_json::from_str(json_str).ok()?;
    let queries: Vec<String> = parsed.followup_queries
        .into_iter()
        .filter(|q| !q.trim().is_empty())
        .collect();

    if queries.is_empty() { None } else { Some(queries) }
}
```

- [ ] **Step 5.5: Write JSON parsing tests in `web_gather.rs`**

Add a `#[cfg(test)]` block at the bottom:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn parses_llm_expansion_json_correctly() {
        let json = r#"{"followup_queries": ["query A", "query B", ""]}"#;
        #[derive(serde::Deserialize)]
        struct Expansion { followup_queries: Vec<String> }
        let parsed: Expansion = serde_json::from_str(json).unwrap();
        let filtered: Vec<String> = parsed.followup_queries
            .into_iter().filter(|q| !q.trim().is_empty()).collect();
        assert_eq!(filtered, vec!["query A", "query B"]);
    }

    #[test]
    fn strips_markdown_fences_to_find_json() {
        let raw = "```json\n{\"followup_queries\": [\"q1\"]}\n```";
        let start = raw.find('{').unwrap();
        let end = raw.rfind('}').unwrap();
        let json_str = &raw[start..=end];
        #[derive(serde::Deserialize)]
        struct Expansion { followup_queries: Vec<String> }
        let parsed: Expansion = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed.followup_queries, vec!["q1"]);
    }
}
```

- [ ] **Step 5.6: Run tests**

```powershell
cargo test -p vox-search crag
cargo test -p vox-research-shim
```

Expected: all pass.

- [ ] **Step 5.7: Commit**

```powershell
cargo fmt -p vox-search
cargo fmt -p vox-research-shim
git add crates/vox-search/src/crag.rs crates/vox-research-shim/src/research/orchestrator/web_gather.rs
git commit -m "feat(vox-search,vox-research-shim): add LLM-driven CRAG expansion with heuristic fallback"
```

---

## Task 6: Free-Tier Research Cascade

**Files:**
- Modify: `crates/vox-research-shim/src/research/orchestrator/config.rs`
- Modify: `crates/vox-secrets/src/spec.rs`

- [ ] **Step 6.1: Add free-tier fields to `ResearchConfig`**

At the end of the `ResearchConfig` struct in `config.rs`, add:

```rust
    /// When true, all research LLM calls use only free-tier OpenRouter models.
    /// Env: `VOX_RESEARCH_FREE_TIER=1`.
    pub research_free_tier_only: bool,
    /// Free-tier model IDs in priority order. Env: `VOX_RESEARCH_FREE_TIER_MODELS`.
    /// Default: deepseek/deepseek-r1:free, google/gemma-3-27b-it:free,
    ///          meta-llama/llama-3.3-70b-instruct:free
    pub research_free_tier_model_ids: Vec<String>,
```

- [ ] **Step 6.2: Add `SecretId` entries**

In `crates/vox-secrets/src/spec.rs`, add to the `SecretId` enum:

```rust
VoxResearchFreeTier,
VoxResearchFreeTierModels,
```

Add `SecretSpec` entries (copy the shape of neighbouring entries):

```rust
SecretSpec {
    id: SecretId::VoxResearchFreeTier,
    env_var: "VOX_RESEARCH_FREE_TIER",
    description: "Set to 1/true to route all research LLM calls through free-tier OpenRouter models only.",
    required: false,
    redact: false,
},
SecretSpec {
    id: SecretId::VoxResearchFreeTierModels,
    env_var: "VOX_RESEARCH_FREE_TIER_MODELS",
    description: "Comma-separated list of OpenRouter :free model IDs for research, in priority order.",
    required: false,
    redact: false,
},
```

- [ ] **Step 6.3: Wire fields into all `ResearchConfig { ... }` construction sites**

Find every construction site:

```powershell
rg "ResearchConfig \{" crates/vox-research-shim/src/ --include="*.rs"
```

For each, add the two new field initializers:

```rust
research_free_tier_only: vox_secrets::resolve_secret(
    vox_secrets::SecretId::VoxResearchFreeTier,
)
.expose()
.map(|v| {
    let v = v.trim();
    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
})
.unwrap_or(false),
research_free_tier_model_ids: vox_secrets::resolve_secret(
    vox_secrets::SecretId::VoxResearchFreeTierModels,
)
.expose()
.map(|v| {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
})
.unwrap_or_else(|| vec![
    "deepseek/deepseek-r1:free".to_string(),
    "google/gemma-3-27b-it:free".to_string(),
    "meta-llama/llama-3.3-70b-instruct:free".to_string(),
]),
```

- [ ] **Step 6.4: Wire free-tier override into model resolution**

Find `resolve_research_models`:

```powershell
rg "resolve_research_models" crates/vox-research-shim/src/ --include="*.rs"
```

After the function resolves model IDs, add before the return:

```rust
    if config.research_free_tier_only && !config.research_free_tier_model_ids.is_empty() {
        let ids = &config.research_free_tier_model_ids;
        let m0 = ids[0].clone();
        let m1 = ids.get(1).cloned().unwrap_or_else(|| m0.clone());
        let m2 = ids.get(2).cloned().unwrap_or_else(|| m1.clone());
        resolved.planner_model = m0.clone();
        resolved.claim_model = m1.clone();
        resolved.synthesis_model = m1;
        resolved.judge_model = m2;
    }
```

Adjust field names to match the actual struct returned by `resolve_research_models`.

- [ ] **Step 6.5: Write a validation test**

Add in `config.rs` or the nearest test module:

```rust
    #[test]
    fn default_free_tier_model_ids_all_have_free_suffix() {
        let ids = vec![
            "deepseek/deepseek-r1:free",
            "google/gemma-3-27b-it:free",
            "meta-llama/llama-3.3-70b-instruct:free",
        ];
        assert!(ids.iter().all(|m| m.ends_with(":free")));
        assert_eq!(ids.len(), 3);
    }
```

- [ ] **Step 6.6: Verify and test**

```powershell
cargo check -p vox-research-shim
cargo check -p vox-secrets
cargo test -p vox-research-shim
```

Expected: all pass.

- [ ] **Step 6.7: Commit**

```powershell
cargo fmt -p vox-research-shim
cargo fmt -p vox-secrets
git add crates/vox-research-shim/src/research/orchestrator/config.rs crates/vox-secrets/src/spec.rs
git commit -m "feat(vox-research-shim): add VOX_RESEARCH_FREE_TIER cascade with prioritized free model list"
```

---

## Task 7: Final Integration Verification

- [ ] **Step 7.1: Full suite**

```powershell
cargo test -p vox-search
cargo test -p vox-actor-runtime
cargo test -p vox-research-shim
cargo test -p vox-secrets
```

Expected: all pass.

- [ ] **Step 7.2: Workspace build check**

```powershell
cargo check --workspace
```

Expected: no errors.

- [ ] **Step 7.3: Commit**

```powershell
git commit --allow-empty -m "chore: Phase 1+2 deep research complete — novelty scoring, LLM CRAG, gate fusion, free-tier, synthesis budget"
```

---

## Quick Reference

```powershell
# Test individual crates
cargo test -p vox-search
cargo test -p vox-actor-runtime
cargo test -p vox-research-shim
cargo test -p vox-secrets

# Format (NEVER cargo fmt --all)
cargo fmt -p vox-search

# Find symbols
rg "struct NoveltyScorer" crates/vox-search/src/
rg "fn expand_queries_with_llm_or_heuristic" crates/vox-search/src/
rg "struct GateInput" crates/vox-research-shim/src/
rg "resolve_research_models" crates/vox-research-shim/src/
rg "SecretId::" crates/vox-secrets/src/spec.rs
```
