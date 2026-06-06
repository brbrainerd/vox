---
title: "Codex HTTP API"
description: "Official documentation for Codex HTTP API for the Vox language. Detailed technical reference, architecture guides, and implementation pat"
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Codex HTTP API

Rust implementation surfaces live in **`vox-db`** (Codex schema, readiness, store ops). There is **no** separate `vox-codex-api` workspace crate; operators integrate HTTP routers built on **`vox_db`** types (see OpenAPI below).

## SSOT

- **OpenAPI 3** — [`contracts/codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml) (validated by **`vox ci check-codex-ssot`**).

## Tests

- `cargo test -p vox-db` — integration tests under [`crates/vox-db/tests/`](../../../crates/vox-db/tests/) (e.g. `ops_codex_tests.rs`) exercise Codex HTTP / store behavior where applicable.

## Defaults

| Item | Value |
|------|--------|
| Bind | `VOX_DASH_HOST` (default `127.0.0.1`) + `VOX_DASH_PORT` (default `3847`) when a dashboard-compatible server is run |
| Readiness | `GET /ready` uses [`vox_db::evaluate_codex_api_readiness`](../../../crates/vox-db/src/codex_schema.rs) (baseline `schema_version` **1** + required tables + manifest digest) |

## Speech ingress (`/api/audio/*`)

OpenAPI paths **`GET /api/audio/status`**, **`POST /api/audio/transcribe`**, **`POST /api/audio/transcribe/upload`** are implemented by the speech ingress stack ([`crates/vox-speech`](../../../crates/vox-speech) library surfaces + thin HTTP adapters): Oratio STT on **paths under `VOX_ORATIO_WORKSPACE`** (or process CWD) or **multipart upload**. Same bind vars as the table above. This is separate from Codex CRUD routes but lives in the shared [`contracts/codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml) catalog for client codegen.

## Related

- [Environment variables (SSOT)](env-vars.md) — `VOX_DASH_*`, Codex DB envs
- [Codex BaaS scaffolding](../archive/research-2026-q1/codex-baas.md)
- [Codex vNext schema](../archive/research-2026-q1/codex-vnext-schema.md)
- [Nomenclature migration map](../archive/research-2026-q1/nomenclature-migration-map.md) — retired `vox-codex-api` name


