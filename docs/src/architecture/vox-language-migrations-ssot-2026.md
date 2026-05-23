---
title: "Vox language migrations hub (research)"
description: "Central index of breaking syntax migrations, codemods, and deprecation paths across compiler, React interop, and ID boundaries."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Reduces agent and contributor confusion when multiple phase plans mention overlapping migrations."
sort_order: 8
---

# Vox language migrations hub (research)

**Not** sole `B-canon` for migrations until registered; links out to phase specs and ADRs.

## Active migration topics

| Topic | Owner doc / tool | Notes |
| --- | --- | --- |
| `Id[T]` at API boundaries | [Phase 3 typecheck rules](vox-language-rules-phase3-typecheck-rules-2026.md) | `vox migrate id-strings` codemod referenced in plan. |
| `@island` retirement / React interop | [Phase 5 React interop spec](../archive/phase5-react-interop-spec-2026.md) | `vox migrate drop-island` helper referenced. |
| Endpoint decorator migration | Root [`AGENTS.md`](../../../AGENTS.md) retired surfaces table | `@server fn` → `@endpoint(kind: …)`. |
| Durability grammar | [ADR 028](../adr/028-deprecate-stub-durability-grammar.md), [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md) | Parse vs runtime truth; align docs before teaching agents new keywords. |
| Database env naming | [`AGENTS.md`](../../../AGENTS.md) | `VOX_DB_*` replaces legacy Turso names. |

## Process

1. Land mechanical migration in compiler / CLI with tests first ([test-first policy](../../../AGENTS.md)).
2. Add row to this hub **and** to the phase plan that owns the syntax.
3. If user-visible, add pointer from [`reference/cli.md`](../reference/cli.md) when the command ships.

## Related

- [Frontend convergence findings](frontend-convergence-findings-2026.md)
- [GUI-native roadmap status](gui-native-roadmap-status-2026.md)
- [language-migration-friction-findings-2026.md](language-migration-friction-findings-2026.md)
