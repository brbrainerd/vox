---
title: "Telemetry remote sink specification"
description: "Normative behavior for optional vox telemetry upload: transport, auth, limits, signing roadmap."
category: "Architecture SSOTs"
last_updated: "2026-04-02"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Telemetry remote sink specification

This document is the **normative** wire and operator contract for **`vox telemetry upload`** ([`commands/telemetry.rs`](../../../crates/vox-cli/src/commands/telemetry.rs)), complementing [ADR 023: Optional telemetry remote upload](../adr/023-optional-telemetry-remote-upload.md).

## Transport

- **Method:** `POST` one JSON object per pending file (body = raw UTF-8 JSON, `Content-Type: application/json; charset=utf-8`).
- **URL:** HTTPS only in production; the CLI does not validate the scheme, but operators MUST use TLS at the edge.
- **Success:** HTTP **2xx** ⇒ the CLI **deletes** the local pending file (ack). Any other status ⇒ file is retained; the CLI logs a warning with truncated response body.
- **Ordering:** Files are uploaded in lexicographic order of filename (UUID-based names from `enqueue`).

## Authentication

- **Bearer (current):** If `VOX_TELEMETRY_UPLOAD_TOKEN` resolves to a non-empty value, the CLI sends `Authorization: Bearer <token>` (trimmed). If missing, no `Authorization` header is sent (public ingest must be a deliberate server choice).

## Rate limiting (client)

- **v1 behavior:** The CLI does not implement a client-side delay between POSTs. Operators SHOULD size batches with `export` / queue depth checks and SHOULD configure server-side rate limits.
- **Recommended server limits (documentation default):** steady **≤ 10 requests/s** per API key / IP with burst **≤ 30** unless the operator documents a different contract for their ingest.

## Payload signing (roadmap)

- **v1:** No request signing beyond TLS + optional bearer token.
- **Future:** When a shared signing secret is added to Clavis, the sink may require an `X-Vox-Telemetry-Signature` header (e.g. HMAC-SHA256 over `timestamp || '\n' || body` with a documented encoding). Until that `SecretId` exists and the CLI emits the header, ingest MUST NOT rely on signed bodies for authentication.

## Redaction

Operators MUST NOT enqueue secrets or raw PII into the spool. Classification and retention for Codex-backed metrics remain [telemetry-retention-sensitivity-ssot](telemetry-retention-sensitivity-ssot.md); this queue is a **separate** path for operator-chosen exports.

## Related

- [telemetry-trust-ssot](telemetry-trust-ssot.md)
- [env-vars SSOT](../reference/env-vars.md) — `VOX_TELEMETRY_*`


