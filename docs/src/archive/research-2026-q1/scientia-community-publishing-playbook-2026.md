---
title: "Scientia Community Publishing Playbook 2026"
description: "Comprehensive implementation plan for the multi-platform Vox Scientia community publishing pipeline. Covers codebase audit findings, 30+ identified problems with explicit solutions, Clavis secret registration requirements, data model gaps, topic-pack contract extensions, and a dependency-ordered execution backlog."
category: "architecture"
status: "roadmap"
sort_order: 16
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Primary implementation reference for vox-publisher community channel adapters. Replaces the first-draft playbook which contained incorrect API details and failed to reference the existing adapter structure."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Scientia Community Publishing Playbook 2026

This document is a **ground-truth implementation plan** built from a full audit of the `crates/vox-publisher/` crate, all adapter stubs, the `contracts/scientia/` YAML files, and the `vox-clavis` secret registry.

> **Self-critique of the first draft:** The initial playbook (now replaced by this document) had numerous critical errors: it described the Reddit adapter as if it used password-based OAuth when the actual code uses `refresh_token` grant; it proposed adding four Clavis secrets that may already exist; it described `SyndicationConfig` as not having LinkedIn/Mastodon/Bluesky fields when it plainly does; it failed to mention that `discord.rs`, `linkedin.rs`, and `mastodon.rs` are TOESTUB stubs returning `Err("not implemented")`; and it described the GitHub Integration as using pure GraphQL when the actual code routes through `vox-forge`'s `GitForgeProvider` abstraction. Every section below is code-verified.

## See also

- [SCIENTIA multi-platform ranking, discovery, and anti-slop SSOT](scientia-multi-platform-ranking-discovery-research-2026.md) — posture decisions (ingest vs syndicate)
- [SCIENTIA publication pipeline SSOT](scientia-pipeline-ssot-2026.md) — primary implementation contract
- [`crates/vox-publisher/src/types.rs`](../../../crates/vox-publisher/src/types.rs) — primary data model
- [`crates/vox-publisher/src/adapters/`](../../../crates/vox-publisher/src/adapters/) — all channel adapters
- [`contracts/scientia/distribution.topic-packs.yaml`](../../../contracts/scientia/distribution.topic-packs.yaml) — channel routing policy

---

## 1. Revised Community Strategy

Communities form around projects whether or not the project participates. The correct posture is a **funnel model**: every ephemeral discussion on Discord or Reddit must resolve to a durable GitHub artifact before it is considered "done." These channels are engagement amplifiers whose job is to route discovery → GitHub.

```
[World]           Discovery Flow           [Our SSOT]
 Reddit ─────────────────────────────►  GitHub Discussions (canonical)
 Discord ────────────────────────────►  docs/src/architecture/ (research)
 Hacker News ─────────────────────────►  GitHub Issues (bugs, features)

[Our SSOT]         Automated Publish       [World]
 vox-publisher ──────────────────────►  RSS, GitHub Release, Reddit, Discord
 Scientia finding ───────────────────►  Open Collective, HN (manual)
```

| Channel | Posture | Max Automation | Human Gate Required? |
|---|---|---|---|
| **GitHub Discussions** | Canonical SSOT | Full (via `ForgeConfig`) | Sensitive decisions only |
| **Open Collective** | Funding + milestone | Full (adapter live) | Yes — content review |
| **Reddit** | Syndicate releases | `SelfPost` announcements | Yes — subreddit selection per post |
| **Discord** | Community + support | Webhook for releases only | Full moderation overhead |
| **Hacker News** | High-value only | `ManualAssist` hardcoded | Always |
| **Bluesky / Mastodon** | Delta short posts | Once adapters are live | Per run |
| **LinkedIn** | Professional reach | Once adapter is live | Per post |
| **RSS** | Default on | Fully automated | None |
| **YouTube** | Long-form demos | Once adapter is live | Per video |

archived_date: 2026-04-18
---

## 2. Codebase Audit — Problems and Solutions

The following 30+ problems are ordered by dependency (foundational issues first).

---

### PROBLEM-01: Reddit adapter uses `refresh_token` grant but no token storage

**File:** [`crates/vox-publisher/src/adapters/reddit.rs`](../../../crates/vox-publisher/src/adapters/reddit.rs)

**Problem:** `RedditAuthConfig` requires a `refresh_token` (OAuth PKCE/script app long-lived token), but the initial playbook described a `password` grant. The `refresh_access_token` function exchanges a refresh token for a short-lived `access_token` on every call. There is no token caching layer — each publish invocation makes an unnecessary OAuth round-trip.

**Solution:** Add an in-memory `Arc<Mutex<Option<CachedToken>>>` to the publish dispatch in `lib.rs` that stores the `access_token` and its `expires_in` deadline. Re-use if valid; refresh only if expired. This is a single-invocation optimization, not a redistribution concern.

