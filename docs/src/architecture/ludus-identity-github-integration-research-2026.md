---
title: "Ludus Identity Federation & GitHub Integration"
description: "Research findings and architecture plan for decentralized Ludus profile storage and GitHub account linking."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Defines identity federation patterns and gamification integration for contributor workflows."
created: "2026-04-20"
---

# Ludus Identity Federation & GitHub Integration

## Executive Summary

This document captures the research findings and proposed architecture for:
1. **Where Ludus profiles currently live** and the gaps that block decentralized/cross-device play.
2. **How to link a Vox account to a GitHub identity** without introducing social media login complexity.
3. **How to award Ludus XP for GitHub contributions** (PRs, reviews, merges) using the same reward policy engine.

---

## 1. Current State: How Identity Works Today

### `local_user_id()` — the entire identity story right now

The current identity chain is completely local and device-scoped:

```
vox-config::paths::local_user_id()
  → env VOX_USER_ID           (explicit override)
  → env USERNAME / USER       (OS login name, e.g. "Owner")
  → "local-user"              (fallback)
```

`canonical_user_id()` in `vox-gamify/src/db/helpers.rs` wraps this and filters the sentinel `"user"`.

**Consequence:** Two devices with the same `$USERNAME` will collide if they ever share a DB. Two devices with different usernames running the same person will have completely separate profiles. There is no authentication whatsoever. The `user_id` is just the OS username string stored in SQLite.

### VoxDB storage — local SQLite only

All gamification state (profile, quests, companions, battles, policy snapshots, etc.) lives in a **local SQLite file** at `<AppData>/vox/vox.db`. The schema is versioned V5–V18. There is no sync, no remote write path, and no multi-device awareness today.

`vox-secrets` (`spec/ids.rs`) has `VoxDbUrl` and `VoxDbToken` secrets — these suggest Turso/libsql remote sync capability exists in `vox-db`, but it is **not plumbed into the gamification write paths yet**.

### vox-secrets has no GitHub secret today

A search of `spec/ids.rs` (553 lines) reveals no `GithubToken`, `GithubOauthClientId`, or equivalent. The codebase has `VoxGithubSha` (a build-time variable, not an OAuth credential). **GitHub OAuth is a greenfield addition.**

---

## 2. The Core Problem: Account Linking

To award XP for GitHub contributions, we need to answer:
> *"When a `pull_request` webhook fires for user `octocat`, which Vox Ludus profile does that belong to?"*

This is the classic **identity federation** problem. The options, from simplest to most complex:

### Option A: GitHub CLI Device Flow (Recommended)

GitHub's **OAuth Device Authorization Grant (RFC 8628)** is the canonical headless CLI authentication flow. It's exactly how `gh auth login` works.

**Flow:**
1. `vox ludus auth github` — Vox registers a GitHub App (or OAuth App).
2. CLI calls `POST https://github.com/login/device/code` with `client_id` and `scope=read:user`.
3. GitHub returns a `device_code`, `user_code`, and `verification_uri`.
4. CLI prints: `"Open https://github.com/login/device and enter: ABCD-1234"`.
5. CLI polls `POST https://github.com/login/oauth/access_token` with the `device_code`.
6. User authenticates in browser → polling succeeds → CLI receives `access_token`.
7. CLI calls `GET https://api.github.com/user` → gets `{ "id": 12345678, "login": "octocat" }`.
8. CLI stores: `github_numeric_id = 12345678`, `github_login = "octocat"`, `access_token` (via vox-secrets).
9. Writes link row: `(vox_user_id="Owner", github_id=12345678)` to VoxDB.

**Key security property:** Use the **stable numeric `id`** field, never the mutable `login` string, for all database foreign keys. GitHub usernames can be changed; numeric IDs cannot.

**No client secret needed** for a CLI device flow public client. The `client_id` is non-secret.

### Option B: Social Media / Web OAuth (Not Recommended for CLI)

A traditional browser redirect OAuth flow (GitHub, Google, Discord) requires:
- A web callback server (even if local `localhost:PORT`)
- Browser availability
- A client secret or PKCE verifier

This is appropriate for a web dashboard but is unnecessarily complex for a CLI-first tool. The Device Flow achieves the same result with a better UX for headless/server/CLI contexts.

### Option C: Manual Token Entry (Escape Hatch)

Allow `vox ludus auth github --token ghp_xxx` for power users, CI, or headless environments. Store the token via vox-secrets; resolve the numeric ID immediately by calling the `/user` API.

---

## 3. Proposed Identity Schema Extension

### New Secrets Required

Add to `crates/vox-secrets/src/spec/ids.rs`:
```rust
VoxGithubClientId,          // GitHub App / OAuth App client_id (non-secret, stored in code)
VoxGithubOauthToken,        // Per-user GitHub access token (device flow result)
VoxLudusRemoteUrl,          // Optional: remote Ludus sync endpoint (if mesh-based)
```

### New VoxDB Table: `vox_identities`

```sql
CREATE TABLE IF NOT EXISTS vox_identities (
    vox_user_id   TEXT NOT NULL,           -- local_user_id() value
    provider      TEXT NOT NULL,           -- 'github', 'google', 'discord', etc.
    provider_id   TEXT NOT NULL,           -- stable numeric ID from provider (e.g. GitHub user.id)
    provider_login TEXT,                   -- mutable display name (e.g. "octocat") — for display only
    access_token_ref TEXT,                 -- vox-secrets key reference (not the token itself)
    linked_at     INTEGER NOT NULL,
    PRIMARY KEY (vox_user_id, provider)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_vox_identities_provider
    ON vox_identities(provider, provider_id);
```

This is a **schema V19** migration in `crates/vox-gamify/src/schema.rs`.

