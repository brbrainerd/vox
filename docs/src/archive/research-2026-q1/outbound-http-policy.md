---
title: "Outbound HTTP policy (reqwest / vox-reqwest-defaults)"
description: "Single policy for building reqwest clients across Vox crates; migration order and exceptions."
category: "Architecture SSOTs"
status: "current"
sort_order: 0
last_updated: "2026-03-29"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Outbound HTTP policy

## SSOT crate

Use [`vox-reqwest-defaults`](../../../crates/vox-reqwest-defaults/src/lib.rs) for **default** outbound HTTP:

- `client_builder()` — sets user-agent (`vox-reqwest-defaults/<version>`), connect timeout (15s), idle pool timeout (90s).
- `client()` — builds from the builder with fallback to `reqwest::Client::new()`.

**Always start from `client_builder()`** when you need extra per-callsite options (e.g. longer overall timeout, custom UA):

```rust
vox_reqwest_defaults::client_builder()
    .timeout(Duration::from_secs(120))
    .user_agent("vox-review/0.1")
    .build()?
```

## Already aligned

Direct `reqwest::Client::builder()` in Rust sources should appear only inside `vox-reqwest-defaults` (the policy implementation).

Workspace crates that build outbound clients through `vox_reqwest_defaults::client_builder()` or `vox_reqwest_defaults::client()` include: `vox-runtime`, `vox-pm`, `vox-skills`, `vox-ludus`, `vox-populi` (transport + mens cloud), `vox-toestub`, `vox-mcp` (lifecycle + OpenClaw tools), `vox-orchestrator` (OpenRouter catalog), `vox-skills`, `vox-forge`, `vox-publisher` (Zenodo/OpenReview), `vox-webhook`, `vox-cli` (`generate`, `openclaw`, `ai/generate`, `ai/train`), and **generated app** `Cargo.toml` + dev-proxy in `vox-compiler` Rust emit.

## Migration priority (remaining ad-hoc `reqwest::Client::builder()`)

1. Prefer **`vox-reqwest-defaults`** for any new outbound HTTP; use plain `reqwest::Client::new()` only in tests or third-party snippets.
2. Third-party / forked templates outside this repo are exempt but should copy the same timeouts/UA policy when possible.

## Exceptions

- **Purposely minimal generated snapshots** may stay plain `reqwest` without `vox-reqwest-defaults`; the default Rust emit path includes `vox-reqwest-defaults` for dev-proxy HTTP. Document any alternate template in codegen comments.
- **Resilient multi-endpoint retry** — `vox-runtime` `resilient_http.rs` already documents why generic `backon` was not adopted; keep domain-specific retry there.

## Related

- [Language surface SSOT](language-surface-ssot.md)
- [OpenAPI contract SSOT](openapi-contract-ssot.md)


