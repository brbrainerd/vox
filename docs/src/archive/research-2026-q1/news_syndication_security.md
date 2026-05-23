---
title: "Unified News Syndication Security & Safety"
description: "Safety mechanisms and architectural constraints preventing accidental automated social media posts."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-05"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Unified News Syndication Security & Safety

This document outlines the safety mechanisms and architectural constraints designed to prevent accidental or malformed automated posts to social media (Twitter/X, GitHub, Open Collective) and RSS by the CI/CD pipeline and Vox Orchestrator agents.

**Related:** searchable incident patterns and external references — [news_syndication_incident_patterns.md](news_syndication_incident_patterns.md).

## 1. The Accidental Post Problem

Automated systems, especially agentic orchestration loops, can rapidly generate content. Without strict constraints, a misconfigured agent or a rogue loop could spam production feeds.

Common causes:

1. **Unbounded retries** — Failing to record completion, causing duplicate posts.
2. **Live credentials in “test” paths** — No dry-run or mock HTTP separation.
3. **Weak typing** — Invalid frontmatter slipping through.

## 2. Safety Mechanisms

### A. `dry_run` (global and per-item)

The `Publisher` honors `config.dry_run || item.syndication.dry_run`. When true:

- No HTTP writes to X, GitHub, or Open Collective.
- RSS file is not mutated (only “would update” logs).
- MCP `vox_news_test_syndicate` forces dry-run and omits tokens.

### B. Single source of truth (types + validation)

- **GitHub**: `GitHubPostType` (`Release` | `Discussion`) with serde-friendly YAML. `Discussion` requires `discussion_category`. `Release` uses `release_tag` (defaults to news id) and supports `draft`.
- **Defaults**: `vox_publisher::contract` centralizes site URL, feed path, and API bases.
- **Templates**: canonical Markdown lives under [`crates/vox-publisher/news-templates/`](../../../crates/vox-publisher/news-templates/) (embedded at compile time). Human-facing copies may exist under `docs/news/templates/` but the crate directory is authoritative when they differ.

### C. Maker–checker (two approvers) + “armed” gate

For **live** syndication (`!orchestrator.news.dry_run` and `!item.syndication.dry_run`):

1. **VoxDb** must be attached.
2. **`publication_approvals`** must contain **two distinct** `approver` values for the publication `id` + current content digest (`content_sha3_256`) (MCP: `vox_news_approve` and scientia publication tools).
3. **`publish_armed`** must be true in `[orchestrator.news]` **or** environment `VOX_NEWS_PUBLISH_ARMED=1` (see [env-vars.md](../reference/env-vars.md)).

If any check fails, `NewsService` skips the item (no publish, no `published_news` row).

### D. Idempotency (`published_news`)

Before work, `NewsService` skips items whose `published_news` row matches the **current** `content_sha3_256` (legacy NULL-digest rows still block until backfilled; digest-aware republish when body changes). Each publish attempt is recorded in **`publication_attempts`** (`news_publish_attempts` is legacy). After a successful **live** publish with no enabled-channel failures, `mark_news_published` stores the content digest plus **GitHub, Twitter, and Open Collective** ids, and the canonical publication state transitions to `published`.

### E. Discovery

`NewsService` walks `news_dir` **recursively** by default (`scan_recursive`), so `docs/news/drafts/*.md` is picked up once drafts are under the configured tree.

## 3. MCP tools

| Tool | Role |
|------|------|
| `vox_news_test_syndicate` | Parse + dry-run `publish_all` (no tokens). |
| `vox_news_draft_research` | Write `docs/news/drafts/{id}.md` from the embedded research template. |
| `vox_news_approve` | Append approval row (requires VoxDb). |
| `vox_news_approval_status` | Distinct approver count / dual flag. |
| `vox_news_simulate_publish_gate` | Explain blockers for live publish without posting. |

Strict JSON input schemas are registered in `vox-mcp` `input_schemas.rs`.

## 4. Tests (no production posts)

- `vox-publisher`: `dry_run_tests`, local HTTP mock tests for X + Open Collective.
- `vox-db`: `news_approval_tests` for dual approval and `published_news` column mapping.


