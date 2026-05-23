---
title: "News syndication: incident patterns and mitigations"
description: "Searchable SSOT for automated outbound publishing failure modes and Vox-specific safety controls."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-05"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# News syndication: incident patterns and mitigations

Searchable SSOT for **why** automated outbound publishing fails in production and how Vox constrains it.

## Common failure modes (industry + API behavior)

1. **Wrong environment / credentials**  
   Tokens scoped to the wrong org, expired OAuth, or CI secrets injected into a job that was assumed to be dry-run only. Mitigation: separate config keys, default `dry_run = true`, and require explicit `publish_armed` + `VOX_NEWS_PUBLISH_ARMED` for live posts.

2. **Missing staging for write APIs**  
   Many social/write APIs (e.g. X posting) do not offer a full “sandbox” identical to production; validation is often **contract testing** (local HTTP mocks) plus dry-run. Mitigation: `vox-publisher` tests hit **local Axum mocks**; production paths stay behind gates.

3. **Retry / idempotency bugs**  
   Marking a post as “done” before all channels succeed causes skipped retries on some channels; marking too late causes duplicate posts. Mitigation: each run records `news_publish_attempts` with per-channel outcomes, and `published_news` is written only for successful **live** runs with no enabled-channel failures.

4. **GitHub releases trigger notifications**  
   GitHub documents that creating a release can trigger notifications; rapid writes can hit secondary rate limits. Mitigation: default research/release templates use **`draft: true`** for GitHub `Release`; prefer draft until human publish. See [GitHub REST: create a release](https://docs.github.com/en/rest/releases/releases#create-a-release) and [best practices for using the REST API](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api).

5. **Schema / feed regressions**  
   Invalid RSS breaks subscribers silently. Mitigation: validate `feed.xml` structure in CI where practical (e.g. W3C Feed Validator docs: [validator.w3.org/feed/docs](https://validator.w3.org/feed/docs/)); keep links and `pubDate` RFC-2822-shaped via `chrono`.

6. **Insufficient human gates**  
   Single-person publish from automation. Mitigation: **two distinct approvers** in `news_publish_approvals_v2` for the current `content_sha3_256` digest before live syndication (enforced in `NewsService`; legacy id-only approvals are migration fallback).

## Vox-specific controls (code pointers)

| Control | Location |
|--------|----------|
| Global + per-item dry run | `vox_publisher::Publisher::publish_all` |
| Recursive draft pickup | `vox_orchestrator::services::news::collect_news_markdown_paths` |
| Dual approval + armed gate | `vox_orchestrator::services::news::NewsService::tick` |
| Approval persistence | `vox_db::VoxDb::record_news_approval_for_digest`, `has_dual_news_approval_with_fallback` |
| MCP tools (no live by default) | `vox_mcp::tools::news_tools` |
| Canonical templates | `crates/vox-publisher/news-templates/*.md` |

## References

- Open Collective API direction (GraphQL v2): [Open Collective API](https://docs.opencollective.com/help/contributing/development/api) → `https://graphql-docs-v2.opencollective.com/`.
- Cross-cutting env vars: [env-vars.md](../reference/env-vars.md).


