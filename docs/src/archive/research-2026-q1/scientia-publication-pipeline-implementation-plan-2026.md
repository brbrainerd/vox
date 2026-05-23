---
title: "Scientia Publication Pipeline — Full Implementation Plan v2 (2026)"
description: "Explicit, task-by-task implementation plan for hardening the Vox Scientia publication pipeline. v2: corrected 9 factual errors from v1, added Bluesky XRPC endpoint fix, filled missing PublisherConfig credential fields, corrected SyndicationResult already-present fields, removed false Zenodo tasks, and added LinkedIn base-path correction. Designed as a self-contained reference for implementation agents."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-13"
training_eligible: false
training_rationale: "Transient roadmap document. Training-ineligible to avoid LLMs learning stale task states."
archived_date: 2026-04-18
---

# Scientia Publication Pipeline — Full Implementation Plan v2 (2026)

> [!IMPORTANT]
> **This is v2 of the implementation plan.** v1 was critiqued against the codebase and found to
> contain 9 factual errors, 6 omissions, and 4 tasks that were already complete.
> v2 corrects all of these. Do NOT follow v1.
>
> **Primary references:**
> - Research doc: `docs/src/architecture/scientia-publication-endpoints-research-2026.md` (v2)
> - Publishing dispatch: `crates/vox-publisher/src/publisher/mod.rs` (605 lines)
> - Channel config types: `crates/vox-publisher/src/types.rs`
> - Secrets registry: `crates/vox-clavis/src/spec/ids.rs` (531 lines — read fully before adding variants)
> - Outcome tracking: `crates/vox-publisher/src/syndication_outcome.rs`
> - Retry infra: `crates/vox-publisher/src/social_retry.rs`
> - Switching/allowlist: `crates/vox-publisher/src/switching.rs`
> - Adapter stubs: `crates/vox-publisher/src/adapters/mastodon.rs` (14L), `adapters/linkedin.rs` (14L)
> - Full implementations: RSS, Twitter, GitHub (via forge), OC, Reddit (feature-gated), YouTube (feature-gated), Discord (52L), HN (manual-assist)

---

## v1 Critique and Corrections

Before reading the task list, read this section. Every correction below was verified by inspecting source files. Implementing any v1 task that this section contradicts would introduce regressions.

### CORRECTION C-001: Bluesky XRPC Endpoint for Creating Records

**v1 claimed**: Post endpoint should be `com.atproto.repo.createRecord` (XRPC method).

**Correct**: Both the method name AND the URL path use `com.atproto.repo.createRecord`. The URL is:
```
POST https://{pds}/xrpc/com.atproto.repo.createRecord
```
The XRPC path IS the NSID. The current code at line 74 of `bluesky.rs` has:
```
"https://bsky.social/xrpc/app.bsky.feed.post"
```
This is wrong for two reasons: (1) hardcoded `bsky.social`, (2) uses the **collection NSID** (`app.bsky.feed.post`) as the **endpoint path** — these are different things. The `app.bsky.feed.post` value belongs in the **`collection` field of the request body**, not in the URL. v1 was right that the endpoint is wrong, but the wording was confusing. The correct URL path is `/xrpc/com.atproto.repo.createRecord`.

### CORRECTION C-002: Bluesky `app.bsky.feed.post` in URL is WRONG — it's a body field

Verification (web research 2026-04-13): The AT Protocol endpoint for posting any record is always `com.atproto.repo.createRecord` (the path NSID). The `app.bsky.feed.post` string is the value of the `collection` field in the JSON body. Current code at line 74 conflates these. This is a separate bug from the hardcoded PDS.

### CORRECTION C-003: `SyndicationResult` Already Has Four Modern Channel Fields

**v1 task T-018 direction** (add fields to `SyndicationResult`): T-018 implied `bluesky`, `mastodon`, `linkedin`, `discord` were missing.

**Reality** (verified in `syndication_outcome.rs` lines 37–44):
```rust
pub bluesky: ChannelOutcome,      // line 38 — EXISTS
pub mastodon: ChannelOutcome,     // line 40 — EXISTS
pub linkedin: ChannelOutcome,     // line 42 — EXISTS
pub discord: ChannelOutcome,      // line 44 — EXISTS
```
These are already present with `#[serde(default)]`. T-018 (add `researchgate_doi_queued`) is still valid but the four channel fields are NOT missing. Remove "add bluesky/mastodon/linkedin/discord to SyndicationResult" from task lists.

### CORRECTION C-004: `all_enabled_channels_succeeded` Also Already Checks bluesky/mastodon/linkedin/discord

Lines 89–92 of `syndication_outcome.rs`:
```rust
let bsky_ok = item.syndication.bluesky.is_none() || ok(&self.bluesky);
let masto_ok = item.syndication.mastodon.is_none() || ok(&self.mastodon);
let linkedin_ok = item.syndication.linkedin.is_none() || ok(&self.linkedin);
let discord_ok = item.syndication.discord.is_none() || ok(&self.discord);
```
These checks are already implemented. The `SyndicationResult` struct is further ahead than the research docs indicated.

### CORRECTION C-005: `PublisherConfig` Does NOT Have Bluesky/Mastodon/LinkedIn/Discord Credential Fields

**v1 task T-020 said**: "Check existing struct, do NOT duplicate." That was correct guidance but the important news is: `PublisherConfig` (`publisher/config.rs`) has **zero fields** for bluesky, mastodon, linkedin, or discord. They must all be added. The credential fields that DO exist (lines 6–29 of `config.rs`):
- `twitter_bearer_token` ✅
- `forge_token` ✅
- `open_collective_token` ✅
- `reddit_client_id/secret/refresh_token/user_agent` ✅
- `youtube_client_id/secret/refresh_token` ✅
- No: `bluesky_handle`, `bluesky_app_password`, `mastodon_access_token`, `discord_webhook_url`, `linkedin_access_token`

Clavis SecretIds for Bluesky, Mastodon, LinkedIn, Discord DO already exist in `ids.rs`:
- `VoxSocialBlueskyHandle` (line 41)
- `VoxSocialBlueskyPassword` (line 42)
- `VoxSocialMastodonToken` (line 51)
- `VoxSocialMastodonDomain` (line 52) ← Note: this is the **instance domain**, not `instance_url`. Plan must align with this.
- `VoxSocialLinkedinAccessToken` (line 53)
- `VoxSocialDiscordWebhook` (line 54)

Also: `VoxOrcidClientId` (line 69) and `VoxOrcidClientSecret` (line 70) already exist. Do NOT re-add them.

### CORRECTION C-006: Discord Adapter Already Resolves Clavis Internally

The `adapters/discord.rs` `post(...)` function (line 12) resolves `VoxSocialDiscordWebhook` from Clavis itself. It does NOT need the webhook URL passed through `PublisherConfig`. However, it falls back to `cfg.webhook_url_override` first (line 11). The `PublisherConfig` does not need a `discord_webhook_url` field — the adapter is self-sufficient. Wire dispatch without a config field.

