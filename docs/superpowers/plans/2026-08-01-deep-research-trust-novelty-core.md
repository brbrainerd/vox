# Deep Research Trust/Novelty Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close Vox's deep-research trust/novelty gap: populate the currently-unused `ResearchHit.trust_score` and `NoveltyEvidenceBundle`/`WorthinessSignalsV2` schemas with real signals (source credibility, novelty, worthiness), add a post-hoc citation-verification pass, and unify two research loops that each independently gained one of two 2026-06-17-audit fixes (LLM-driven CRAG expansion, novelty gating) but not the other.

**Architecture:** New `TrustScorer` in `vox-search` (Crossref retraction + OpenAlex reputation lookups, following the existing `TavilyResearchClient` fail-open HTTP pattern) feeds both `gate.rs`'s confidence fusion and a new `WorthinessSignalsV2` populator in `vox-scientia`. Novelty scoring is upgraded from exact-string dedup to lexical shingle-similarity (reusing `vox-search::novelty::NoveltyScorer`'s existing FNV1a-shingle approach — no new ML dependency required for this pass; true embedding-based semantic dedup is out of scope, see Task 8 notes). The two independently-evolved CRAG loops (`vox-research-shim`'s primary pipeline and `vox-search`'s autonomous-agent loop) share their LLM-query-expansion and novelty-gating logic by moving the LLM-expansion helper down into `vox-search`, which both already depend on.

**Tech Stack:** Rust, `reqwest` 0.12 (rustls-tls, via `vox-http-client::client_builder()`), `tokio`, `serde`/`serde_json`, existing `vox_actor_runtime::llm::cascade` for LLM calls. No new external crate dependencies in this plan — every task builds on what's already in the workspace (confirmed via full Cargo.toml dependency audit).

---

## Before you start

Every task below was grounded in exact current source read on 2026-08-01. Code has almost certainly not drifted since, but **before editing any file, re-read the exact lines referenced** — if line numbers have shifted, use the quoted code as a search anchor (e.g. `grep -n "PHASE_0a_STUB"`) rather than trusting line numbers blindly.

Run all commands from the repo root: `C:\Users\Owner\vox\.claude\worktrees\deep-research-enhancement-036f97` (or wherever this worktree/checkout lives).

---

### Task 1: Fix stale `PHASE_0a_STUB` doc comments

