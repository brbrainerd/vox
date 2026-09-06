---
title: "Documentation governance"
description: "Authority map, taxonomy, status vocabulary, and maintenance rules for Vox documentation."
category: "Contributors"
status: "current"
sort_order: 10
training_eligible: true
training_rationale: "Defines how all docs are organized, which agents must understand to write compliant documentation."

schema_type: "TechArticle"
---

# Documentation governance

This page defines how Vox documentation is organized and how to keep it from drifting.

## Authority map

| Surface | Primary audience | Owns | Must not become |
| --- | --- | --- | --- |
| [`README.md`](../../../README.md) | evaluators, first-time visitors | short front door, quick start, tone, links into the book | a second FAQ or architecture dump |
| [`docs-astro`](../../../docs-astro/README.md) | site visitors | Starlight site entry + local preview (`pnpm dev` under `docs-astro/`) | a contributor policy page |
| [`docs/src/explanation/faq.md`](../explanation/faq.md) | readers and evaluators | common product and architecture questions | a troubleshooting runbook |
| [`docs/src/how-to/troubleshooting-faq.md`](../how-to/troubleshooting-faq.md) | operators and contributors | operational fixes and environment troubleshooting | the main public FAQ |
| [`AGENTS.md`](../../../AGENTS.md) | contributors and agents | required cross-tool contributor policy, secret-management entry point, short architecture pointers | the general table of contents for the whole repo or a tool-specific troubleshooting log |
| `docs/src/reference/` | readers and contributors | lookup material, contracts mirrored in prose, stable operational references | speculative planning or marketing copy |
| `docs/src/architecture/` | contributors | current architecture, authority maps, research, and roadmaps | quick-start or beginner onboarding |
| `docs/src/contributors/` | contributors | contributor hub, documentation governance, contributor-facing process guidance | public product marketing |
| [`docs/agents/`](../../agents/) | contributors and automation | inventories, governance, machine-oriented support docs | duplicated public documentation |
| [`docs/agents/*.json`](../../agents/) | contributors and automation | machine-readable IDE feature matrix, doc inventory, script registry | must not become hand-edited prose |
| [`contracts/`](../../../contracts/) | code and CI | machine-readable specs and schemas | long-form human explanation |

## Taxonomy

Folder placement communicates ownership. Frontmatter communicates how a page should appear in the book.

### Category vocabulary

Use one of these `category` values in frontmatter:

| `category` | Meaning |
| --- | --- |
| `Getting Started` | first-stop pages and front-door onboarding |
| `Tutorials` | guided learning |
| `How-To Guides` | goal-oriented instructions |
| `Concepts` | conceptual understanding |
| `Language Reference` | stable lookup information |
| `API Reference — Crates` | per-crate API surface |
| `Examples` | worked examples |
| `Architecture Decisions (ADRs)` | architecture decisions |
| `Architecture SSOTs` | current architecture, authority maps, research pages (sidebar-listed via frontmatter), roadmaps |
| `CI & Quality` | CI and quality-specific references |
| `Contributors` | contributor-facing governance and process docs |
| `Operations` | runbooks and operational catalogs |
| `archive` | tombstoned pages (excluded from the sidebar) |

These display labels are the enforced vocabulary — `VALID_CATEGORIES` in
`crates/vox-doc-pipeline/src/pipeline/lint.rs` matches them exactly, with no
alias normalisation. They must stay in sync with the `sections` array in
`contracts/documentation/docs-sidebar-section-order.v1.json`.

### Status vocabulary

Use `status` when the distinction matters to readers:

| `status` | Use for |
| --- | --- |
| `approved` | accepted ADR or policy text that is normative but not yet reflected everywhere in code/docs |
| `current` | documented behavior or process the repo actively relies on |
| `experimental` | implemented but intentionally unstable or gated |
| `legacy` | still present but not the preferred path |
| `research` | investigation, findings, or synthesis not equivalent to shipped behavior |
| `roadmap` | future-facing implementation plans |
| `deprecated` | retained only for migration or compatibility notice |

