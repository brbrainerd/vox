---
title: "Language migration friction — findings (2026)"
description: "Probe E: map deprecated paths and codemods to docs; identify missing entrypoints and ADR vs AGENTS conflicts."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Reduces failed migrations for agents trained on stale snippets."
sort_order: 104
---

# Language migration friction — findings (2026)

## Probe charter

Enumerate **high-friction migrations** (syntax retired in prose but still parse-correct, or documented in multiple places with conflicting status).

## Evidence (2026-05-11)

- Retired surfaces table in root [`AGENTS.md`](../../../AGENTS.md) vs ADRs such as [ADR 028](../adr/028-deprecate-stub-durability-grammar.md) — durability keywords remain parse-visible per [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md).
- React interop migrations centralized partially in [phase5-react-interop-spec-2026.md](../archive/phase5-react-interop-spec-2026.md).

## Friction patterns

1. **Dual truth:** Roadmap says “supported grammar,” runtime audit says “no scheduler” — contributors must read both.
2. **Scattered codemod names:** Phase 3 (`id-strings`) vs Phase 5 (`drop-island`) — hub doc reduces search cost ([vox-language-migrations-ssot-2026.md](vox-language-migrations-ssot-2026.md)).
3. **IDE lag:** VS Code extension deprecation ([ADR 031](../adr/031-deprecate-vox-vscode.md)) vs LSP-first workflow — migration messaging must stay synchronized.

## Next steps

1. Add CLI `--help` cross-links when codemods ship ([reference/cli.md](../reference/cli.md)).
2. For each ADR that changes grammar, add a row to the migrations hub and a release note stub under `docs/news/` when cutting versions.

## Related

- [vox-language-migrations-ssot-2026.md](vox-language-migrations-ssot-2026.md)
- [legacy-tombstone-remediation-ledger-2026.md](legacy-tombstone-remediation-ledger-2026.md)