### CORRECTION C-007: Mastodon Clavis Has `VoxSocialMastodonDomain` Not `instance_url`

The existing Clavis `SecretId::VoxSocialMastodonDomain` (line 52 of `ids.rs`) provides the instance **domain** (e.g., `scholar.social`), not a full URL. The `PublisherConfig` field should resolve this domain and compute the full URL as `https://{domain}`. Do NOT add an `instance_url` field to `MastodonConfig` — instead pull from Clavis. However, `MastodonConfig` should keep an `instance_url_override: Option<String>` for per-item overrides.

### CORRECTION C-008: Mastodon API Accepts JSON Body (Not Only Form-Encoded)

**v1 T-021** showed form-encoding with a warning "Do NOT use `.json()`". This is **incorrect** — Mastodon's API accepts both `application/x-www-form-urlencoded` and `application/json`. Both are equally supported. JSON is often cleaner for handling optional boolean fields (avoids the "sensitive"/"true" string-encoding issue). The implementation may use either — but using `.json()` is correct and simpler.

### CORRECTION C-009: Zenodo Adapter is FULLY IMPLEMENTED

**v1 T-028** said: "Audit Zenodo adapter for HTTP completeness — does it create a deposit, upload files, publish?"

**Reality** (verified by reading all 564 lines of `scholarly/zenodo.rs`): The Zenodo adapter is **complete and production-grade**:
- ✅ `create_deposition_draft` — creates deposit via `POST /deposit/depositions`
- ✅ `put_bucket_object` — uploads files via `PUT {bucket_url}/{name}` with retry
- ✅ `publish_deposition` — mints DOI via `POST /deposit/depositions/{id}/actions/publish`
- ✅ Retry with exponential backoff and `Retry-After` header parsing
- ✅ Sandbox/production routing via `VOX_ZENODO_API_BASE` or `sandbox` bool
- ✅ Checksum verification via `staging_checksums.json`
- ✅ File allowlist via `VOX_ZENODO_UPLOAD_ALLOWLIST`
- ✅ Draft-only mode via `VOX_ZENODO_DRAFT_ONLY`
- ✅ Metadata parity check via `VOX_ZENODO_REQUIRE_METADATA_PARITY`

**Delete T-028 and T-029 (Zenodo audit and publish gate) from the task backlog.** These are already done. The Zenodo HTTP layer is not a gap.

### CORRECTION C-010: LinkedIn Base URL is `/rest/` Not `/v2/`

The LinkedIn Posts API (the non-deprecated replacement for `ugcPosts`) uses:
```
POST https://api.linkedin.com/rest/posts
```
**NOT** `https://api.linkedin.com/v2/posts`. The v1 plan referenced `https://api.linkedin.com/v2/posts` which is the legacy/deprecated endpoint pattern. The new REST API requires the path `/rest/` and the `LinkedIn-Version: YYYYMM` header.

### CORRECTION C-011: LinkedIn Token is `VoxSocialLinkedinAccessToken` — Already in Clavis

`SecretId::VoxSocialLinkedinAccessToken` exists at line 53 of `ids.rs`. Do NOT add a new Clavis entry for it. Add only the `PublisherConfig` field that resolves it.

### CORRECTION C-012: ORCID Already Has `VoxOrcidClientId` and `VoxOrcidClientSecret` in Clavis

Lines 69–70 of `ids.rs`. However, there is **no `VoxOrcidAccessToken`** — only client credentials (for the OAuth 2.0 client credentials flow). The implementation must perform the OAuth exchange to get a user access token. Per ORCID member API: the token used for posting to a user's record must be obtained via 3-legged OAuth (`/activities/update` scope). The client credentials (`client_id`/`client_secret`) cannot replace this — they are for `read-public` or institutional flows.

### CORRECTION C-013: v1 Anti-Hallucination Block Overstated `social_retry.rs` as Dead Code

v1 said "zero call sites for `run_with_retries`" — this was based on an early grep. After reading `publisher/mod.rs` in full (605 lines), `run_with_retries` IS called in:
- RSS (line 225)
- Twitter (line 257)
- GitHub/forge (line 299)
- OpenCollective (line 343)
- Reddit (line 403)
- YouTube (line 536)

This correction was already applied to the v2 research doc. The anti-hallucination block in v1 of this plan incorrectly stated all six were missing. The actual gap is: Discord, Bluesky, Mastodon, LinkedIn are missing from `publish_all` because their dispatch blocks don't exist yet.

archived_date: 2026-04-18
---

## Verified File Layout (Updated)

```
crates/vox-publisher/src/
  publisher/
    mod.rs         (605 lines) — publish_all() dispatch; RSS/Twitter/GitHub/OC/Reddit/HN/YouTube/crates_io dispatched ✅
                                  Discord/Bluesky/Mastodon/LinkedIn NOT dispatched ❌
    config.rs      (198 lines) — PublisherConfig; NO bluesky/mastodon/discord/linkedin credential fields ❌
    heuristics.rs  (6860 bytes) — social text helpers
  adapters/
    mod.rs         (18 lines)  — re-exports; forge{} wraps github::post ✅
    bluesky.rs     (95 lines)  — BROKEN: wrong JWT field + wrong XRPC URL + no dry_run param ❌
    discord.rs     (52 lines)  — implemented; resolves webhook from Clavis internally ✅
    github.rs      (102 lines) — implemented ✅
    hacker_news.rs (849 bytes) — ManualAssist ✅
    linkedin.rs    (398 bytes, 14 lines) — hard stub ❌
    mastodon.rs    (401 bytes, 14 lines) — hard stub (has dry_run param) ❌
    opencollective.rs (79 lines) — partial (wrong header, makePublicOn not wired) ⚠️
    reddit.rs      (129 lines) — correct (User-Agent IS sent) ✅
    rss.rs         (5658 bytes) — implemented ✅
    twitter.rs     (3381 bytes) — implemented ✅
    youtube.rs     (7070 bytes) — feature-gated; dry_run guarded in publisher/mod.rs line 482 ✅
  scholarly/
    zenodo.rs      (564 lines) — FULLY IMPLEMENTED (create+upload+publish+retry) ✅
    openreview.rs  (16248 bytes) — implemented ⚠️ (MFA risk 2026)
    mod.rs, error.rs, flags.rs, idempotency.rs — infrastructure ✅
  syndication_outcome.rs (211 lines) — SyndicationResult has bluesky/mastodon/linkedin/discord ✅
  types.rs                (576 lines) — SyndicationConfig + per-channel Config structs
  gate.rs                 (252 lines) — dual-approval gate ✅
  social_retry.rs         (82 lines) — IS wired (RSS/Twitter/GitHub/OC/Reddit/YouTube)
  contract.rs             (166 lines) — constants + clamp_text

crates/vox-clavis/src/spec/ids.rs (531 lines) — Already has:
  VoxSocialBlueskyHandle, VoxSocialBlueskyPassword
  VoxSocialMastodonToken, VoxSocialMastodonDomain
  VoxSocialLinkedinAccessToken
  VoxSocialDiscordWebhook
  VoxOrcidClientId, VoxOrcidClientSecret
  VoxZenodoAccessToken
  (NOT: VoxOrcidAccessToken — this must be an explicit per-user Bearer token added separately)
```

