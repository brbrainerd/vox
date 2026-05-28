---
title: "Doc-to-code acceptance checklist"
description: "High-value checks for keeping Vox documentation aligned with the current code, contracts, and contributor workflow."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Doc-to-code acceptance checklist

Use this before merging changes that affect user-visible behavior or agent guidance.

- [ ] Front-door docs still have distinct jobs: `README.md` (repo front door), `docs/src/index.md` (site landing page), `docs/src/explanation/faq.md` (product FAQ), `docs/src/how-to/troubleshooting-faq.md` (operational fixes), `AGENTS.md` (contributor/secret policy).
- [ ] [`docs/src/contributors/documentation-governance.md`](../contributors/documentation-governance.md) still matches the real repo layout when docs are moved or reclassified.
- [ ] `docs/src/reference/cli.md` matches `crates/vox-cli/src/lib.rs` `Cli` subcommands (dispatch lives there; `main.rs` only calls `run_vox_cli`).
- [ ] Capability or command-registry edits: **`contracts/capability/capability-registry.yaml`** stays valid vs schema; **`vox ci command-compliance`** and **`vox ci capability-sync --write`** (then verify) green; see [Capability registry SSOT](capability-registry-ssot.md).
- [ ] `AGENTS.md` Phase / crate bullets match workspace reality (`Cargo.toml` members / excludes).
- [ ] [orphan-surface-inventory.md](orphan-surface-inventory.md) updated if a crate or CLI surface changed.
- [ ] ADR 004 cross-links still valid if Codex/Turso boundaries changed.
- [ ] [Codex / Arca compatibility boundaries](codex-arca-compatibility-boundaries.md) updated if `DbConfig`, env vars, or migration rules changed.
- [ ] WebIR planning claims are synchronized across [ADR 012](../adr/012-internal-web-ir-strategy.md), [implementation blueprint](internal-web-ir-implementation-blueprint.md), and planning-meta Tier 1 docs (`01`, `05`, `08`, `10`) when gate language or ownership policy changes.
- [ ] “Current production path” statements in [Compiler Architecture](../explanation/expl-architecture.md) and [Compiler Lowering Phases](../explanation/expl-compiler-lowering.md) remain consistent with compiler code-path behavior (`codegen_ts/emitter.rs`, `codegen_ts/reactive.rs`) when docs are updated.
- [ ] **`cargo run -p vox-cli -- ci check-codex-ssot`** passes (or shim `scripts/check_codex_ssot.sh`).
- [ ] **`cargo run -p vox-cli -- ci check-docs-ssot`** passes (or shim `scripts/check_docs_ssot.sh`).
- [ ] **`cargo run -p vox-cli -- ci check-links`** passes for internal docs links.
- [ ] When **`apps/editor/vox-vscode/`** (extension host, webview, Oratio/MCP wiring) changes { **`npm run compile`** and **`npm run lint`** in **`vox-vscode`** pass; update [VS Code ↔ MCP compatibility](../reference/vscode-mcp-compat.md) and speech/Oratio docs ([speech capture](../reference/speech-capture-architecture.md), [Oratio SSOT](../reference/oratio-speech.md)) if tool names, activation, or capture contracts change.


