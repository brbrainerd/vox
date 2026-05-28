---
title: "Scientia publication: what you type vs what the system derives"
description: "Per-surface and per-platform manual inputs versus derived fields for VoxGiantia publication."
category: "How-To Guides"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "HowTo"
---

# Scientia publication: operator inputs vs system-derived fields

Use this with [How-To: Publish Scientia findings](how-to-scientia-publication.md) and the [publication playbook](../reference/scientia-publication-playbook.md).

## Surfaces (same manifest, different entry points)

| Surface | You provide | System derives |
|--------|-------------|----------------|
| **CLI** `vox db publication-*` | Flags, paths, `publication_id`, approver id, optional `--channels` CSV | Digest (`content_sha3_256`), attempt rows, gate evaluation (dual approval + armed), worthiness score from default contract + manifest (for per-channel policy floors), optional **live** block via `VOX_SOCIAL_WORTHINESS_ENFORCE` / `VOX_SOCIAL_WORTHINESS_SCORE_MIN` |
| **MCP** `vox_scientia_publication_*` | Tool params (`publication_id`, `dry_run`, optional `channels`, `json`) | Same as CLI; MCP also merges orchestrator `[news].dry_run` and `publish_armed` with tool `dry_run` for the live gate; worthiness **live** enforcement follows `[news].worthiness_*` or the same `VOX_SOCIAL_WORTHINESS_*` env overrides |
| **Orchestrator** `NewsService` | Markdown under `news_dir`; `[orchestrator.news]` config | `UnifiedNewsItem` from file content; digest; worthiness score probe; DB upsert for manifest |

**Live publish gate (all surfaces):** two distinct digest-bound approvers in VoxDb, `publish_armed` (config and/or `VOX_NEWS_PUBLISH_ARMED`), no overriding dry-run on item + surface. CLI armed uses **env only**; MCP/orchestrator use **config OR env**.

If `syndication.distribution_policy.dry_run` is `true` in metadata, the runtime **forces** `syndication.dry_run` on (stricter than omitting the flag).

**Config precedence (MCP publication):** env vars read by `PublisherConfig::from_operator_environment` win over orchestrator TOML for Twitter chunk/suffix and API bases; orchestrator fills gaps only when env left those fields unset. Site URLs use `[news]` then `VOX_NEWS_SITE_BASE_URL` / `VOX_NEWS_RSS_FEED_PATH`. CLI publication uses contract defaults plus the same **news site env** overrides (no orchestrator TOML).

## Rough character budgets (typed by you vs derived)

Approximate **UTF-8 characters**; platforms may count code points differently. “You” = manifest fields + syndication overrides; “System” = truncation/summaries from `content_markdown` / title.

| Destination | You (typical) | System (typical) | Contract / env knobs |
|-------------|---------------|------------------|----------------------|
| **Body / long-form** | Full markdown (unbounded in DB; keep under ~50k chars pragmatically) | Digest hash, templates | — |
| **Twitter single** | Optional `short_text` (0–~240 if you set it) | Else derived summary capped by `TWITTER_TEXT_CHUNK_MAX` minus margin (`VOX_NEWS_TWITTER_TEXT_CHUNK_MAX`, `VOX_SOCIAL_TWITTER_SUMMARY_MARGIN_CHARS`) | `vox_publisher::contract` |
| **Reddit title** | Often implicit from item title | Clamped ~300 | `REDDIT_TITLE_MAX` |
| **Reddit self-post body** | Optional `text_override` | Derived summary cap | `VOX_SOCIAL_REDDIT_SELFPOST_SUMMARY_MAX` |
| **Hacker News** | `title_override` if set (~80) | Else title shortened | `HACKER_NEWS_TITLE_MAX` |
| **YouTube title** | Optional override (~100) | From item title | `YOUTUBE_TITLE_MAX` |
| **YouTube description** | Optional override | From body | `YOUTUBE_DESCRIPTION_MAX` |
| **GitHub release** | `repo`, tag, body fragments | Rendered from templates | — |
| **Open Collective** | `collective_slug` + privacy | Short text from markdown | — |

## Per-channel: typical manual burden

| Channel | You usually set | Derived / automatic |
|--------|-----------------|---------------------|
| **RSS** | Enable + site `base_url` / `feed_path` (config) | Feed XML rewrite paths from item body/title |
| **Twitter** | Optional `short_text`, `thread`; API token (Clavis / env) | Summary truncation using `twitter_text_chunk_max` and margin env |
| **GitHub** | `repo`, release/discussion fields | Release tag text from title/version patterns when using templates |
| **Open Collective** | `collective_slug`, privacy | GraphQL payload from markdown summary |
| **Reddit** | Subreddit, post kind, overrides | Title/body caps from contract env overrides |
| **Hacker News** | `manual_assist` mode (no official post API) | Assist text only; no automated submit |
| **YouTube** | `video_asset_ref` + OAuth secrets | Upload uses repo-root asset resolution; skips cleanly if asset missing |
| **crates.io** | Payload in contract only | **Not implemented:** runtime returns explicit dry-run / failure, never silent publish |

Scholarly submit: `VOX_SCHOLARLY_ADAPTER` — `local_ledger` (default, Codex-friendly ledger id) or `echo_ledger` (deterministic id, no external repo call; tests/CI). Unknown values **fail fast**.

## Metadata keys (DB / frontmatter)

Persist syndication policy under `metadata_json` as **`syndication`**, not a top-level `scientia_distribution` key. Optional **`topic_pack`** string merges topic-pack YAML. See `contracts/scientia/distribution.schema.json`.