---

## Anti-Hallucination: Critical Facts for Implementation Agents

1. **`publish_all` is in `publisher/mod.rs`** (605 lines). The dispatch section handles RSS, Twitter, GitHub, OC, Reddit, HN, YouTube, crates_io. Discord/Bluesky/Mastodon/LinkedIn blocks **do not exist** and must be added, following the existing pattern verbatim.

2. **The Bluesky endpoint URL is wrong in two ways**: (a) hardcoded `bsky.social`, (b) wrong XRPC method — it uses `app.bsky.feed.post` as the path (a Lexicon collection name), which should be `com.atproto.repo.createRecord`. The collection name `app.bsky.feed.post` belongs in the **request body's `collection` field**, not in the URL.

3. **`SyndicationResult` already has `bluesky`, `mastodon`, `linkedin`, `discord`** (lines 38–44 of `syndication_outcome.rs`). Do not add them again.

4. **`switching.rs` does NOT have these channels** in `apply_channel_allowlist`, `failed_channels`, `successful_channels`, or `outcome_for_channel`. These four functions need updating.

5. **Zenodo is fully implemented** (564 lines, creates deposit + uploads + publishes + retries + checksum validation). The Zenodo gap story from earlier in the session was wrong. Do not "implement" Zenodo.

6. **Mastodon's `post()` stub already accepts `dry_run: bool` as 4th param** — matching the parameter the dispatch block must pass. The function signature is correct; only the body needs implementation.

7. **Discord resolves its own secret** from Clavis internally. No `PublisherConfig` field needed for it. The dispatch block just needs: token lookup removed, call `adapters::discord::post(&self.config, item, discord_cfg, is_dry_run)`.

8. **LinkedIn Posts API base URL is `https://api.linkedin.com/rest/posts`** — NOT `/v2/posts`. v2 is the deprecated ugcPosts path.

9. **`VoxSocialMastodonDomain`** gives the instance hostname (e.g., `scholar.social`). Convert to URL in `PublisherConfig`: `format!("https://{}", domain)`. The `MastodonConfig` struct should have `instance_url_override: Option<String>` for per-item-manifest overrides, defaulting to the Clavis-resolved domain.

10. **ORCID client credentials (`VoxOrcidClientId`/`VoxOrcidClientSecret`) are for the MEMBER API OAuth client registration.** They do not directly authorize writing to a specific user's record. A user-specific `access_token` (from 3-legged OAuth) is required. The implementation must manage per-user tokens, stored per-user, NOT as a single system secret.

11. **Reddit is feature-gated**: `#[cfg(feature = "scientia-reddit")]` on the module and the dispatch block. LinkedIn/Mastodon are not feature-gated (no `#[cfg]` on their `pub mod` lines in `adapters/mod.rs`). Bluesky uses `pub mod bluesky;` — also not feature-gated.

12. **The `adapters/mod.rs` forge module** is a re-export shim: `pub mod forge { pub use super::github::post; }`. The dispatch in `publisher/mod.rs` calls `adapters::forge::post(...)`. This is correct as-is.

13. **`PublisherConfig::from_operator_environment`** ends with `..Default::default()` (line 194). New fields must EITHER be added to the explicit initializer block OR have a `Default` of `None` and be covered by the `..Default::default()` spread. The latter is safe for `Option<String>` fields. Prefer explicit initialization for new credential fields.

archived_date: 2026-04-18
---

## Task List v2

Tasks marked `[ALREADY DONE]` are verified complete. Do not re-implement them.

### Wave 0 — Critical Single-File Fixes (No Dependencies)

---

#### T-001: Fix Bluesky `accessJwt` Field Name

**File**: `crates/vox-publisher/src/adapters/bluesky.rs`, lines 13–17

**Problem**: `CreateSessionResponse.access_token` should be `accessJwt` (with `refreshJwt` captured too).

**Replace** (lines 13–17):
```rust
#[derive(Deserialize)]
struct CreateSessionResponse {
    access_token: String,
    did: String,
}
```
**With**:
```rust
#[derive(Deserialize)]
struct CreateSessionResponse {
    /// AT Protocol field name for the short-lived bearer token.
    /// This is ALWAYS "accessJwt" — NOT "access_token". Serde silently
    /// deserializes empty string without this rename, causing silent 401s.
    #[serde(rename = "accessJwt")]
    access_jwt: String,
    /// Long-lived refresh token. Store this to avoid re-creating sessions.
    #[serde(rename = "refreshJwt")]
    refresh_jwt: String,
    did: String,
}
```

**Also fix line 75**: change `.bearer_auth(&session.access_token)` to `.bearer_auth(&session.access_jwt)`.

**Verification test**: Deserialize `{"accessJwt":"tok","refreshJwt":"ref","did":"did:plc:abc"}`, assert `.access_jwt == "tok"`.

archived_date: 2026-04-18
---

#### T-002: Fix Bluesky XRPC URL (Two Bugs)

**File**: `crates/vox-publisher/src/adapters/bluesky.rs`

**Bug 1 (line 46)**: Session URL hardcoded to `bsky.social`:
```rust
// WRONG:
.post("https://bsky.social/xrpc/com.atproto.server.createSession")
// CORRECT (use pds_base parameter):
.post(format!("{}/xrpc/com.atproto.server.createSession", pds_base.trim_end_matches('/')))
```

**Bug 2 (line 74)**: Two errors — hardcoded host AND wrong XRPC path:
```rust
// WRONG — app.bsky.feed.post is a collection name, NOT an XRPC method:
.post("https://bsky.social/xrpc/app.bsky.feed.post")
// CORRECT:
.post(format!("{}/xrpc/com.atproto.repo.createRecord", pds_base.trim_end_matches('/')))
```

The request body must also include `collection: "app.bsky.feed.post"` in the `CreateRecordRequest` struct — this is already present at line 31. So the body is correct, only the URL path is wrong.

Add `pds_base: &str` as a new parameter to the `post` function signature (4th parameter, after `password`).

---

#### T-003: Add `dry_run` to Bluesky `post()` Signature

**File**: `crates/vox-publisher/src/adapters/bluesky.rs`

Add `dry_run: bool` as 6th parameter. Add guard at top of function body before any HTTP calls:
```rust
if dry_run {
    return Ok(format!("dry-run-bluesky-{}", item.id));
}
```

Note: Unlike mastodon.rs where `_dry_run` was already in the signature (line 9), bluesky.rs currently has no dry_run parameter at all.

archived_date: 2026-04-18
---

#### T-004: Add `pds_url` to `BlueskyConfig`

**File**: `crates/vox-publisher/src/types.rs`

