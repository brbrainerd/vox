---
title: "ADR 002 — Diátaxis Three-Tier Documentation Architecture"
description: "Grounded documentation architecture for Vox: mdBook front door, contributor surfaces, contracts, and status vocabulary."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 002 — Diátaxis Three-Tier Documentation Architecture

**Status**: Accepted
**Date**: 2026-03-02

---

## Context

Vox needed a reader-facing documentation structure, but the repository also grew contributor governance, machine-readable contracts, research notes, and planning material that do not fit a prefix-only Diataxis model.

The early policy in this ADR leaned on filename prefixes such as `tut-` and `ref-`. That helped the first migration, but the current repository organizes most docs by directory, frontmatter category, and intended audience:

- `docs/src/` is the published mdBook corpus.
- `docs/src/architecture/` contains both current architecture pages and research or roadmap material.
- `docs/src/reference/` mirrors machine-backed contracts in reader-facing prose.
- `docs/src/contributors/` and `docs/agents/` serve contributors and automation.
- `contracts/` contains machine-readable SSOT.

---

## Decision

**Keep Diátaxis as the reader-facing organizing principle for user documentation, but ground the overall documentation system in audience and authority boundaries rather than filename prefixes alone.**

### Reader-facing categories

| Category | Purpose | Primary need |
|----------|---------|--------------|
| `getting-started` | front door and first steps | "Where do I begin?" |
| `tutorial` | guided learning | "Teach me step by step." |
| `how-to` | goal-oriented tasks | "Help me accomplish something." |
| `explanation` | conceptual understanding | "Help me understand why." |
| `reference` | lookup and exact behavior | "I need the details." |
| `adr` | design decisions | "Why was this chosen?" |
| `architecture` | system shape, SSOT, research, roadmap | "How is the repo organized and where is the design described?" |
| `contributor` | contributor process and governance | "How do I work safely in this repo?" |
| `ci` | quality and CI contracts | "What does automation enforce?" |

### Frontmatter Standard

Published pages should use YAML frontmatter. At minimum, new pages should carry:

```yaml
---
title: "Human-readable Title"
description: "One-sentence summary"
category: "Getting Started"|tutorial|how-to|explanation|reference|adr|architecture|contributor|ci
last_updated: "2026-03-01"
training_eligible: true
status: current|experimental|legacy|research|roadmap|deprecated  # when needed
---
```

`training_eligible` controls whether eligible doc content may feed the documentation extraction pipeline for Mens-related corpora. `status` is required whenever a page could otherwise be mistaken for current shipped behavior.

### Authority boundaries

The docs system is intentionally split:

| Surface | Role |
|---------|------|
| `README.md` | short public front door |
| `docs/src/index.md` | site landing page |
| `docs/src/` | published human documentation |
| `docs/src/contributors/` | contributor-facing documentation in the book |
| `docs/agents/` | inventories, governance, automation support |
| `contracts/` | machine-readable SSOT |

### Naming

Filename prefixes are allowed when they improve scanability, but they are no longer the core organizational rule. Folder placement, frontmatter, and authority boundaries are canonical.

---

## Consequences

**Positive:**
- mdBook navigation can stay reader-first without pretending every document has the same audience.
- Contributor guidance becomes discoverable without moving machine-oriented docs into the public front door.
- Research and roadmap pages can stay in-tree while being labeled honestly.
- Contracts, prose, and contributor governance can each keep a clear job.

**Negative:**
- Frontmatter and boundaries must be maintained as the repo evolves.
- Some legacy filename conventions remain in the tree and will coexist with the newer boundary model.
- Tooling must validate category vocabulary and catch drift instead of silently accepting it.

---

## References
- [Diátaxis framework](https://diataxis.fr/)
- [`../contributors/documentation-governance.md`](../contributors/documentation-governance.md)
- `crates/vox-doc-pipeline/src/main.rs` — SUMMARY generation
- `.github/workflows/docs-deploy.yml` — docs deploy integration