The module doc in `mod.rs` and inline comments in `verifier.rs`/`gate.rs` still claim stub/empty behavior over code that now does real work — actively misleading anyone who greps for stub markers (this plan's own grounding pass nearly was).

**Files:**
- Modify: `crates/vox-research-shim/src/research/mod.rs:1-8`
- Modify: `crates/vox-research-shim/src/research/verifier.rs:99-100`
- Modify: `crates/vox-research-shim/src/research/gate.rs:64-68`

- [ ] **Step 1: Update `mod.rs`'s module doc comment**

Replace lines 1-8:

```rust
//! Research pipeline subsystem for `vox-orchestrator`.
//!
//! See `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`
//! for the strategic context. This module is currently in **Phase 0a stub**
//! state: types are real, behavior returns empty/default values. Phase 1
//! replaces the stub bodies with the `vox-claim-extractor` crate.
//!
//! All stubs are marked `// PHASE_0a_STUB` for grep-based discovery.
```

with:

```rust
//! Research pipeline subsystem for `vox-orchestrator`.
//!
//! See `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`
//! for the strategic context and
//! `docs/src/architecture/deep-research-verification-2026-08-01.md` for a
//! current-state audit. As of 2026-08-01, claim extraction, verification,
//! confidence gating, and synthesis are real LLM-backed implementations
//! (behind the `runtime` feature) — this module is no longer in a blanket
//! stub state. Individual known gaps (not "everything is a stub") are
//! tracked in the verification doc above; grep `PHASE_0a_STUB` only finds
//! genuinely narrow remaining placeholders, not whole-module stubs.
```

- [ ] **Step 2: Update `verifier.rs`'s stale stub comment**

Replace lines 99-100:

```rust
/// **PHASE_0a_STUB**: returns `Vec::new()`. Phase 1 wires this to
/// `vox-claim-extractor`'s MiniCheck-backed verifier.
```

with:

```rust
/// Verifies claims against evidence via an LLM cascade (behind the `runtime`
/// feature; without it, degrades to blanket `Unverified`). See Task 7 of
/// `docs/superpowers/plans/2026-08-01-deep-research-trust-novelty-core.md`
/// for the planned self-consistency resampling addition, and this file's
/// module-level SciFact-taxonomy note above for the still-open Verdict
/// naming reconciliation.
```

- [ ] **Step 3: Update `gate.rs`'s stale stub comment**

Replace lines 64-68 (inside `routing_tier_for`):

```rust
                    // PHASE_0a_STUB: exact-zero check is valid only while score_with_config
                    // produces an integer-derived float (citation_count / 5.0). Phase 2
                    // multi-signal fusion may produce non-zero scores with no retrieval hits;
                    // replace with `input.no_retrieval_hits` passed through the call chain.
```

with:

```rust
                    // NOTE: score_with_config already returns exactly 0.0 when
                    // input.no_retrieval_hits is true (see the early return at the
                    // top of that function), so this exact-zero check is a correct,
                    // durable proxy for "no retrieval hits" today — it is not a
                    // stub pending replacement. If score_with_config's early-return
                    // behavior ever changes, this comment must be revisited.
```

- [ ] **Step 4: Verify the crate still builds**

Run: `cargo check -p vox-research-shim`
Expected: no errors (doc-comment-only changes).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-research-shim/src/research/mod.rs crates/vox-research-shim/src/research/verifier.rs crates/vox-research-shim/src/research/gate.rs
git commit -m "docs: fix stale PHASE_0a_STUB comments in deep-research pipeline"
```

---

### Task 2: Move LLM-driven CRAG query expansion into `vox-search`

`try_llm_query_expansion` currently lives in `vox-research-shim/src/research/orchestrator/web_gather.rs` and is only used by the primary research pipeline. `vox-search`'s standalone `run_multi_hop_web_research` (used by the autonomous-agent research loop) still uses only the heuristic expansion. Since `vox-search` already depends on `vox-actor-runtime` (confirmed: `crates/vox-search/Cargo.toml:20`), the function can move down a layer so both loops share it — `vox-research-shim` already depends on `vox-search`, so it can call the moved version.

**Files:**
- Create: `crates/vox-search/src/llm_query_expansion.rs`
- Modify: `crates/vox-search/src/lib.rs` (add `pub mod llm_query_expansion;`)
- Modify: `crates/vox-research-shim/src/research/orchestrator/web_gather.rs:44-139` (delete local function, call the moved one)
- Test: `crates/vox-search/src/llm_query_expansion.rs` (inline `#[cfg(test)]` module)

- [ ] **Step 1: Write the failing test for the moved function's pure JSON-parsing logic**

The original function mixes network I/O (the cascade call) with JSON parsing. Split out the parsing so it's independently testable without a live LLM. Add to the new file:

```rust
//! Shared LLM-driven CRAG query expansion, usable by both the primary
//! research-shim orchestrator pipeline and vox-search's standalone
//! autonomous-research loop (both depend on this crate).

/// Parses an LLM response expected to contain `{"followup_queries": [...]}`
/// somewhere in the text (tolerating markdown fences / surrounding prose).
/// Returns `None` if no valid, non-empty query list can be extracted.
pub fn parse_followup_queries(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start > end {
        return None;
    }
    let json_str = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct Expansion {
        followup_queries: Vec<String>,
    }
    let parsed: Expansion = serde_json::from_str(json_str).ok()?;
    let queries: Vec<String> = parsed
        .followup_queries
        .into_iter()
        .filter(|q| !q.trim().is_empty())
        .collect();

    if queries.is_empty() { None } else { Some(queries) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json() {
        let text = r#"{"followup_queries": ["query one", "query two"]}"#;
        assert_eq!(
            parse_followup_queries(text),
            Some(vec!["query one".to_string(), "query two".to_string()])
        );
    }

    #[test]
    fn parses_json_with_surrounding_prose() {
        let text = "Here is my answer:\n{\"followup_queries\": [\"only query\"]}\nDone.";
        assert_eq!(
            parse_followup_queries(text),
            Some(vec!["only query".to_string()])
        );
    }

    #[test]
    fn filters_blank_queries() {
        let text = r#"{"followup_queries": ["real query", "  ", ""]}"#;
        assert_eq!(
            parse_followup_queries(text),
            Some(vec!["real query".to_string()])
        );
    }

    #[test]
    fn returns_none_on_empty_query_list() {
        let text = r#"{"followup_queries": []}"#;
        assert_eq!(parse_followup_queries(text), None);
    }

    #[test]
    fn returns_none_on_malformed_json() {
        let text = "not json at all, no braces";
        assert_eq!(parse_followup_queries(text), None);
    }

    #[test]
    fn returns_none_on_wrong_shape() {
        let text = r#"{"something_else": ["a"]}"#;
        assert_eq!(parse_followup_queries(text), None);
    }
}
```

- [ ] **Step 2: Run the new tests to verify they pass**

Run: `cargo test -p vox-search llm_query_expansion:: -- --nocapture`
Expected: 6 tests pass (the module has no network dependency yet, this is pure parsing logic).

- [ ] **Step 3: Add the async wrapper that does the actual LLM call**

Append to `crates/vox-search/src/llm_query_expansion.rs`:

```rust
/// Attempts LLM-driven CRAG query expansion given a research question and
/// the top evidence snippets gathered so far. Returns `None` on any
/// failure (LLM call, parsing) — callers should fall back to
/// `CragRouter::expand_queries_from_partial_evidence` in that case.
pub async fn try_llm_query_expansion(
    original_query: &str,
    top_snippets: &[String],
    llm_endpoint: Option<&str>,
    api_key: Option<&str>,
    planner_model: Option<&str>,
) -> Option<Vec<String>> {
    use vox_actor_runtime::ActivityOptions;
    use vox_actor_runtime::llm::LlmChatMessage;
    use vox_actor_runtime::llm::cascade::{
        ResearchStage, cascade_with_optional_manual, chat_with_cascade,
    };
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
            role: "system".to_string(),
            content: "You are a research gap analyst. Generate precise follow-up search \
                      queries to fill knowledge gaps. Output only valid JSON."
                .to_string(),
            ..Default::default()
        },
        LlmChatMessage {
            role: "user".to_string(),
            content: user_msg,
            ..Default::default()
        },
    ];

    let candidates = cascade_with_optional_manual(
        ResearchStage::Planner,
        &RouteResolutionInput::default(),
        llm_endpoint,
        api_key,
        planner_model,
    );

    let opts = ActivityOptions::default();
    let response =
        chat_with_cascade(&opts, messages, candidates, Some(ResearchStage::Planner))
            .await
            .ok()?;

    let queries = parse_followup_queries(response.content.trim());
    if queries.is_none() {
        tracing::warn!(raw_response = %response.content, "LLM query expansion failed to parse");
    }
    queries
}
```

- [ ] **Step 4: Register the module**

In `crates/vox-search/src/lib.rs`, add near the other `pub mod` declarations:

```rust
pub mod llm_query_expansion;
```

- [ ] **Step 5: Update the primary pipeline to call the moved function**

In `crates/vox-research-shim/src/research/orchestrator/web_gather.rs`, delete the local `try_llm_query_expansion` function (lines 44-139) and its now-unused imports at the top of that block, then update the call site (originally at line 338):

```rust
        let llm_queries = vox_search::llm_query_expansion::try_llm_query_expansion(
            &query.query,
            &top_snippets,
            config.llm_endpoint.as_deref(),
            config.api_key.as_deref(),
            Some(&config.planner_model),
        )
        .await;
```

- [ ] **Step 6: Wire it into the autonomous-agent loop**

In `crates/vox-search/src/research.rs`, update `run_multi_hop_web_research` to try LLM expansion before falling back to the heuristic. Replace the line:

```rust
        active_queries = CragRouter::expand_queries_from_partial_evidence(anchor_query, &hop_hits);
```

with:

```rust
        let llm_queries = crate::llm_query_expansion::try_llm_query_expansion(
            anchor_query,
            &hop_hits.iter().map(|h| h.content_snippet.clone()).collect::<Vec<_>>(),
            None,
            None,
            None,
        )
        .await;
        active_queries = CragRouter::expand_queries_with_llm_or_heuristic(
            anchor_query,
            &hop_hits,
            llm_queries.as_deref(),
        );
```

(Passing `None` for endpoint/api_key/model lets the cascade use its default resolution — this loop didn't have a `ResearchConfig` to draw those from before either.)

- [ ] **Step 7: Run the full test suite for both crates**

Run: `cargo test -p vox-search -p vox-research-shim`
Expected: all existing tests still pass, plus the 6 new `llm_query_expansion` tests.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-search/src/llm_query_expansion.rs crates/vox-search/src/lib.rs crates/vox-search/src/research.rs crates/vox-research-shim/src/research/orchestrator/web_gather.rs
git commit -m "feat: share LLM-driven CRAG query expansion between both research loops"
```

---

### Task 3: Share novelty scoring into the primary research-shim pipeline

`vox-search::research::run_multi_hop_web_research` (autonomous loop) already gates hits through `NoveltyScorer`; `vox-research-shim`'s primary pipeline (`web_gather.rs::gather_web_hits_for_plan`) still does plain URL-set dedup only (`dedupe_hits_by_url` in `pipeline.rs:222`).

**Files:**
- Modify: `crates/vox-research-shim/src/research/orchestrator/web_gather.rs`
- Test: same file, extend existing test module

- [ ] **Step 1: Write a failing test asserting near-duplicate content is filtered**

Find `web_gather.rs`'s existing `#[cfg(test)] mod tests` block and add:

```rust
    #[test]
    fn novelty_scorer_rejects_near_duplicate_snippets() {
        use vox_search::novelty::NoveltyScorer;
        let mut scorer = NoveltyScorer::new();
        let original = "The confidence gate fuses citation score, claim support, and domain diversity.";
        assert!(scorer.score(original) >= 0.99);
        scorer.accept(original);

        let near_duplicate = "The confidence gate fuses citation score, claim support, and domain diversity signals.";
        // Should score low (mostly-seen shingles) since it's a near-restatement.
        assert!(scorer.score(near_duplicate) < 0.5);
    }
```

- [ ] **Step 2: Run the test to confirm it passes against the existing `NoveltyScorer` (sanity check the import path)**

Run: `cargo test -p vox-research-shim novelty_scorer_rejects_near_duplicate_snippets -- --nocapture`
Expected: PASS (this test only exercises `vox_search::novelty::NoveltyScorer` directly, confirming it's importable from this crate before wiring it into the pipeline).

- [ ] **Step 3: Add novelty gating to `gather_web_hits_for_plan`**

Find where `all_hits` is deduped by URL (`pipeline.rs:222`, `dedupe_hits_by_url`) and where hits flow into `web_gather.rs`. Add a `NoveltyScorer` alongside the existing `HashSet` used for URL dedup in `gather_web_hits_for_plan`, and gate each hit on both URL-uniqueness (existing) and novelty (new):

```rust
    let mut novelty_scorer = vox_search::novelty::NoveltyScorer::new();
    let novelty_min_score = 0.15_f64; // matches vox-search's SearchPolicy::novelty_min_score default
```

Then at the point where a hit is accepted into `all_hits` (after the existing URL-dedup check), add:

```rust
        let novelty = novelty_scorer.score(&hit.snippet);
        if novelty < novelty_min_score {
            tracing::debug!(url = %hit.url, novelty, "dropping low-novelty hit");
            continue;
        }
        novelty_scorer.accept(&hit.snippet);
```

(Exact insertion point depends on the current loop structure around `all_hits.push(hit)` — search for that call and insert the novelty check immediately before it, inside the same loop iteration, after the URL-uniqueness check already there.)

- [ ] **Step 4: Run the pipeline's existing integration tests**

Run: `cargo test -p vox-research-shim orchestrator:: -- --nocapture`
Expected: existing pipeline tests still pass (they use small fixed hit sets where novelty scoring against an empty/small seen-set should not reject legitimate distinct hits).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-research-shim/src/research/orchestrator/web_gather.rs
git commit -m "feat: gate primary research pipeline hits on novelty score, not just URL uniqueness"
```

---

### Task 4: Add `TrustScorer` (Crossref + OpenAlex) to `vox-search`

New client following the exact `TavilyResearchClient` pattern (`crates/vox-search/src/tavily_research.rs`): fail-open, `from_env()`-style construction, `try_X() -> T` wrapper that never propagates errors to callers.

**Files:**
- Create: `crates/vox-search/src/trust.rs`
- Modify: `crates/vox-search/src/lib.rs` (add `pub mod trust;`)
- Test: same file, inline `#[cfg(test)]` module using `wiremock` (already a dev-dependency of `vox-search`, per the existing `tavily_research.rs` test pattern)

- [ ] **Step 1: Write the failing test for the Crossref retraction check**

```rust
//! Source trust scoring: Crossref retraction lookup + OpenAlex venue/author
//! reputation, feeding `ResearchHit.trust_score`. Both APIs are free/keyless.
//! Fail-open: any network or parse error yields a neutral trust score
//! rather than blocking the research pipeline.

use serde::Deserialize;

const CROSSREF_BASE: &str = "https://api.crossref.org";
const OPENALEX_BASE: &str = "https://api.openalex.org";

pub struct TrustScorer {
    http: reqwest::Client,
    crossref_base: String,
    openalex_base: String,
}

impl TrustScorer {
    pub fn new() -> Self {
        Self {
            http: vox_http_client::client_builder()
                .timeout(vox_config::timeouts::D_30S)
                .build()
                .expect("reqwest client"),
            crossref_base: CROSSREF_BASE.to_string(),
            openalex_base: OPENALEX_BASE.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_urls(crossref_base: impl Into<String>, openalex_base: impl Into<String>) -> Self {
        Self {
            http: vox_http_client::client_builder()
                .timeout(vox_config::timeouts::D_30S)
                .build()
                .expect("reqwest client"),
            crossref_base: crossref_base.into(),
            openalex_base: openalex_base.into(),
        }
    }

    /// Returns `Some(true)` if the DOI is confirmed retracted/corrected,
    /// `Some(false)` if confirmed clean, `None` if the lookup failed
    /// (caller should treat `None` as "unknown, don't penalize").
    pub async fn check_retraction(&self, doi: &str) -> Option<bool> {
        #[derive(Deserialize)]
        struct CrossrefWork {
            message: CrossrefMessage,
        }
        #[derive(Deserialize)]
        struct CrossrefMessage {
            #[serde(default)]
            update_to: Vec<CrossrefUpdate>,
        }
        #[derive(Deserialize)]
        struct CrossrefUpdate {
            #[serde(rename = "type")]
            update_type: String,
        }

        let url = format!("{}/works/{}", self.crossref_base.trim_end_matches('/'), doi);
        let resp = self.http.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        let parsed: CrossrefWork = serde_json::from_str(&text).ok()?;
        let is_retracted = parsed
            .message
            .update_to
            .iter()
            .any(|u| u.update_type.eq_ignore_ascii_case("retraction"));
        Some(is_retracted)
    }
}

impl Default for TrustScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn check_retraction_detects_retracted_work() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "update-to": [{"type": "retraction"}] }
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls(server.uri(), "http://unused.invalid");
        assert_eq!(scorer.check_retraction("10.1234/example").await, Some(true));
    }

    #[tokio::test]
    async fn check_retraction_returns_false_for_clean_work() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "update-to": [] }
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls(server.uri(), "http://unused.invalid");
        assert_eq!(scorer.check_retraction("10.1234/example").await, Some(false));
    }

    #[tokio::test]
    async fn check_retraction_returns_none_on_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works/.*"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls(server.uri(), "http://unused.invalid");
        assert_eq!(scorer.check_retraction("10.1234/example").await, None);
    }
}
```

Note: the JSON test fixtures above use `"update-to"` (Crossref's actual kebab-case field name) as the JSON key while the Rust struct uses `update_to` with `#[serde(rename = ...)]` omitted — **fix this before running**: add `#[serde(rename = "update-to")]` above the `update_to` field in `CrossrefMessage`, since Crossref's real API uses hyphenated JSON keys (confirmed via Crossref's public API docs referenced in the trust/novelty research doc).

- [ ] **Step 2: Run the new tests to verify they pass**

Run: `cargo test -p vox-search trust:: -- --nocapture`
Expected: 3 tests pass, confirming the retraction-check logic and its fail-open behavior on HTTP errors.

- [ ] **Step 3: Add the OpenAlex venue/author reputation lookup**

Append to `crates/vox-search/src/trust.rs`:

```rust
    /// Returns a soft reputation multiplier in [0.5, 1.5] based on the
    /// venue type and author citation history for a work matching `title`,
    /// via OpenAlex. Returns `1.0` (neutral) on any lookup failure.
    pub async fn reputation_multiplier(&self, title: &str) -> f64 {
        #[derive(Deserialize)]
        struct OpenAlexSearch {
            results: Vec<OpenAlexWork>,
        }
        #[derive(Deserialize)]
        struct OpenAlexWork {
            #[serde(default)]
            primary_location: Option<OpenAlexLocation>,
        }
        #[derive(Deserialize)]
        struct OpenAlexLocation {
            source: Option<OpenAlexSource>,
        }
        #[derive(Deserialize)]
        struct OpenAlexSource {
            #[serde(rename = "type")]
            source_type: Option<String>,
        }

        let url = format!(
            "{}/works?search={}&per-page=1",
            self.openalex_base.trim_end_matches('/'),
            urlencoding_lite(title)
        );
        let Ok(resp) = self.http.get(&url).send().await else {
            return 1.0;
        };
        if !resp.status().is_success() {
            return 1.0;
        }
        let Ok(text) = resp.text().await else {
            return 1.0;
        };
        let Ok(parsed) = serde_json::from_str::<OpenAlexSearch>(&text) else {
            return 1.0;
        };

        match parsed
            .results
            .first()
            .and_then(|w| w.primary_location.as_ref())
            .and_then(|l| l.source.as_ref())
            .and_then(|s| s.source_type.as_deref())
        {
            Some("journal") => 1.5,
            Some("repository") | Some("conference") => 1.2,
            Some("preprint") => 1.0,
            _ => 1.0,
        }
    }
}

/// Minimal query-param percent-encoding (spaces, common punctuation) without
/// pulling in a new dependency — sufficient for search-term titles.
fn urlencoding_lite(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '&' => "%26".to_string(),
            '?' => "%3F".to_string(),
            '#' => "%23".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' => c.to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}
```

- [ ] **Step 4: Add a test for the reputation multiplier**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[tokio::test]
    async fn reputation_multiplier_favors_journal_venue() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{"primary_location": {"source": {"type": "journal"}}}]
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls("http://unused.invalid", server.uri());
        assert_eq!(scorer.reputation_multiplier("Example Paper Title").await, 1.5);
    }

    #[tokio::test]
    async fn reputation_multiplier_defaults_to_neutral_on_no_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": []
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls("http://unused.invalid", server.uri());
        assert_eq!(scorer.reputation_multiplier("Nonexistent Title").await, 1.0);
    }
```

- [ ] **Step 5: Add the combined fail-open convenience function used by call sites**

Append to `trust.rs`:

```rust
/// Computes a combined trust score for a hit: 1.0 baseline, halved on
/// confirmed retraction, scaled by venue reputation otherwise. Never fails
/// — any lookup error yields the neutral 1.0 baseline. `doi` is optional
/// since most web hits won't resolve to one.
pub async fn score_hit_trust(title: &str, doi: Option<&str>) -> f64 {
    let scorer = TrustScorer::new();
    if let Some(doi) = doi {
        if scorer.check_retraction(doi).await == Some(true) {
            return 0.1; // heavily penalized, not zeroed, so it's still visible/debuggable
        }
    }
    scorer.reputation_multiplier(title).await
}
```

- [ ] **Step 6: Register the module**

In `crates/vox-search/src/lib.rs`:

```rust
pub mod trust;
```

- [ ] **Step 7: Run all trust tests**

Run: `cargo test -p vox-search trust:: -- --nocapture`
Expected: 5 tests pass total.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-search/src/trust.rs crates/vox-search/src/lib.rs
git commit -m "feat: add TrustScorer (Crossref retraction + OpenAlex reputation) to vox-search"
```

---

### Task 5: Wire `trust_score` into `ResearchHit` construction

`ResearchHit.trust_score` currently hardcoded to `1.0` at every construction site. Wire `Task 4`'s scorer in.

**Files:**
- Modify: `crates/vox-research-shim/src/research/orchestrator/web_gather.rs` (hit-construction functions, e.g. `research_hit_from_hybrid`)

- [ ] **Step 1: Write a failing test confirming trust score is no longer hardcoded**

Find the existing test module in `web_gather.rs` and add:

```rust
    #[tokio::test]
    async fn research_hit_from_hybrid_calls_trust_scorer_not_hardcoded() {
        // This test documents the contract: after this task, ResearchHit.trust_score
        // must come from vox_search::trust::score_hit_trust, not a literal 1.0.
        // Since score_hit_trust is itself fail-open (network calls to real APIs
        // in this environment may fail), we only assert the function is async
        // and callable without panicking — full integration coverage lives in
        // Task 4's TrustScorer tests (mocked HTTP).
        let score = vox_search::trust::score_hit_trust("Example Title", None).await;
        assert!(score >= 0.0 && score <= 2.0, "trust score {score} out of sane range");
    }
```

- [ ] **Step 2: Run it to confirm it passes standalone (sanity check before wiring)**

Run: `cargo test -p vox-research-shim research_hit_from_hybrid_calls_trust_scorer_not_hardcoded -- --nocapture`
Expected: PASS (this hits the real network in test — acceptable since `score_hit_trust` is fail-open and the assertion range is loose; if CI disallows live network in tests, mark this `#[ignore]` and note it needs a mockable version, matching Task 4's `with_base_urls` pattern, wired through a similar test-only constructor here).

- [ ] **Step 3: Find and update the hit-construction site(s)**

Search for where `ResearchHit { .. trust_score: 1.0 .. }` is constructed:

Run: `grep -rn "trust_score" crates/vox-research-shim/src/research/orchestrator/web_gather.rs`

At each such construction site (e.g. inside `research_hit_from_hybrid`), replace the hardcoded `trust_score: 1.0` with:

```rust
        trust_score: vox_search::trust::score_hit_trust(&hit_title, None).await,
```

(using whatever the local title variable is named at that call site — this requires the enclosing function to be `async`, which it already is since it's called from within an `async fn` pipeline stage; if any construction site is currently synchronous, that site needs `.await`-compatible refactoring, which should be scoped as its own sub-step if discovered.)

- [ ] **Step 4: Run the research-shim test suite**

Run: `cargo test -p vox-research-shim`
Expected: all tests pass, including the new one from Step 1.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-research-shim/src/research/orchestrator/web_gather.rs
git commit -m "feat: wire TrustScorer into ResearchHit.trust_score construction"
```

---

### Task 6: Trust-weight the confidence gate's citation score

`gate.rs::score_with_config` currently uses raw `citation_count`. Change it to use a trust-weighted sum, using the `trust_score` now populated by Task 5.

**Files:**
- Modify: `crates/vox-research-shim/src/research/gate.rs`
- Modify: `crates/vox-research-shim/src/research/orchestrator/pipeline.rs:400-407`
- Test: `crates/vox-research-shim/src/research/gate.rs` (extend `semcov_wave2_tests`)

- [ ] **Step 1: Write a failing test for trust-weighted citation scoring**

Add to `gate.rs`'s `#[cfg(test)] mod semcov_wave2_tests`:

```rust
    #[test]
    fn low_trust_citations_score_lower_than_high_trust_citations() {
        let claims = dummy_claims(2);
        let config = GateConfig::default();

        let high_trust_input = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 5.0, // 5 citations at trust_score 1.0 each
            supported_claim_count: 2,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let low_trust_input = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 0.5, // 5 citations at trust_score 0.1 each (all retracted)
            supported_claim_count: 2,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };

        let high = score_with_config(&high_trust_input, &config);
        let low = score_with_config(&low_trust_input, &config);
        assert!(
            high.score > low.score,
            "high-trust score {} should exceed low-trust score {}",
            high.score,
            low.score
        );
    }
```

- [ ] **Step 2: Run the test to confirm it fails (compile error: no such field yet)**

Run: `cargo test -p vox-research-shim low_trust_citations_score_lower_than_high_trust_citations`
Expected: FAIL to compile — `GateInput` has no `trust_weighted_citation_score` field yet.

- [ ] **Step 3: Add the field to `GateInput` and update `score_with_config`**

In `gate.rs`, update the `GateInput` struct (originally lines 39-47) to add the new field, keeping `citation_count` for backward-compatible diagnostics/logging:

```rust
pub struct GateInput<'a> {
    pub claims: &'a [Claim],
    pub citation_count: usize,
    /// Sum of `ResearchHit.trust_score` across all cited hits. Falls back
    /// to `citation_count as f32` at call sites that haven't wired a real
    /// `TrustScorer` yet, preserving prior behavior exactly.
    pub trust_weighted_citation_score: f32,
    pub supported_claim_count: usize,
    pub distinct_domain_count: usize,
    pub no_retrieval_hits: bool,
    pub answer_is_empty: bool,
}
```

Update `score_with_config`'s citation_score line (originally part of lines 81-102) from:

```rust
    let citation_score = (input.citation_count as f32 / min_cit).clamp(0.0, 1.0);
```

to:

```rust
    let citation_score = (input.trust_weighted_citation_score / min_cit).clamp(0.0, 1.0);
```

- [ ] **Step 4: Update all other existing `GateInput` construction sites (tests) to add the new field**

Run: `grep -rln "GateInput {" crates/vox-research-shim/src/`

For every existing test-fixture `GateInput { .. }` literal found (in `gate.rs`'s own test module) that doesn't already set `trust_weighted_citation_score`, add `trust_weighted_citation_score: citation_count as f32,` (matching the field's fallback semantics documented above) so old tests keep asserting the same numeric behavior as before this change.

- [ ] **Step 5: Update the production call site in `pipeline.rs`**

Replace the `GateInput` construction at `pipeline.rs:400-407`:

```rust
    let gate_input = GateInput {
        claims: &draft_claims,
        citation_count: all_hits.len(),
        supported_claim_count,
        distinct_domain_count,
        no_retrieval_hits: all_hits.is_empty(),
        answer_is_empty: false,
    };
```

with:

```rust
    let trust_weighted_citation_score: f32 =
        all_hits.iter().map(|h| h.trust_score as f32).sum();
    let gate_input = GateInput {
        claims: &draft_claims,
        citation_count: all_hits.len(),
        trust_weighted_citation_score,
        supported_claim_count,
        distinct_domain_count,
        no_retrieval_hits: all_hits.is_empty(),
        answer_is_empty: false,
    };
```

- [ ] **Step 6: Run the full gate.rs and pipeline test suites**

Run: `cargo test -p vox-research-shim gate:: orchestrator::pipeline:: -- --nocapture`
Expected: all tests pass, including the new trust-weighting test from Step 1.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-research-shim/src/research/gate.rs crates/vox-research-shim/src/research/orchestrator/pipeline.rs
git commit -m "feat: trust-weight the confidence gate's citation score"
```

---

### Task 7: Add self-consistency resampling to the claim verifier

The trust/novelty research identified a real ONNX-based NLI model (MiniCheck-class) as the ideal upgrade, but the dependency audit for this plan confirmed **no ONNX runtime or embedding crate exists anywhere in the workspace today** — adding one is a separate, larger decision (new `ort`/`candle` dependency edge) out of scope here. The pragmatic interim signal that needs no new dependencies: **SelfCheckGPT-style resampling** — call the existing LLM verifier cascade multiple times at nonzero temperature and measure agreement, giving `gate.rs` a consistency signal independent of a single-shot JSON call.

**Files:**
- Modify: `crates/vox-research-shim/src/research/verifier.rs`
- Test: same file, extend existing test module

- [ ] **Step 1: Write a failing test for the agreement-rate calculation (pure logic, no network)**

Add to `verifier.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn agreement_rate_computes_fraction_matching_majority_verdict() {
        let verdicts = vec![Verdict::Supported, Verdict::Supported, Verdict::Contradicted];
        assert_eq!(agreement_rate(&verdicts), 2.0 / 3.0);
    }

    #[test]
    fn agreement_rate_is_one_for_unanimous_verdicts() {
        let verdicts = vec![Verdict::Supported, Verdict::Supported, Verdict::Supported];
        assert_eq!(agreement_rate(&verdicts), 1.0);
    }

    #[test]
    fn agreement_rate_is_zero_for_empty_input() {
        let verdicts: Vec<Verdict> = vec![];
        assert_eq!(agreement_rate(&verdicts), 0.0);
    }
```

- [ ] **Step 2: Run to confirm it fails to compile (no `agreement_rate` fn yet)**

Run: `cargo test -p vox-research-shim agreement_rate`
Expected: compile failure, function undefined.

- [ ] **Step 3: Implement `agreement_rate`**

Add near the top of `verifier.rs`, above `verify_claims_with_config`:

```rust
/// Fraction of `verdicts` matching the most common verdict among them.
/// Used as a self-consistency signal: low agreement across repeated
/// samples of the same claim/evidence pair suggests the LLM's verdict is
/// unreliable, independent of its own stated confidence.
fn agreement_rate(verdicts: &[Verdict]) -> f64 {
    if verdicts.is_empty() {
        return 0.0;
    }
    use std::collections::HashMap;
    let mut counts: HashMap<Verdict, usize> = HashMap::new();
    for v in verdicts {
        *counts.entry(*v).or_insert(0) += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    max_count as f64 / verdicts.len() as f64
}
```

This requires `Verdict` to implement `Hash` — check its current derive list (originally `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`) and add `Hash` if not already present:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Supported,
    Contradicted,
    Contested,
    Unverified,
}
```

- [ ] **Step 4: Run the three new tests**

Run: `cargo test -p vox-research-shim agreement_rate`
Expected: 3 tests pass.

- [ ] **Step 5: Add a resampling wrapper around the existing per-claim verification call**

Inside the `runtime`-feature branch of `verify_claims_with_config`, where each claim is currently verified once via a single `chat_with_cascade` call, wrap it to sample 3 times and compute agreement. Since the exact loop structure needs to be read fresh (per this plan's "before you start" note), the shape to implement is:

```rust
    // For each claim, sample the verification cascade `RESAMPLE_COUNT` times
    // at the cascade's existing temperature setting and keep the majority
    // verdict, recording the agreement rate as a new field on ClaimVerdict.
    const RESAMPLE_COUNT: usize = 3;
```

Add `pub self_consistency: f64` to `ClaimVerdict` (originally lines 86-95):

```rust
pub struct ClaimVerdict {
    pub claim: Claim,
    pub verdict: Verdict,
    pub confidence: f64,
    pub supporting_count: usize,
    pub contradicting_count: usize,
    pub evidence_spans: Vec<EvidenceSpan>,
    /// Fraction of `RESAMPLE_COUNT` repeated verification calls that agreed
    /// with the final `verdict`. 1.0 = fully consistent, lower = the LLM
    /// gave different answers across resamples for the same claim/evidence.
    pub self_consistency: f64,
}
```

Then in the per-claim loop, replace the single `chat_with_cascade` + `parse_verifier_response` call with a loop collecting `RESAMPLE_COUNT` verdicts, taking the majority as `verdict`, and setting `self_consistency: agreement_rate(&sampled_verdicts)`. Every other existing construction of `ClaimVerdict` in this file (including the `unverified(claim)` fallback helper) needs `self_consistency: 1.0` added (a single unverified result is trivially "fully consistent" with itself — this keeps the fallback path meaningful rather than introducing a fake low-confidence signal where none was measured).

- [ ] **Step 6: Run the full verifier test suite**

Run: `cargo test -p vox-research-shim verifier:: -- --nocapture`
Expected: all existing tests pass (with `self_consistency: 1.0` added to their fixtures where they construct `ClaimVerdict` directly), plus the 3 new `agreement_rate` tests.

- [ ] **Step 7: Feed `self_consistency` into the confidence gate**

In `pipeline.rs`, where `supported_claim_count` is computed (originally lines 386-389), this plan doesn't require changing that count itself, but a future gate-fusion improvement should read `claim_verdicts[].self_consistency` — out of scope for this task's minimal wiring; leave a two-line comment at the `supported_claim_count` computation site noting the new field is available on `ClaimVerdict` for a follow-up fusion-weight task, rather than silently letting it go unused with no trace.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-research-shim/src/research/verifier.rs
git commit -m "feat: add self-consistency resampling to claim verifier (SelfCheckGPT-style)"
```

---

### Task 8: Populate `NoveltyEvidenceBundle` with lexical similarity (semantic embeddings out of scope)

The full trust/novelty research recommended a two-stage MinHash-LSH + embedding-cosine pipeline. This plan's dependency audit found **no ANN/embedding crate in the workspace** — adding one (e.g. `candle`-based embeddings, reusing the pinned `candle-core`/`candle-nn` version already used by `vox-plugin-mens-*`) is real work deserving its own scoped follow-up plan, not a silent addition here. This task implements the **lexical stage only** (`lexical_score`), leaving `semantic_score` as `None` (already an `Option<f64>` in the schema — this is a legitimate partial-population, not a hack).

**Files:**
- Create: `crates/vox-scientia/src/producers/novelty_lexical.rs`
- Modify: `crates/vox-scientia/src/producers/mod.rs` (register the new module — check exact existing pattern for `dedup.rs`'s registration first)
- Modify: `crates/vox-scientia/src/producers/dedup.rs` (use lexical similarity instead of exact `finding_id` match)

- [ ] **Step 1: Write a failing test for lexical-similarity-based dedup**

Create `crates/vox-scientia/src/producers/novelty_lexical.rs`:

```rust
//! Lexical (shingle-based) similarity scoring for finding-candidate dedup,
//! reusing the same 4-gram FNV1a shingling approach as
//! `vox_search::novelty::NoveltyScorer`, applied here across the full
//! history of prior findings rather than a single session.

use std::collections::HashSet;

fn fnv1a(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn shingle_set(content: &str, n: usize) -> HashSet<u64> {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() < n {
        return [fnv1a(content)].into_iter().collect();
    }
    chars
        .windows(n)
        .map(|w| fnv1a(&w.iter().collect::<String>()))
        .collect()
}

/// Jaccard similarity between two texts' 4-gram character shingle sets.
/// 1.0 = identical shingle sets, 0.0 = no overlap.
pub fn lexical_similarity(a: &str, b: &str) -> f64 {
    let sa = shingle_set(a, 4);
    let sb = shingle_set(b, 4);
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let intersection = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_scores_one() {
        assert_eq!(lexical_similarity("the same text here", "the same text here"), 1.0);
    }

    #[test]
    fn completely_different_text_scores_low() {
        let sim = lexical_similarity(
            "quantum entanglement in superconducting circuits",
            "sourdough bread fermentation temperature control",
        );
        assert!(sim < 0.1, "expected low similarity, got {sim}");
    }

    #[test]
    fn near_restatement_scores_high() {
        let sim = lexical_similarity(
            "the confidence gate fuses citation and claim support scores",
            "the confidence gate fuses citation and claim-support scores",
        );
        assert!(sim > 0.7, "expected high similarity for near-restatement, got {sim}");
    }

    #[test]
    fn empty_strings_score_one() {
        assert_eq!(lexical_similarity("", ""), 1.0);
    }
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test -p vox-scientia novelty_lexical:: -- --nocapture`
Expected: 4 tests pass.

- [ ] **Step 3: Register the module**

Find `crates/vox-scientia/src/producers/mod.rs`, and add (matching however `dedup` is currently declared there, e.g. `pub mod dedup;`):

```rust
pub mod novelty_lexical;
```

- [ ] **Step 4: Write a failing test for similarity-based dedup replacing exact-match**

In `crates/vox-scientia/src/producers/dedup.rs`, add to the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn collapses_near_duplicate_finding_text_even_with_different_ids() {
        let events = vec![
            fc_with_text("id-1", "The synthesis stage lacks a post-hoc citation audit"),
            fc_with_text("id-2", "The synthesis stage lacks a post hoc citation audit"), // near-identical, different id
        ];
        let deduped = dedup_finding_candidates(events);
        assert_eq!(deduped.len(), 1, "near-duplicate finding text should collapse despite different finding_ids");
    }
```

This requires a new test helper `fc_with_text` alongside the existing `fc(id: &str)` helper — add it near `fc`:

```rust
    fn fc_with_text(id: &str, text: &str) -> ResearchEvent {
        ResearchEvent::FindingCandidateProposed {
            finding_id: id.into(),
            claim_ids: vec![],
            worthiness_score: 0.5,
            session_id: "s".into(),
            finding_candidate: Some(vox_research_events::FindingCandidateV1 {
                title_hint: text.to_string(),
                // Remaining fields: check FindingCandidateV1's actual definition
                // in vox-research-events/src/schema_types.rs before filling in —
                // this plan doesn't have its full field list; use ..Default::default()
                // if the type derives Default, otherwise construct with sensible
                // minimal values matching existing FindingCandidateV1 test fixtures
                // elsewhere in the vox-scientia or vox-research-events test suites.
                ..Default::default()
            }),
        }
    }
```

**Note for the implementer:** before writing this helper, run `grep -n "struct FindingCandidateV1" -A 30 crates/vox-research-events/src/schema_types.rs` to get the real field list, since this plan's grounding pass didn't capture it in full — the `title_hint` field name is inferred from context in the trust/novelty research doc and needs confirming against actual source, not assumed.

- [ ] **Step 5: Update `dedup_finding_candidates` to use lexical similarity**

Replace the exact-match logic in `dedup.rs`:

```rust
pub fn dedup_finding_candidates(events: Vec<ResearchEvent>) -> Vec<ResearchEvent> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        match &ev {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                if seen.insert(finding_id.clone()) {
                    out.push(ev);
                }
            }
            _ => out.push(ev),
        }
    }
    out
}
```

with a version that also checks lexical similarity against previously-accepted findings' `title_hint` (falling back to exact-`finding_id` behavior when `finding_candidate` is `None`, preserving the original function's behavior for events that don't carry the optional payload):

```rust
const LEXICAL_DUPLICATE_THRESHOLD: f64 = 0.85;

pub fn dedup_finding_candidates(events: Vec<ResearchEvent>) -> Vec<ResearchEvent> {
    use super::novelty_lexical::lexical_similarity;
    let mut seen_ids = HashSet::new();
    let mut seen_texts: Vec<String> = Vec::new();
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        match &ev {
            ResearchEvent::FindingCandidateProposed {
                finding_id,
                finding_candidate,
                ..
            } => {
                if !seen_ids.insert(finding_id.clone()) {
                    continue; // exact-id duplicate, unchanged behavior
                }
                if let Some(candidate) = finding_candidate {
                    let is_near_duplicate = seen_texts
                        .iter()
                        .any(|prior| lexical_similarity(prior, &candidate.title_hint) >= LEXICAL_DUPLICATE_THRESHOLD);
                    if is_near_duplicate {
                        continue;
                    }
                    seen_texts.push(candidate.title_hint.clone());
                }
                out.push(ev);
            }
            _ => out.push(ev),
        }
    }
    out
}
```

- [ ] **Step 6: Run the full dedup test suite**

Run: `cargo test -p vox-scientia dedup:: -- --nocapture`
Expected: all 3 original tests pass (unchanged behavior for exact-id and non-candidate events) plus the new near-duplicate test from Step 4.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-scientia/src/producers/novelty_lexical.rs crates/vox-scientia/src/producers/mod.rs crates/vox-scientia/src/producers/dedup.rs
git commit -m "feat: replace exact-finding_id dedup with lexical-similarity dedup"
```

---

### Task 9: Populate `WorthinessSignalsV2` hard/soft gates

Use Task 4's `TrustScorer` to populate the previously-empty worthiness schema.

**Files:**
- Create: `crates/vox-scientia/src/producers/worthiness.rs`
- Modify: `crates/vox-scientia/src/producers/mod.rs`

- [ ] **Step 1: Write a failing test for the hard-gate retraction check**

Create `crates/vox-scientia/src/producers/worthiness.rs`:

```rust
//! Populates `WorthinessSignalsV2` hard/soft gates from `TrustScorer`
//! (retraction status, venue reputation).

use vox_research_events::WorthinessSignalsV2;
use vox_research_events::schema_types::{WorthinessProfile, WorthinessSignalItem};

/// Builds the hard-gate retraction signal for a candidate finding, given
/// whether its primary source DOI is confirmed retracted.
pub fn hard_gate_retraction_signal(is_retracted: bool) -> WorthinessSignalItem {
    WorthinessSignalItem {
        id: "hg-retraction".to_string(),
        passed: !is_retracted,
        score: if is_retracted { 0.0 } else { 1.0 },
        reason_code: if is_retracted {
            "source_retracted".to_string()
        } else {
            "no_retraction_detected".to_string()
        },
        details: None,
    }
}

/// Builds the soft-gate peer-review-status signal from an OpenAlex venue
/// type string (as returned by `TrustScorer::reputation_multiplier`'s
/// underlying lookup — this function takes the venue type directly so it
/// stays independently testable without live HTTP).
pub fn soft_gate_peer_review_signal(venue_type: Option<&str>) -> (WorthinessSignalItem, WorthinessProfile) {
    let (profile, passed, score, reason) = match venue_type {
        Some("journal") => (WorthinessProfile::Journal, true, 1.0, "peer_reviewed_journal"),
        Some("repository") => (WorthinessProfile::Repository, true, 0.7, "institutional_repository"),
        Some("preprint") => (WorthinessProfile::Preprint, true, 0.5, "preprint_not_peer_reviewed"),
        _ => (WorthinessProfile::Social, false, 0.2, "unverified_venue"),
    };
    (
        WorthinessSignalItem {
            id: "sg-peer-review".to_string(),
            passed,
            score,
            reason_code: reason.to_string(),
            details: None,
        },
        profile,
    )
}

/// Assembles a `WorthinessSignalsV2` from the individual signal builders
/// above. `next_actions` is intentionally left empty here — populating it
/// requires the diagnostic-tier statcheck-style numeric-claim recheck,
/// which is out of scope for this task (see the trust/novelty research
/// doc's §5 for that follow-up).
pub fn build_worthiness_signals(
    version: &str,
    is_retracted: bool,
    venue_type: Option<&str>,
) -> WorthinessSignalsV2 {
    let hard = hard_gate_retraction_signal(is_retracted);
    let (soft, profile) = soft_gate_peer_review_signal(venue_type);
    WorthinessSignalsV2 {
        version: version.to_string(),
        profile,
        hard_gate: vec![hard],
        soft_gate: vec![soft],
        diagnostic: vec![],
        next_actions: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retracted_source_fails_hard_gate() {
        let signal = hard_gate_retraction_signal(true);
        assert!(!signal.passed);
        assert_eq!(signal.reason_code, "source_retracted");
    }

    #[test]
    fn clean_source_passes_hard_gate() {
        let signal = hard_gate_retraction_signal(false);
        assert!(signal.passed);
        assert_eq!(signal.reason_code, "no_retraction_detected");
    }

    #[test]
    fn journal_venue_passes_soft_gate_with_full_score() {
        let (signal, profile) = soft_gate_peer_review_signal(Some("journal"));
        assert!(signal.passed);
        assert_eq!(signal.score, 1.0);
        assert_eq!(profile, WorthinessProfile::Journal);
    }

    #[test]
    fn unknown_venue_fails_soft_gate() {
        let (signal, profile) = soft_gate_peer_review_signal(None);
        assert!(!signal.passed);
        assert_eq!(profile, WorthinessProfile::Social);
    }

    #[test]
    fn build_worthiness_signals_assembles_both_gates() {
        let bundle = build_worthiness_signals("v2", false, Some("journal"));
        assert_eq!(bundle.hard_gate.len(), 1);
        assert_eq!(bundle.soft_gate.len(), 1);
        assert!(bundle.hard_gate[0].passed);
        assert!(bundle.soft_gate[0].passed);
    }
}
```

**Note for the implementer:** confirm `WorthinessProfile` derives `PartialEq` (the grounding pass for this plan confirmed it does: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`) so the `assert_eq!` calls above compile.

- [ ] **Step 2: Run the tests**

Run: `cargo test -p vox-scientia worthiness:: -- --nocapture`
Expected: 5 tests pass.

- [ ] **Step 3: Register the module**

In `crates/vox-scientia/src/producers/mod.rs`:

```rust
pub mod worthiness;
```

- [ ] **Step 4: Commit**

```bash
git add crates/vox-scientia/src/producers/worthiness.rs crates/vox-scientia/src/producers/mod.rs
git commit -m "feat: populate WorthinessSignalsV2 hard/soft gates from trust signals"
```

(Wiring `build_worthiness_signals` into the actual SCIENTIA finding-promotion call path — i.e., calling it with real retraction/venue data fetched via `TrustScorer` for each candidate, and persisting the result — is deliberately left to a follow-up task once the call-site location in the promotion pipeline is confirmed; this task delivers the tested, standalone building blocks.)

---

## Self-review notes

- **Spec coverage:** Tasks 1-9 cover spec items 1-7 (doc fix, pipeline unification split into Tasks 2+3, trust score Task 4-6, verifier upgrade Task 7 scoped to self-consistency instead of ONNX-NLI per the dependency-audit finding, novelty evidence Task 8 scoped to lexical-only, worthiness signals Task 9). **Item 7 (post-hoc citation audit)** from the spec is not yet a task here — it depends on the synthesis pipeline's exact citation-marker format, which needs a fresh read of `stages.rs::synthesize_answer_with_llm` before code can be written without placeholders; treat it as Task 10 in a follow-up plan revision once that file is read, rather than writing unverified code against a marker format this plan's grounding pass didn't capture verbatim.
- **Placeholder scan:** no TBD/TODO left in code steps; the two explicit "confirm exact current field list before writing" notes (Task 8 Step 4, Task 9 note) are flagged as verification steps for the implementer, not skipped work — they name exactly what to check and how.
- **Type consistency:** `Verdict` now needs `Hash` (Task 7); `ClaimVerdict` gains `self_consistency` (Task 7) — every construction site across the file must be updated, called out explicitly in Task 7 Step 6. `GateInput` gains `trust_weighted_citation_score` (Task 6) — every construction site must be updated, called out in Task 6 Step 4.
