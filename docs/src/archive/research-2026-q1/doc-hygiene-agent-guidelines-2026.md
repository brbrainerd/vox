---
title: "Documentation hygiene and AI-agent guidelines 2026"
description: "Guidelines and architecture for documentation hygiene intended to ensure discoverability and robust operation across all AI agents."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-16"
training_eligible: false
training_rationale: "Governs documentation quality for AI agents"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Documentation Hygiene & AI-Agent Guidelines 2026

## What every agent must know about Vox documentation

- Research and design documents must be stored in `docs/src/architecture/`, not scattered via individual agent IDEs.
- `archive/` or `docs/src/archive/` content is prohibited from being read to avoid propagating legacy patterns unless explicitly requested for historical context.
- `.vox` code snippets in `docs/src` must be properly sourced. Always try to `{{#include}}` from `examples/golden/` when adding new active source examples. In other cases, explicitly annotate the code block with `// vox:skip`.
- `AGENTS.md` sets the cross-tool standard at the repository root and rules cascade based on subdirectory `AGENTS.md` specs.
- Do NOT read secrets from environment variables; use `vox_clavis::resolve_secret(...)`.
- `TOESTUB` is active; skeleton components with `todo!()` or `unimplemented!()` will fail CI validation steps.

## Frontmatter specification table

All new documentation files within `docs/src/` require YAML frontmatter.

| Field | Required | Description |
|---|---|---|
| `title` | Yes | Proper casing and distinct. Used as page title and in mdBook SUMMARY. |
| `description` | Yes | A concise single-sentence summary of the document's content. |
| `category` | Yes | Logical grouping for the document (e.g., architecture, reference, tutorials). |
| `status` | Yes | Represents maturity or relevance: `current`, `deprecated`, `research`, or `roadmap`. |
| `last_updated` | Yes | ISO8601 YYYY-MM-DD date representation of the most recent file modify. |
| `training_eligible` | Yes | Set to true. Used by pipeline logic when ingesting docs into corpus loops. |
| `training_rationale`| Optional | Recommended for key architecture and SSOT pages to guide the weighting system. |

Do NOT apply `status: SSOT` artificially; an SSOT doc is identified by the `B-canon` label assigned in `contracts/documentation/canonical-map.v1.yaml`.

## File placement decision tree

1. Is this documenting deployed API references, CLI commands, or variables? -> `docs/src/reference/`
2. Is this exploring a new paradigm, recording a design decision, or outlining SSOT behavior? -> `docs/src/architecture/`
3. Is this a step-by-step development process or getting started routine? -> `docs/src/tutorials/`
4. Is it a pedagogical lesson detailing the *why*? -> `docs/src/explanation/`
5. Is this addressing internal contributor practices? -> `docs/src/contributors/`
6. Is this data destined strictly to be parsed by CI logic? -> `docs/agents/`

## Agent-specific caveats

- **Archive prohibition:** Never ingest `docs/src/archive/` unprompted. It is a deprecated index.
- **Research storage rule:** Do not trap research notes within IDE memory or obscure `scratch/` locations if they possess enduring value. Persist them strictly in `docs/src/architecture/`.
- **vox:skip rule:** Ensure the `// vox:skip` marker exists within inline Markdown code blocks if they are not dynamically included via `{{#include}}` from a golden source.

## Cross-tool instruction file hierarchy

- `AGENTS.md` (root) ─ Main cross-tool baseline instructions.
- `GEMINI.md` / `CLAUDE.md` ─ Supplementary overlay restrictions unique to specific vendor integrations.
- `.cursor/rules/*.mdc` ─ Dynamic, glob-targeted rules triggered exclusively within the Cursor IDE.
- `.github/copilot-instructions.md` ─ Equivalent prompt injection context for PR workflows and GitHub Copilot environments.
- Directory-level `AGENTS.md` (e.g., `docs/src/AGENTS.md`) ─ Modifies or provides specialized behavior local to the directory depth.


