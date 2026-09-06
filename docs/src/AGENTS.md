---
title: "AGENTS.md"
description: "Documentation for AGENTS.md."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Project architecture context."
---
# Documentation Rules (docs/src/ scope)

## Code block policy
- Fenced blocks tagged `vox`: use `{{#include}}` from `examples/golden/` or tag `// vox:skip` only when the snippet is intentionally illustrative or non-compiling; other language fences are not run through the Vox parser
- All code blocks must specify a language identifier
- Do not place golden examples inline without the include directive

## Frontmatter requirements (all new pages)
- `title`, `description`, `category`, `status`, `training_eligible` are required (`last_updated` is optional metadata; Git history is authoritative per `documentation-governance.md`)
- Use `status: research` for evidence docs; `status: roadmap` for unshipped plans
- Do NOT label a page SSOT unless it is the sole B-canon in `contracts/documentation/canonical-map.v1.yaml`

## Research storage
- Research findings → `docs/src/architecture/`
- Naming: `*-research-2026.md` or `*-findings-2026.md`
- After writing, set frontmatter (`title`, `description`, `category`, `status`).

## Archive prohibition
- Do NOT read or modify files in `docs/src/archive/` for new work
