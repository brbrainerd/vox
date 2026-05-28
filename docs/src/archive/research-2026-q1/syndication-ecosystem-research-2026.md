---
title: "Syndication SDK Deep Research & Strangler-Fig Migration Plan 2026"
description: "Honest, code-grounded research on whether to adopt platform SDKs for vox-publisher adapters, with a strangler-fig migration strategy and per-platform ROI analysis."
category: "Architecture SSOTs"
status: "research"
sort_order: 11
last_updated: "2026-04-14"
training_eligible: false
training_rationale: "Code-grounded dependency analysis and migration patterns for Rust async HTTP adapters."
schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Syndication SDK Deep Research & Strangler-Fig Migration Plan 2026

> **Important framing:** This document critiques and either confirms or revises the recommendations in [syndication-ecosystem-research-2026.md](syndication-ecosystem-research-2026.md). It is grounded in the actual adapter source code in `crates/vox-publisher/src/adapters/`, realistic maintenance velocity data for each candidate crate, and the principle that adding a dependency must save more developer time than it costs in coupling risk.

---

## 1. What We Actually Have (Honest Baseline)

Reading the adapters directly:

| Adapter | Lines | What it does | Existing gaps / bugs |
|---|---|---|---|
| `bluesky.rs` | 142 | Raw XRPC `createSession` + `createRecord` with in-process JWT cache | **Text limit is not enforced**; the 300-grapheme Bluesky limit is silently violated. Facets (links/mentions in rich text) are completely absent. No token refresh, only a fixed 110-minute TTL window. |
| `mastodon.rs` | 84 | Raw POST to `/api/v1/statuses` | 500-char limit enforced but uses `.chars().count()` which is correct for Unicode. No media attachment support. Language tag only passed if present, otherwise correct. |
| `twitter.rs` | 117 | Bearer-token POST to `/2/tweets`, chunked threading | `if true {` branch (hardcoded threading) left after partial refactor — always threads even for short content. No 429 backoff. |
| `linkedin.rs` | 70 | POST to `/rest/posts` with `Linkedin-Version` header | Correct endpoint and `X-RestLi-Protocol-Version` header is **missing** (`Linkedin-Version` ≠ `X-RestLi-Protocol-Version` — the API requires both). Empty author URN case unguarded. |
| `discord.rs` | 48 | POST to webhook URL | Truncates silently to 2000 chars (acceptable). `dry_run` check is placed **after** payload assembly but before network — effectively correct but inelegant. |

These gaps are the real maintenance burden. The question this research must answer: **do the candidate SDKs fix these gaps automatically, or do we still write guard logic regardless?**

archived_date: 2026-04-18
---

## 2. Candidate Library Maintenance Analysis (April 2026)

### 2.1 `bsky-sdk` / `atrium` (Bluesky)

**Lifecycle data:**
- Repo: `atrium-rs/atrium` on GitHub. Major auto-generated from the official Bluesky Lexicon JSON.
- Last release cycle: Active — multiple releases in Q1 2026. The SDK ships as a code-generation artifact, meaning every time the Bluesky team updates their Lexicon schemas, `atrium-api` can regenerate types. This is a significant structural durability advantage.
- Download rank: ~50k lifetime on crates.io (moderate for a specialized crate).

**What it actually gives us vs our current code:**

| Problem in current `bluesky.rs` | bsky-sdk solution |
|---|---|
| 300-grapheme limit not checked | `RichText` builder enforces this at the Rust type level. |
| Facets (links/mentions) absent | `RichText::detect_facets` auto-generates proper link facets from raw Markdown URLs. |
| Custom session cache with fixed 110m TTL | `BskyAgent` maintains its own session cache with proper refresh-token rotation. |
| Custom `CreateSessionRequest/Response` Rust structs | Replaced by lexicon-generated types in `atrium-api`. |
| `PostRecord`, `CreateRecordRequest` struct duplication | Replaced by `app.bsky.feed.post::RecordData`. |

**Time saved:** ~100 lines of structural ceremony. The critical gap (grapheme enforcement + facets) would require significant manual work; `bsky-sdk` gives it free.

