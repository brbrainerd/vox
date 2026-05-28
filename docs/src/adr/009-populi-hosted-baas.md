---
title: "ADR 009: Hosted mens / BaaS (future scope)"
description: "Official documentation for ADR 009: Hosted mens / BaaS (future scope) for the Vox language. Detailed technical reference, architecture gu"
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 009: Hosted mens / BaaS (future scope)

## Status

Proposed / documentation-only — **no in-tree hosted control plane** in this milestone.

## Context

Self-hosted mens today uses:

- Optional **`VOX_MESH_TOKEN`** and **`VOX_MESH_SCOPE_ID`** for LAN/small-team isolation ([mens SSOT](../reference/populi.md)).
- HTTP control plane in-process (`vox populi serve`) or behind a TLS terminator ([ADR 008](008-populi-transport.md)).

Product demand may include a **managed** mens (discovery, quotas, org billing) without operators running their own control plane on the public internet.

## Decision (scoped)

1. **Default remains self-hosted**: `git clone` + default env does **not** connect to any remote mens.
2. **Future hosted offering** (if built) will use a **distinct origin** (e.g. `https://mens.<provider>/…`), **org- or project-scoped credentials** (not raw `VOX_MESH_TOKEN` file sharing), and **no cross-tenant node listing**.
3. **Client integration** stays in [`vox-populi`](../../../crates/vox-populi): HTTPS + bearer (or OAuth device flow) + **explicit** `VOX_MESH_CONTROL_ADDR` / hosted URL — never ambient multicast discovery in the default `vox` binary.
4. **OpenAPI** for the **local** API lives at [`contracts/populi/control-plane.openapi.yaml`](../../../contracts/populi/control-plane.openapi.yaml); a hosted product may extend with versioned paths under a separate spec revision.
5. **Org-bound scope:** hosted `scope_id` (or successor claim) is **issued per org/project**, not reusable across tenants; control-plane list APIs must enforce **authz on scope** server-side.
6. **OAuth / device flow (outline):** human operators obtain a short-lived token via standard OAuth2 authorization code or device-code grant against the provider’s IdP; the `vox` CLI stores refresh material in the OS secret store — **never** in repo dotfiles. Service accounts use client-credentials with **narrow** `mens:read` / `mens:write` style scopes.
7. **Forbidden:** listing or mutating nodes outside the caller’s tenant; using one tenant’s bearer against another org’s `scope_id`; logging bearer tokens or refresh tokens.

## Consequences

- Self-hosted and hosted meshes are **separate trust domains**; migrating workloads requires explicit re-enrollment and new credentials.
- Distributed training / remote execute remain **non-goals** until artifact staging, authz, and NCCL (or equivalent) are designed (see mens capability plan non-goals).
- **Stub:** [`PopuliHttpClient::for_hosted_control_plane`](../../../crates/vox-populi/src/http_client.rs) documents the intended entrypoint for HTTPS bases; behavior matches `new` until hosted auth plumbing lands.
- **Non-goal:** no in-tree account database, billing, or multi-tenant admin UI until product scope is explicit.

## Related

- [Mens SSOT](../reference/populi.md)
- [ADR 008 — Mens transport](008-populi-transport.md)


