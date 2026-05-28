# SCIENTIA Phase H — Discovery Dashboard Panel

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** outline.

**Goal:** A dashboard panel surfacing the publication-pipeline queue at a glance: candidates by class, claims awaiting verification, manifests in reply-window, retraction queue, cost rollup. Driven from existing tables; primarily UI work.

**Architecture:** Add a `Scientia` panel to the existing Vox dashboard. Backend: new REST routes `/api/v2/scientia/queue` and `/api/v2/scientia/cost` (per [SSOT §5.6](../../../src/architecture/mesh-and-language-distribution-ssot-2026.md) route convention) plus WS topic `scientia.queue.changed` on `/v1/ws`. Frontend: a panel rendering five sections — Candidates, Claims pending, Manifests in reply window, Retraction queue, Cost rollup. No new DB schema; all data from existing tables (`finding_candidates`, `claims`, `publication_manifests`, `publication_status_events`, `external_submission_attempts`).

**Tech Stack:** Whatever the dashboard frontend uses (per [SSOT §5.6](../../../src/architecture/mesh-and-language-distribution-ssot-2026.md); verify in Task H1); existing dashboard auth; existing dashboard WS infra.

**Strategic context:** [Gap-map §2 Gap H](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-h--discovery-dashboard-panel); [dashboard-control plan](../../../src/architecture/mesh-phase4-dashboard-control-plan-2026.md).

**Out of scope:**
- New persistence (read-only over existing tables).
- Authoring (Phase G's edit surface boundary).
- Cross-workspace aggregation (single-workspace view in Phase H).

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Modify | dashboard backend routes module (verify path in Task H1) | Add `/api/v2/scientia/queue` + `/api/v2/scientia/cost` |
| Modify | dashboard WS topic registry | Register `scientia.queue.changed` |
| Modify | dashboard frontend (verify path) | Add Scientia panel component |
| Create | frontend `ScientiaPanel/index.tsx` (or framework analog) | Five-section render |
| Create | backend route handler module | Query existing tables, compose response |
| Modify | `docs/src/architecture/where-things-live.md` | Add row: "Scientia dashboard panel" |

LoC budget: ~600 LoC backend + ~800 LoC frontend + ~300 tests.

---

## Tasks (headings only)

### Task H1: Identify dashboard surface
Confirm the dashboard frontend framework, backend route registry, and WS topic registry per the dashboard-control plan.

### Task H2: Backend `/api/v2/scientia/queue` route
Returns JSON:
```jsonc
{
  "candidates": {
    "total": <int>,
    "by_class": {<class>: <count>},
    "top_5_by_confidence": [<candidate-row>]
  },
  "claims_pending": {
    "verifiable": <int>,
    "abstained": <int>,
    "extraction_running": <int>
  },
  "manifests_in_reply_window": [<manifest-id>],
  "retraction_queue": [<manifest-id>],
  "stale": {
    "evidence_incomplete_over_30d": <int>
  }
}
```

### Task H3: Backend `/api/v2/scientia/cost` route
Returns JSON cost rollup:
```jsonc
{
  "this_quarter": {
    "extraction_usd": <float>,
    "critic_usd": <float>,
    "novelty_retrieval_usd": <float>,
    "scholarly_submission_usd": <float>,
    "total_usd": <float>
  },
  "per_finding_average_usd": <float>,
  "by_provider": [{"provider": <str>, "usd": <float>}]
}
```

### Task H4: WS topic
`scientia.queue.changed` published when any of: new candidate, claim verified, reply-window state change, retraction issued, cost rollup tick.

### Task H5: Frontend panel
Five sections per the goal. Use existing dashboard chrome (header, refresh, error toasts).

### Task H6: "Stalls" surface
Highlight any candidate with `evidence_incomplete` for >30 days; surface a "needs attention" badge.

### Task H7: Tests
- Backend: route returns valid JSON over a fixture DB with known candidate counts.
- Frontend: component renders all five sections; loading state; empty state; error state.

### Task H8: Documentation
- Dashboard user-guide entry.
- Architecture cross-reference.

---

## Acceptance criteria

1. `GET /api/v2/scientia/queue` returns the documented JSON shape.
2. `GET /api/v2/scientia/cost` returns the documented JSON shape.
3. WS topic `scientia.queue.changed` publishes within 1s of a candidate insert.
4. Frontend panel renders all five sections with loading + empty + error states.
5. Stall badge fires correctly on a 31-day-old `evidence_incomplete` fixture.
6. Backend + frontend tests green.

---

## Open questions

- **OQ-H1.** Cost data source. Where is per-provider usage actually recorded today? Recommendation: trace through `external_submission_attempts` and the LLM telemetry surface in Task H1 exploration.
- **OQ-H2.** Real-time vs polled. WS topic is preferred but polling every 30s works. Recommendation: WS topic; fall back to polling if the dashboard WS infra isn't trivially extensible.
- **OQ-H3.** Multi-user view. If the dashboard supports multi-user, do all users see all candidates? Recommendation: workspace-scoped for Phase H; per-user permissions defer to follow-up.

---

## Dependencies

- **Upstream:** Existing tables (`finding_candidates`, `publication_manifests`, `publication_status_events`, `external_submission_attempts`) ✅. Dashboard plumbing per [dashboard-control plan](../../../src/architecture/mesh-phase4-dashboard-control-plan-2026.md).
- **Downstream:** None.

---

## Cross-references

- Gap: [gap-map §2 Gap H](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-h--discovery-dashboard-panel)
- Dashboard control plan: [`mesh-phase4-dashboard-control-plan-2026.md`](../../../src/architecture/mesh-phase4-dashboard-control-plan-2026.md)
- Route convention: [SSOT §5.6](../../../src/architecture/mesh-and-language-distribution-ssot-2026.md)
