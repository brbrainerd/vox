---
title: "ADR 002 — Diátaxis Three-Tier Documentation"
category: adr
constructs: []
last_updated: 2026-03-02
training_eligible: false
difficulty: beginner
---

# ADR 002 — Diátaxis Three-Tier Documentation Architecture

**Status**: Accepted
**Date**: 2026-03-02

---

## Context

Vox's docs had no discoverability structure: all files flat in `docs/src/`, no naming convention, no clear differentiation between "learn how to use" vs "look up an API" vs "understand why it works."

---

## Decision

**Adopt [Diátaxis](https://diataxis.fr/) with file-naming prefixes to categorize all documentation.**

| Prefix | Category | Purpose | User need |
|--------|----------|---------|-----------|
| `tut-` | Tutorial | Step-by-step learning | "I want to learn" |
| `how-to-` | How-To | Goal-oriented guides | "I want to solve a specific problem" |
| `expl-` | Explanation | Background understanding | "I want to understand why" |
| `ref-` | Reference | Look-up information | "I need to check a detail" |
| `adr/` | Architecture Decision | Decision rationale | "Why was this built this way?" |

### Frontmatter Standard

Every doc file must have YAML frontmatter:

```yaml
---
title: "Human-readable Title"
category: tutorial|how-to|explanation|reference|adr
constructs: [function, actor, workflow]  # Vox constructs shown
last_updated: 2026-03-01
training_eligible: true
difficulty: beginner|intermediate|advanced
---
```

The `training_eligible` field controls whether the doc's code blocks feed into the ML training corpus.

---

## Consequences

**Positive:**
- `vox-doc-pipeline` can auto-categorize new files by prefix → correct SUMMARY section
- Training data pipeline knows which docs contain ground-truth code examples
- Users can distinguish "learn" from "look up" at a glance
- ADRs preserve architectural reasoning for future contributors

**Negative:**
- Requires renaming existing files (one-time migration)
- Frontmatter must be maintained per-file (tooling can warn when missing)

---

## References
- [Diátaxis framework](https://diataxis.fr/)
- `crates/vox-doc-pipeline/src/main.rs` — SUMMARY generation
- `.github/workflows/docs-deploy.yml` — docs CI integration