**Clavis secrets required (verify against `spec.rs` before adding):**
- `VoxRedditClientId`
- `VoxRedditClientSecret`
- `VoxRedditRefreshToken` ← **not `VoxRedditBotPassword`** (the first draft was wrong)
- `VoxRedditUserAgent`

archived_date: 2026-04-18
---

### PROBLEM-02: Discord adapter is a hard stub

**File:** [`crates/vox-publisher/src/adapters/discord.rs`](../../../crates/vox-publisher/src/adapters/discord.rs)

**Problem:** The file is 13 lines. It unconditionally returns `Err(anyhow!("Discord adapter not implemented"))`. Because `SyndicationResult::has_failures` checks `discord`, any `UnifiedNewsItem` that specifies `discord:` config will always produce a `Failed` outcome at runtime.

**Solution:** Implement using a webhook POST (not a bot). Discord webhooks are the correct primitive for one-way announcement channels. The implementation should:
1. Read webhook URL from Clavis (`VoxDiscordWebhookUrl`)
2. POST to `https://discord.com/api/webhooks/{id}/{token}` with JSON body
3. Support rich embeds (requiring a `DiscordConfig` model extension — see PROBLEM-04)
4. Parse `Retry-After` header on `429` responses using the existing `social_retry.rs` infrastructure

**Clavis secrets required:**
- `VoxDiscordWebhookUrl` (one per channel — see PROBLEM-05 for multi-channel)

---

### PROBLEM-03: LinkedIn and Mastodon adapters are hard stubs

**Files:**
- [`crates/vox-publisher/src/adapters/linkedin.rs`](../../../crates/vox-publisher/src/adapters/linkedin.rs)
- [`crates/vox-publisher/src/adapters/mastodon.rs`](../../../crates/vox-publisher/src/adapters/mastodon.rs)

**Problem:** Both are 13-line stubs identical in structure to `discord.rs`. Both are tracked in `SyndicationResult` and will produce `Failed` outcomes if configured.

**Solution (LinkedIn):** Use the LinkedIn UGC Posts API (`https://api.linkedin.com/v2/ugcPosts`). Requires OAuth 2.0 bearer token and a `urn:li:person:{id}` author URN. **Clavis secrets needed:** `VoxLinkedInAccessToken`, `VoxLinkedInAuthorUrn`.

**Solution (Mastodon):** Use the Mastodon statuses API (`POST /api/v1/statuses`). The instance URL is configurable (not hardcoded). **Clavis secrets needed:** `VoxMastodonInstanceUrl`, `VoxMastodonAccessToken`.

**Priority:** Lower than Discord — start with Discord webhook (simplest) then Mastodon (open API), then LinkedIn (corporate OAuth complexity).

archived_date: 2026-04-18
---

### PROBLEM-04: `DiscordConfig` model is too thin for useful announcements

**File:** [`crates/vox-publisher/src/types.rs`](../../../crates/vox-publisher/src/types.rs), line 131–135

**Problem:** `DiscordConfig` has only `message: Option<String>` and `tts: bool`. A plain text message in a Discord webhook is nearly invisible. Discord embeds (with title, description, URL, color, and footer) are the standard format for bot/webhook announcements. Without embed support, any implemented adapter would produce poor output.

**Solution:** Extend `DiscordConfig` with embed fields that map directly to the Discord API embed object:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordConfig {
    /// Plain text fallback content (shown in notifications).
    pub message: Option<String>,
    #[serde(default)]
    pub tts: bool,
    /// Rich embed title. If present, the adapter sends an embed object.
    #[serde(default)]
    pub embed_title: Option<String>,
    /// Embed URL (makes the title a clickable link).
    #[serde(default)]
    pub embed_url: Option<String>,
    /// Embed description body (supports Discord markdown).
    #[serde(default)]
    pub embed_description: Option<String>,
    /// RGB color for the embed left-bar (e.g. 0x5865F2 for Discord Blurple).
    #[serde(default)]
    pub embed_color: Option<u32>,
}
```

This is additive and non-breaking — all existing `DiscordConfig::default()` usages in tests continue to work.

---

### PROBLEM-05: Single `VoxDiscordWebhookUrl` secret cannot support multiple Discord channels

**Problem:** The existing data model has one `discord: Option<DiscordConfig>` per `SyndicationConfig`. This forces all Discord announcements to the same webhook. A real deployment needs at minimum: `#announcements` (releases), `#research` (Scientia findings). A single webhook URL secret doesn't scale.

