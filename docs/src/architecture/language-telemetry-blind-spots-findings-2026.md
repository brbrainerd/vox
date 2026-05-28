---
title: "Language quality telemetry — blind spots (2026)"
description: "Probe D: constraints from telemetry-trust SSOT vs planned language feedback loops (idiom events, diagnostic analytics)."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Aligns language roadmap with privacy-preserving telemetry constraints."
sort_order: 103
---

# Language quality telemetry — blind spots (2026)

## Probe charter

Map **planned language-quality signals** ([Phase 4 runtime monitors](vox-language-rules-phase4-runtime-monitors-2026.md): `vox.idiom.*`) against **privacy guarantees** ([telemetry-trust-ssot.md](telemetry-trust-ssot.md)).

## Evidence (2026-05-11)

- Trust SSOT forbids shipping source by default; aggregates must be carefully bounded.
- Phase 4 describes idiom fingerprint export for corpus weighting — **opt-in channel** must be explicit in implementation.

## Blind spots

| Topic | Risk |
| --- | --- |
| Diagnostic content | Full messages may leak identifiers; need hashing/redaction tiering. |
| File paths | Paths are PII in many workspaces — strip or hash. |
| Cross-session joins | Even aggregates become identifiable when joined with mesh/orchestrator IDs. |

## Next steps

1. Specify **payload schema** for any `vox.idiom.*` event in `contracts/` before enabling emission.
2. Cross-link evaluation harness docs when Mn-T12 eval gating lands ([mesh MENS plan](mesh-mens-distributed-training-and-execution-plan-2026.md)).
3. Update [mens-training-data-contract.md](../reference/mens-training-data-contract.md) if corpus weighting consumes telemetry-derived tags.

## Related

- [telemetry-unification-design-2026.md](telemetry-unification-design-2026.md)
- [reference/telemetry-metric-contract.md](../reference/telemetry-metric-contract.md)
