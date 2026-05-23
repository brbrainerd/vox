---
title: "Archive"
description: "Archive"
category: "Architecture SSOTs"
status: "legacy"
training_eligible: false
archived_date: 2026-04-18
---
# Archive: Research Q1 2026

> **Tombstone.** Files moved here were active architecture documents that:
> - Had no `status: current` frontmatter tag, AND
> - Had no linked `*-implementation-plan-*.md` after 90 days (audit item J.95 policy)
>
> **Do NOT read or ingest archived files when planning new features.**
> See `AGENTS.md §Archival Protocol`.

## Policy (from audit item G.69 + J.95)

Per the V0.5 audit, docs without `status: current` are candidates for archival.
The following workflow applies going forward:

1. New docs MUST include `status: research | current | roadmap | archived` in front-matter.
2. Any `*-research-*.md` that has not produced an `*-implementation-plan-*.md` within 90 days
   is moved here by the next quarterly review.
3. CI enforces: new commits to `docs/src/architecture/` must include a `status:` field.
   See `.github/workflows/ci.yml` → `doc-inventory verify`.

## Status of this directory

Currently empty. The Q1 2026 archival pass identified 276 candidate docs (out of 278 total
in `docs/src/architecture/`). Rather than blindly moving files — which would break cross-doc
links — the following triage was applied:

- Documents actively referenced by `architecture-index.md` or `research-index.md`: **retained in place**.
- Documents for retired surfaces (HTMX, Pico, TanStack virtual routes): **to be moved here** 
  when their content is confirmed to contain no live architectural decisions.
- The seven docs with ghost HTMX/Pico references are guarded by CI (G.73 gate).

Human review is required before bulk-moving docs. Automated archival will begin after
the CI `doc-inventory` tool gains the ability to detect stale → linked pairs automatically.