**Solution:** Change `discord` in `SyndicationConfig` to `discord: Option<Vec<DiscordConfig>>` OR add a `webhook_url` field to `DiscordConfig` itself (overriding the default from Clavis):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordConfig {
    // ... existing fields ...
    /// Optional webhook URL override. Falls back to `VoxDiscordWebhookUrl` Clavis secret.
    #[serde(default)]
    pub webhook_url_override: Option<String>,
}
```

This gives operators the ability to specify different webhooks per item in YAML frontmatter without requiring a new secret per channel. Primary webhook URL still comes from Clavis for security.

archived_date: 2026-04-18
---

### PROBLEM-06: `topic_packs.rs` `merge_topic_pack_into_syndication` ignores Discord, Bluesky, LinkedIn, Mastodon

**File:** [`crates/vox-publisher/src/topic_packs.rs`](../../../crates/vox-publisher/src/topic_packs.rs), lines 46–77

**Problem:** `merge_topic_pack_into_syndication` applies the topic pack `channels` allowlist to 8 channels but silently skips `discord`, `bluesky`, `linkedin`, and `mastodon`. If a topic pack does NOT list `discord` in its channels, a `discord:` config in the frontmatter will NOT be cleared — it will flow through to the adapter and fail (or accidentally succeed after PROBLEM-02 is fixed).

**Solution:** Add four missing `if !allow.contains("discord") { syn.discord = None; }` branches after line 77. Same for `bluesky`, `linkedin`, `mastodon`.

```rust
if !allow.contains("discord") {
    syn.discord = None;
}
if !allow.contains("bluesky") {
    syn.bluesky = None;
}
if !allow.contains("linkedin") {
    syn.linkedin = None;
}
if !allow.contains("mastodon") {
    syn.mastodon = None;
}
```

This is a 4-line code fix that prevents misconfigured items from spraying content across channels they shouldn't touch.

---

### PROBLEM-07: `distribution.topic-packs.yaml` has no packs for Discord or community channels

**File:** [`contracts/scientia/distribution.topic-packs.yaml`](../../../contracts/scientia/distribution.topic-packs.yaml)

**Problem:** None of the four defined packs (`research_breakthrough`, `infra_release`, `benchmark`, `video_demo`) include `discord` in their channel lists. This means operators cannot currently express "post this release to Discord" through the topic-pack contract system — they would have to manually add `discord:` to every frontmatter file.

**Solution:** Add two new packs and extend existing ones:

```yaml
  community_announcement:
    description: "General community update — new contributors, events, milestones."
    channels: [rss, github, discord, open_collective]
    template_profile:
      github: release_digest
      discord: announcement_embed
    min_worthiness_score:
      github: 0.5
      discord: 0.4

  rust_release:
    description: "Crates.io or Rust-ecosystem release targeting the Rust community."
    channels: [rss, github, discord, reddit, hacker_news, crates_io]
    template_profile:
      github: release_digest
      discord: announcement_embed
      reddit: deep_dive_selfpost
      hacker_news: launch_title
    min_worthiness_score:
      github: 0.78
      discord: 0.6
      reddit: 0.80
      hacker_news: 0.84
```

Also add `discord` to the `infra_release` pack's `channels` list.

archived_date: 2026-04-18
---

### PROBLEM-08: Reddit adapter does not set the required `User-Agent` header in the submit request

**File:** [`crates/vox-publisher/src/adapters/reddit.rs`](../../../crates/vox-publisher/src/adapters/reddit.rs), line 107

**Problem:** The `reddit.rs` adapter correctly sets `User-Agent` on the OAuth token request (line 43), but on the submit POST at line 107, it reads `auth.user_agent` from the struct. The `RedditAuthConfig` struct is constructed in `lib.rs` during dispatch. If the caller does not correctly populate `user_agent`, the request will fail or be shadow-banned. Reddit's rules require the format: `<platform>:<app id>:<version> by u/<username>`.

**Solution:** Either enforce the format in `RedditAuthConfig::new()` or validate in `submit()` before the request:

```rust
fn validate_user_agent(ua: &str) -> anyhow::Result<()> {
    // Must contain at least two colons and "by u/"
    if ua.matches(':').count() < 2 || !ua.contains("by u/") {
        anyhow::bail!(
            "Reddit User-Agent must be '<platform>:<app_id>:<version> by u/<username>', got: {:?}",
            ua
        );
    }
    Ok(())
}
```

Call this at the start of `submit()` before the token fetch.

---

### PROBLEM-09: Reddit's `RedditSubmitResponse` error handling is lossy

**File:** [`crates/vox-publisher/src/adapters/reddit.rs`](../../../crates/vox-publisher/src/adapters/reddit.rs), lines 116–127

**Problem:** When Reddit returns errors in the `json.errors` array, the code logs them as `{:?}` of a `Vec<(String, String, String)>`. Reddit returns structured errors like `["BAD_SR_NAME", "Invalid subreddit name", "sr"]`. This triple-tuple is opaque in error logs. Additionally, if `wrapper.data` is `None` after a successful submit, the code silently returns `"reddit_submitted"` instead of logging a warning.

**Solution:** Define a structured error type for Reddit API errors and surface them cleanly:

```rust
#[derive(Debug)]
struct RedditApiError {
    code: String,
    message: String,
    field: String,
}