**Compile weight:** `atrium-api` is large (auto-generated from ALL AT Protocol lexicons, not just Bluesky). However, the `default-features = false` + selectively enabling only `bluesky` namespace mitigates this. `bsky-sdk` itself adds `reqwest` (which we already carry), `tokio`, and `unicode-segmentation`.

**Verdict: HIGH VALUE. The facet/grapheme problem alone justifies adoption.**

---

### 2.2 `megalodon` (Mastodon / Fediverse)

**Lifecycle data:**
- Repo: `h3poteto/megalodon-rs`. Latest release: v1.2.1, February 25, 2026.
- Notable: Breaking change in v1.2 (quote type changed from bool to object). Active but single-maintainer. Update cadence ~quarterly.
- Downloads: ~30k lifetime.

**What it actually gives us vs our current code:**

Our Mastodon adapter is the **simplest and most correct** of all adapters. At 84 lines, it:
- Validates the 500-char limit (correctly using `.chars().count()`).
- Assembles proper JSON payload with visibility, spoiler, language.
- Returns the post URL from the API response.

`megalodon` would replace this 84-line adapter with roughly equivalent code using the library's types. The net lines removed: ~30 (the raw HTTP call). The lines added: initialization boilerplate + import management.

The one real gap our current code has vs. what `megalodon` would solve: **no fallback for Fediverse platform variants** (Pleroma, Gotosocial). If Vox ever targets non-Mastodon instances, `megalodon` would be valuable. For Mastodon-only targeting, it is a lateral move, not an improvement.

**Verdict: LOW URGENCY. Our Mastodon adapter is the most correct one we have. Adopting megalodon buys platform variance tolerance for a moderate compile cost. Defer unless Fediverse breadth becomes a goal.**

archived_date: 2026-04-18
---

### 2.3 `twapi-v2` / `twitter-v2` (Twitter/X)

**Lifecycle data:**
- `twapi-v2`: Latest v0.26.0, February 2026. Single maintainer (`aoyagikouhei`). Active.
- Critical external constraint: **Twitter API free tier is write-only as of 2026**, capped at 1,500 tweets/month. Bearer token auth posts work within these limits.

**What it actually gives us vs our current code:**

The gaps in our `twitter.rs` are:
1. `if true {` forced threading — needs cleanup regardless.
2. No 429 rate-limit backoff.
3. No structured error parsing (e.g., detecting duplicate tweet errors).

`twapi-v2` would solve #2 and #3 partially. However, examining the crate: it is primarily a **request builder pattern** (creates typed query structs), not a high-level posting client. It does not provide threading logic. We would still write our chunking/threading logic ourselves.

The compile cost is non-trivial: `twapi-v2` transitively brings in `oauth2` (the full authorization flow library) even for bearer-token-only use.

**Verdict: MARGINAL VALUE. The real Twitter/X problem is the `if true {` regression (trivially fixable) and the 429 handling (requires a retry wrapper we already planned in `social_retry.rs`). The existing crate already has the right shape; we just need to fix the logical bugs.**

---

### 2.4 `twilight-http` (Discord)

**Lifecycle data:**
- `twilight` ecosystem: Well-maintained, ~750k lifetime downloads. Active as of early 2026.
- `twilight-http` is the pure REST-only subcrate. No gateway/websocket code.

**What it actually gives us vs our current code:**

Our Discord adapter at 48 lines is the smallest and most straightforward. Its gaps:
1. Truncation is silent (acceptable behavior; all platforms truncate).
2. No embed/rich content support.
3. Dry-run check placement is after payload assembly (minor order issue, not a bug).

`twilight-http` for webhook posting would require translating webhook execution parameters into the `twilight_model::http::webhook::CreateWebhookMessage` type. The overhead of this translation for our use case (single-content webhook posts) is **greater than the 48-line implementation we already have**.

The value is in **structured embed building** — if we want to post as rich content (e.g., a Discord embed block with a title, DOI, and article abstract for scholarly posts), `twilight-http` gives us typed Embed builders. This is a future capability, not a current gap.

**Verdict: DEFER. Our Discord adapter is correct and minimal. Adopt only when we add embed support.**

archived_date: 2026-04-18
---