Do not use `status` to make aspirational pages sound shipped.

### Frontmatter starter template

Use this template for new pages so docs lint passes on first run:

```md
---
title: "Page title"
description: "One specific sentence about what this page covers."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
---
```

**Note on temporal metadata:** The `last_updated` field is automatically derived from the file's Git commit history by the documentation pipeline and AI search engine. You do *not* need to manually update dates in frontmatter. Manual dates are considered legacy and will be superseded by Git metadata.

Fast local lint loop:

- `cargo run -p vox-doc-pipeline -- --lint-only --paths architecture/my-page.md`
- `cargo run -p vox-doc-pipeline -- --lint-only --paths architecture/my-page.md --fix`

Authoring guardrail:

- Do not start a line with a single backtick in prose (for example `` `vox ...`` at line start). Use normal prose with inline code or a full triple-backtick fence.

## Documentation Reality Audit

For a sustained **doc vs code vs contract** triage loop (aspiration vs fulfillment, historicity, and prioritized backlog), use the [Documentation Reality Audit Program](docs-reality-audit-program.md). Machine-readable claims and findings live under `contracts/reports/docs-reality-audit/`; CI validates them via `vox ci docs-reality-audit verify` (also included in `vox ci ssot-drift`).

## Authority tiers (A-D)

Use one authority tier per documentation domain. The canonical registry is
[`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml).

| Tier | Meaning | Typical location | CI expectation |
| --- | --- | --- | --- |
| `A-spec` | normative machine-readable contract | `contracts/`, schema-backed registries | contract validator must pass |
| `B-canon` | one canonical human page for the domain | usually `docs/src/reference/` (or one ADR) | no second canon for same domain id |
| `C-generated` | code-derived docs | `*.generated.md` and include fragments | generation verify command must pass |
| `D-index` | navigation, index, compatibility stubs, research maps; do not maintain a committed research-index.md — Starlight sidebar is the D-index for architecture/research pages | `architecture`/`ci` pointers and index pages | must link to canon, not restate canonical behavior |

Rules:

- Do not label a page as "SSOT" unless it is the sole `B-canon` page for its domain id in the canonical map.
- `D-index` pages should summarize links only. If behavior text duplicates a `B-canon` page, remove it.

## Placement guide

When adding or moving a page:

1. If the source of truth is machine-readable, put the contract in `contracts/` and link to it from `docs/src/reference/`.
2. Register the domain in [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml) with `spec_paths`, one `canon_doc`, and any alias stubs.
3. If the subject is a communication protocol or transport boundary, make the machine-readable artifact discoverable from `contracts/index.yaml` and mirror it from one canonical `docs/src/reference/` page.
4. If the page teaches or explains the user-facing language, keep it in `docs/src/`.
5. If the page is mainly for contributors or automation, prefer `docs/src/contributors/` or `docs/agents/`.
6. If the page is research or planning, keep it under `docs/src/architecture/` and label it clearly with `status: research` or `status: roadmap`.
7. If a page exists only as a compatibility stub, make it a short redirect and avoid duplicating the canonical content.

## Claim policy

Forward-facing docs should describe the architecture that exists now.

Prefer:

- "Vox documents a compiler pipeline that generates Rust and TypeScript artifacts."
- "Mens currently defaults to code-oriented training lanes."
- "This page is research, not a claim that the capability is fully shipped."

Avoid:

- "Vox already does everything in this section automatically" unless the code path is current and documented.
- "Mens answers architecture questions" unless that retrieval or QA path is explicitly wired and tested.
- "SSOT" in titles when the page is only a convenience summary, pointer, or index.

## Maintenance protocol

Use this lightweight review matrix for high-drift surfaces:

