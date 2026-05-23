---
title: "Codex BaaS scaffolding"
description: "Official documentation for Codex BaaS scaffolding for the Vox language. Detailed technical reference, architecture guides, and implementa"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Codex BaaS scaffolding

**Codex** is the API and metadata SSOT on **Turso**. Large blobs (exports, weights, attachments) use an **object storage** trait (S3/R2-compatible), not a second relational engine.

## Components (target)

1. **Codex API** — Query/mutation routes, auth/tenant boundary, schema digest sync.
2. **Reactive layer** — `codex_change_log` + subscriptions (SSE/WebSocket); included in baseline DDL (manifest fragment `v8`).
3. **Skills registry** — Backed by `skill_manifests` + CAS objects.
4. **Workflow runtime API** — Journal from `execution_log` / future dedicated workflow tables.
5. **Object storage adapter** — Metadata in Turso; bytes in R2/S3.

## Deployment

- **Compose hub (profiles, CI, Docker vs Podman):** [Deployment compose SSOT](../reference/deployment-compose.md).
- **Coolify / compose:** [`infra/coolify/docker-compose.yml`](../../../infra/coolify/docker-compose.yml) — template; set `VOX_DB_URL`, `VOX_DB_TOKEN`, `VOX_DB_PATH` (or embedded replica trio) per [ADR 004](../adr/004-codex-arca-turso-ssot.md).
- **Static frontends:** GitHub Pages or CDN; point to hosted Codex API.

## Environment (canonical)

| Variable | Role |
|----------|------|
| `VOX_DB_URL` | Turso / libSQL remote URL |
| `VOX_DB_TOKEN` | Auth token (env only) |
| `VOX_DB_PATH` | Local file or replica local path |

Optional object storage: `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET_NAME`, `R2_PUBLIC_URL` (documented when adapter lands).

## HTTP contract

- OpenAPI: [`contracts/codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml)
- Human reference: [Codex HTTP API](../reference/codex-http-api.md)

## Related

- [Environment variables (SSOT)](../reference/env-vars.md) — full `VOX_*` / Turso precedence
- [Codex vNext schema](codex-vnext-schema.md)
- Roadmap tasks: `.cursor/plans/vox_context_baas_deployment_roadmap.md` (internal backlog)