Locate `BlueskyConfig` struct (search for `pub struct BlueskyConfig`). Add:
```rust
/// PDS base URL. Default: "https://bsky.social".
/// Third-party PDS users must set this to their PDS URL.
#[serde(default = "bluesky_default_pds_url")]
pub pds_url: String,
```
Add the default function after the struct:
```rust
fn bluesky_default_pds_url() -> String {
    "https://bsky.social".to_string()
}
```

---

#### T-005: Fix OpenCollective `Personal-Token` Auth Header

**File**: `crates/vox-publisher/src/adapters/opencollective.rs`, line 46

Replace:
```rust
.header("Api-Key", token)
```
With:
```rust
.header("Personal-Token", token)
```

archived_date: 2026-04-18
---

#### T-006: Wire `makePublicOn` from `OpenCollectiveConfig`

**File**: `crates/vox-publisher/src/adapters/opencollective.rs`, line 37

Replace:
```rust
"makePublicOn": null,
```
With:
```rust
"makePublicOn": config.scheduled_publish_at.map(|dt| dt.to_rfc3339()),
```

Verify that `config.scheduled_publish_at` is `Option<DateTime<Utc>>` by checking `OpenCollectiveConfig` in `types.rs` before making this change.

---

#### T-007: Add Missing Visibility/Language Fields to `MastodonConfig`

**File**: `crates/vox-publisher/src/types.rs`

> [!WARNING]
> Do NOT add `instance_url: String` as the primary field. The instance is resolved from
> `VoxSocialMastodonDomain` in Clavis (domain only, e.g. "scholar.social").
> Add `instance_url_override: Option<String>` for per-manifest overrides.

Find `MastodonConfig` and add:
```rust
/// Override the instance resolved from VoxSocialMastodonDomain.
/// Format: full URL including scheme, e.g. "https://scholar.social".
#[serde(default)]
pub instance_url_override: Option<String>,
/// Post visibility: "public" | "unlisted" | "private" | "direct".
/// Default: "public".
#[serde(default = "mastodon_default_visibility")]
pub visibility: String,
/// ISO 639-1 language code e.g. "en". Improves discoverability.
#[serde(default)]
pub language: Option<String>,
```
Add:
```rust
fn mastodon_default_visibility() -> String { "public".to_string() }
```
Check what fields already exist in `MastodonConfig` before adding. Do not duplicate.

archived_date: 2026-04-18
---

#### T-008: Add `author_urn` and `api_version` to `LinkedInConfig`

**File**: `crates/vox-publisher/src/types.rs`

Find `LinkedInConfig` and add:
```rust
/// LinkedIn author URN. "urn:li:person:{id}" or "urn:li:organization:{id}".
/// REQUIRED. Find person ID via GET https://api.linkedin.com/rest/me
pub author_urn: String,
/// LinkedIn versioned API date YYYYMM. Required in Linkedin-Version header.
/// One year support window — update when LinkedIn sunsets the version in use.
#[serde(default = "linkedin_default_api_version")]
pub api_version: String,
```
Add:
```rust
fn linkedin_default_api_version() -> String {
    // LinkedIn versions are supported for at least 1 year.
    // Update this value when the current version reaches end-of-life.
    // Current: April 2026.
    "202504".to_string()
}
```

---

#### T-009: Add `comment_draft` to `HackerNewsConfig`

**File**: `crates/vox-publisher/src/types.rs`

Add to `HackerNewsConfig`:
```rust
/// First-comment text to display in the manual-assist output.
#[serde(default)]
pub comment_draft: Option<String>,
```

archived_date: 2026-04-18
---

#### T-010: Add Discord Content-Length Validation

**File**: `crates/vox-publisher/src/adapters/discord.rs`

After building `message_content` (line 17) and before building the payload, add:
```rust
const DISCORD_CONTENT_MAX: usize = 2000;
if message_content.chars().count() > DISCORD_CONTENT_MAX {
    return Err(anyhow!(
        "Discord content ({} chars) exceeds {DISCORD_CONTENT_MAX} char limit",
        message_content.chars().count()
    ));
}
```

---

#### T-011: Add Reddit 40,000-Char Selfpost Validation

**File**: `crates/vox-publisher/src/adapters/reddit.rs`

Add a constant (or add to `contract.rs`):
```rust
/// Reddit self-post body hard server limit (does not include link posts).
pub const REDDIT_SELFPOST_BODY_MAX: usize = 40_000;
```

In the submit function, before building the form, validate:
```rust
if let Some(text) = &reddit_cfg.text_override {
    if text.chars().count() > REDDIT_SELFPOST_BODY_MAX {
        return Err(anyhow!(
            "Reddit self-post body ({} chars) exceeds 40,000 char server limit",
            text.chars().count()
        ));
    }
}
```
Read `reddit.rs` fully to find the correct variable name for the text body before writing this.

archived_date: 2026-04-18
---

### Wave 1 — Credential Plumbing (Required Before Any New Dispatch Block)

---

#### T-012: Add New Credential Fields to `PublisherConfig`

**File**: `crates/vox-publisher/src/publisher/config.rs`

Add these fields to the `PublisherConfig` struct definition (lines 5–30):
```rust
// Bluesky (both exist in Clavis: VoxSocialBlueskyHandle, VoxSocialBlueskyPassword)
pub bluesky_handle: Option<String>,
pub bluesky_app_password: Option<String>,

// Mastodon — domain is resolved here; full URL computed as https://{domain}
// (Clavis: VoxSocialMastodonToken, VoxSocialMastodonDomain)
pub mastodon_access_token: Option<String>,
pub mastodon_instance_url: Option<String>,  // computed: "https://{domain}"

// LinkedIn — token already in Clavis: VoxSocialLinkedinAccessToken
pub linkedin_access_token: Option<String>,

// Discord resolves its own token internally — no field needed here.
// ORCID — complex 3-legged OAuth; do not add a single flat token here yet.
// See T-030 for the ORCID implementation design.
```

Add to `Default::default()` initializer (or cover via `..Default::default()`):
```rust
bluesky_handle: None,
bluesky_app_password: None,
mastodon_access_token: None,
mastodon_instance_url: None,
linkedin_access_token: None,
```

Add to `from_operator_environment` resolution block:
```rust
bluesky_handle: Self::syndication_secret(vox_clavis::SecretId::VoxSocialBlueskyHandle),
bluesky_app_password: Self::syndication_secret(vox_clavis::SecretId::VoxSocialBlueskyPassword),
mastodon_access_token: Self::syndication_secret(vox_clavis::SecretId::VoxSocialMastodonToken),
mastodon_instance_url: Self::syndication_secret(vox_clavis::SecretId::VoxSocialMastodonDomain)
    .map(|domain| format!("https://{}", domain.trim())),
linkedin_access_token: Self::syndication_secret(vox_clavis::SecretId::VoxSocialLinkedinAccessToken),
```

archived_date: 2026-04-18
---

#### T-013: Add Missing Channels to `switching.rs` Allowlist

