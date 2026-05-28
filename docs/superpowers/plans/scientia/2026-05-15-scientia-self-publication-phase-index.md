# SCIENTIA Self-Publication Phase Index (Gap Map 2026)

> **Strategic source of truth:** [`docs/src/architecture/scientia-self-publication-gap-map-2026.md`](../../../src/architecture/scientia-self-publication-gap-map-2026.md).
> Each phase plan below cites back to a gap in that document.
>
> **Antecedent:** [SCIENTIA Self-Publication Finalization Plan
> (2026)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md).
> Finalization Phases 0–10 are complete (per the §12 phase index of that plan)
> as of 2026-05-09. The phases below are *additive* — they fill the
> remaining holes between the Finalization Plan's terminal state and a
> complete developer-facing self-publication workflow.

## Phase index

| Phase | Gap | Plan | Severity | Effort | Depends on | Status |
|---|---|---|---|---|---|---|
| A | Self-observation candidate producers | [2026-05-15-scientia-phase-A-signal-producers.md](./2026-05-15-scientia-phase-A-signal-producers.md) | high | medium | Finalization Phase 6 ✅ | **detailed** |
| B | Replay runner (measured `artifact_replayability`) | [2026-05-15-scientia-phase-B-replay-runner.md](./2026-05-15-scientia-phase-B-replay-runner.md) | high | medium-small | Finalization Phase 4 ✅ | **detailed** |
| C | Long-form IMRaD manuscript scaffolder | [2026-05-15-scientia-phase-C-imrad-scaffolder.md](./2026-05-15-scientia-phase-C-imrad-scaffolder.md) | high | medium | Finalization Phases 1, 2, 4 ✅ | outline |
| D | Solo-author audited-critic gate | [2026-05-15-scientia-phase-D-solo-critic-gate.md](./2026-05-15-scientia-phase-D-solo-critic-gate.md) | high (solo) | small-medium | Finalization Phases 7, 8 ✅ | outline |
| E | AI/SWE micro-publication track (non-Atlas) | [2026-05-15-scientia-phase-E-ai-swe-micro-track.md](./2026-05-15-scientia-phase-E-ai-swe-micro-track.md) | medium | small + medium | Phase C | outline |
| F | `vox scientia scout` single-command surface | [2026-05-15-scientia-phase-F-scout-command.md](./2026-05-15-scientia-phase-F-scout-command.md) | medium | small | Phase A | **detailed** |
| G | Vox-native publication reading surface | [2026-05-15-scientia-phase-G-reading-surface.md](./2026-05-15-scientia-phase-G-reading-surface.md) | medium | medium | Finalization Phase 4 ✅ | outline |
| H | Discovery dashboard panel | [2026-05-15-scientia-phase-H-dashboard-panel.md](./2026-05-15-scientia-phase-H-dashboard-panel.md) | low | small-medium | existing tables ✅ | outline |

**Status legend.** Following Finalization Plan §12 convention:
- **outline** — Goal, architecture, file inventory, task headings, acceptance
  criteria, dependencies, and open questions are specified. Step-by-step
  TDD code is *not yet written*. Expands to **detailed** when the phase
  becomes next-to-execute.
- **detailed** — Full TDD-step-by-step plan with test code, expected
  outputs, exact commands, and commit checkpoints. Ready for
  `executing-plans` / `subagent-driven-development` skill.
- **complete** — Phase shipped; closing PR(s) linked in the plan.

## Recommended execution order

The gap-map's §3 dependency analysis recommends this order:

```
        ┌─> A ──> F           ("scout my repo" front door)
        │        
P1 root ┼─> B                  (measured replayability)
        │        
        ├─> C ──> E            (IMRaD scaffolder → SWE micro-track)
        │        
        ├─> D                  (solo critic gate, independent)
        │        
        └─> G, H               (UX wins, independent)
```

Phases A and B can run in parallel; C and D can run in parallel; F follows
A; E follows C; G and H are independent of all others and can interleave.

**First slice (gap-map §4).** Phases A (narrowest cut) + B (narrowest cut) +
F (thin wrapper) deliver an end-to-end "commits → measured-replay
candidate → scouted on the CLI" path. Recommended as the first execution
chunk.

## Plan-to-detailed promotion gate

A phase moves from **outline** to **detailed** when:
1. Its predecessors are **complete** (so the file inventory is stable).
2. The plan author has done code-surface exploration of every file in the
   inventory and recorded the relevant API signatures, schemas, and
   existing tests in the plan body.
3. The plan's open-questions list is resolved or explicitly punted with
   acceptance criteria.

The promotion writes TDD steps with no placeholders, matching the fidelity
of [Phase 0a](./2026-05-09-scientia-phase-0a-phantom-imports.md).

## Cross-references

- Gap map: [`scientia-self-publication-gap-map-2026.md`](../../../src/architecture/scientia-self-publication-gap-map-2026.md)
- Finalization Plan: [`scientia-self-publication-finalization-plan-2026.md`](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md)
- SCIENTIA SSOT handbook: [`scientia-ssot-handbook.md`](../../../src/reference/scientia-ssot-handbook.md)
- Worthiness rules: [`scientia-publication-worthiness-rules.md`](../../../src/reference/scientia-publication-worthiness-rules.md)
- Operator how-to: [`how-to-scientia-publication.md`](../../../src/how-to/how-to-scientia-publication.md)
- Where Things Live: [`where-things-live.md`](../../../src/architecture/where-things-live.md)