### Decentralized Profile Sync

For cross-device profile carry-over, the cleanest path in the existing architecture:
- `VoxDbUrl` and `VoxDbToken` in vox-secrets already support **Turso/libsql embedded replicas** (local reads, remote writes).
- The `gamify_profiles` table (and all linked gamification tables) can be synced to a remote Turso database scoped to the user's GitHub numeric ID as the `user_id`.
- This is **additive** — local-only mode continues to work when `VoxDbUrl` is absent.

**Identity flow for sync:**
```
GitHub device flow → github_id → used as vox_user_id in remote DB
                                ↕ synced
local SQLite (vox.db) ← → remote Turso (keyed to github_id)
```

---

## 4. GitHub Contribution Scoring

### Event Ingestion Approaches

| Approach | Mechanism | Latency | Complexity |
|---|---|---|---|
| **GitHub App webhook** (server) | Push events to a Vox-owned endpoint | Real-time | High (needs server) |
| **Polling via `gh` CLI** | `vox ludus sync-github` fetches recent events | Minutes | Low |
| **GitHub Actions step** | Vox action fires after PR merge | Near-real-time | Medium |

**Recommended for V1:** GitHub Actions step + polling. A `vox-gamify-action` GitHub Action (composite or Docker) that fires on `pull_request.closed` (merged) and `pull_request_review.submitted`. The action calls `vox ludus award --event <type> --actor <github_id>` against a user's personal Turso remote. No server needed.

### Mapping GitHub Events → Ludus Policy Events

These map cleanly to **existing reward policy events** in `reward_policy.rs`:

| GitHub event | Ludus event | Base XP |
|---|---|---|
| PR merged (author) | `bug_fix` or `task_resolved` | 200 / 20 |
| PR merged with zero review round-trips | `build_clean_streak_3` proxy | 200 |
| PR review submitted (approved) | `peer_teach_session` (light weight) | 50 |
| PR review with comment (constructive) | `refactor` | 150 |
| Conflict resolved in PR | `conflict_resolved` | 100 + 10 Lumens |
| Security fix merged | `security_review_passed` | 1500 + 50 Lumens |
| Docs-only PR merged | `doc_added` | 28 per file |
| Test-only PR merged | `test_pass` | 55 |

Anti-grind protection applies automatically via the existing `PolicyEngine` — the same grind cap that prevents spamming `build_completed` applies equally to spamming trivial PRs.

### Attribution Safety

- **Always use GitHub numeric `id`**, never `login`, for attribution. Verified via `GET /user` with the stored token.
- GitHub webhook payloads post-2025 are lean — always enrich via REST API call on receipt.
- Rate limit: use authenticated requests (5000 req/hr per token) for polling.

---

## 5. Implementation Waves

### Wave 1 — Identity Foundation (Prerequisite)
- [ ] Add `VoxGithubClientId` and `VoxGithubOauthToken` to `spec/ids.rs`
- [ ] Register a GitHub OAuth App (or GitHub App) for Vox
- [ ] Implement `vox ludus auth github` using device flow (`reqwest` calls, vox-secrets storage)
- [ ] Add `vox_identities` table (schema V19) to `crates/vox-gamify/src/schema.rs`
- [ ] Resolve and store numeric GitHub user ID on successful auth

### Wave 2 — Remote Profile Sync
- [ ] Validate Turso embedded replica path (`VoxDbUrl` + `VoxDbToken`)
- [ ] Plumb remote sync into `db_util::get_db()` when `VoxDbUrl` is set
- [ ] Use `github_id` as the `user_id` namespace for remote profiles
- [ ] `vox ludus sync` — explicit sync command for testing
- [ ] `merge_default_profile_into_user` path for first-time remote linking

### Wave 3 — GitHub Contribution Rewards
- [ ] Polling: `vox ludus sync-github` — fetches last N events from `/users/{github_login}/events`
- [ ] Maps GitHub events to Ludus policy events via `award_github_event()`
- [ ] GitHub Actions composite action: `vox-gamify-award-action`
- [ ] Anti-grind: daily counter persistence ensures GitHub events respect same grind caps
- [ ] `vox ludus audit` shows GitHub-sourced events tagged with `source: github`

### Wave 4 — Leaderboard & Social
- [ ] Public Ludus leaderboard keyed on `github_login` (display) / `github_id` (SSOT)
- [ ] Collegium (team) scores sum from all member GitHub contributions
- [ ] Arena events triggered by org-level contribution milestones

---

## 6. Key Decisions

| Decision | Recommendation | Rationale |
|---|---|---|
| Auth flow | GitHub Device Flow (RFC 8628) | CLI-native, no client secret, no redirect server |
| Identity key | GitHub numeric `id` | Immutable; survives username changes |
| Social login | Deferred (device flow sufficient) | Avoids web server dependency for V1 |
| Remote sync | Turso/libsql (`VoxDbUrl`) | Already in vox-secrets spec; additive to local-only mode |
| Contribution scoring | Map to existing policy events | Reuses anti-grind, multipliers, quest engine |
| Webhook server | Deferred to Wave 4 | GitHub Actions + polling sufficient for V1-V3 |

---

## References

- GitHub Device Flow: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow
- GitHub stable user IDs: `GET /user` → `id` field (integer, never changes)
- Existing reward policy: `crates/vox-gamify/src/reward_policy.rs`
- Identity schema location: `crates/vox-gamify/src/schema.rs` (add as V19)
- Secrets spec: `crates/vox-secrets/src/spec/ids.rs`
- VoxDB remote: `VOX_DB_URL` / `VOX_DB_TOKEN` (vox-secrets `VoxDbUrl` / `VoxDbToken`)
