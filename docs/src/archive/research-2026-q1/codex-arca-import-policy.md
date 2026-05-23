---
title: "Codex, Arca, and Rust import policy"
description: "Official documentation for Codex, Arca, and Rust import policy for the Vox language. Detailed technical reference, architecture guides, a"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Codex, Arca, and Rust import policy

## Names

| Name | Meaning |
|------|---------|
| **Codex** | Product name for the persisted data API. |
| **`VoxDb`** | Stable Rust type for the database facade (`crates/vox-db`). |
| **`Codex`** (Rust) | Type alias for `VoxDb` in `vox_db` — same type. |
| **Arca** | Internal schema / CAS ownership in **`vox-pm`** (`CodeStore`). There is **no** `vox_arca` crate in this workspace. |
| **`vox-codex`** | Compatibility crate: `pub use vox_db::*`. New code should depend on **`vox-db`** directly. |

## Rules

1. Prefer **`vox_db::VoxDb`** (or `vox_db::Codex` alias) in signatures and new modules.
2. Do not introduce new dependencies on the `vox-codex` crate path unless bridging legacy tooling; migrate call sites to `vox-db` when touched.
3. Unwired CLI modules should import **`vox_pm::` / `vox_db::` / `vox_codex`** (shim) only — the historical `vox_arca*` crate names are not used in-tree. Staging crates (e.g. minimal **`vox-orchestrator`**) follow the same rule: do not link them from **`vox-cli`** until explicitly decided.

See [ADR 004](../adr/004-codex-arca-turso-ssot.md).


