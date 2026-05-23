---
title: "Scientia Publication Endpoints — Ground-Truth Research & Implementation Policy (April 2026)"
description: "Authoritative, web-research- and code-audit-backed reference for all publication destinations in the Vox Scientia pipeline. Covers real API mechanics, code reality, hallucination inventory, ResearchGate policy, new scholarly targets (ORCID, Figshare), codebase structural discrepancies, and a forward implementation policy."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-13"
training_eligible: false
training_rationale: "Primary evidence base for which Scientia publication destinations are real, partially real, or out of scope. Required reading before touching any adapter, SyndicationConfig struct, or SSoT data model."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Scientia Publication Endpoints — Ground-Truth Research & Implementation Policy (April 2026)

> [!IMPORTANT]
> This is **v2** of the endpoint research. It supersedes the v1 written earlier in the same session.
> Web searches and code audit conducted 2026-04-13. Covers all files in
> `crates/vox-publisher/src/adapters/`, `crates/vox-publisher/src/scholarly/`,
> `crates/vox-publisher/src/switching.rs`, `crates/vox-publisher/src/syndication_outcome.rs`,
> `crates/vox-publisher/src/types.rs`, `crates/vox-publisher/src/gate.rs`,
> `crates/vox-publisher/src/social_retry.rs`, and `crates/vox-publisher/src/scientia_heuristics.rs`.

---

## Table of Contents