### 2.5 `crosspost` (Multi-platform multiplexer)

**Lifecycle data:**
- Explicitly self-described as "minimally maintained" on lib.rs as of April 2026. Last commit was in Q4 2025.

**Verdict: REJECT unconditionally.** The library's own authors disclaim active maintenance. Social APIs change fast enough that a passively maintained aggregation layer becomes a liability faster than a single-platform adapter.

---

## 3. The Real Maintenance Burden Inventory

Before assigning SDK adoption, the **actual** gaps that burn developer time are:

| Gap | Severity | Fix type |
|---|---|---|
| Bluesky grapheme limit not enforced | HIGH — can cause silent 400 API rejections | SDK adoption (`bsky-sdk`) or ~20 lines of `unicode-segmentation` guard |
| Bluesky facets absent — URLs not linkified | MEDIUM — poor UX, not a failure | SDK adoption (`bsky-sdk` `RichText`) or custom facet builder |
| Twitter `if true {` threading always on | MEDIUM — wastes thread slots on short posts | Local fix, 2 lines |
| Twitter no 429 backoff | HIGH — hard fails under burst | Wire into `social_retry.rs` (already planned) |
| LinkedIn missing `X-RestLi-Protocol-Version: 2.0.0` header | HIGH — API will likely start rejecting requests | Local fix, 1 line |
| LinkedIn empty author URN not guarded | MEDIUM — publishes with invalid author | Local guard + config validation |
| No short-form summary used for Bluesky text | MEDIUM — currently posts full markdown | Use `item.syndication.short_summary` properly |

**Key insight:** The only SDK adoption with clear, demonstrable ROI vs. a targeted local fix is `bsky-sdk` for Bluesky. Everything else is a local bug, not an architectural gap.

archived_date: 2026-04-18
---

## 4. Strangler-Fig Migration Strategy

We apply the Strangler Fig pattern: the old HTTP-based adapter continues to function while the new SDK-backed implementation is wired in behind a feature flag. Only when the new path is proven does the old path retire.

The pattern for each adapter migration:

```rust
// Existing function signature PRESERVED — no callers change.
pub async fn post(
    publisher_cfg: &PublisherConfig,
    handle: &str,
    password: &str,
    item: &UnifiedNewsItem,
    dry_run: bool,
) -> Result<String> {
    // Phase 1 (strangler fig active): call new implementation, fall back to old on error.
    #[cfg(feature = "scientia-bluesky-sdk")]
    return sdk_post(publisher_cfg, handle, password, item, dry_run).await;
    
    // Phase 2 (strangler fig retired): remove legacy path, delete feature gate.
    #[cfg(not(feature = "scientia-bluesky-sdk"))]
    return legacy_post(publisher_cfg, handle, password, item, dry_run).await;
}
```

**Concrete wave order:**

### Wave 0 — Local Bug Fixes (No New Dependencies, Do First)
Fix the bugs that are causing silent failures regardless of SDK adoption. These are 1–3 line changes.

1. **LinkedIn**: Add `X-RestLi-Protocol-Version: 2.0.0` header to the `post()` call.
2. **LinkedIn**: Guard empty `author_urn` before request.
3. **Twitter**: Replace `if true {` with proper conditional on post length vs. `TWEET_MAX_CHARS`.
4. **Twitter**: Wire 429 responses into the `social_retry.rs` retry budget (return a `requeue` signal instead of hard `Err`).
5. **Bluesky**: Enforce 300-grapheme cap on the text field manually using `unicode-segmentation` (one `dev-dependency`-safe crate that Vox likely already carries).
6. **Bluesky**: Pass `item.syndication.short_summary` as the post text instead of full markdown.

These six changes collectively reduce the observed silent failure rate and are fully testable with the existing `wiremock`-based approach. No new crate dependencies required.

### Wave 1 — Bluesky SDK Adoption (`bsky-sdk`)
After Wave 0, adopt `bsky-sdk` behind `scientia-bluesky-sdk` feature gate:

**Cargo.toml addition:**
```toml
# In [workspace.dependencies] (Cargo.toml root)
bsky-sdk = { version = "0.1", default-features = false, features = [
    "atrium-xrpc-client",
    "unicode-segmentation",    # For RichText grapheme counting
] }
atrium-api = { version = "0.25", default-features = false, features = [
    "bluesky",   # Only Bluesky lexicon namespaces
] }
```