| If you change | Also review |
| --- | --- |
| authority ownership, stubs, or canonical pathing | [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml), `vox ci check-docs-ssot`, and affected alias pages |
| `crates/vox-cli/src/**` command surface | [`docs/src/reference/cli.md`](../reference/cli.md), command-compliance docs, contributor references that mention the command |
| secret or env handling | [`AGENTS.md`](../../../AGENTS.md), [Secrets SSOT](../reference/secrets-ssot.md) |
| agent instruction layering or shell-discipline policy | [`AGENTS.md`](../../../AGENTS.md), [Agent instruction architecture](agent-instruction-architecture.md), and relevant tool-specific overlays such as `GEMINI.md` |
| doc structure, nav, or new pages | this page, [`docs/src/adr/002-diataxis-doc-architecture.md`](../adr/002-diataxis-doc-architecture.md), [`docs-astro sidebar`](../../../docs-astro/README.md) |
| architecture claims | [Doc-to-code acceptance checklist](../archive/research-2026-q1/doc-to-code-acceptance-checklist.md), relevant explanation/reference pages |
| contracts or schema-backed behavior | matching `contracts/` files and the mirrored reference pages |
| communication protocols, transport routes, or streaming semantics | [`contracts/communication/protocol-catalog.yaml`](../../../contracts/communication/protocol-catalog.yaml), [Communication protocols reference](../reference/communication-protocols.md), and the owning protocol page such as MCP / Populi / runtime docs |
| Mens training or corpus behavior | [Mens native training SSOT](../reference/mens-training.md), [Mens training data contract](../reference/mens-training-data-contract.md) |
| Codex `research_metrics`, mesh/cost telemetry env knobs, or telemetry trust boundaries | [Telemetry and research_metrics contract](../reference/telemetry-metric-contract.md), [env-vars](../reference/env-vars.md), [Telemetry trust SSOT](../architecture/telemetry-trust-ssot.md) |
| **`apps/editor/vox-vscode/`** (extension host, webview UI, Oratio/MCP wiring) | [`apps/editor/vox-vscode/README.md`](../../../apps/editor/vox-vscode/README.md), [VS Code to MCP compatibility](../reference/vscode-mcp-compat.md); speech capture / Oratio pages when capture or tool surfaces change |

## Review cadence

- Front door surfaces: review on every material product-language or contributor-experience change.
- Architecture and reference pages: review when the owning code path changes.
- Research and roadmap pages: keep their status current even if the implementation does not move.
- Contributor and governance pages: review whenever CI, inventory rules, or workflow expectations change.

## Related

- [Contributor hub](contributor-hub.md)
- [Doc-to-code acceptance checklist](../archive/research-2026-q1/doc-to-code-acceptance-checklist.md)
- [Architectural governance (TOESTUB)](../../agents/governance.md)
- [Doc inventory verifier](../reference/doc-inventory.md)

## Documentation Update Checklist

Before committing documentation to the repository, verify the following constraints:

1. **Syntax correctness**: All code snippets must parse and type-check cleanly. You must write raw, inline ````vox``` code blocks within the document. The `vox-doc-pipeline` will dynamically extract and validate all inline Vox blocks. Only use `{{#include}}` for legacy files, or use `// vox:skip` inside the block to explicitly disable dynamic validation for pseudo-code.
2. **Authority registration**: New canonical pages must be reflected in `contracts/documentation/canonical-map.v1.yaml`; aliases must remain link-only.
3. **Status marker**: Use `status` only when needed (`current`, `experimental`, `legacy`, `research`, `roadmap`, `deprecated`).
4. **Terminology**: Use established nomenclature (Codex vs Arca, Mens vs Populi, Islands vs Components).
5. **Navigation integrity**: If creating a user-facing document, update Starlight nav metadata (frontmatter `title` / `category` / `sort_order`) and run `vox-doc-pipeline --lint-only` or `pnpm --dir docs-astro build` as appropriate.