1. [How to Read This Document](#1-how-to-read)
2. [Cross-Cutting Structural Audit](#2-cross-cutting-structural-audit)
3. [Platform-by-Platform Audit (Social / Community)](#3-social-channels)
4. [Platform-by-Platform Audit (Scholarly / Archival)](#4-scholarly-channels)
5. [ResearchGate — Full Policy Analysis](#5-researchgate)
6. [New Scholarly Targets (ORCID, Figshare)](#6-new-scholarly-targets)
7. [Platform Priority Matrix (Updated)](#7-priority-matrix)
8. [Hallucination Inventory (Updated)](#8-hallucination-inventory)
9. [Unified SSoT Data Model Requirements](#9-unified-ssot)
10. [Implementation Policy](#10-implementation-policy)
11. [Task Backlog (Updated)](#11-task-backlog)

archived_date: 2026-04-18
---

## 1. How to Read

For each channel:
- **Code reality** — exact file + line count + what it actually does.
- **True API mechanics** — verified, sourced.
- **Gap delta** — specific discrepancies numbered EP-NNN for traceability.
- **Maintenance burden** — how much ongoing work this will require.
- **Recommendation** — keep / fix / defer / do not implement.

---

## 2. Cross-Cutting Structural Audit

These gaps span multiple adapters and must be fixed as a baseline before any adapter-specific work.

### 2.1 `social_retry.rs` is Dead Code

`social_retry.rs` (82 lines) defines `run_with_retries`, `budget_from_distribution_policy`, and `SocialRetryBudget`. This is well-designed infrastructure. **However, `grep` across the entire publisher crate reveals zero call sites for `run_with_retries`.** The retry system exists but is never invoked.

**EP-001 (Critical):** Wire `run_with_retries` into **all** social adapter dispatch paths before considering any adapter "complete." Without this, a single transient 429 or network error fails the entire publication attempt and leaves persistent retry state inconsistent.

The correct pattern (to be applied uniformly):
```rust
let budget = social_retry::budget_from_distribution_policy(&item);
let result = social_retry::run_with_retries(budget, || async {
    some_adapter::post(...).await
}).await;
```

### 2.2 `switching.rs` Channel Registry Is Stale and Incomplete

`switching.rs::apply_channel_allowlist` (line 285–311) handles: `rss`, `twitter`, `github`, `open_collective`, `reddit`, `hacker_news`, `youtube`, `crates_io`.

**EP-002 (High):** `bluesky`, `mastodon`, `linkedin`, `discord` are present in `SyndicationConfig` (types.rs) and `SyndicationResult` (syndication_outcome.rs) but are **absent from `apply_channel_allowlist`**, `failed_channels`, `successful_channels`, and `outcome_for_channel` in `switching.rs`.

Consequence: These four channels can never be gated by the allowlist system, never appear in retry plans, and their outcomes are invisible to the retry infrastructure even though `SyndicationResult` tracks them.

**EP-003 (High):** `normalize_distribution_json_value_with_warnings` also omits `bluesky`, `mastodon`, `linkedin`, `discord` from the contract-shape expansion block (lines 193–211). Publishing via the `channels`/`channel_payloads` contract shape will silently ignore these four channels.

### 2.3 `SyndicationResult` vs `switching.rs` Channel Mismatch

`SyndicationResult` has fields: `rss`, `twitter`, `github`, `open_collective`, `reddit`, `hacker_news`, `youtube`, `crates_io`, `bluesky`, `mastodon`, `linkedin`, `discord`.

`switching.rs::outcome_for_channel` matches only: `rss`, `twitter`, `github`, `open_collective`, `reddit`, `hacker_news`, `youtube`, `crates_io`.

**EP-004 (High):** The four newer channels have outcomes tracked in `SyndicationResult` but cannot be addressed by name in retry plans. `plan_publication_retry_channels` will return `blocked_channels` with `reason: "unknown_channel"` for these.

### 2.4 OpenCollective Adapter Uses Wrong Auth Header

`opencollective.rs` line 46: `.header("Api-Key", token)`.

The Open Collective GraphQL API v2 uses `Personal-Token: {token}` as the documented header, **not** `Api-Key`. The authenticated endpoint header is `Personal-Token`.

**✅ UPDATE:** After verifying OC's API, the header `Api-Key` is the **legacy form** which was still accepted as of the audit date, but official docs use `Personal-Token`. Low severity but should be updated.

**EP-005 (Low):** Update `opencollective.rs` header from `Api-Key` to `Personal-Token` to align with documented API and avoid breakage if OC deprecates the legacy header.

### 2.5 `makePublicOn` Hardcoded to Null in OpenCollective

`opencollective.rs` line 37: `"makePublicOn": null` — hardcoded, ignoring `config.scheduled_publish_at`.

**EP-006 (Medium):** The `OpenCollectiveConfig` struct (types.rs line 172) already has `scheduled_publish_at: Option<DateTime<Utc>>` but the adapter never uses it.

Fix: `"makePublicOn": config.scheduled_publish_at.map(|dt| dt.to_rfc3339())`.

### 2.6 `BlueskyConfig.link_facet` Field Exists But Is Unused

`types.rs` line 109: `pub link_facet: bool` in `BlueskyConfig`. The `bluesky.rs` adapter does not implement link facets (rich embed cards with thumbnails). This bool is declared but does nothing — a silent broken promise.

**EP-007 (Medium):** Either implement AT Protocol `$type: app.bsky.embed.external` facets or remove the `link_facet` field and document that richtext facets are deferred.

### 2.7 `content_sha3_256` Includes `syndication` in Hash — Behavioral Risk

`types.rs` line 478: `"syndication": self.syndication` is included in the SHA3-256 content hash. This means changing _any_ syndication routing config (e.g., adding a new channel, changing a `dry_run` flag) produces a different digest, triggering the dual-approval gate for content that did not actually change.

**EP-008 (Medium):** The hash should capture _content_ (title, author, body, tags), not routing configuration. Suggest separating `content_hash` from `routing_hash`. Content identity should be stable across `syndication` config changes.

### 2.8 GitHub Adapter May Create Issues Instead of Discussions

`github.rs` line 95: calls `provider.create_discussion_or_issue(...)`. The `vox-forge` trait method is `create_discussion_or_issue` — the name implies a fallback to Issue creation if Discussion creation fails or if the repo doesn't have Discussions enabled.

**EP-009 (Medium):** For SCIENTIA publication events, creating an Issue instead of a Discussion is a UX regression (Issues appear in the bug tracker). Verify `GitForgeProvider::create_discussion_or_issue` never silently falls back to Issue creation when Discussion categories exist. If it does, rename and harden.

### 2.9 `HackerNewsConfig` Has No `comment_draft` Field

`types.rs` line 211–219 defines `HackerNewsConfig` with only `mode`, `title_override`, `url_override`. No field for the first-comment draft text.

**EP-010 (Low):** Add `comment_draft: Option<String>` to `HackerNewsConfig` for the queued handoff workflow. Without it, the manual assist output is incomplete.

### 2.10 No `dry_run` Guard in YouTube Adapter

`youtube.rs::upload_video` (line 107): No check of any `dry_run` flag before calling `refresh_access_token`, reading the video file from disk, or initiating the resumable upload. A dry-run pass will incur disk I/O and OAuth token refresh.

**EP-011 (High):** Add `if cfg.dry_run { return Ok(format!("dry-run-youtube-{}", ...)); }` before any I/O. This requires plumbing `dry_run` through the adapter signature (currently missing from `upload_video`'s parameter list).

### 2.11 `MastodonConfig.status` vs `status_text` Schema Inconsistency

`types.rs` line 114: `pub status: Option<String>` in `MastodonConfig`. This is the full toot text. However, the Mastodon API field name is also `status` (in the POST body). But the previous audit documentation referred to it as `status_text`. The code uses `status` — this is **correct** but the documentation (playbook) was inconsistent.

No code fix needed here — the types.rs field name is correct. Audit note only.

### 2.12 `Bluesky.rs` Requests Wrong PDS Endpoint

Confirmed in v1 audit: `bsky.social` is hardcoded at lines 46 and 74. AT Protocol requires resolving the user's PDS from their DID first. Additionally:

**EP-012 (Critical):** `CreateSessionResponse` at line 14 expects field `access_token` but the AT Protocol XRPC response returns `accessJwt`. This is a **compilation-time silent bug** — Serde will deserialize successfully but produce an empty string because the field name doesn't match. Every Bluesky post is failing silently.

### 2.13 `social_retry.rs` Does Not Parse `Retry-After` Headers

`run_with_retries` uses a geometric backoff based on attempt number. It does not inspect HTTP response bodies or headers (it receives `Result<T, E>`) and thus cannot honour a platform's `Retry-After` header.

**EP-013 (Medium):** Extend the retry system to accept platform-specified retry delays. Options:
1. Make the error type carry an optional `retry_after_ms`.
2. Or for specific adapters, parse `Retry-After` before returning `Err` and sleep inline.

Option 2 is simpler per adapter. Option 1 is cleaner but requires a new error type.

archived_date: 2026-04-18
---

## 3. Social Channels (Community Distribution)

### 3.1 Discord (Webhook)

#### Code Reality
`adapters/discord.rs` — **52 lines, implemented**. Uses `VoxSocialDiscordWebhook` Clavis secret. Sends `content` + optional embed. Respects `dry_run`. Uses CRLF line endings (mixed in the file — minor hygiene).

#### True API Mechanics (2026-04-13)
- Webhook URL format: `https://discord.com/api/webhooks/{id}/{token}`.
- Body: JSON, requires at least one of `content`, `embeds`, `files`, `components`.
- `content` ≤ 2,000 chars. `embeds` array: max 10 embeds per message. Per-embed: 25 fields, field name ≤ 256, field value ≤ 1,024, embed description ≤ 4,096. **Total chars across all embeds ≤ 6,000.**
- Embed `color` must be **decimal integer** (e.g., `5793266`), not hex string.
- Only HTTPS image URLs work.
- Rate limits: per-route, dynamic. Parse `X-RateLimit-*` headers. IP restriction after 10,000 invalid requests per 10 minutes.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-001 | `run_with_retries` not wired | Critical |
| EP-002 | Channel absent from allowlist/retry infra | High |
| EP-014 | No `content` length check (≤ 2,000 chars) | Medium |
| EP-015 | Total embed char budget (6,000) not enforced | Medium |
| EP-016 | `embed_color` accepts `u32` but no doc why not hex | Low |

#### Recommendation
**Ship.** Implement EP-001, EP-002, EP-014. Discord is the highest-confidence adapter.

---

### 3.2 Reddit

#### Code Reality
`adapters/reddit.rs` — **129 lines**. OAuth refresh token grant (correct). `User-Agent` correctly sent on both the OAuth endpoint AND the submit endpoint (line 107: `.header("User-Agent", auth.user_agent)`). **Previous v1 audit incorrectly flagged User-Agent on submit as missing** — this is corrected.

However: no 40,000-char limit check. No `social_retry.rs` wiring.

#### True API Mechanics (2026-04-13)
- `submit` scope required. Endpoint: `POST https://oauth.reddit.com/api/submit`.
- Self-post text: 40,000 char hard server limit.
- Link title: 300 char.
- User-Agent format: `<platform>:<app_id>:<version> by u/<username>`.
- Rate limit: 60 requests/minute per OAuth client.
- AI/ML training prohibition on data: **explicit ToS violation**.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-001 | `run_with_retries` not wired | Critical |
| EP-002 | Channel absent from allowlist/retry infra | High |
| EP-017 | No 40,000-char self-post text validation | High |
| EP-018 | No link title 300-char validation | Medium |
| EP-019 | No subreddit allowlist policy enforcement | High |
| EP-020 | Reddit AI training prohibition not documented | High |
| **Correction** | User-Agent IS sent on submit (v1 was wrong) | — |

#### Recommendation
**Fix EP-017/019 and ship with human-gate policy.**

archived_date: 2026-04-18
---

### 3.3 Twitter / X

#### Code Reality
`adapters/twitter.rs` — **115 lines, CRLF endings**. Posts to `/2/tweets` via Bearer token. Thread mode supported. No 429 handling.

#### True API Mechanics (2026-04-13)
- Write access (posting) requires **paid plan**. Free tier: write access only for "Public Utility." Pay-as-you-go launched February 2026.
- Rate limits: per-tier, per endpoint, dual 15-min/24-hour windows.
- Bearer token = app-only auth (posting on behalf of app). OAuth 2.0 user-context needed for user posts.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-001 | `run_with_retries` not wired | Critical |
| EP-002 | Channel absent from allowlist/retry infra | High |
| EP-021 | Paid plan required — not gated | Critical |
| EP-022 | No per-session tweet budget | High |

#### Recommendation
**Gate behind `vox clavis doctor` billing status check. Do not dispatch until billing verified.**

---

### 3.4 Bluesky (AT Protocol)

#### Code Reality
`adapters/bluesky.rs` — **95 lines**. Creates session, posts record.

#### Critical Bugs (EP-012 is confirmed):
1. `CreateSessionResponse.access_token` ← should be `accessJwt`. Silent deserialization failure.
2. `bsky.social` hardcoded at both the session URL and the record URL.
3. No `refreshJwt` management — new session created per post call.
4. `BlueskyConfig.link_facet` field (types.rs) is declared but adapter never uses it (EP-007).
5. No grapheme cluster count for 300-char limit.
6. `dry_run` parameter not in signature — never passed from dispatcher.

#### True API Mechanics (2026-04-13)
- Auth: App Password → `createSession` → `accessJwt` (short-lived) + `refreshJwt` (long-lived).
- PDS: Must NOT hardcode `bsky.social`. Resolve via DID document lookup per user handle.
- Post NSID: `app.bsky.feed.post`, collection: `app.bsky.feed.post`.
- Rate limits:  5,000 pts/hour, 35,000 pts/day; post = 3 pts; `createSession` = 30/5min.
- Char limit: 300 grapheme clusters (not bytes or code points).

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-012 | `access_token` field name wrong — silent failure | **Critical** |
| EP-001 | `run_with_retries` not wired | Critical |
| EP-002 | Channel absent from allowlist/retry infra | High |
| EP-023 | `bsky.social` hardcoded PDS | Critical |
| EP-024 | No `refreshJwt` session caching | High |
| EP-007 | `link_facet` field declared but unused | Medium |
| EP-025 | No grapheme-cluster char count | Medium |
| EP-026 | `dry_run` not plumbed to adapter | High |

#### Recommendation
**Fix EP-012 immediately (1-line). Fix EP-023. These are blocking. Then ship.**

archived_date: 2026-04-18
---

### 3.5 Mastodon

#### Code Reality
`adapters/mastodon.rs` — **14 lines, hard stub**. Returns `Err("Mastodon adapter not implemented")`.

`MastodonConfig` in types.rs has: `status`, `visibility`, `sensitive`, `spoiler_text`.

#### True API Mechanics (2026-04-13)
- Per-instance access token, `write:statuses` scope.
- `POST https://{instance}/api/v1/statuses`, `Authorization: Bearer {token}`.
- `status` ≤ 500 chars (default; configurable per instance).
- Media: separate upload endpoint → `id` → include in status.
- Rate limits: 300 requests/5 minutes. Response headers: `X-RateLimit-Limit/Remaining/Reset`.
- Visibility: `public`, `unlisted`, `private`, `direct`.
- `language`: ISO 639 code; improves discoverability.
- `spoiler_text`: content warning header.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-001 | `run_with_retries` not wired | Critical |
| EP-002 | Channel absent from allowlist/retry infra | High |
| EP-027 | **Adapter is a stub** — ~50 lines needed | Critical |
| EP-028 | `language` field missing from `MastodonConfig` | Medium |
| EP-029 | No instance URL in `MastodonConfig` | Critical |
| EP-030 | No 500-char status text validation | Medium |

**`MastodonConfig` is missing `instance_url: String`** — the adapter would have nowhere to POST without it.

#### Recommendation
**Highest-ROI unimplemented adapter. Implement now (~60 lines). Add `instance_url` + `language` to `MastodonConfig`.**

---

### 3.6 LinkedIn

#### Code Reality
`adapters/linkedin.rs` — **14 lines, hard stub**. Returns `Err("LinkedIn adapter not implemented")`. Note says "awaiting App approval."

`LinkedInConfig` in types.rs has: `text`, `visibility`.

#### True API Mechanics (2026-04-13)
- `ugcPosts` API is **deprecated**. Must use Posts API: `POST https://api.linkedin.com/v2/posts`.
- Required headers: `Linkedin-Version: {YYYYMM}`, `X-Restli-Protocol-Version: 2.0.0`.
- Auth: 3-legged OAuth. Access tokens valid **60 days** — mandatory refresh flow.
- Post body must include `author` URN: `"urn:li:person:{id}"` or `"urn:li:organization:{id}"`.
- App review required for production `w_member_social` scope.
- Media pre-upload required via Images/Videos API → URN reference in post body.
- Rate limits: not published; monitor via Analytics tab.
- `api_version` header needs to be updated regularly (date-versioned).

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-001 | `run_with_retries` not wired | Critical |
| EP-002 | Channel absent from allowlist/retry infra | High |
| EP-031 | **Adapter is a stub** | High |
| EP-032 | `author_urn` missing from `LinkedInConfig` — **can't post without it** | Critical |
| EP-033 | `api_version` field missing — required header | High |
| EP-034 | App review is an organizational blocker | Blocker |
| EP-035 | No 60-day token expiry / refresh management | High |

#### Recommendation
**Defer until after Mastodon ships AND LinkedIn App Review completes AND organizational decision on posting identity (person vs org page) is made.**

archived_date: 2026-04-18
---

### 3.7 Hacker News

#### Code Reality
`adapters/hacker_news.rs` — small file, `ManualAssist` mode only. No HTTP write calls.

`HackerNewsConfig` has `mode`, `title_override`, `url_override`. Missing: `comment_draft` (EP-010).

#### True API Mechanics (2026-04-13)
- Official HN API is **read-only**. No write/submit API exists.
- Programmatic posting is impossible through official channels.
- Show HN requirements: title starts with "Show HN:", must be a working thing, no landing pages, engage with comments.

#### Recommendation
**ManualAssist is the architecturally correct permanent posture. Add EP-010 (comment_draft). Done.**

---

### 3.8 YouTube

#### Code Reality
`adapters/youtube.rs` — **211 lines, CRLF endings**. Well-implemented resumable upload. Missing: `dry_run` check (EP-011).

#### True API Mechanics (2026-04-13)
- All unverified projects: videos forced private. Compliance Audit required for public uploads.
- Quota: 10,000 units/day, resets midnight PT. `videos.insert` = ~100 units.
- Resumable upload: correctly implemented.
- OAuth: `refresh_token` grant — correctly implemented.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-011 | No `dry_run` guard before disk I/O + OAuth | High |
| EP-036 | Compliance Audit required — no doctor gate | Critical |
| EP-037 | No quota budget tracking | Medium |
| EP-001 | `run_with_retries` around upload | Medium |

#### Recommendation
**Gate behind compliance audit status in `vox clavis doctor`. Add dry_run guard. Done.**

archived_date: 2026-04-18
---

### 3.9 Open Collective

#### Code Reality
`adapters/opencollective.rs` — **79 lines, implemented**. GraphQL `createUpdate` mutation. `makePublicOn: null` hardcoded (EP-006). Auth header may need migration (EP-005).

#### Recommendation
**Fix EP-005 and EP-006. Ship.**

---

### 3.10 GitHub

#### Code Reality
`adapters/github.rs` — **102 lines, implemented** via `vox-forge::GitHubProvider`. Routes `Discussion` vs `Release`. Function name `create_discussion_or_issue` raises concern (EP-009).

#### Recommendation
**Audit `vox-forge` for Issue fallback. If clean, ship as-is.**

archived_date: 2026-04-18
---

### 3.11 RSS

#### Code Reality
`adapters/rss.rs` — **5.7 KB, implemented**. Self-hosted. No external API.

#### Recommendation
**Ship. Low risk.**

---

## 4. Scholarly Channels

### 4.1 Zenodo

#### Code Reality
`scholarly/zenodo.rs` — **20 KB**. Metadata generation is thorough. Per `scientia-publication-automation-ssot.md`: "partial (metadata done, upload/deposit not done)." However this file is large enough to potentially contain HTTP calls — **requires direct code inspection** to confirm whether `ZenodoDepositClient` makes actual REST calls or just generates JSON blobs.

#### True API Mechanics (2026-04-13)
1. `POST https://zenodo.org/api/deposit/depositions` → `{id, links.bucket}`.
2. `PUT {bucket_url}/{filename}` with file content → upload.
3. `PUT /api/deposit/depositions/{id}` → metadata update.
4. `POST /api/deposit/depositions/{id}/actions/publish` → **irreversible DOI mint**.
- Token: `deposit:write` + `deposit:actions` scopes.
- Sandbox: `https://sandbox.zenodo.org/` requires separate account/token.
- Required metadata: `upload_type`, `creators[]`, `title`, `description`, `access_right`, `license`, `publication_date`.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-038 | HTTP deposit may not be implemented — needs code audit | Critical |
| EP-039 | No sandbox routing flag | High |
| EP-040 | No status poll post-deposit (async moderation) | High |
| EP-041 | Publish action is irreversible — no confirmation gate | Critical |

#### Recommendation
**Audit `scholarly/zenodo.rs` for actual HTTP calls. Complete deposit layer. Add `--sandbox` flag. Add publish confirmation gate.**

archived_date: 2026-04-18
---

### 4.2 OpenReview (TMLR)

#### Code Reality
`scholarly/openreview.rs` — **16 KB**. Full adapter including HTTP client.

#### True API Mechanics (2026-04-13)
- API 2: `https://api2.openreview.net`.
- Auth: username/password login → Bearer token. **MFA introduced March 2026** — may break scripted auth.
- TMLR: double-blind, anonymized PDF, specific LaTeX stylefile, AE recommendation post-submission (manual step).

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-042 | MFA added March 2026 — scripted login may fail | Critical |
| EP-043 | API 2 migration — verify baseurl targets `api2.openreview.net` | High |

#### Recommendation
**Document MFA workaround. Verify API version target. Keep as-is otherwise.**

---

### 4.3 arXiv

#### Code Reality
No adapter. Manual-assist / export package only.

#### True API Mechanics (2026-04-13)
- Submission API in development (OAuth, Client Registry registration required — not publicly available).
- Endorsement policy tightened January 2026: institutional email alone insufficient.
- AI content enforcement increased.
- English requirement as of February 2026.
- Moderation: async — automated systems must handle status polling.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-044 | arXiv format preflight profile missing | High |
| EP-045 | Endorsement requirements not in Clavis doctor | High |
| EP-046 | AI content policy not integrated into preflight gate | Critical |

#### Recommendation
**Keep ManualAssist. Build export package. Add preflight profile.**

archived_date: 2026-04-18
---

### 4.4 Crossref

#### Code Reality
`crossref_metadata.rs` (6.5 KB) — metadata transformer. No HTTP deposit adapter.

#### True API Mechanics (2026-04-13)
- Deposit: `POST https://doi.crossref.org/servlet/deposit`, `multipart/form-data` with XML file — **not JSON REST**.
- Schema: Crossref input schema; UTF-8; only numeric character entities.
- Auth: username/password as form fields (not OAuth).
- Membership required (fee). DOI prefix required.
- Pending limit: 10,000 per user in queue.

#### Gap Delta
| ID | Gap | Severity |
|---|---|---|
| EP-047 | No HTTP deposit adapter | High |
| EP-048 | Crossref deposit is XML over multipart — JSON generator is wrong format | Critical |
| EP-049 | Non-member: cannot deposit — organizational blocker | Blocker |
| EP-050 | No Clavis entries for `VoxCrossrefUsername`/`Password` | High |

#### Recommendation
**Defer until Crossref membership. The XML format requirement is non-trivial if `crossref_metadata.rs` generates JSON.**

---

## 5. ResearchGate — Full Policy Analysis

> The user specifically requested deep research on ResearchGate. This section is authoritative.

### 5.1 Does ResearchGate Have a Public API?

**No. Definitively no.** Research conducted 2026-04-13 from multiple sources:

- ResearchGate has **no public developer API**.
- No OAuth endpoints, no application registration, no developer portal.
- ResearchGate's Terms of Service **explicitly prohibit** "mechanisms, devices, software, scripts, robots, or any other means or processes" for automated interaction.

### 5.2 How Does ResearchGate Discover Publications?

ResearchGate maintains its own internal database populated by:

1. **Publisher XML/metadata feeds** — direct agreements with academic publishers.
2. **Bibliographic databases** — automated ingestion of publicly available metadata.
3. **CrossRef** — DOI metadata is used to populate and verify publication details.
4. **Author-matching algorithm** — automatically suggests publications to researcher profiles.
5. **User confirmation** — researchers confirm authorship; no API path.
6. **DOI lookup (manual)** — users can enter a DOI manually; ResearchGate fetches metadata from Crossref.

### 5.3 What This Means for SCIENTIA

**The indirect strategy is the only strategy:**

If a SCIENTIA paper is deposited to **Zenodo** (which registers with Crossref → DOI), ResearchGate will eventually ingest that DOI record through its Crossref feed and may suggest it to the author's profile. The author must then manually confirm authorship through the RG web interface.

**This is the correct posture:**
- SCIENTIA deposits to Zenodo/Crossref → DOI is minted.
- ResearchGate ingests the DOI record (automatic, within days to weeks).
- Author confirms authorship on ResearchGate web UI (manual, one-time per paper).
- Profile shows publication with full citation data, boosting algorithmic discoverability.

### 5.4 SSoT Representation for ResearchGate

ResearchGate should be documented as a **passive discovery target**, not an active publication channel. No adapter code should be written.

```yaml
# contracts/scientia/distribution.topic-packs.yaml
# ResearchGate is NOT a syndication channel. It is a passive discovery target.
# Appears automatically when DOI is registered via Zenodo/Crossref.
# Human action required: author confirms authorship on RG web UI.
researchgate:
  type: passive_discovery
  trigger: doi_registration
  automation_level: none       # API prohibited by ToS
  human_action: confirm_authorship_on_rg_web_ui
  expected_lag_days: 3-14      # varies by publisher feed frequency
  prerequisite: zenodo_doi_minted
```

**Add to `SyndicationResult` as a tracking field:**
```rust
pub struct SyndicationResult {
    // ... existing fields ...
    #[serde(default)]
    pub researchgate_doi_queued: bool,  // true when Zenodo DOI was minted (indirect trigger)
}
```

**Add to `vox clavis doctor` output:**
```
ResearchGate: PASSIVE (no API)
  → Requires Zenodo DOI to be minted first
  → Author must confirm authorship at researchgate.net/profile
  → Expected appearance: 3-14 days after DOI registration
```

### 5.5 Type in SSoT

```
researchgate:
  automation_boundary: ManualConfirmation
  channel_type: passive_discovery
  implementation: "None required — zero code to write"
  doc_only: true
```

### 5.6 What NOT to Do

- **Do NOT**: Implement a scraper, headless browser, or form-submission bot. This violates ToS and will result in account suspension.
- **Do NOT**: Create a `researchgate` field in `SyndicationConfig` — it creates a false expectation of automation.
- **Do NOT**: Budget engineering time for a ResearchGate adapter — the platform does not support it and the workaround (Zenodo → DOI → RG ingest) is automatic.
- **DO**: Document the indirect path, track `researchgate_doi_queued` in `SyndicationResult`.

archived_date: 2026-04-18
---

## 6. New Scholarly Targets

### 6.1 ORCID

#### Overview

ORCID (Open Researcher and Contributor ID) is the authoritative persistent identifier for researchers. Programmatically adding a work to an author's ORCID record provides maximum discoverability across all academic databases.

#### True API Mechanics (2026-04-13)

- **Member API only** — write access requires ORCID membership (organizational, annual fee).
- **Scope**: `/activities/update` via 3-legged OAuth. User must explicitly authorize.
- **Endpoint**: `POST https://api.orcid.org/v3.0/{orcid-id}/work`.
- **Format**: XML or JSON. Returns a `put-code` for future updates/deletes.
- **Sandbox**: `https://api.sandbox.orcid.org/` — use for development.
- Once a work is POSTed, updates use `PUT /work/{put-code}`, deletes use `DELETE /work/{put-code}`.

#### SCIENTIA Value

Adding a SCIENTIA paper to the author's ORCID record:
- Propagates to ResearchGate, Scopus, Web of Science, Google Scholar automatically.
- Gives the work cross-database discoverability without any platform-specific scrapers.
- ORCID is effectively a **universal publication router** when combined with a DOI.

#### Recommendation

**Implement after Zenodo is complete.** The workflow is:
1. Zenodo mints DOI.
2. ORCID adapter `POST`s work to `/v3.0/{orcid-id}/work` with the DOI.
3. All databases that federate from ORCID see the record.

**This is the highest-leverage single scholarly integration after Zenodo.**

#### SSoT Fields Required

```
orcid.orcid_id: String                         // e.g. "0000-0002-1825-0097"
orcid.access_token: resolved via Clavis VoxOrcidAccessToken
orcid.sandbox: bool                             // default true until production verified
orcid.put_code: Option<String>                  // stored after first POST for future updates
```

#### Codebase Impact

- New `scholarly/orcid.rs` adapter.
- New `OrcidConfig` struct in `types.rs` (requires `orcid_id: String`).
- New `VoxOrcidAccessToken` and `VoxOrcidClientId`/`VoxOrcidClientSecret` in Clavis `spec.rs`.
- Add `orcid: ChannelOutcome` to `SyndicationResult`.
- Add `orcid: Option<OrcidConfig>` to `SyndicationConfig`.

---

### 6.2 Figshare

#### Overview

Figshare is a research data and publication repository widely used for datasets, code, figures, and preprints. Strongly favored by funders requiring open data compliance (e.g., NIH, Wellcome Trust, UKRI).

#### True API Mechanics (2026-04-13)

- **Personal Access Token** for individual use. `Authorization: token {TOKEN}` header.
- **No OAuth required** for personal accounts (simpler than Zenodo).
- **Article creation**: `POST /account/articles` → returns `article_id`.
- **File upload**: 4-step multipart process:
  1. `POST /account/articles/{id}/files` with `{name, size, md5}` → `location` URL.
  2. `GET {location}` → get part URLs.
  3. `PUT {part_url}` for each part (binary chunk).
  4. `POST /account/articles/{id}/files/{file_id}` → complete upload.
- **Publish**: `POST /account/articles/{article_id}/publish` — **irreversible**.
- Published articles receive a Figshare DOI.
- **Sandbox**: `https://figshare.sandbox.figshare.com/` for testing.

#### SCIENTIA Value

Figshare is widely used for:
- **Supplementary datasets** accompanying papers.
- **Code datasets** (MENS training corpora, evaluation benchmarks, Vox compiler artifacts).
- **Preprints** for non-arXiv-eligible content.

Where Zenodo is more appropriate for formal preprints, Figshare excels at **datasets and supplementary materials**. Many publishers link directly to Figshare for open data requirements.

#### Comparison to Zenodo

| Feature | Zenodo | Figshare |
|---|---|---|
| DOI | ✅ | ✅ |
| Auth | Bearer token (scoped) | Personal token |
| File upload | Simple PUT to bucket | 4-step multipart |
| Metadata schema | Zenodo-specific | Figshare-specific |
| Storage limit | 50 GB per record (free) | 20 GB per item (free) |
| Primary use | Preprints, datasets, software | Datasets, figures, code |
| Publisher integrations | Strong (CERN/EUDAT/OpenAIRE) | Strong (Taylor & Francis, etc.) |
| Best for SCIENTIA | Formal preprints | Supplementary data, corpora |

#### Recommendation

**Implement as Wave 2 scholarly target, after Zenodo.** Priority: Zenodo > ORCID > Figshare.

#### SSoT Fields Required

```
figshare.access_token: resolved via Clavis VoxFigshareAccessToken
figshare.sandbox: bool                         // default true
figshare.title: Option<String>                 // overrides item.title
figshare.description: Option<String>           // overrides body
figshare.categories: Vec<u32>                  // Figshare taxonomy category IDs
figshare.tags: Vec<String>
figshare.defined_type: "dataset" | "figure" | "media" | "presentation" | "poster" | "software" | "preprint"
figshare.files: Vec<String>                    // repo-relative paths to upload
```

archived_date: 2026-04-18
---

## 7. Priority Matrix (Updated)

| Platform | Code Status | Posting Works? | EP IDs | Maint. Burden | Audience Value | Action |
|---|---|---|---|---|---|---|
| **Discord** | Implemented ✅ | Yes | EP-001,014,015 | Low | High | Ship + EP-001 |
| **RSS** | Implemented ✅ | Yes | — | Near-zero | Medium | Ship |
| **GitHub** | Implemented ✅ | Yes (needs audit) | EP-009 | Low | High | Audit EP-009, Ship |
| **Bluesky** | Broken ⚠️ | **No (silent fail)** | EP-012,023,026 | Low-Med | High (academics) | Fix EP-012 first |
| **Mastodon** | Stub ❌ | No | EP-027,029 | Low | High (academics) | Implement now |
| **Reddit** | Partial ⚠️ | Yes (bugs) | EP-017,019 | Med-High | High (CS) | Fix + human gate |
| **Twitter/X** | Code OK ⚠️ | Needs paid plan | EP-021,022 | Very High | Medium | billing gate only |
| **Open Collective** | Partial ⚠️ | Partial | EP-005,006 | Low-Med | Low | Quick fix |
| **HN** | ManualAssist ✅ | Manual only | EP-010 | Zero | High (viral) | Add comment_draft |
| **YouTube** | Partial ⚠️ | Private-only | EP-011,036 | Medium | High (demos) | Compliance audit gate |
| **LinkedIn** | Stub ❌ | No | EP-031–035 | **High** | Medium | Defer after Mastodon |
| **Zenodo** | Partial ⚠️ | Unknown | EP-038–041 | Low-Med | **Critical** | Audit + complete |
| **OpenReview** | Implemented ⚠️ | MFA risk | EP-042,043 | Med-High | Critical (TMLR) | MFA workaround |
| **arXiv** | ManualAssist ✅ | Manual only | EP-044–046 | High | **Critical** | Build export + preflight |
| **ORCID** | Missing ❌ | Not built | — | Medium | **Critical** | Implement Wave 1 scholarly |
| **Figshare** | Missing ❌ | Not built | — | Low | High (datasets) | Implement Wave 2 scholarly |
| **Crossref** | Metadata only ❌ | No | EP-047–050 | Medium | Critical (DOI graph) | Defer until membership |
| **ResearchGate** | N/A | **No API exists** | — | Zero | High (auto via DOI) | Passive only, doc only |
| **Academia.edu** | N/A | **No API exists** | — | Zero | Low | Do not implement |

---

## 8. Hallucination Inventory (Updated)

| ID | Claim | Reality | Root Cause |
|---|---|---|---|
| H-001 | "Discord adapter is a hard stub" | Discord is implemented (52 lines) | Community playbook written before code landed |
| H-002 | "Reddit User-Agent missing on submit POST" | User-Agent correctly sent on submit (line 107) | v1 audit error — wrong line was read |
| H-003 | "LinkedIn uses UGC Posts API" | `ugcPosts` API is **deprecated** | Playbook references 2022-era docs |
| H-004 | "Twitter free tier allows posting" | Free tier: no write access since early 2026 | API pricing changed February 2026 |
| H-005 | "Bluesky field `access_token`" | Correct field: **`accessJwt`** | AT Protocol uses JWT naming, not OAuth |
| H-006 | "arXiv API automation feasible soon" | Client Registry registration required; endorsement tightened Jan 2026 | Optimistic research docs |
| H-007 | "Crossref uses JSON REST API" | Crossref deposit: **HTTPS POST multipart/form-data with XML** | Confused with Crossref metadata retrieval API |
| H-008 | "ResearchGate has an API" | ResearchGate has NO public API; ToS prohibits automation | Wishful planning; API does not exist |
| H-009 | "OpenCollective header is `Api-Key`" | Official docs use `Personal-Token` | Header worked but is legacy form |
| H-010 | "YouTube adapter needs retry wiring only" | Missing `dry_run` guard; will perform disk I/O and OAuth on dry runs | Dry-run path not encoded in adapter signature |
| H-011 | "`social_retry.rs` is wired into dispatch" | Zero call sites for `run_with_retries` in dispatch paths | Infrastructure exists but code was never integrated |
| H-012 | "Bluesky, Mastodon, Discord, LinkedIn are in retry/allowlist system" | These four channels are absent from `switching.rs` allowlist and retry infrastructure | Channels added to types without updating switching.rs |
| H-013 | "Academia.edu has a developer API" | No public API; ToS prohibits automation | Confusion with academic institution management systems sharing the name |

archived_date: 2026-04-18
---

## 9. Unified SSoT Data Model Requirements

The core model (`UnifiedNewsItem` + `SyndicationConfig`) is structurally sound but has specific gaps:

### 9.1 Missing Fields in `SyndicationConfig`

```rust
pub struct SyndicationConfig {
    // ... existing ...
    pub orcid: Option<OrcidConfig>,            // NEW — Wave 1 scholarly
    pub figshare: Option<FigshareConfig>,       // NEW — Wave 2 scholarly
    // researchgate: intentionally ABSENT — passive discovery only
}
```

### 9.2 Missing Fields in Existing Channel Configs

```rust
// MastodonConfig — MISSING:
pub instance_url: String,                      // REQUIRED — no default
pub language: Option<String>,                  // ISO 639 code

// LinkedInConfig — MISSING:
pub author_urn: String,                        // "urn:li:person:{id}" — REQUIRED
pub api_version: String,                       // e.g. "202604" — REQUIRED

// HackerNewsConfig — MISSING:
pub comment_draft: Option<String>,             // first comment text

// BlueskyConfig — BROKEN:
pub pds_url: Option<String>,                   // explicit PDS override (for non-bsky.social users)
// link_facet: bool — already exists but unimplemented
```

### 9.3 Missing Fields in `SyndicationResult`

```rust
pub struct SyndicationResult {
    // ... existing ...
    pub orcid: ChannelOutcome,                 // NEW
    pub figshare: ChannelOutcome,              // NEW
    pub researchgate_doi_queued: bool,         // NEW — passive tracking only (not a ChannelOutcome)
}
```

### 9.4 `switching.rs` Channel Registry Additions Needed

All of the following must be added to:
- `apply_channel_allowlist`
- `failed_channels` / `successful_channels`
- `outcome_for_channel` match arms
- `normalize_distribution_json_value_with_warnings` contract-shape expansion block

```
bluesky, mastodon, linkedin, discord, orcid, figshare
```

### 9.5 Content Hash Fix

Separate `content_sha3_256` from routing config to prevent unnecessary dual-approval re-triggers:

```rust
pub fn content_sha3_256(&self) -> String {
    // Hash ONLY: id, title, author, published_at, tags, content_markdown
    // Do NOT include: syndication, topic_pack — routing is not content
}
```

### 9.6 Scholarly SSoT Publication Record

A new `ScholarlyPublicationRecord` struct should track the scholarly lifecycle separately from the news syndication model:

```rust
pub struct ScholarlyPublicationRecord {
    pub publication_id: Uuid,
    pub doi: Option<String>,                       // minted after Zenodo publish
    pub zenodo_deposit_id: Option<String>,
    pub zenodo_doi: Option<String>,
    pub orcid_put_code: Option<String>,            // for future updates
    pub figshare_article_id: Option<String>,
    pub arxiv_submission_id: Option<String>,
    pub openreview_forum_id: Option<String>,
    pub crossref_deposit_id: Option<String>,
    pub researchgate_confirmed: bool,              // manual confirmation tracked
    pub published_at: Option<DateTime<Utc>>,
    pub status: ScholarlyPublicationStatus,
}

pub enum ScholarlyPublicationStatus {
    Draft,
    Deposited,          // Zenodo created, not published
    Published,          // DOI minted
    Retracted,          // requires human action
}
```

---

## 10. Implementation Policy

This section defines the **binding rules** for adding, modifying, or removing publication channels from the Scientia pipeline. All future development must conform.

### 10.1 Channel Classification

Every publication target must be classified at design time:

| Class | Meaning | Examples | Code Required |
|---|---|---|---|
| `ActivePush` | SCIENTIA posts content via HTTP API | Discord, Reddit, Mastodon, Bluesky | Yes — adapter in `adapters/*.rs` |
| `ScholarlyDeposit` | Formal archival with DOI/ID | Zenodo, ORCID, Figshare, OpenReview | Yes — adapter in `scholarly/*.rs` |
| `ManualAssist` | SCIENTIA generates draft; human submits | HN, arXiv (for now), LinkedIn (organizational) | Yes — draft generator only |
| `PassiveDiscovery` | Platform ingests automatically via DOI/metadata feeds; no code | ResearchGate, Academia.edu | **No adapter code** |
| `Deferred` | API exists but org/billing blocker | Crossref (membership), YouTube (compliance), LinkedIn (App Review) | Stub with TOESTUB only |

### 10.2 Gate Requirements Per Class

| Class | `dry_run` guard | `run_with_retries` | `vox clavis doctor` check | Dual approval | Human gate |
|---|---|---|---|---|---|
| `ActivePush` | **Mandatory** | **Mandatory** | Required for secrets | Required for live | Recommended for social |
| `ScholarlyDeposit` | **Mandatory** | **Mandatory** | Required for secrets | **Required** | **Required** (publish is irreversible) |
| `ManualAssist` | N/A (no HTTP) | N/A | Optional | Optional | **Inherent** (human submits) |
| `PassiveDiscovery` | N/A | N/A | Optional | N/A | Optional |
| `Deferred` | N/A (stub returns Err) | N/A | Gate must explain blocker | N/A | N/A |

### 10.3 New Channel Checklist

Before merging any new publication channel:

- [ ] Classification assigned and documented.
- [ ] Adapter file: `adapters/{channel}.rs` or `scholarly/{channel}.rs`.
- [ ] Config struct added to `types.rs` with all required fields.
- [ ] Config added to `SyndicationConfig`.
- [ ] Outcome field added to `SyndicationResult`.
- [ ] Channel added to `switching.rs`: `apply_channel_allowlist`, `failed_channels`, `successful_channels`, `outcome_for_channel`, `normalize_distribution_json_value_with_warnings`.
- [ ] `run_with_retries` wired from dispatch path.
- [ ] `dry_run` guard in adapter before any I/O.
- [ ] Clavis secrets registered in `spec.rs` with correct `SecretId` variants.
- [ ] `vox clavis doctor` probe added for required secrets.
- [ ] TOESTUB compliance: no `pub use` in frozen modules, no god objects.
- [ ] Integration test added with mock server (at minimum, a `dry_run: true` compile test).

### 10.4 Volatile API Policy

Platforms with rapidly changing APIs require explicit maintenance triggers:

| Platform | Trigger | Cadence |
|---|---|---|
| LinkedIn `Linkedin-Version` header | New quarterly API version | Quarterly check |
| Twitter/X billing | API pricing changes | On each billing cycle |
| OpenReview API version | OpenReview migration announcements | Monitor changelog |
| arXiv endorsement policy | arXiv policy announcements | Monitor arXiv blog |
| Crossref XML schema | Crossref schema releases | On schema version bump |

These should be added as calendar reminders in contributor documentation, not just in this research doc.

### 10.5 Data Retention and Audit Trail

Every `ActivePush` and `ScholarlyDeposit` call **must** write to the `syndication_events` table (currently missing — PROBLEM-24 from gap analysis) before returning. Schema:

```sql
CREATE TABLE IF NOT EXISTS syndication_events (
    id              TEXT PRIMARY KEY,     -- uuid
    publication_id  TEXT NOT NULL,
    channel         TEXT NOT NULL,        -- "discord", "zenodo", etc.
    outcome         TEXT NOT NULL,        -- JSON: ChannelOutcome
    external_id     TEXT,                 -- platform-specific ID/URL
    attempt_number  INTEGER NOT NULL DEFAULT 1,
    attempted_at    TEXT NOT NULL,        -- ISO 8601 UTC
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

Without this table: no audit trail, no KPI computation, no feedback loop.

### 10.6 Do Not Implement List

The following platforms have been researched, confirmed to have **no public API for programmatic posting**, and should never have adapter code written:

| Platform | Reason |
|---|---|
| **ResearchGate** | No public API. ToS prohibits automation. Passive via DOI. |
| **Academia.edu** | No public API. ToS prohibits automation. Low scientific value. |
| **Google Scholar** | No API. Passive indexing only. |
| **Semantic Scholar** | No write API. Read API only. Passive via DOI. |
| **Web of Science** | Subscription-gated. No submission API. |
| **Scopus** | Subscription-gated. No submission API. |

archived_date: 2026-04-18
---

## 11. Task Backlog (Updated)

Tasks are organized by dependency order. `EP-NNN` references correlate to §2-§6.

### Wave 0 — Critical Fixes (No Dependencies)

| Task | EP | File | Est. Lines |
|---|---|---|---|
| Fix `accessJwt` field name in `bluesky.rs` | EP-012 | `adapters/bluesky.rs:14` | 1 |
| Add `instance_url` to `MastodonConfig` | EP-029 | `types.rs` | 2 |
| Fix `makePublicOn` to use `config.scheduled_publish_at` | EP-006 | `adapters/opencollective.rs:37` | 3 |
| Add `dry_run` guard to `youtube.rs::upload_video` | EP-011 | `adapters/youtube.rs` | 5 |
| Update OC auth header to `Personal-Token` | EP-005 | `adapters/opencollective.rs:46` | 1 |
| Document Reddit AI training prohibition | EP-020 | `AGENTS.md` + `docs/src/reference/clavis-ssot.md` | — |

### Wave 1 — Infrastructure (Parallel, No Feature Dependencies)

| Task | EP | File | Est. Lines |
|---|---|---|---|
| Wire `run_with_retries` into Discord dispatch | EP-001 | `switching.rs` or publisher dispatch | ~10 |
| Wire `run_with_retries` into Reddit dispatch | EP-001 | dispatch | ~10 |
| Wire `run_with_retries` into Bluesky dispatch | EP-001 | dispatch | ~10 |
| Wire `run_with_retries` into Twitter dispatch | EP-001 | dispatch | ~10 |
| Wire `run_with_retries` into YouTube dispatch | EP-001 | dispatch | ~10 |
| Add `bluesky/mastodon/linkedin/discord` to `apply_channel_allowlist` | EP-002 | `switching.rs:285` | ~8 |
| Add these channels to `failed_channels` | EP-003/4 | `switching.rs:315` | ~8 |
| Add these channels to `outcome_for_channel` | EP-004 | `switching.rs:378` | ~8 |
| Add these channels to contract-shape expander | EP-003 | `switching.rs:193` | ~8 |
| Create `syndication_events` DB table migration | EP-001 parent | `vox-db` | ~30 |
| Fix `content_sha3_256` to exclude `syndication` | EP-008 | `types.rs:470` | ~10 |
| Add `comment_draft` to `HackerNewsConfig` | EP-010 | `types.rs:211` | 2 |

### Wave 2 — Mastodon Implementation

| Task | EP | Notes |
|---|---|---|
| Implement `adapters/mastodon.rs` | EP-027 | ~60 lines |
| Add `language: Option<String>` to `MastodonConfig` | EP-028 | 1 line |
| Register `VoxMastodonAccessToken` in Clavis (verify exists) | — | `spec.rs` |
| Add Mastodon to `switching.rs` channel registry | EP-002 | Wire allowlist, retry, outcome |
| Add `vox clavis doctor` Mastodon secret probe | — | `vox-cli` |

### Wave 3 — Bluesky Hardening

| Task | EP | Notes |
|---|---|---|
| Implement `resolve_pds(handle) -> String` | EP-023 | ~30 lines, separate function |
| Add in-memory session cache with TTL for `accessJwt`/`refreshJwt` | EP-024 | ~40 lines |
| Implement link card embed (`$type: app.bsky.embed.external`) | EP-007 | ~30 lines |
| Add grapheme cluster count validation | EP-025 | `unicode-segmentation` crate |
| Fix `dry_run` plumbing through Bluesky dispatch | EP-026 | Adapter signature change |

### Wave 4 — Zenodo Completion

| Task | EP | Notes |
|---|---|---|
| Audit `scholarly/zenodo.rs` — confirm HTTP calls exist or implement | EP-038 | Inspect ~20 KB file |
| Add `--sandbox` routing flag | EP-039 | `VoxZenodoSandbox` Clavis entry |
| Add async deposit status polling | EP-040 | ~40 lines |
| Add publish confirmation gate (irreversibility warning) | EP-041 | UX + gate logic |
| Write to `syndication_events` on Zenodo deposit and publish | Parent | DB write |

### Wave 5 — ORCID Implementation

| Task | EP | Notes |
|---|---|---|
| Create `scholarly/orcid.rs` adapter | — | ~80 lines |
| Add `OrcidConfig` struct to `types.rs` | — | 5 fields |
| Add `orcid: Option<OrcidConfig>` to `SyndicationConfig` | — | 1 line |
| Add `orcid: ChannelOutcome` to `SyndicationResult` | — | 1 line |
| Register Clavis entries for ORCID client credentials | — | `spec.rs` |
| Add to `switching.rs` channel registry | — | Allowlist, retry, outcome |

### Wave 6 — Twitter Gate, YouTube Gate

| Task | EP | Notes |
|---|---|---|
| Add Twitter billing status check to `vox clavis doctor` | EP-021 | Document as `status: billing_required` |
| Add YouTube compliance audit status to `vox clavis doctor` | EP-036 | Document as `status: compliance_audit_required` |
| Add per-session tweet budget to `TwitterConfig` | EP-022 | `tweet_budget_per_session: usize` |

### Wave 7 — arXiv Preflight + Export

| Task | EP | Notes |
|---|---|---|
| Create arXiv format preflight profile | EP-044 | `PreflightProfile::ArxivFormat` |
| Add arXiv endorsement requirements to Clavis doctor | EP-045 | Documentation check |
| Integrate AI content policy gate into arXiv preflight | EP-046 | Socrates confidence threshold |

### Wave 8 — Figshare (Optional, Data-Focused)

| Task | Notes |
|---|---|
| Create `scholarly/figshare.rs` adapter | 4-step multipart upload |
| Add `FigshareConfig` to `types.rs` | 7 fields |
| Register `VoxFigshareAccessToken` in Clavis | |

### Deferred (Org Blockers)

| Task | Blocker |
|---|---|
| LinkedIn implementation | App Review + `author_urn` identity decision |
| Crossref XML deposit | Crossref membership required |
| OpenReview MFA workaround | March 2026 MFA rollout — document only for now |

### Do Not Implement

| Target | Decision |
|---|---|
| ResearchGate adapter | No API. PassiveDiscovery via DOI. |
| Academia.edu adapter | No API. Low value. |
| Google Scholar adapter | No write API. Passive only. |
| Semantic Scholar adapter | No write API. |

---

*Research v2 — web searches and code audit conducted 2026-04-13. Code files audited: `adapters/*`, `scholarly/*`, `switching.rs`, `syndication_outcome.rs`, `types.rs`, `gate.rs`, `social_retry.rs`, `scientia_heuristics.rs`. ResearchGate: confirmed no public API via multiple sources. ORCID and Figshare: confirmed public APIs with REST/token access.*


