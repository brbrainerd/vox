---
title: "Known Documentation Gaps & Backlog"
description: "Living checklist of documentation gaps, backlog items, and recently completed doc work for contributors."
category: "API Reference — Crates"
status: current
last_updated: "2026-05-08"

schema_type: "TechArticle"
---

# Known Documentation Gaps & Backlog

This is a living checklist for the Vox open source community and core contributors to track undocumented or under-documented language features.

## High Priority
- ~~[ ] Add deep dive for `workflow` and `activity` compilation phases~~ — deferred: `workflow` and `activity` keywords were removed from the public grammar in frontend interop Phase 4 (see [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md) §Phase 4); no longer part of public grammar.
- [ ] Document difference between `query` and `mutation` transactional boundaries natively
- [ ] Expand the `Codex` abstraction API reference 
- [ ] List all compiler auto-injected properties for `@table` types (`id`, `created_at`, `updated_at`)

## Medium Priority
- [ ] Explain the underlying generic instantiation (`<T>`) algorithm used by HIR logic
- [ ] Detail all `mcp.tool` options regarding rate limits and user confirmation schemas
- [ ] Add explicit HTTP request payload mapping examples for `@endpoint(kind: server)` endpoints

## Completed 
- [x] Standard library built-ins (completed 2026-04-06)
- [x] Correct `component` declaration syntax (completed 2026-04-06; `@island` retired 2026-05-03)
- [x] Example pipeline validation documentation (completed 2026-04-06)


