---
title: "Frontend Surface Ownership"
description: "Canonical ownership and lifecycle policy for Vox frontend surfaces."
category: "Language Reference"
status: "current"
last_updated: "2026-05-04"
training_eligible: true
training_rationale: "Defines canonical frontend ownership and lifecycle boundaries used for maintenance and CI."
---

# Frontend Surface Ownership

This page is the canonical ownership map for GUI surfaces in Vox. The machine-readable registry is [`contracts/frontend/surface-ownership.v1.yaml`](../../../contracts/frontend/surface-ownership.v1.yaml).
GUI syntax and React compatibility aliases are separately versioned in [`contracts/frontend/gui-compatibility.v1.yaml`](../../../contracts/frontend/gui-compatibility.v1.yaml).
Frontend dependency drift limits are versioned in [`contracts/frontend/dependency-policy.v1.yaml`](../../../contracts/frontend/dependency-policy.v1.yaml).

## Worked examples

**Surface class:** a new dashboard panel is **`canonical`** — implement under `crates/vox-dashboard` first; only then mirror stubs into `apps/interop/marquee_app` if interop needs proving.

**React attribute mapping:** follow event / prop aliases in [`gui-compatibility.v1.yaml`](../../../contracts/frontend/gui-compatibility.v1.yaml) (e.g. wire `on:click` in Vox source to the emitted React prop convention documented there).

**CI:** `cargo test -p vox-cli --test frontend_dependency_policy_test` reads `contracts/frontend/*.yaml`; update contracts and this page together when adding a new GUI surface class.

## Canonical classification

| Surface | Status | Why it exists | Change policy |
| --- | --- | --- | --- |
| `crates/vox-dashboard` | canonical | Primary Vox user-facing GUI and orchestration UX | New product UX lands here first |
| `apps/interop/marquee_app` | interoperability-reference | Demonstrates external React app consuming generated Vox artifacts | Keep minimal and realistic; do not fork product UX |
| `apps/experimental/visualizer` | experimental | Sandbox for visualization prototypes | Promote to dashboard or delete; avoid long-lived duplication |
| `tests/fixtures/frontend/test_app_bundle` | fixture-only | Deterministic scaffold fixture and generated snapshot surface | Treat as fixture data, not product UX |
| `apps/editor/vox-vscode` | deprecated-primary-surface | Extension/LSP compatibility host | No new primary GUI workflows |

## Necessary and unnecessary usage

- **Necessary:** `vox-dashboard` and one external interop exemplar (`apps/interop/marquee_app`) to validate "Vox backend + React frontend" workflows.
- **Necessary:** generated TSX in compiler-owned trees where Vox source is authoritative.
- **Unnecessary:** implementing the same visualization feature in both dashboard and `apps/experimental/visualizer` without an explicit promotion plan.
- **Unnecessary:** treating `tests/fixtures/frontend/test_app_bundle` as a production surface.

## Governance checks

- New GUI behavior must declare target surface class (canonical, interoperability-reference, experimental, fixture-only, deprecated-primary-surface).
- Canonical UX changes require updates in `crates/vox-dashboard` first.
- Experimental surfaces must define either:
  - a promotion path into the canonical surface, or
  - a decommission date.

## Related

- [`vox-web-stack.md`](vox-web-stack.md)
- [`external-frontend-interop-plan-2026.md`](../architecture/external-frontend-interop-plan-2026.md)
- [`031-deprecate-vox-vscode.md`](../adr/031-deprecate-vox-vscode.md)