**File**: `crates/vox-publisher/src/switching.rs`

Locate `apply_channel_allowlist` function. It currently handles 8 channels. Add after the last existing line in the function body:
```rust
if !has("bluesky") { item.syndication.bluesky = None; }
if !has("mastodon") { item.syndication.mastodon = None; }
if !has("linkedin") { item.syndication.linkedin = None; }
if !has("discord") { item.syndication.discord = None; }
```

**Verify field names** by checking `SyndicationConfig` in `types.rs` for the exact field names (`bluesky`, `mastodon`, `linkedin`, `discord`).

---

#### T-014: Add Missing Channels to `failed_channels` and `successful_channels`

**File**: `crates/vox-publisher/src/switching.rs`

In `failed_channels` function, after the last existing `maybe(...)` call:
```rust
maybe("bluesky",  &result.bluesky);
maybe("mastodon", &result.mastodon);
maybe("linkedin", &result.linkedin);
maybe("discord",  &result.discord);
```

Do the same in `successful_channels`. Read both functions to find the exact pattern being used and the name of the local closure before writing.

archived_date: 2026-04-18
---

#### T-015: Add Missing Channels to `outcome_for_channel`

**File**: `crates/vox-publisher/src/switching.rs`

In `outcome_for_channel`, add match arms before the `_ => return None` arm:
```rust
"bluesky"  => &result.bluesky,
"mastodon" => &result.mastodon,
"linkedin" => &result.linkedin,
"discord"  => &result.discord,
```

---

#### T-016: Add Missing Channels to Contract-Shape Expander

**File**: `crates/vox-publisher/src/switching.rs`

In `normalize_distribution_json_value_with_warnings`, find the `for key in [...]` loop and add: `"bluesky"`, `"mastodon"`, `"linkedin"`, `"discord"` to the key array.

Also check if `channel_allows_empty_payload` (if it exists) should list `"discord"` — Discord only needs the webhook URL and uses `item.title` as the fallback message content.

archived_date: 2026-04-18
---

#### T-017: Create `syndication_events` DB Table

**Crate**: `vox-db`

Run `Get-ChildItem -Path crates/vox-db -Filter "*.sql" -Recurse | Sort-Object Name` to find the migration file naming convention before creating a new one.

**Migration SQL**:
```sql
CREATE TABLE IF NOT EXISTS syndication_events (
    id               TEXT    PRIMARY KEY,
    publication_id   TEXT    NOT NULL,
    channel          TEXT    NOT NULL,
    outcome          TEXT    NOT NULL,
    external_id      TEXT,
    attempt_number   INTEGER NOT NULL DEFAULT 1,
    retryable        INTEGER NOT NULL DEFAULT 0,
    attempted_at     TEXT    NOT NULL,
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_syndication_events_pub
    ON syndication_events (publication_id);
CREATE INDEX IF NOT EXISTS idx_syndication_events_channel
    ON syndication_events (channel, attempted_at DESC);
```

Do NOT add `researchgate` as a channel in this table — it has no API and its state is tracked as `researchgate_doi_queued` in `SyndicationResult`.

---

#### T-018: Add `researchgate_doi_queued` to `SyndicationResult`

**File**: `crates/vox-publisher/src/syndication_outcome.rs`

Add after line 44 (after `discord` field), before `decision_reasons`:
```rust
/// True when a Zenodo DOI was minted, which triggers ResearchGate to ingest
/// the record automatically within 3–14 days via DOI/CrossRef feeds.
/// This is NOT a channel outcome — ResearchGate has no public API.
/// Author must manually confirm authorship at researchgate.net after DOI appears.
#[serde(default)]
pub researchgate_doi_queued: bool,
```