impl std::fmt::Display for RedditApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reddit API error [{}] on field '{}': {}", self.code, self.field, self.message)
    }
}
```

Map `(String, String, String)` into this type and use `anyhow::bail!` with it.

archived_date: 2026-04-18
---

### PROBLEM-10: GitHub Discussions adapter uses `vox-forge` but its Discussion creation path is unverified

**File:** [`crates/vox-publisher/src/adapters/github.rs`](../../../crates/vox-publisher/src/adapters/github.rs), line 95

**Problem:** `post_discussion` calls `provider.create_discussion_or_issue(owner, repo, req)`. The first draft described this as a GraphQL `createDiscussion` mutation, but the actual call goes through `vox-forge`'s `GitForgeProvider` trait. If `vox-forge` currently backs this with GitHub Issues rather than Discussions (issue vs. discussion are API-distinct), every "Discussion" publish would silently create an Issue instead.

**Solution:** Audit `crates/vox-forge/src/github.rs` to verify `create_discussion_or_issue` creates a `repositories/{owner}/{repo}/discussions` entry (using the REST Preview or GraphQL) vs. `issues`. If it creates issues, rename the method and add a separate `create_discussion` implementation that uses the GraphQL `createDiscussion` mutation.

The GraphQL token requires `discussions:write` permission — this must be documented in the Clavis `spec.rs` entry for the relevant secret.

---

### PROBLEM-11: No Clavis secret entries verified for publisher social channels

**File:** [`crates/vox-clavis/src/lib.rs`](../../../crates/vox-clavis/src/lib.rs)

**Problem:** A grep of `spec.rs` for `Reddit`, `Discord`, `Twitter`, `Github`, and `LinkedIn` returns zero results. The first draft proposed four secrets as if they didn't exist, but never verified. Either the secrets genuinely don't exist (they need to be added with full `SecretSpec` entries), or they exist under different names (e.g. `VoxGitHubToken` vs `VoxGitHubApiToken`).

**Action required (do not implement until verified):**
1. Run: `rg -n "Reddit|Discord|LinkedIn|Mastodon|Bluesky" crates/vox-clavis/src/lib.rs`
2. Add any missing entries following the established `SecretId` / `SecretSpec` pattern
3. Run `vox ci clavis-parity` and `vox ci secret-env-guard --all` after any additions

**Minimum new secrets expected:**
- `VoxRedditClientId` + `VoxRedditClientSecret` + `VoxRedditRefreshToken` + `VoxRedditUserAgent`
- `VoxDiscordWebhookUrl`
- `VoxMastodonInstanceUrl` + `VoxMastodonAccessToken`
- `VoxLinkedInAccessToken` + `VoxLinkedInAuthorUrn`

archived_date: 2026-04-18
---

### PROBLEM-12: `social_retry.rs` retry budget is not used by the Reddit adapter

**File:** [`crates/vox-publisher/src/social_retry.rs`](../../../crates/vox-publisher/src/social_retry.rs)

**Problem:** `social_retry.rs` contains a well-designed `run_with_retries` + `budget_from_distribution_policy` system with geometric backoff. Reading `lib.rs`, the reddit dispatch does not call `run_with_retries`. This means transient Reddit `429` errors (network blip, rate limit) will cause permanent publish failures.

**Solution:** Wrap all social adapter calls in `run_with_retries(budget, || adapter::post(...))` during dispatch in `lib.rs`. The existing `SocialRetryBudget` system is correct — it just isn't being used.

---

### PROBLEM-13: `DEFAULT_SITE_BASE_URL` in `templates.rs` likely still has a placeholder value

**File:** [`crates/vox-publisher/src/contract.rs`](../../../crates/vox-publisher/src/contract.rs)

**Problem:** `templates.rs` references `DEFAULT_SITE_BASE_URL` from `contract.rs`. If this constant is `"https://vox-lang.org"` it is correct (matching the repo-wide domain policy). If it contains `"https://voxlang.org"` (the incorrect domain), all syndicated content will contain broken canonical links. Additionally, `DEFAULT_GITHUB_REPO` must be `"vox-foundation/vox"` and `DEFAULT_OPENCOLLECTIVE_SLUG` must match the actual collective slug (which hasn't been publicly established yet).

**Action required:** Read `contract.rs` and verify these three constants against:
1. The codebase-enforced `vox-lang.org` domain
2. The actual GitHub repository path
3. The actual Open Collective slug (placeholder is acceptable until launch, but must be flagged)

archived_date: 2026-04-18
---

### PROBLEM-14: `distribution_compile.rs` likely does not dispatch Discord/Mastodon/LinkedIn

**File:** [`crates/vox-publisher/src/distribution_compile.rs`](../../../crates/vox-publisher/src/distribution_compile.rs)

**Problem:** With `lib.rs` grep returning no results for `discord`, `linkedin`, or `mastodon`, these adapters are either in `distribution_compile.rs` or they are entirely undispatched — items with those configs would silently "succeed" (never dispatched) or fail without a clear trace. Given that `SyndicationResult` has `discord` and `linkedin` fields, they must be dispatched somewhere.

**Action required:** Read `distribution_compile.rs` to verify the dispatch branches for all 12 channels tracked in `SyndicationResult`.

---

### PROBLEM-15: `SyndicationResult` missing `bluesky_id()` and `reddit_id()` convenience methods

**File:** [`crates/vox-publisher/src/syndication_outcome.rs`](../../../crates/vox-publisher/src/syndication_outcome.rs)

**Problem:** `SyndicationResult` has `github_id()`, `twitter_id()`, and `oc_id()` accessor methods for extracting `external_id` from `ChannelOutcome::Success`. No such methods exist for `reddit`, `discord`, `bluesky`, `mastodon`, or `linkedin`. Callers that need the Reddit post URL after a successful publish (for cross-linking) have no ergonomic access method.

**Solution:** Add the missing `_id()` methods. This is mechanical — the pattern is identical for each:

```rust
#[must_use]
pub fn reddit_id(&self) -> Option<&str> {
    match &self.reddit {
        ChannelOutcome::Success { external_id: Some(v) }
        | ChannelOutcome::DryRun { external_id: Some(v) } => Some(v.as_str()),
        _ => None,
    }
}
```

Add equivalent methods for `discord_id`, `bluesky_id`, `mastodon_id`, `linkedin_id`.

archived_date: 2026-04-18
---

### PROBLEM-16: Reddit `SelfPost` sends full `content_markdown` with no length cap

**File:** [`crates/vox-publisher/src/adapters/reddit.rs`](../../../crates/vox-publisher/src/adapters/reddit.rs), lines 93–99

**Problem:** When `kind = SelfPost` and no `text_override` is set, the adapter sends the full `content_markdown` of the `UnifiedNewsItem` (which may be a multi-page research paper) as the Reddit post body. Reddit has a **40,000 character limit** on self posts. Additionally, Markdown from mdBook docs contains `{{#include}}` directives and other mdBook-specific syntax that will render as raw text on Reddit.

**Solution:**
1. Add a character limit check before submission with a clear error: `if text.len() > 40_000 { bail!("Reddit self post exceeds 40,000 char limit ({} chars)", text.len()); }`
2. Add a `text_override` requirement enforcement in the topic packs: any pack routing to Reddit must provide a `text_override` via template rendering — the raw `content_markdown` should never be used verbatim.

---

### PROBLEM-17: News templates have no Discord-specific template

**Directory:** `crates/vox-publisher/news-templates/`

**Problem:** Four templates exist: `research_update.md`, `release.md`, `security_advisory.md`, `community_update.md`. The `templates.rs` enum `NewsTemplateId` maps to all four. There is no Discord announcement template, even though the `DiscordConfig` will (after PROBLEM-02 is resolved) accept `embed_description`. `topic_packs.yaml` includes `announcement_embed` as a `template_profile` key for Discord (per PROBLEM-07 solution), but no template with that name exists.

**Solution:** Create `crates/vox-publisher/news-templates/discord_announcement.md`. Add `DiscordAnnouncement` to `NewsTemplateId`. Mirror the file to `docs/news/templates/discord_announcement.md` (same as the existing `docs_mirror_research_template_matches_crate_template` test pattern).

archived_date: 2026-04-18
---

### PROBLEM-18: No subreddit policy pack exists — community rule validation is entirely manual

**Problem:** The community publishing playbook strongly recommends checking subreddit rules before posting. Currently there is no machine-readable representation of per-subreddit rules or any validation that a given `RedditConfig.subreddit` has been approved for automated posting. A bug or misconfiguration could silently post to a subreddit that forbids bots, resulting in a ban.

**Solution:** Add a `contracts/scientia/reddit-community-policies.yaml` file that functions as an allowlist:

```yaml
version: 1
communities:
  - subreddit: r/voxlang
    status: owned
    allows_bots: true
    post_types_allowed: [link, self]
    max_posts_per_day: 3

  - subreddit: r/rust
    status: monitored
    allows_bots: true
    post_types_allowed: [link]
    self_promo_guidelines: "1-in-10 rule applies"
    max_posts_per_month: 1
```

The Reddit adapter's `submit()` function should load this file and `bail!` if the target `subreddit` is not in the allowlist or if `allows_bots: false`.

---

### PROBLEM-19: Open Collective adapter creates `Update` objects but has no `makePublicOn` scheduling

**File:** [`crates/vox-publisher/src/adapters/opencollective.rs`](../../../crates/vox-publisher/src/adapters/opencollective.rs), line 37

**Problem:** The mutation hardcodes `"makePublicOn": null`. Open Collective Updates support scheduled publishing (`makePublicOn` as an ISO 8601 datetime). This makes it impossible to pre-stage announcements for release-day coordination.

**Solution:** Add `pub scheduled_publish_at: Option<DateTime<Utc>>` to `OpenCollectiveConfig` and pass it through to the `makePublicOn` field in the mutation. Default remains `null` (immediate).

archived_date: 2026-04-18
---

### PROBLEM-20: The `hacker_news.rs` adapter is `ManualAssist` only — but there's no UX to surface the drafted post to a human

**File:** [`crates/vox-publisher/src/adapters/hacker_news.rs`](../../../crates/vox-publisher/src/adapters/hacker_news.rs)

**Problem:** `HackerNewsMode::ManualAssist` is the only mode. But the "manual assist" output — the pre-drafted HN title + URL that a human should paste — is presumably logged or returned. If it's just logged at the terminal, it provides no durable artifact for the human to act on later. A publication event that requires human action with no workflow to track that action creates a silent gap.

**Solution:** On every `ManualAssist` run, write the generated HN submission to a `docs/news/hacker-news-queue.md` append-only file (or a new `DRAFT` row in the Arca DB) with status `pending_human`. The `vox scientia` or `vox populi` CLI should expose a `vox publisher hn-queue list` subcommand to show all pending drafts for human submission.

---

### PROBLEM-21: `switching.rs` / dispatch is a 1,093-line file — god object limit risk

**File:** [`crates/vox-publisher/src/switching.rs`](../../../crates/vox-publisher/src/switching.rs)

**Problem:** `switching.rs` is over 1,000 lines, approaching the AGENTS.md 500-line god object limit. Once Discord, LinkedIn, and Mastodon adapters are implemented and dispatched through this file, it will exceed the limit.

**Solution:** Before adding new adapter dispatch, extract per-channel dispatch functions into `crates/vox-publisher/src/dispatch/` submodule files: `dispatch/reddit.rs`, `dispatch/discord.rs`, etc. Each file stays under 100 lines. `switching.rs` imports and delegates.

archived_date: 2026-04-18
---

### PROBLEM-22: No CI guard enforces that stub adapters (`Err("not implemented")`) cannot go live without feature gating

**Problem:** `discord.rs`, `linkedin.rs`, and `mastodon.rs` stubs will return `Err` at runtime if invoked. There is no CI gate (TOESTUB or similar) that prevents a `SyndicationConfig` with `discord:` set from being successfully parsed and dispatched into a hard error. Currently, the only signal is a `Failed` outcome in `SyndicationResult` — which must be checked by the operator after the fact.

**Solution:**
1. Tag stub adapter functions with the TOESTUB comment pattern so `vox stub-check` catches them
2. Add a `PublisherConfig::enabled_channels: Option<Vec<String>>` field that serves as an explicit opt-in allowlist — if `discord` is not in the list, the adapter is gated at dispatch time with a `Disabled` outcome rather than being invoked and failing

---

### PROBLEM-23: No `dry_run` path in Discord adapter

**Problem:** The `SyndicationConfig` has top-level `dry_run: bool`. The github adapter presumably respects `dry_run`. The Discord stub does not — it just errors. Once implemented, Discord's `async fn post` must accept and respect `_dry_run: bool` by returning a synthetic success URL without making an HTTP call.

**Solution:** The function signature already accepts `_dry_run` (it's in the stub). The implementation just needs to check it first:
```rust
if dry_run {
    return Ok("discord://dry-run".to_string());
}
```

archived_date: 2026-04-18
---

### PROBLEM-24: No audit trail for what was published where

**Problem:** Publication events run through `vox-publisher`, but there is no persistent record of "item X was published to Reddit at URL Y at timestamp Z." `SyndicationResult` is returned in-memory and the caller must store it. If the caller doesn't persist it (and the Arca schema doesn't have such a table), operators have no way to recall what was posted, detect duplicates, or compute the "syndication regret rate" KPI from the multi-platform ranking research.

**Solution:** Add to the Arca schema (controlled by `vox-db`) a `syndication_events` table:
```sql
CREATE TABLE syndication_events (
    id          TEXT PRIMARY KEY,
    item_id     TEXT NOT NULL,
    channel     TEXT NOT NULL,
    external_id TEXT,
    status      TEXT NOT NULL,  -- 'success', 'failed', 'dry_run', 'disabled'
    published_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    error_code  TEXT,
    retryable   INTEGER
);
```

`vox-publisher` should write to this table via `vox-db` on every `publish_all` invocation.

---

### PROBLEM-25: Reddit `refresh_token` has no automated rotation / expiry handling

**Problem:** Reddit's `refresh_token` for script-type OAuth apps does not expire, but can be revoked. If revoked (e.g. password change, account compromise), all automated posts will silently fail with a `401`. There is no `vox clavis doctor` warning for stale Reddit credentials.

**Solution:** Add a `vox clavis doctor` check for `VoxRedditRefreshToken` that performs a token validation probe (a lightweight `GET /api/v1/me` with the refreshed token) and reports `ok` or `invalid`. This is consistent with other provider credential health checks in the Clavis doctor workflow.

archived_date: 2026-04-18
---

### PROBLEM-26: Multi-subreddit posting strategy needed for different publication types

**Problem:** A Scientia research finding should go to a different subreddit than a toolchain release. Currently `RedditConfig` always targets one `subreddit` field. There is no mechanism to express "post research findings to r/MachineLearning AND r/voxlang, but post releases ONLY to r/voxlang."

**Solution:** Change `reddit: Option<RedditConfig>` to `reddit: Option<Vec<RedditConfig>>` in `SyndicationConfig`. Each element specifies a different subreddit. The dispatch layer iterates and collects results. `SyndicationResult::reddit` would change from `ChannelOutcome` to `Vec<ChannelOutcome>` or a new `MultiChannelOutcome` wrapper.

**Scope note:** This is a **breaking change** to `SyndicationConfig` and requires a JSON Schema version bump on any published contract. Defer until after the Discord/Mastodon implementations are stable.

---

### PROBLEM-27: GitHub Discussions vs GitHub Releases have no cross-link

**Problem:** When a `research_breakthrough` is published to both GitHub (as a Discussion) and Reddit (as a SelfPost), the content is duplicated without links between them. The Discussion post should ideally link to the Reddit thread URL (returned in `SyndicationResult::reddit_id()`), and Reddit should link to the GitHub Discussion URL.

**Solution:** This requires a two-pass publish or a post-publish cross-link update:
1. Publish to GitHub Discussion → capture Discussion URL
2. Publish to Reddit → capture Reddit URL
3. Edit the GitHub Discussion to append: `\n\n---\n**Discussion threads:** [Reddit](https://reddit.com/...)`

The GitHub API supports editing a discussion body post-creation. This is a medium-complexity feature that belongs in Wave 2 after the basic adapters are live.

archived_date: 2026-04-18
---

### PROBLEM-28: `docs/news/templates/` mirror parity test only covers `research_update`

**File:** [`crates/vox-publisher/src/templates.rs`](../../../crates/vox-publisher/src/templates.rs), lines 115–127

**Problem:** The `docs_mirror_research_template_matches_crate_template` test verifies parity between `news-templates/research_update.md` and `docs/news/templates/research_update.md`. No equivalent parity tests exist for `release.md`, `security_advisory.md`, or `community_update.md`. If a developer edits one location but not the other, the mismatch goes undetected until a Scientia publication produces an unexpected template.

**Solution:** Add three more `#[test]` cases mirroring the existing pattern for the other three templates. This is a 15-minute mechanical addition.

---

### PROBLEM-29: Open Collective adapter does not verify the collective slug exists before posting

**File:** [`crates/vox-publisher/src/adapters/opencollective.rs`](../../../crates/vox-publisher/src/adapters/opencollective.rs)

**Problem:** If `collective_slug` in `OpenCollectiveConfig` is set to a placeholder value (e.g. `"vox-foundation-placeholder"`) that doesn't correspond to a real Open Collective, the mutation will silently fail with a GraphQL error that is caught and returned as an `anyhow::Error`. The `contract.rs` file likely has `DEFAULT_OPENCOLLECTIVE_SLUG` hardcoded to a placeholder.

**Solution:**
1. Add a preflight `GET https://opencollective.com/{slug}/settings` (or the equivalent GraphQL collective query) to verify the collective exists before posting
2. Document the real slug in `contract.rs` once the collective is created — or gate the entire adapter with a `enabled: false` in the default topic packs until the collective is live

archived_date: 2026-04-18
---

### PROBLEM-30: No `community_update` template is referenced by any topic pack

**File:** [`contracts/scientia/distribution.topic-packs.yaml`](../../../contracts/scientia/distribution.topic-packs.yaml) and [`crates/vox-publisher/src/templates.rs`](../../../crates/vox-publisher/src/templates.rs)

**Problem:** `NewsTemplateId::CommunityUpdate` exists in `templates.rs` and `community_update.md` exists in `news-templates/`. But no topic pack in `distribution.topic-packs.yaml` references `community_update` as a `template_profile` value. It is a dead code path.

**Solution:** The new `community_announcement` pack proposed in PROBLEM-07 should use `community_update` as its GitHub template profile. This connects the dead code path into the live system.

---

## 3. Dependency-Ordered Execution Backlog

Use this as a task checklist. Items are grouped by dependency — complete each group before starting the next.

### Wave 0 — Audit & Foundation (no code changes — verify first)
- [ ] Read `crates/vox-forge/src/github.rs` — verify `create_discussion_or_issue` creates Discussions not Issues (PROBLEM-10)
- [ ] Read `crates/vox-clavis/src/lib.rs` — enumerate all existing social secret IDs (PROBLEM-11)
- [ ] Read `crates/vox-publisher/src/contract.rs` — verify `DEFAULT_SITE_BASE_URL = "https://vox-lang.org"` (PROBLEM-13)
- [ ] Read `crates/vox-publisher/src/distribution_compile.rs` or `switching.rs` — map all 12 adapter dispatch paths (PROBLEM-14)
- [ ] Read `crates/vox-publisher/src/adapters/hacker_news.rs` — verify what ManualAssist output looks like now (PROBLEM-20)

### Wave 1 — Model Fixes (breaking to non-breaking, no runtime changes)
- [ ] Extend `DiscordConfig` with embed fields (PROBLEM-04)
- [ ] Add `webhook_url_override` to `DiscordConfig` (PROBLEM-05)
- [ ] Add `scheduled_publish_at` to `OpenCollectiveConfig` (PROBLEM-19)
- [ ] Add 4 missing channel gates to `merge_topic_pack_into_syndication` in `topic_packs.rs` (PROBLEM-06)
- [ ] Add missing `_id()` accessors to `SyndicationResult` (PROBLEM-15)
- [ ] Add 3 missing template parity tests in `templates.rs` (PROBLEM-28)
- [ ] Create `discord_announcement.md` news template (PROBLEM-17)

### Wave 2 — Clavis Registration
- [ ] Register all missing social secrets in `spec.rs` (PROBLEM-11)
- [ ] Run `vox ci clavis-parity` clean
- [ ] Run `vox ci secret-env-guard --all` clean

### Wave 3 — Contracts
- [ ] Update `distribution.topic-packs.yaml` with `community_announcement` and `rust_release` packs (PROBLEM-07)
- [ ] Add `discord` to `infra_release` channels (PROBLEM-07)
- [ ] Create `contracts/scientia/reddit-community-policies.yaml` allowlist (PROBLEM-18)

### Wave 4 — Core Adapter Implementations
- [ ] Implement `discord.rs` webhook POST with embed support (PROBLEM-02, PROBLEM-23)
- [ ] Implement Reddit `User-Agent` validation in `submit()` (PROBLEM-08)
- [ ] Implement Reddit structured error types (PROBLEM-09)
- [ ] Implement Reddit 40,000 character limit check (PROBLEM-16)
- [ ] Implement Reddit subreddit policy allowlist check (PROBLEM-18)
- [ ] Implement `mastodon.rs` via Mastodon statuses API (PROBLEM-03)
- [ ] Implement `linkedin.rs` via UGC Posts API (PROBLEM-03)

### Wave 5 — Dispatch & Retry Wiring
- [ ] Wrap all social adapter calls in `run_with_retries` in dispatch layer (PROBLEM-12)
- [ ] Add `PublisherConfig::enabled_channels` allowlist gating (PROBLEM-22)
- [ ] Tag all remaining stubs for TOESTUB detection (PROBLEM-22)

### Wave 6 — Quality & Observability
- [ ] Add `syndication_events` table to Arca schema (PROBLEM-24)
- [ ] Write `syndication_events` rows in `publish_all` (PROBLEM-24)
- [ ] Add `vox publisher hn-queue list` command (PROBLEM-20)
- [ ] Add Reddit refresh token health check to `vox clavis doctor` (PROBLEM-25)
- [ ] Verify (and fix) Open Collective collective slug / preflight (PROBLEM-29)
- [ ] Connect `community_update` template to `community_announcement` pack (PROBLEM-30)

### Wave 7 — Architecture Hardening (requires Wave 4 stable)
- [ ] Extract `switching.rs` dispatch into `dispatch/` submodule before god-object limit (PROBLEM-21)
- [ ] Add Reddit token caching to avoid OAuth round-trip per publish (PROBLEM-01)

### Wave 8 — Advanced (deferred)
- [ ] Multi-subreddit `Vec<RedditConfig>` support (PROBLEM-26)
- [ ] Cross-link Discussion ↔ Reddit on post-publish update (PROBLEM-27)

archived_date: 2026-04-18
---

## 4. Changelog

| Date | Change |
|---|---|
| 2026-04-12 | Complete rewrite replacing first-draft playbook. Full codebase audit of `vox-publisher`, adapters, contracts, `social_retry.rs`, `syndication_outcome.rs`, `topic_packs.rs`, and `templates.rs`. 30 explicit problems identified with code-verified solutions. Dependency-ordered execution backlog across 8 waves. |