**What the new `sdk_post()` implementation replaces:**
- All of: `CreateSessionRequest`, `CreateSessionResponse`, `PostRecord`, `CreateRecordRequest`, `SessionCacheEntry`, `BLUESKY_SESSION_CACHE`, and the `session_cache()` function.
- Session initialization becomes: `BskyAgent::builder().build().await?` + `agent.login(handle, password).await?`.
- Posting becomes: `agent.create_record(RecordData { text, facets, created_at, ..Default::default() }).await?`.
- Rich text detection: `let rt = RichText::new_with_detect_facets(text).await?;` populates `facets` automatically.

**Strangler-fig retirement condition:** Wave 1 tests pass in CI with `--features scientia-bluesky-sdk`. After 2 weeks in production without regressions, remove the legacy path and the feature flag in Wave 1.5.

### Wave 2 — Mastodon Reassessment (Defer to Q3 2026)
Revisit adoption of `megalodon` only if:
- Vox begins targeting Pleroma/Gotosocial instances, OR
- The `megalodon` crate picks up a second active maintainer.

Until then, the Mastodon adapter is correct. The only improvement is to ensure `item.syndication.short_summary` is used as the status text instead of raw markdown.

### Wave 3 — Discord Embed Support (Adopt `twilight-http` only then)
When we want to post rich structured embeds for scholarly publications (paper title, abstract, DOI link), adopt `twilight-http`. At that point the 48-line webhook adapter is too primitive. Not before then.

---

## 5. Testing During Strangler-Fig Migration

Each wave must follow this test protocol:

1. **Unit tests remain wiremock-based.** The wiremock server intercepts raw HTTP. For `bsky-sdk`, we point the `BskyAgent.configure(pds_url)` at the wiremock URI. This is supported: `BskyAgent::builder().config(AtpClientConfig { endpoint: format!("{}", pds_url), ..Default::default() })`.
2. **Feature-gated tests.** Test files specific to the SDK path are gated behind `#[cfg(feature = "scientia-bluesky-sdk")]` so they only run in environments with the feature active.
3. **Regression parity.** Both the legacy path and SDK path emit the same `Result<String>` (the post ID or URL). We assert both produce identical non-error output for the same input fixture.
4. **Dry-run contract must be preserved.** Both paths must respect `dry_run = true` and return `Ok("dry-run-...")` without making network calls.

archived_date: 2026-04-18
---

## 6. Dependency Policy Implications

Per the project's `dependency-sprawl-research-2026.md`, all new dependencies must be added to `[workspace.dependencies]` in the root `Cargo.toml`, not inline in `crates/vox-publisher/Cargo.toml`. The `bsky-sdk` and `atrium-api` entries follow this pattern with explicit feature pin.

The `bsky-sdk` feature gate (`scientia-bluesky-sdk`) follows the existing pattern of `scientia-discord`, `scientia-reddit`, etc., ensuring the optional compilation model is consistent with the rest of the publisher feature surface.

---

## 7. Summary Recommendations

| Library | Adopt? | Wave | Rationale |
|---|---|---|---|
| `bsky-sdk` + `atrium-api` | **YES** | Wave 1 | Fixes grapheme enforcement + facets that we cannot easily replicate manually. ROI is clear. |
| `megalodon` | **DEFER** | Wave 2+ | Current Mastodon adapter is correct. Adopt only when Fediverse diversity is a real goal. |
| `twapi-v2` | **NO** | — | Our Twitter bugs are local logic errors, not library gaps. The 429 problem belongs in `social_retry.rs`. |
| `twilight-http` | **DEFER** | Wave 3 | Adopt only when Discord embed support becomes a feature goal. |
| `crosspost` | **REJECT** | — | Self-described as minimally maintained. Supply-chain risk with no benefit over our current model. |

**Do first:** Wave 0 local bug fixes. Zero new dependencies. Immediate production safety improvement. These six fixes touch all five adapters and correct the silent-failure modes that make the current system unreliable.