Also add `&self.researchgate_doi_queued` to neither `has_failures` (bool isn't a ChannelOutcome) nor `all_enabled_channels_succeeded`. It is informational only.

archived_date: 2026-04-18
---

### Wave 2 — Mastodon Implementation

---

#### T-019: Implement Mastodon Adapter

**File**: `crates/vox-publisher/src/adapters/mastodon.rs` (replace the 14-line stub entirely)

**Verified API facts** (2026-04-13):
- Endpoint: `POST https://{instance}/api/v1/statuses`
- Auth: `Authorization: Bearer {access_token}`
- Content-Type: `application/json` (accepted equally with form-encoded — use JSON for clarity)
- Status max: 500 chars default (use 480 as safe limit to leave room for link)
- Response: `{"id": "...", "url": "...", ...}`
- Rate limit: 300 req / 5 minutes

```rust
use crate::types::{MastodonConfig, UnifiedNewsItem};
use crate::PublisherConfig;
use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const MASTODON_STATUS_MAX: usize = 500;
const MASTODON_STATUS_SAFE: usize = 480;

#[derive(Serialize)]
struct StatusRequest<'a> {
    status: String,
    visibility: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    spoiler_text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
    /// CW/sensitive media flag. Separate from spoiler_text.
    sensitive: bool,
}

#[derive(Deserialize)]
struct StatusResponse {
    id: String,
    url: Option<String>,
}

pub async fn post(
    _publisher_cfg: &PublisherConfig,
    instance_url: &str,
    access_token: &str,
    item: &UnifiedNewsItem,
    cfg: &MastodonConfig,
    dry_run: bool,
) -> Result<String> {
    if dry_run {
        return Ok(format!("dry-run-mastodon-{}", item.id));
    }

    let instance = instance_url.trim().trim_end_matches('/');
    if instance.is_empty() {
        return Err(anyhow!("Mastodon instance URL must not be empty"));
    }

    let status_text = cfg.status.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| {
            let body = item.content_markdown.trim();
            if body.chars().count() <= MASTODON_STATUS_SAFE {
                body.to_string()
            } else {
                let t: String = body.chars().take(MASTODON_STATUS_SAFE - 3).collect();
                format!("{}...", t)
            }
        });

    if status_text.chars().count() > MASTODON_STATUS_MAX {
        return Err(anyhow!(
            "Mastodon status text ({} chars) exceeds {MASTODON_STATUS_MAX} char limit",
            status_text.chars().count()
        ));
    }

    let req = StatusRequest {
        status: status_text,
        visibility: cfg.visibility.as_str(),
        spoiler_text: cfg.spoiler_text.as_deref().filter(|s| !s.is_empty()),
        language: cfg.language.as_deref().filter(|s| !s.is_empty()),
        sensitive: cfg.sensitive,
    };

    let endpoint = format!("{}/api/v1/statuses", instance);
    let res = Client::new()
        .post(&endpoint)
        .bearer_auth(access_token)
        .json(&req)
        .send()
        .await
        .context("mastodon status POST")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(anyhow!("Mastodon POST failed ({status}): {body}"));
    }

    let parsed: StatusResponse = res.json().await.context("mastodon response parse")?;
    let url = parsed.url
        .unwrap_or_else(|| format!("{}/statuses/{}", instance, parsed.id));
    Ok(url)
}
```

**Key adapter call signature change**: added `instance_url: &str` and `access_token: &str` as explicit parameters (2nd and 3rd). The dispatch block must pass `self.config.mastodon_instance_url.as_deref()` and `self.config.mastodon_access_token.as_deref()`.

archived_date: 2026-04-18
---

#### T-020: Wire Mastodon into `publish_all`

**File**: `crates/vox-publisher/src/publisher/mod.rs`

Add a new dispatch block after the crates_io block (after line 600). Follow the **exact** pattern of the Twitter dispatch block (lines 245–284). Key differences: use `mastodon` as the channel name, call `adapters::mastodon::post` with instance_url and access_token:

```rust
if let Some(mastodon_cfg) = &item.syndication.mastodon {
    if let Some(reason) = policy_block_reason(item, "mastodon", &self.config) {
        result.mastodon = ChannelOutcome::Disabled;
        result.decision_reasons.insert("mastodon".to_string(), reason);
    } else if is_dry_run {
        info!(
            "[DRY RUN] Would post to Mastodon instance {:?}",
            mastodon_cfg.instance_url_override
                .as_deref()
                .or(self.config.mastodon_instance_url.as_deref())
                .unwrap_or("(from VoxSocialMastodonDomain)")
        );
        result.mastodon = ChannelOutcome::DryRun {
            external_id: Some(format!("dry-run-mastodon-{}", item.id)),
        };
    } else {
        let instance = mastodon_cfg.instance_url_override
            .as_deref()
            .or(self.config.mastodon_instance_url.as_deref());
        match (instance, self.config.mastodon_access_token.as_deref()) {
            (Some(inst), Some(token)) => {
                match social_retry::run_with_retries(social_retry_budget, || {
                    adapters::mastodon::post(
                        &self.config,
                        inst,
                        token,
                        item,
                        mastodon_cfg,
                        false,
                    )
                })
                .await
                {
                    Ok(url) => {
                        result.mastodon = ChannelOutcome::Success {
                            external_id: Some(url),
                        };
                        info!("Posted to Mastodon.");
                    }
                    Err(e) => {
                        result.mastodon = ChannelOutcome::Failed {
                            code: "mastodon_post_failed".to_string(),
                            message: e.to_string(),
                            retryable: true,
                        };
                    }
                }
            }
            _ => {
                warn!("Mastodon config present but instance URL or token missing (VoxSocialMastodonDomain / VoxSocialMastodonToken).");
                result.mastodon = ChannelOutcome::Failed {
                    code: "missing_mastodon_credentials".to_string(),
                    message: "Mastodon requires VoxSocialMastodonDomain and VoxSocialMastodonToken.".to_string(),
                    retryable: false,
                };
            }
        }
    }
}
```

---

#### T-021: Wire Discord into `publish_all`

**File**: `crates/vox-publisher/src/publisher/mod.rs`

> [!IMPORTANT]
> Discord resolves its webhook URL from Clavis INTERNALLY (`VoxSocialDiscordWebhook`).
> There is no credential field needed in `PublisherConfig` for Discord.
> The dispatch block signature: `adapters::discord::post(&self.config, item, discord_cfg, is_dry_run)`

```rust
if let Some(discord_cfg) = &item.syndication.discord {
    if let Some(reason) = policy_block_reason(item, "discord", &self.config) {
        result.discord = ChannelOutcome::Disabled;
        result.decision_reasons.insert("discord".to_string(), reason);
    } else {
        match social_retry::run_with_retries(social_retry_budget, || {
            adapters::discord::post(&self.config, item, discord_cfg, is_dry_run)
        })
        .await
        {
            Ok(id) => {
                result.discord = ChannelOutcome::Success { external_id: Some(id) };
                info!("Posted to Discord.");
            }
            Err(e) => {
                result.discord = ChannelOutcome::Failed {
                    code: "discord_post_failed".to_string(),
                    message: e.to_string(),
                    retryable: true,
                };
            }
        }
    }
}
```

Note: Discord's `post()` handles dry_run internally (line 34 of `discord.rs`: `if dry_run { return Ok(...) }`). So we pass `is_dry_run` directly and let the adapter handle it, rather than an outer `else if is_dry_run` guard. This is different from the Mastodon pattern — Discord IS already armed with its own dry_run check.

archived_date: 2026-04-18
---

#### T-022: Wire Bluesky into `publish_all`

**File**: `crates/vox-publisher/src/publisher/mod.rs`

**Only implement AFTER T-001 and T-002 are merged and verified.** A broken adapter being dispatched will silently fail on every run.

```rust
if let Some(bluesky_cfg) = &item.syndication.bluesky {
    if let Some(reason) = policy_block_reason(item, "bluesky", &self.config) {
        result.bluesky = ChannelOutcome::Disabled;
        result.decision_reasons.insert("bluesky".to_string(), reason);
    } else if is_dry_run {
        info!("[DRY RUN] Would post to Bluesky PDS {}", bluesky_cfg.pds_url);
        result.bluesky = ChannelOutcome::DryRun {
            external_id: Some(format!("dry-run-bluesky-{}", item.id)),
        };
    } else if let (Some(handle), Some(password)) = (
        self.config.bluesky_handle.as_deref(),
        self.config.bluesky_app_password.as_deref(),
    ) {
        match social_retry::run_with_retries(social_retry_budget, || {
            adapters::bluesky::post(
                &self.config,
                handle,
                password,
                bluesky_cfg.pds_url.as_str(),
                item,
                bluesky_cfg,
                false, // dry_run already checked above
            )
        })
        .await
        {
            Ok(url) => {
                result.bluesky = ChannelOutcome::Success { external_id: Some(url) };
                info!("Posted to Bluesky.");
            }
            Err(e) => {
                result.bluesky = ChannelOutcome::Failed {
                    code: "bluesky_post_failed".to_string(),
                    message: e.to_string(),
                    retryable: true,
                };
            }
        }
    } else {
        warn!("Bluesky config present but handle or app password missing.");
        result.bluesky = ChannelOutcome::Failed {
            code: "missing_bluesky_credentials".to_string(),
            message: "Bluesky requires VoxSocialBlueskyHandle and VoxSocialBlueskyPassword.".to_string(),
            retryable: false,
        };
    }
}
```

---

### Wave 3 — Bluesky Hardening

archived_date: 2026-04-18
---

#### T-023: Bluesky Grapheme-Cluster Count Validation

**File**: `crates/vox-publisher/src/adapters/bluesky.rs`

The AT Protocol enforces 300 **grapheme clusters** (not `char` count or byte count). Emoji like 🏳️‍🌈 count as 1 grapheme cluster but multiple code points.

First check workspace `Cargo.toml` to see if `unicode-segmentation` is already a workspace dependency:
```powershell
Select-String -Path "Cargo.toml" -Pattern "unicode-segmentation"
```

If not present, add to `[workspace.dependencies]`. Add the crate dep in `crates/vox-publisher/Cargo.toml` as `unicode-segmentation.workspace = true`.

In the adapter, after deriving `text`:
```rust
use unicode_segmentation::UnicodeSegmentation;
const BLUESKY_GRAPHEME_MAX: usize = 300;
let cluster_count = text.graphemes(true).count();
if cluster_count > BLUESKY_GRAPHEME_MAX {
    return Err(anyhow!(
        "Bluesky post exceeds 300 grapheme cluster limit ({cluster_count} clusters)"
    ));
}
```

---

#### T-024: Bluesky Session Caching (Avoid Per-Post `createSession`)

**File**: `crates/vox-publisher/src/adapters/bluesky.rs` + a new cache type

`createSession` costs 30 rate-limit points per 5 minutes (max 30/5min). Processing N articles in one run without caching will hit this limit at N ≥ 1.

Design: add a `BlueskySessionCache` struct with a `tokio::sync::Mutex<Option<CachedSession>>`. Store it in `Publisher` (or as a lazy_static/OnceLock per PDS). On each call:
1. Try to read cached session — if `access_jwt_expires > now + 5min`, use it.
2. Otherwise call `refreshSession` with `refresh_jwt`.
3. Only call `createSession` if refresh fails or no cache.

This is an architectural change and should be done carefully after Wave 2 is stable.

archived_date: 2026-04-18
---

### Wave 4 — LinkedIn Stub Hardening

#### T-025: Update LinkedIn Stub Error Message

**File**: `crates/vox-publisher/src/adapters/linkedin.rs`

Update the stub to include accurate blocker information:
```rust
Err(anyhow!(
    "LinkedIn adapter not yet implemented. Blockers: \
     (1) LinkedIn app review required (w_member_social scope). \
     (2) Posts API endpoint: POST https://api.linkedin.com/rest/posts (NOT /v2/posts). \
     (3) Required header: LinkedIn-Version: YYYYMM (date-versioned). \
     (4) Required field: author_urn (urn:li:person:{{id}} or urn:li:organization:{{id}}). \
     (5) 60-day access token expiry management not implemented. \
     See: docs/src/architecture/scientia-publication-endpoints-research-2026.md §3.6"
))
```

---

### Wave 5 — ORCID Scholarly Adapter

> [!WARNING]
> ORCID membership is required for write access. Before implementing,
> confirm that the Vox project has ORCID member organization status.
> Without it, the adapter will receive 403 on all POST requests.

#### T-026: Design ORCID Token Strategy

**This is a design task, not a code task.** ORCID write access requires per-user 3-legged OAuth. A system-level adapter token does not exist. Options:

1. **OAuth proxy**: An operator authenticates via ORCID, grants the ORCID app permission, and the resulting `access_token` is stored manually in Clavis as a personal token. This works for a single-researcher use case but does not scale.

2. **ORCID Public API + DOI redirect**: For read-only use, no credentials needed. For write, option 1 is required.

**Recommended approach for SCIENTIA**: Store the user-specific `access_token` as `VoxOrcidAccessToken` (a new SecretId, NOT the same as `VoxOrcidClientId`/`VoxOrcidClientSecret`). This token is obtained manually via the ORCID OAuth flow using the client credentials.

Add `VoxOrcidAccessToken` to `ids.rs` after confirming it does not already exist. `VoxOrcidClientId` and `VoxOrcidClientSecret` already exist (for the OAuth client, not the user session).

archived_date: 2026-04-18
---

#### T-027: Implement ORCID Adapter

**File**: Create `crates/vox-publisher/src/scholarly/orcid.rs`

**API facts** (2026-04-13, verified):
- Production: `POST https://api.orcid.org/v3.0/{orcid-id}/work`
- Sandbox: `POST https://api.sandbox.orcid.org/v3.0/{orcid-id}/work`
- Auth: `Authorization: Bearer {access_token}` (user-level token, NOT client token)
- Content-Type: `application/vnd.orcid+json`
- Accept: `application/vnd.orcid+json`
- Returns: `put-code` (integer) in response body for future updates
- DO NOT re-POST the same DOI without reading existing works first — creates duplicates

**Minimal JSON body** (required fields only):
```json
{
  "title": { "title": { "value": "Your Paper Title" } },
  "type": "preprint",
  "external-ids": {
    "external-id": [{
      "external-id-type": "doi",
      "external-id-value": "10.xxxx/yyyy",
      "external-id-url": { "value": "https://doi.org/10.xxxx/yyyy" },
      "external-id-relationship": "self"
    }]
  }
}
```

**Add `OrcidConfig`** to `types.rs`:
```rust
pub struct OrcidConfig {
    /// ORCID iD in hyphenated form: "0000-0002-1825-0097".
    pub orcid_id: String,
    /// DOI of the work to register. Required.
    /// Format: "10.xxxx/yyyy" (without https://doi.org/ prefix).
    pub doi: String,
    /// Work type. Use "preprint" for SCIENTIA preprints.
    /// Valid: "journal-article" | "preprint" | "conference-paper" | "dataset" | etc.
    #[serde(default = "orcid_default_work_type")]
    pub work_type: String,
    /// Use ORCID sandbox endpoint. Default: false.
    #[serde(default)]
    pub sandbox: bool,
    /// After first successful POST, store the returned put-code here for future updates.
    #[serde(default)]
    pub put_code: Option<u64>,
}
fn orcid_default_work_type() -> String { "preprint".to_string() }
```

Add `orcid: Option<OrcidConfig>` to `SyndicationConfig` in `types.rs`.
Add `orcid: ChannelOutcome,` to `SyndicationResult` in `syndication_outcome.rs`.
Register ORCID in all four `switching.rs` functions.
Add `orcid_access_token: Option<String>` to `PublisherConfig`.
Add dispatch block to `publish_all` (scholarly path, not social).

---

### Wave 6 — Billing and Compliance Gating

archived_date: 2026-04-18
---

#### T-028: Add Twitter Billing Gate to `vox clavis doctor`

Required SecretId: Add `VoxTwitterBillingVerified` to `ids.rs` first (verify it doesn't exist — grep for "Twitter" in ids.rs).

Doctor check output example:
```
Twitter: ⚠️  BILLING NOT VERIFIED
  Write access requires paid X/Twitter API plan (≥$100/month, Feb 2026).
  Set VOX_TWITTER_BILLING_VERIFIED=1 after confirming active paid plan.
  Without this, posts will return HTTP 403 Forbidden.
```

Find the doctor command implementation (likely under `crates/vox-cli/` in a doctor-related file — run `Get-ChildItem -Path crates/vox-cli -Filter "*.rs" -Recurse | Select-String "doctor"` to locate it).

---

#### T-029: Add YouTube Compliance Audit Gate

Required SecretId: Add `VoxYouTubeComplianceAuditVerified` to `ids.rs`.

Doctor check + in `publisher/mod.rs` YouTube dispatch: if `privacy_status == "public"` and `VoxYouTubeComplianceAuditVerified != "1"`, downgrade to `"private"` and record in `decision_reasons`:
```rust
result.decision_reasons.insert(
    "youtube_privacy_downgrade".to_string(),
    "public→private: compliance audit not verified (VOX_YOUTUBE_COMPLIANCE_AUDIT_VERIFIED)".to_string(),
);
```

archived_date: 2026-04-18
---

### Wave 7 — Scholarly Record Persistence

---

#### T-030: Add `ScholarlyPublicationRecord` to `vox-db`

**Crate**: `vox-db` — add a new migration.

```sql
CREATE TABLE IF NOT EXISTS scholarly_publication_records (
    id                    TEXT PRIMARY KEY,
    publication_id        TEXT NOT NULL UNIQUE,
    doi                   TEXT,
    zenodo_deposit_id     TEXT,
    zenodo_doi            TEXT,
    orcid_put_code        INTEGER,        -- returned integer from ORCID POST
    figshare_article_id   TEXT,
    arxiv_submission_id   TEXT,
    openreview_forum_id   TEXT,
    crossref_deposit_id   TEXT,
    researchgate_confirmed INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    -- status: 'draft' | 'deposited' | 'published' | 'retracted'
    published_at          TEXT,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_scholarly_pub_doi
    ON scholarly_publication_records (doi) WHERE doi IS NOT NULL;
```

archived_date: 2026-04-18
---

### Wave 8 — arXiv Export Preflight

#### T-031: Implement arXiv Format Preflight Profile

**File**: `crates/vox-publisher/src/publication_preflight/` — list the directory first:
```powershell
Get-ChildItem -Path "crates/vox-publisher/src/publication_preflight" -Recurse | Select-Object Name, Length
```

arXiv submission rules (verified 2026-04-13):
- Abstract ≤ 1,920 chars (enforced by arXiv moderation)
- Title ≤ ~100 chars (soft cap)
- Endorsement required for new categories — institutional email not sufficient (Jan 2026 tightening)
- AI content must be disclosed (Feb 2026 policy)

Add `PreflightProfile::ArXiv` variant that checks these and returns structured `Vec<PreflightWarning>`. Never block silently.

---

### Deferred / Do-Not-Implement

#### DEFERRED: LinkedIn Full Implementation

Blocked by:
1. LinkedIn App Review (separate organizational process, 2–4 weeks)
2. `author_urn` identity decision (personal vs organization page)
3. 60-day access token refresh implementation

Do not attempt until blockers 1 and 2 are resolved at the organizational level.

#### DEFERRED: Figshare

Lower priority than ORCID. Implement after T-027 (ORCID) is stable.

#### DEFERRED: Crossref XML Deposit

Blocked by Crossref membership. The XML deposit format is also not currently generated by `crossref_metadata.rs` (that file produces JSON for citation use, not for deposit). Both the organizational blocker and the format mismatch must be resolved before implementation.

#### DO NOT IMPLEMENT (Permanent)

| Platform | Reason |
|---|---|
| ResearchGate | No API. ToS prohibits automation. Passive via DOI. |
| Academia.edu | No API. ToS prohibits automation. |
| Google Scholar | No write API. Passive indexing only. |
| Semantic Scholar | Read-only API only. |
| Web of Science | Subscription-gated, no submission API. |
| Scopus | Subscription-gated, no submission API. |

If you encounter an issue, PR, or request to add any of the above as an active-push adapter, reject it and cite this document.

archived_date: 2026-04-18
---

## Verification Steps by Wave

### After Wave 0 (T-001 to T-011):
```powershell
cargo check -p vox-publisher
cargo test -p vox-publisher bluesky
```
Verify field rename via tests. Check `opencollective.rs` manually for header.

### After Wave 1 (T-012 to T-018):
```powershell
cargo check -p vox-clavis
vox ci clavis-parity
vox ci secret-env-guard
cargo check -p vox-publisher
Select-String -Path "crates/vox-publisher/src/switching.rs" -Pattern "bluesky|mastodon|linkedin|discord"
```
Expected: 4+ matches per pattern across all four switching functions.

### After Wave 2 (T-019 to T-022):
```powershell
cargo check -p vox-publisher --all-features
cargo test -p vox-publisher mastodon
cargo test -p vox-publisher discord
```
Dry-run integration test:
```powershell
vox db publication-publish --id test-mastodon --dry-run
```
Expected: `DryRun` outcome for mastodon and discord.

### After Each Wave:
```powershell
vox stub-check --path crates/vox-publisher
```
Expected: no TOESTUB violations in non-test code.

---

## File Change Summary

| File | Changes | Tasks |
|---|---|---|
| `adapters/bluesky.rs` | JWT field rename, XRPC URL fix, dry_run, pds_url param | T-001, T-002, T-003 |
| `adapters/mastodon.rs` | Full implementation (replace stub) | T-019 |
| `adapters/discord.rs` | Content-length validation | T-010 |
| `adapters/opencollective.rs` | Auth header, makePublicOn | T-005, T-006 |
| `adapters/reddit.rs` | 40k char validation | T-011 |
| `adapters/linkedin.rs` | Stub error message | T-025 |
| [NEW] `scholarly/orcid.rs` | Full ORCID adapter | T-027 |
| `switching.rs` | Add 4 channels to all registry functions | T-013–T-016 |
| `types.rs` | BlueskyConfig.pds_url, MastodonConfig fields, LinkedInConfig fields, HNConfig.comment_draft, OrcidConfig | T-004, T-007, T-008, T-009, T-027 |
| `syndication_outcome.rs` | `researchgate_doi_queued`, `orcid: ChannelOutcome` | T-018, T-027 |
| `publisher/mod.rs` | Mastodon/Discord/Bluesky dispatch blocks | T-020, T-021, T-022 |
| `publisher/config.rs` | bluesky/mastodon/linkedin credential fields | T-012 |
| `contract.rs` | DISCORD_CONTENT_MAX, REDDIT_SELFPOST_BODY_MAX | T-010, T-011 |
| `crates/vox-clavis/src/spec/ids.rs` | VoxOrcidAccessToken, VoxTwitterBillingVerified, VoxYouTubeComplianceAuditVerified | T-026, T-028, T-029 |
| [DB migration] | `syndication_events` table, `scholarly_publication_records` table | T-017, T-030 |
| CLI doctor | Twitter billing + YouTube compliance checks | T-028, T-029 |
| `publication_preflight/` | arXiv profile | T-031 |

archived_date: 2026-04-18
---

*Implementation plan v2 — 2026-04-13. Critiqued against: `publisher/mod.rs` (605L), `publisher/config.rs` (198L), `adapters/discord.rs` (52L), `adapters/mastodon.rs` (14L), `adapters/bluesky.rs` (95L), `scholarly/zenodo.rs` (564L), `syndication_outcome.rs` (211L), `spec/ids.rs` (531L). Corrects 13 factual errors from v1. Removes 2 tasks already done (Zenodo audit/gate). Adds 5 tasks discovered during critique (C-001 through C-013).*


